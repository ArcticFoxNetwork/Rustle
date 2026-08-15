//! Audio thread implementation
//!
//! This module runs the AudioPlayer in a dedicated thread, processing
//! commands from the UI thread and sending events back.
//!
//! The audio thread may block on streaming operations (e.g., seeking to
//! unbuffered positions), but this doesn't affect the UI thread.

use std::collections::HashMap;
use std::path::PathBuf;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use rodio::Sink;

use super::PlaybackStatus;
use super::chain::{AudioProcessingChain, PlaybackProcessingRuntime};
use super::events::{
    AudioCommand, AudioCommandReceiver, AudioEvent, AudioEventSender, BufferDataMailbox,
    BufferDataUpdate, LatestControlMailbox, SharedPlaybackState, audio_command_channel,
    audio_event_channel,
};
use super::handle::AudioHandle;
use super::player::{AudioPlayer, PreparedStreamingSource, prepare_streaming_source};
use super::streaming::{
    HIGH_WATER_MARK_BYTES, LOW_WATER_MARK_BYTES, SharedBuffer, StreamingBuffer,
};

const STREAMING_PREPARATION_QUEUE_CAPACITY: usize = 4;
const STREAMING_PREPARATION_RESULT_CAPACITY: usize = 8;
const CONTROL_MAINTENANCE_INTERVAL: Duration =
    Duration::from_millis(super::automix::SCHEDULER_POLL_MS);

#[derive(Default)]
struct FinishedGuard {
    generation: Option<super::identity::PlaybackGeneration>,
    emitted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FinishReason {
    Natural,
    Stop,
    SeekRebuild,
    TransitionDisposed,
    Stale,
}

impl FinishedGuard {
    fn reset(&mut self) {
        self.generation = None;
        self.emitted = false;
    }

    fn try_mark(
        &mut self,
        generation: super::identity::PlaybackGeneration,
        reason: FinishReason,
    ) -> bool {
        if reason != FinishReason::Natural {
            return false;
        }
        if self.generation != Some(generation) {
            self.generation = Some(generation);
            self.emitted = false;
        }
        if self.emitted {
            false
        } else {
            self.emitted = true;
            true
        }
    }
}

/// Preloaded sink with metadata
struct PreloadedSink {
    identity: super::identity::PreloadIdentity,
    sink: Sink,
    duration: Duration,
    path: PathBuf,
    track_gain: f32,
    runtime: PlaybackProcessingRuntime,
    is_streaming: bool,
    shared_buffer: Option<SharedBuffer>,
}

struct ScheduledPreloadedTransition {
    owner: super::identity::PlaybackContext,
    identity: super::identity::PreloadIdentity,
    playback_request_id: u64,
    fade_in: bool,
    transition: super::automix::TransitionDirective,
}

enum PlaybackPreparationMode {
    Play { fade_in: bool },
    LoadPaused { position: Duration },
}

struct PlaybackPreparationRequest {
    context: super::identity::PlaybackContext,
    request_id: u64,
    buffer: StreamingBuffer,
    duration: Duration,
    cache_path: Option<PathBuf>,
    track_gain: f32,
    mode: PlaybackPreparationMode,
}

struct PreparedPlayback {
    context: super::identity::PlaybackContext,
    request_id: u64,
    shared_buffer: SharedBuffer,
    source: Result<PreparedStreamingSource, String>,
    duration: Duration,
    cache_path: Option<PathBuf>,
    track_gain: f32,
    mode: PlaybackPreparationMode,
}

struct PreloadPreparationRequest {
    identity: super::identity::PreloadIdentity,
    buffer: StreamingBuffer,
    duration: Duration,
    track_gain: f32,
}

struct PreparedPreload {
    identity: super::identity::PreloadIdentity,
    shared_buffer: SharedBuffer,
    source: Result<PreparedStreamingSource, String>,
    duration: Duration,
    track_gain: f32,
}

enum StreamingPreparationResult {
    Playback(PreparedPlayback),
    Preload(PreparedPreload),
}

enum ControlEvent {
    Command(AudioCommand),
    Prepared(StreamingPreparationResult),
    Maintenance,
    Closed,
}

async fn next_control_event(
    command_rx: &mut AudioCommandReceiver,
    preparation_result_rx: &mut tokio::sync::mpsc::Receiver<StreamingPreparationResult>,
    maintenance_deadline: tokio::time::Instant,
) -> ControlEvent {
    tokio::select! {
        command = command_rx.recv() => match command {
            Some(command) => ControlEvent::Command(command),
            None => ControlEvent::Closed,
        },
        Some(prepared) = preparation_result_rx.recv() => ControlEvent::Prepared(prepared),
        _ = tokio::time::sleep_until(maintenance_deadline) => ControlEvent::Maintenance,
    }
}

struct StreamingPreparationPool {
    playback_tx: std::sync::mpsc::SyncSender<PlaybackPreparationRequest>,
    preload_tx: std::sync::mpsc::SyncSender<PreloadPreparationRequest>,
}

impl StreamingPreparationPool {
    fn new(
        result_tx: tokio::sync::mpsc::Sender<StreamingPreparationResult>,
    ) -> Result<Self, String> {
        let (playback_tx, playback_rx) =
            std::sync::mpsc::sync_channel(STREAMING_PREPARATION_QUEUE_CAPACITY);
        let playback_result_tx = result_tx.clone();
        thread::Builder::new()
            .name("audio-playback-prepare".to_string())
            .spawn(move || {
                while let Ok(request) = playback_rx.recv() {
                    let PlaybackPreparationRequest {
                        context,
                        request_id,
                        buffer,
                        duration,
                        cache_path,
                        track_gain,
                        mode,
                    } = request;
                    let shared_buffer = buffer.shared().clone();
                    let started = Instant::now();
                    let source = prepare_streaming_source(buffer);
                    tracing::debug!(
                        request_id,
                        generation = context.generation.0,
                        elapsed_ms = started.elapsed().as_millis(),
                        success = source.is_ok(),
                        "Streaming playback preparation completed"
                    );
                    if playback_result_tx
                        .blocking_send(StreamingPreparationResult::Playback(PreparedPlayback {
                            context,
                            request_id,
                            shared_buffer,
                            source,
                            duration,
                            cache_path,
                            track_gain,
                            mode,
                        }))
                        .is_err()
                    {
                        break;
                    }
                }
            })
            .map_err(|error| format!("Failed to spawn playback preparation worker: {error}"))?;

        let (preload_tx, preload_rx) =
            std::sync::mpsc::sync_channel(STREAMING_PREPARATION_QUEUE_CAPACITY);
        thread::Builder::new()
            .name("audio-preload-prepare".to_string())
            .spawn(move || {
                while let Ok(request) = preload_rx.recv() {
                    let PreloadPreparationRequest {
                        identity,
                        buffer,
                        duration,
                        track_gain,
                    } = request;
                    let shared_buffer = buffer.shared().clone();
                    let started = Instant::now();
                    let source = prepare_streaming_source(buffer);
                    tracing::debug!(
                        request_id = identity.request_id,
                        generation = identity.generation.0,
                        elapsed_ms = started.elapsed().as_millis(),
                        success = source.is_ok(),
                        "Streaming preload preparation completed"
                    );
                    if result_tx
                        .blocking_send(StreamingPreparationResult::Preload(PreparedPreload {
                            identity,
                            shared_buffer,
                            source,
                            duration,
                            track_gain,
                        }))
                        .is_err()
                    {
                        break;
                    }
                }
            })
            .map_err(|error| format!("Failed to spawn preload preparation worker: {error}"))?;

        Ok(Self {
            playback_tx,
            preload_tx,
        })
    }

    fn prepare_playback(&self, request: PlaybackPreparationRequest) -> Result<(), String> {
        self.playback_tx
            .try_send(request)
            .map_err(|error| match error {
                std::sync::mpsc::TrySendError::Full(_) => {
                    "streaming playback preparation queue is full".to_string()
                }
                std::sync::mpsc::TrySendError::Disconnected(_) => {
                    "streaming playback preparation worker is unavailable".to_string()
                }
            })
    }

    fn prepare_preload(&self, request: PreloadPreparationRequest) -> Result<(), String> {
        self.preload_tx
            .try_send(request)
            .map_err(|error| match error {
                std::sync::mpsc::TrySendError::Full(_) => {
                    "streaming preload preparation queue is full".to_string()
                }
                std::sync::mpsc::TrySendError::Disconnected(_) => {
                    "streaming preload preparation worker is unavailable".to_string()
                }
            })
    }
}

pub struct AudioThreadHandle {
    pub handle: AudioHandle,
    pub event_rx: Option<super::events::AudioEventReceiver>,
    thread_handle: Option<JoinHandle<()>>,
}

impl AudioThreadHandle {
    pub fn take_event_rx(&mut self) -> Option<super::events::AudioEventReceiver> {
        self.event_rx.take()
    }
}

impl Drop for AudioThreadHandle {
    fn drop(&mut self) {
        self.handle.shutdown();
        // Intentionally detach in every case. The audio thread owns its backend
        // resources and must never be joined from arbitrary UI teardown paths.
        let _ = self.thread_handle.take();
    }
}

/// Spawn the audio thread
///
/// Creates an AudioPlayer in a dedicated thread and returns handles for
/// communication. The audio thread processes commands and sends events.
///
/// # Arguments
/// * `device_name` - Optional audio output device name
/// * `chain` - Audio processing chain (EQ, preamp, etc.)
///
/// # Returns
/// * `AudioThreadHandle` containing the handle and event receiver
pub fn spawn_audio_thread(
    device_name: Option<&str>,
    chain: AudioProcessingChain,
) -> Result<AudioThreadHandle, String> {
    // Create channels
    let (command_tx, command_rx) = audio_command_channel();
    let (event_tx, event_rx) = audio_event_channel();
    let latest_controls = LatestControlMailbox::new(command_tx.clone());
    let (preparation_result_tx, preparation_result_rx) =
        tokio::sync::mpsc::channel(STREAMING_PREPARATION_RESULT_CAPACITY);
    let preparation_pool = StreamingPreparationPool::new(preparation_result_tx)?;

    // Create shared state
    let state = SharedPlaybackState::new();
    let state_clone = state.clone();

    // The mailbox owns the only callback wake sender; the audio thread itself
    // does not need a command sender once the handle has been created.
    let buffer_mailbox = BufferDataMailbox::new(command_tx.clone());

    // Create handle for UI
    let handle = AudioHandle::new(command_tx, latest_controls.clone(), state);
    let generation_controller = handle.generation_controller();

    // Clone device name for thread
    let device_name_owned = device_name.map(|s| s.to_string());

    // Spawn audio thread
    let thread_handle = thread::Builder::new()
        .name("audio-player".to_string())
        .spawn(move || {
            // Create player in audio thread
            let player_result = if let Some(ref name) = device_name_owned {
                AudioPlayer::with_device(Some(name), chain)
            } else {
                AudioPlayer::new(chain)
            };

            match player_result {
                Ok(player) => {
                    let runtime = tokio::runtime::Builder::new_current_thread()
                        .enable_time()
                        .build();
                    match runtime {
                        Ok(runtime) => runtime.block_on(audio_thread_main(
                            player,
                            command_rx,
                            buffer_mailbox,
                            latest_controls,
                            event_tx,
                            state_clone,
                            generation_controller,
                            preparation_pool,
                            preparation_result_rx,
                        )),
                        Err(error) => {
                            tracing::error!("Failed to create audio control runtime: {}", error);
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to create audio player: {}", e);
                }
            }
        })
        .map_err(|e| format!("Failed to spawn audio thread: {}", e))?;

    Ok(AudioThreadHandle {
        handle,
        event_rx: Some(event_rx),
        thread_handle: Some(thread_handle),
    })
}

/// Main loop for the audio thread
///
/// Processes commands from the UI thread and updates shared state.
/// The control actor waits on ordered lifecycle commands, immutable streaming
/// preparation results, or a coalesced maintenance deadline. Periodic work is
/// never represented as a critical command.
#[allow(clippy::too_many_arguments)]
async fn audio_thread_main(
    mut player: AudioPlayer,
    mut command_rx: AudioCommandReceiver,
    buffer_mailbox: BufferDataMailbox,
    latest_controls: LatestControlMailbox,
    event_tx: AudioEventSender,
    state: SharedPlaybackState,
    generation: super::identity::PlaybackGenerationController,
    preparation_pool: StreamingPreparationPool,
    mut preparation_result_rx: tokio::sync::mpsc::Receiver<StreamingPreparationResult>,
) {
    tracing::info!("Audio thread started");

    // Storage for preloaded sinks (request_id -> PreloadedSink)
    let mut preloaded_sinks: HashMap<u64, PreloadedSink> = HashMap::new();

    // Current streaming buffer reference (for data availability checks)
    // Set when PlayStreaming command is processed, cleared on Play/Stop
    let mut current_buffer: Option<SharedBuffer> = None;

    let mut current_context: Option<super::identity::PlaybackContext> = None;
    let mut pause_after_preparation: Option<super::identity::PlaybackContext> = None;
    let mut finished_guard = FinishedGuard::default();
    let mut scheduled_transition: Option<ScheduledPreloadedTransition> = None;
    let mut transition_scheduler = super::automix::AudioClockScheduler::new(Duration::from_millis(
        super::automix::SCHEDULER_HORIZON_MS,
    ));

    let mut next_maintenance = tokio::time::Instant::now() + CONTROL_MAINTENANCE_INTERVAL;

    loop {
        let event = next_control_event(
            &mut command_rx,
            &mut preparation_result_rx,
            next_maintenance,
        )
        .await;
        let now = tokio::time::Instant::now();
        let maintenance_due = now >= next_maintenance;
        if maintenance_due {
            // Missed timer wake-ups are intentionally coalesced. Never replay a
            // backlog of periodic ticks after the actor was busy.
            let lateness = now.saturating_duration_since(next_maintenance);
            if lateness > CONTROL_MAINTENANCE_INTERVAL {
                tracing::warn!(
                    lateness_ms = lateness.as_millis(),
                    "Audio control maintenance deadline was delayed"
                );
            }
            next_maintenance = now + CONTROL_MAINTENANCE_INTERVAL;
        }

        let cmd = match event {
            ControlEvent::Closed => break,
            ControlEvent::Maintenance => {
                if run_control_maintenance(
                    &mut player,
                    &buffer_mailbox,
                    &latest_controls,
                    &event_tx,
                    &state,
                    &generation,
                    &mut current_buffer,
                    &mut current_context,
                    &mut finished_guard,
                    &mut preloaded_sinks,
                    &mut scheduled_transition,
                    &mut transition_scheduler,
                    maintenance_due,
                ) {
                    break;
                }
                continue;
            }
            ControlEvent::Prepared(prepared) => {
                handle_streaming_preparation_result(
                    prepared,
                    &mut player,
                    &buffer_mailbox,
                    &event_tx,
                    &state,
                    &generation,
                    &mut current_buffer,
                    &mut current_context,
                    &mut pause_after_preparation,
                    &mut finished_guard,
                    &mut preloaded_sinks,
                    &mut scheduled_transition,
                    &mut transition_scheduler,
                );
                if run_control_maintenance(
                    &mut player,
                    &buffer_mailbox,
                    &latest_controls,
                    &event_tx,
                    &state,
                    &generation,
                    &mut current_buffer,
                    &mut current_context,
                    &mut finished_guard,
                    &mut preloaded_sinks,
                    &mut scheduled_transition,
                    &mut transition_scheduler,
                    maintenance_due,
                ) {
                    break;
                }
                continue;
            }
            ControlEvent::Command(cmd) => cmd,
        };

        if matches!(
            &cmd,
            AudioCommand::Play { .. }
                | AudioCommand::LoadPaused { .. }
                | AudioCommand::LoadPausedStreaming { .. }
                | AudioCommand::PlayAt { .. }
                | AudioCommand::PlayStreaming { .. }
                | AudioCommand::Stop { .. }
                | AudioCommand::Seek { .. }
                | AudioCommand::PlayPreloaded { .. }
                | AudioCommand::SwitchDevice { .. }
        ) {
            cancel_scheduled_transition(
                &mut scheduled_transition,
                &mut transition_scheduler,
                &mut preloaded_sinks,
            );
        }
        match cmd {
            AudioCommand::Play {
                context,
                request_id,
                path,
                fade_in,
                track_gain,
            } => {
                if !generation.accepts(&context) {
                    let _ = finished_guard.try_mark(context.generation, FinishReason::Stale);
                    continue;
                }
                finished_guard.reset();
                if let Some(ref old_buffer) = current_buffer {
                    old_buffer.clear_buffer_callback();
                }
                // Local file playback
                current_buffer = None;
                current_context = Some(context.clone());
                handle_play(
                    &mut player,
                    &event_tx,
                    &state,
                    context,
                    request_id,
                    path,
                    fade_in,
                    track_gain,
                );
            }

            AudioCommand::LoadPaused {
                context,
                request_id,
                path,
                position,
                track_gain,
            } => {
                if !generation.accepts(&context) {
                    continue;
                }
                finished_guard.reset();
                if let Some(ref old_buffer) = current_buffer {
                    old_buffer.clear_buffer_callback();
                }
                current_buffer = None;
                current_context = Some(context.clone());
                handle_load_paused(
                    &mut player,
                    &event_tx,
                    &state,
                    context,
                    request_id,
                    path,
                    position,
                    track_gain,
                );
            }

            AudioCommand::LoadPausedStreaming {
                context,
                request_id,
                buffer,
                duration,
                cache_path,
                position,
                track_gain,
            } => {
                if !generation.accepts(&context) {
                    continue;
                }
                let error_context = context.clone();
                pause_after_preparation = None;
                if let Err(error) = preparation_pool.prepare_playback(PlaybackPreparationRequest {
                    context,
                    request_id,
                    buffer,
                    duration,
                    cache_path,
                    track_gain,
                    mode: PlaybackPreparationMode::LoadPaused { position },
                }) {
                    tracing::warn!(request_id, %error, "Streaming playback preparation rejected");
                    let _ = event_tx.send(AudioEvent::Error {
                        context: error_context,
                        request_id: Some(request_id),
                        message: error,
                        error_kind: None,
                    });
                }
            }

            AudioCommand::PlayAt {
                context,
                request_id,
                path,
                position,
                fade_in,
                track_gain,
            } => {
                if !generation.accepts(&context) {
                    continue;
                }
                finished_guard.reset();
                if let Some(ref old_buffer) = current_buffer {
                    old_buffer.clear_buffer_callback();
                }
                current_buffer = None;
                current_context = Some(context.clone());
                handle_play_at(
                    &mut player,
                    &event_tx,
                    &state,
                    context,
                    request_id,
                    path,
                    position,
                    fade_in,
                    track_gain,
                );
            }

            AudioCommand::PlayStreaming {
                context,
                request_id,
                buffer,
                duration,
                cache_path,
                fade_in,
                track_gain,
            } => {
                if !generation.accepts(&context) {
                    continue;
                }
                let error_context = context.clone();
                pause_after_preparation = None;
                if let Err(error) = preparation_pool.prepare_playback(PlaybackPreparationRequest {
                    context,
                    request_id,
                    buffer,
                    duration,
                    cache_path,
                    track_gain,
                    mode: PlaybackPreparationMode::Play { fade_in },
                }) {
                    tracing::warn!(request_id, %error, "Streaming playback preparation rejected");
                    let _ = event_tx.send(AudioEvent::Error {
                        context: error_context,
                        request_id: Some(request_id),
                        message: error,
                        error_kind: None,
                    });
                }
            }

            AudioCommand::Pause { context, fade_out } => {
                if !generation.accepts(&context) {
                    continue;
                }
                if current_context.as_ref() != Some(&context) {
                    pause_after_preparation = Some(context.clone());
                }
                if fade_out {
                    player.pause_with_fade(true);
                } else {
                    player.pause();
                    update_state_from_player(&player, &state);
                    let pos = player.get_info().position;
                    let _ = event_tx.send(AudioEvent::Paused {
                        context,
                        request_id: None,
                        position: pos,
                    });
                }
            }

            AudioCommand::Resume { context, fade_in } => {
                if !generation.accepts(&context) {
                    continue;
                }
                if pause_after_preparation.as_ref() == Some(&context) {
                    pause_after_preparation = None;
                }
                if let Some(ref buf) = current_buffer {
                    let remaining_bytes = buf.buffered_ahead();
                    let source_has_all_required_bytes =
                        buf.is_complete() || buf.remote_eof_reached();
                    if should_enter_buffering(remaining_bytes, source_has_all_required_bytes) {
                        tracing::info!(
                            "Resume: remaining {} bytes < {} (low water mark), entering Buffering",
                            remaining_bytes,
                            LOW_WATER_MARK_BYTES
                        );
                        let position = player.get_info().position;
                        enter_buffering(&mut player, &state, &event_tx, position, context.clone());
                        continue;
                    }
                }

                if fade_in {
                    player.resume_with_fade(true);
                } else {
                    player.resume();
                }
                update_state_from_player(&player, &state);
                let _ = event_tx.send(AudioEvent::Resumed { context });
            }

            AudioCommand::Stop { context } => {
                if !generation.accepts(&context) {
                    continue;
                }
                if let Some(ref old_buffer) = current_buffer {
                    old_buffer.clear_buffer_callback();
                }
                current_buffer = None;
                pause_after_preparation = None;
                let _ = finished_guard.try_mark(context.generation, FinishReason::Stop);
                player.stop();
                update_state_from_player(&player, &state);
                state.set_current_path(None);
                let _ = event_tx.send(AudioEvent::Stopped {
                    context: context.clone(),
                });
            }

            AudioCommand::Seek {
                context,
                nonce,
                position,
            } => {
                if context.cancellation.is_cancelled() {
                    continue;
                }
                let _ = finished_guard.try_mark(context.generation, FinishReason::SeekRebuild);
                handle_seek(
                    &mut player,
                    &event_tx,
                    &state,
                    &generation,
                    context,
                    nonce,
                    position,
                );
                finished_guard.reset();
            }

            AudioCommand::CreatePreloadSink {
                identity,
                path,
                track_gain,
            } => {
                if !generation.accepts_preload(&identity) {
                    continue;
                }
                handle_create_preload_sink(
                    &player,
                    &event_tx,
                    &mut preloaded_sinks,
                    identity,
                    path,
                    track_gain,
                );
            }

            AudioCommand::CreatePreloadSinkStreaming {
                identity,
                buffer,
                duration,
                track_gain,
            } => {
                if !generation.accepts_preload(&identity) {
                    continue;
                }
                let error_identity = identity.clone();
                if let Err(error) = preparation_pool.prepare_preload(PreloadPreparationRequest {
                    identity,
                    buffer,
                    duration,
                    track_gain,
                }) {
                    tracing::warn!(
                        request_id = error_identity.request_id,
                        %error,
                        "Streaming preload preparation rejected"
                    );
                    let _ = event_tx.send(AudioEvent::PreloadFailed {
                        identity: error_identity,
                        error,
                    });
                }
            }

            AudioCommand::PlayPreloaded {
                context,
                identity,
                playback_request_id,
                fade_in,
                transition,
            } => {
                if !generation.accepts(&context) {
                    continue;
                }
                let _ =
                    finished_guard.try_mark(context.generation, FinishReason::TransitionDisposed);
                finished_guard.reset();
                if let Some(ref old_buffer) = current_buffer {
                    old_buffer.clear_buffer_callback();
                }
                // Keep the preload's source identity separate from the new playback generation.
                if let Some(preloaded) = preloaded_sinks.get(&identity.request_id)
                    && preloaded.identity == identity
                {
                    current_buffer = preloaded.shared_buffer.clone();
                } else {
                    current_buffer = None;
                }
                current_context = Some(context.clone());
                handle_play_preloaded(
                    &mut player,
                    &buffer_mailbox,
                    &event_tx,
                    &state,
                    &mut preloaded_sinks,
                    context,
                    playback_request_id,
                    identity,
                    fade_in,
                    transition,
                );
            }

            AudioCommand::SchedulePreloadedTransition {
                owner,
                identity,
                playback_request_id,
                trigger_at,
                fade_in,
                transition,
            } => {
                let owner_current = generation.accepts(&owner);
                let preload_ready = preloaded_sinks
                    .get(&identity.request_id)
                    .is_some_and(|preloaded| preloaded.identity == identity);
                if !owner_current {
                    continue;
                }
                if !generation.accepts_preload(&identity)
                    || owner.generation != identity.generation
                    || owner.cancellation != identity.cancellation
                    || !preload_ready
                {
                    release_preloaded_identity(&mut preloaded_sinks, &identity);
                    let _ = event_tx.send(AudioEvent::Error {
                        context: owner,
                        request_id: Some(playback_request_id),
                        message: "Scheduled preload is no longer ready".to_string(),
                        error_kind: None,
                    });
                    continue;
                }
                if let Some(existing) = scheduled_transition.as_ref() {
                    if existing.identity == identity
                        && existing.transition.group == transition.group
                    {
                        continue;
                    }
                    if existing.identity == identity {
                        let existing = scheduled_transition
                            .take()
                            .expect("scheduled transition exists");
                        transition_scheduler.cancel(existing.transition.group);
                    } else {
                        cancel_scheduled_transition(
                            &mut scheduled_transition,
                            &mut transition_scheduler,
                            &mut preloaded_sinks,
                        );
                    }
                }
                transition_scheduler.schedule(transition.group, trigger_at);
                scheduled_transition = Some(ScheduledPreloadedTransition {
                    owner,
                    identity,
                    playback_request_id,
                    fade_in,
                    transition,
                });
            }

            AudioCommand::CancelScheduledTransition { owner } => {
                if scheduled_transition
                    .as_ref()
                    .is_some_and(|scheduled| scheduled.owner == owner)
                {
                    cancel_scheduled_transition(
                        &mut scheduled_transition,
                        &mut transition_scheduler,
                        &mut preloaded_sinks,
                    );
                }
            }

            AudioCommand::ReleasePreload { identity } => {
                if scheduled_transition
                    .as_ref()
                    .is_some_and(|scheduled| scheduled.identity == identity)
                {
                    cancel_scheduled_transition(
                        &mut scheduled_transition,
                        &mut transition_scheduler,
                        &mut preloaded_sinks,
                    );
                    continue;
                }
                // Exact-identity cleanup is allowed for stale generations; unlike publication,
                // releasing an old sink must not leak resources after generation rollover.
                if preloaded_sinks
                    .get(&identity.request_id)
                    .is_some_and(|preloaded| preloaded.identity == identity)
                {
                    let preloaded = preloaded_sinks
                        .remove(&identity.request_id)
                        .expect("preload exists");
                    if let Some(shared_buffer) = preloaded.shared_buffer {
                        shared_buffer.clear_buffer_callback();
                    }
                    tracing::debug!("Released stale preload sink: identity={:?}", identity);
                }
            }

            AudioCommand::SwitchDevice { device_name } => {
                // Clear all preloaded sinks when switching device (they use old mixer)
                preloaded_sinks.clear();
                if let Some(ref old_buffer) = current_buffer {
                    old_buffer.clear_buffer_callback();
                }
                current_buffer = None;
                handle_switch_device(&mut player, &event_tx, &state, device_name);
            }

            AudioCommand::LatestMailboxWake => {}

            AudioCommand::BufferDataAvailable => {
                while let Some(update) = buffer_mailbox.take_latest() {
                    handle_buffer_data_update(
                        &mut player,
                        &event_tx,
                        &state,
                        &generation,
                        current_buffer.as_ref(),
                        update,
                    );
                }
            }
        }

        if run_control_maintenance(
            &mut player,
            &buffer_mailbox,
            &latest_controls,
            &event_tx,
            &state,
            &generation,
            &mut current_buffer,
            &mut current_context,
            &mut finished_guard,
            &mut preloaded_sinks,
            &mut scheduled_transition,
            &mut transition_scheduler,
            maintenance_due,
        ) {
            break;
        }
    }

    tracing::info!("Audio thread exiting (command channel closed)");
}

#[allow(clippy::too_many_arguments)]
fn run_control_maintenance(
    player: &mut AudioPlayer,
    buffer_mailbox: &BufferDataMailbox,
    latest_controls: &LatestControlMailbox,
    event_tx: &AudioEventSender,
    state: &SharedPlaybackState,
    generation: &super::identity::PlaybackGenerationController,
    current_buffer: &mut Option<SharedBuffer>,
    current_context: &mut Option<super::identity::PlaybackContext>,
    finished_guard: &mut FinishedGuard,
    preloaded_sinks: &mut HashMap<u64, PreloadedSink>,
    scheduled_transition: &mut Option<ScheduledPreloadedTransition>,
    transition_scheduler: &mut super::automix::AudioClockScheduler,
    force_tick: bool,
) -> bool {
    let (latest_volume, tick_pending, shutdown) = latest_controls.take();
    if let Some(volume) = latest_volume {
        player.set_volume(volume);
        state.set_volume(volume);
    }
    while let Some(update) = buffer_mailbox.take_latest() {
        handle_buffer_data_update(
            player,
            event_tx,
            state,
            generation,
            current_buffer.as_ref(),
            update,
        );
    }
    if tick_pending || force_tick {
        process_tick(
            player,
            state,
            event_tx,
            generation,
            current_buffer.as_ref(),
            current_context.as_ref(),
        );
    }
    if let Some(context) = shutdown {
        cancel_scheduled_transition(scheduled_transition, transition_scheduler, preloaded_sinks);
        handle_shutdown(
            player,
            event_tx,
            state,
            current_buffer,
            preloaded_sinks,
            context,
        );
        return true;
    }

    if let Some(action) = transition_scheduler.poll(player.get_info().position)
        && scheduled_transition
            .as_ref()
            .is_some_and(|scheduled| scheduled.transition.group == action.group)
    {
        let scheduled = scheduled_transition
            .take()
            .expect("matching scheduled transition exists");
        let owner_current = generation.accepts(&scheduled.owner);
        if owner_current
            && generation.accepts_preload(&scheduled.identity)
            && preloaded_sinks
                .get(&scheduled.identity.request_id)
                .is_some_and(|preloaded| preloaded.identity == scheduled.identity)
        {
            if let Some(context) = generation.activate_preloaded_generation(&scheduled.identity) {
                let mut transition = scheduled.transition;
                if action.underrun && transition.kind == super::automix::TransitionKind::Automix {
                    transition =
                        super::automix::TransitionDirective::baseline_natural(transition.group);
                }
                let _ =
                    finished_guard.try_mark(context.generation, FinishReason::TransitionDisposed);
                finished_guard.reset();
                if let Some(old_buffer) = current_buffer.as_ref() {
                    old_buffer.clear_buffer_callback();
                }
                *current_buffer = preloaded_sinks
                    .get(&scheduled.identity.request_id)
                    .and_then(|preloaded| preloaded.shared_buffer.clone());
                *current_context = Some(context.clone());
                handle_play_preloaded(
                    player,
                    buffer_mailbox,
                    event_tx,
                    state,
                    preloaded_sinks,
                    context,
                    scheduled.playback_request_id,
                    scheduled.identity,
                    scheduled.fade_in,
                    transition,
                );
            } else {
                release_preloaded_identity(preloaded_sinks, &scheduled.identity);
            }
        } else {
            release_preloaded_identity(preloaded_sinks, &scheduled.identity);
            if owner_current {
                let _ = event_tx.send(AudioEvent::Error {
                    context: scheduled.owner,
                    request_id: Some(scheduled.playback_request_id),
                    message: "Scheduled preload became unavailable before its deadline".to_string(),
                    error_kind: None,
                });
            }
        }
    }

    let _ = player.poll_transition();

    if player.poll_pending_pause() {
        update_state_from_player(player, state);
        if let Some(context) = current_context.as_ref()
            && generation.accepts(context)
        {
            let _ = event_tx.send(AudioEvent::Paused {
                context: context.clone(),
                request_id: None,
                position: player.get_info().position,
            });
        }
    }

    check_playback_finished(
        player,
        event_tx,
        state,
        current_buffer.as_ref(),
        current_context.as_ref(),
        generation,
        finished_guard,
    );
    false
}

fn release_preloaded_identity(
    preloaded_sinks: &mut HashMap<u64, PreloadedSink>,
    identity: &super::identity::PreloadIdentity,
) {
    if preloaded_sinks
        .get(&identity.request_id)
        .is_some_and(|preloaded| preloaded.identity == *identity)
        && let Some(preloaded) = preloaded_sinks.remove(&identity.request_id)
        && let Some(shared_buffer) = preloaded.shared_buffer
    {
        shared_buffer.clear_buffer_callback();
    }
}

fn cancel_scheduled_transition(
    scheduled: &mut Option<ScheduledPreloadedTransition>,
    scheduler: &mut super::automix::AudioClockScheduler,
    preloaded_sinks: &mut HashMap<u64, PreloadedSink>,
) {
    if let Some(cancelled) = scheduled.take() {
        scheduler.cancel(cancelled.transition.group);
        release_preloaded_identity(preloaded_sinks, &cancelled.identity);
    }
}

fn process_tick(
    player: &mut AudioPlayer,
    state: &SharedPlaybackState,
    event_tx: &AudioEventSender,
    generation: &super::identity::PlaybackGenerationController,
    current_buffer: Option<&SharedBuffer>,
    current_context: Option<&super::identity::PlaybackContext>,
) {
    if let Some(buf) = current_buffer
        && let Some(context) = current_context.cloned()
    {
        check_buffer_status(player, state, event_tx, buf, generation, context);
    }

    let info = player.get_info();
    state.set_position(info.position);
    let current_status = state.get_info().status;
    if !matches!(
        current_status,
        PlaybackStatus::Buffering { .. } | PlaybackStatus::Paused
    ) {
        state.set_status(info.status);
    }
}

fn handle_shutdown(
    player: &mut AudioPlayer,
    event_tx: &AudioEventSender,
    state: &SharedPlaybackState,
    current_buffer: &mut Option<SharedBuffer>,
    preloaded_sinks: &mut HashMap<u64, PreloadedSink>,
    context: super::identity::PlaybackContext,
) {
    if let Some(old_buffer) = current_buffer.take() {
        old_buffer.clear_buffer_callback();
    }
    for (_, preloaded) in preloaded_sinks.drain() {
        if let Some(shared_buffer) = preloaded.shared_buffer {
            shared_buffer.clear_buffer_callback();
        }
    }
    player.stop();
    update_state_from_player(player, state);
    state.set_current_path(None);
    let _ = event_tx.send(AudioEvent::Stopped { context });
}

// ============ Command Handlers ============

#[allow(clippy::too_many_arguments)] // Command handler mirrors the protocol payload.
fn handle_play(
    player: &mut AudioPlayer,
    event_tx: &AudioEventSender,
    state: &SharedPlaybackState,
    context: super::identity::PlaybackContext,
    request_id: u64,
    path: PathBuf,
    fade_in: bool,
    track_gain: f32,
) {
    state.set_pending_seek(None);
    state.set_buffer_bytes(1, 1);

    match player.play_with_fade(path.clone(), fade_in, track_gain) {
        Ok(_) => {
            update_state_from_player(player, state);
            state.set_current_path(Some(path.clone()));
            let _ = event_tx.send(AudioEvent::Started {
                context: context.clone(),
                request_id,
                path: Some(path),
            });
        }
        Err(e) => {
            let kind = super::player::classify_playback_error(&e);
            let _ = event_tx.send(AudioEvent::Error {
                context: context.clone(),
                request_id: Some(request_id),
                message: e,
                error_kind: Some(kind),
            });
        }
    }
}

#[allow(clippy::too_many_arguments)] // Command handler mirrors the protocol payload.
fn handle_load_paused(
    player: &mut AudioPlayer,
    event_tx: &AudioEventSender,
    state: &SharedPlaybackState,
    context: super::identity::PlaybackContext,
    request_id: u64,
    path: PathBuf,
    position: Duration,
    track_gain: f32,
) {
    state.set_pending_seek(None);
    state.set_buffer_bytes(1, 1);

    match player.load_paused(path.clone(), position, track_gain) {
        Ok(_) => {
            update_state_from_player(player, state);
            state.set_current_path(Some(path));
            let _ = event_tx.send(AudioEvent::Paused {
                context: context.clone(),
                request_id: Some(request_id),
                position: player.get_info().position,
            });
        }
        Err(e) => {
            let kind = super::player::classify_playback_error(&e);
            let _ = event_tx.send(AudioEvent::Error {
                context: context.clone(),
                request_id: Some(request_id),
                message: e,
                error_kind: Some(kind),
            });
        }
    }
}

#[allow(clippy::too_many_arguments)] // Command handler mirrors the protocol payload.
fn handle_play_at(
    player: &mut AudioPlayer,
    event_tx: &AudioEventSender,
    state: &SharedPlaybackState,
    context: super::identity::PlaybackContext,
    request_id: u64,
    path: PathBuf,
    position: Duration,
    fade_in: bool,
    track_gain: f32,
) {
    state.set_pending_seek(None);
    state.set_buffer_bytes(1, 1);

    match player.play_from_position_with_fade(path.clone(), position, fade_in, track_gain) {
        Ok(seek_error) => {
            update_state_from_player(player, state);
            state.set_current_path(Some(path.clone()));
            let _ = event_tx.send(AudioEvent::Started {
                context: context.clone(),
                request_id,
                path: Some(path),
            });
            if let Some(error) = seek_error {
                let kind = super::player::classify_playback_error(&error);
                let _ = event_tx.send(AudioEvent::Error {
                    context: context.clone(),
                    request_id: Some(request_id),
                    message: error,
                    error_kind: Some(kind),
                });
            }
        }
        Err(e) => {
            let kind = super::player::classify_playback_error(&e);
            let _ = event_tx.send(AudioEvent::Error {
                context: context.clone(),
                request_id: Some(request_id),
                message: e,
                error_kind: Some(kind),
            });
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_streaming_preparation_result(
    prepared: StreamingPreparationResult,
    player: &mut AudioPlayer,
    buffer_mailbox: &BufferDataMailbox,
    event_tx: &AudioEventSender,
    state: &SharedPlaybackState,
    generation: &super::identity::PlaybackGenerationController,
    current_buffer: &mut Option<SharedBuffer>,
    current_context: &mut Option<super::identity::PlaybackContext>,
    pause_after_preparation: &mut Option<super::identity::PlaybackContext>,
    finished_guard: &mut FinishedGuard,
    preloaded_sinks: &mut HashMap<u64, PreloadedSink>,
    scheduled_transition: &mut Option<ScheduledPreloadedTransition>,
    transition_scheduler: &mut super::automix::AudioClockScheduler,
) {
    match prepared {
        StreamingPreparationResult::Playback(prepared) => {
            let PreparedPlayback {
                context,
                request_id,
                shared_buffer,
                source,
                duration,
                cache_path,
                track_gain,
                mode,
            } = prepared;
            if !generation.accepts(&context) || context.cancellation.is_cancelled() {
                tracing::debug!(
                    request_id,
                    generation = context.generation.0,
                    "Discarded stale streaming playback preparation"
                );
                take_pending_preparation_pause(pause_after_preparation, &context);
                return;
            }

            let pause_requested = take_pending_preparation_pause(pause_after_preparation, &context);

            let source = match source {
                Ok(source) => source,
                Err(error) => {
                    let kind = super::player::classify_playback_error(&error);
                    let _ = event_tx.send(AudioEvent::Error {
                        context,
                        request_id: Some(request_id),
                        message: error,
                        error_kind: Some(kind),
                    });
                    return;
                }
            };

            cancel_scheduled_transition(
                scheduled_transition,
                transition_scheduler,
                preloaded_sinks,
            );
            finished_guard.reset();
            if let Some(old_buffer) = current_buffer.take() {
                old_buffer.clear_buffer_callback();
            }

            let loaded = match mode {
                PlaybackPreparationMode::Play { fade_in } => {
                    if pause_requested {
                        handle_load_paused_streaming(
                            player,
                            buffer_mailbox,
                            event_tx,
                            state,
                            context.clone(),
                            request_id,
                            source,
                            shared_buffer.clone(),
                            duration,
                            cache_path,
                            Duration::ZERO,
                            track_gain,
                        )
                    } else {
                        handle_play_streaming(
                            player,
                            buffer_mailbox,
                            event_tx,
                            state,
                            context.clone(),
                            request_id,
                            source,
                            shared_buffer.clone(),
                            duration,
                            cache_path,
                            fade_in,
                            track_gain,
                        )
                    }
                }
                PlaybackPreparationMode::LoadPaused { position } => handle_load_paused_streaming(
                    player,
                    buffer_mailbox,
                    event_tx,
                    state,
                    context.clone(),
                    request_id,
                    source,
                    shared_buffer.clone(),
                    duration,
                    cache_path,
                    position,
                    track_gain,
                ),
            };

            if loaded {
                *current_buffer = Some(shared_buffer);
                *current_context = Some(context);
            } else {
                *current_context = None;
            }
        }
        StreamingPreparationResult::Preload(prepared) => {
            let PreparedPreload {
                identity,
                shared_buffer,
                source,
                duration,
                track_gain,
            } = prepared;
            if !generation.accepts_preload(&identity) || identity.cancellation.is_cancelled() {
                tracing::debug!(
                    request_id = identity.request_id,
                    generation = identity.generation.0,
                    "Discarded stale streaming preload preparation"
                );
                return;
            }
            match source {
                Ok(source) => handle_create_preload_sink_streaming(
                    player,
                    event_tx,
                    preloaded_sinks,
                    identity,
                    source,
                    shared_buffer,
                    duration,
                    track_gain,
                ),
                Err(error) => {
                    let _ = event_tx.send(AudioEvent::PreloadFailed { identity, error });
                }
            }
        }
    }
}

fn take_pending_preparation_pause(
    pending: &mut Option<super::identity::PlaybackContext>,
    context: &super::identity::PlaybackContext,
) -> bool {
    if pending.as_ref() == Some(context) {
        *pending = None;
        true
    } else {
        false
    }
}

#[allow(clippy::too_many_arguments)] // Command handler mirrors the protocol payload.
fn handle_play_streaming(
    player: &mut AudioPlayer,
    buffer_mailbox: &BufferDataMailbox,
    event_tx: &AudioEventSender,
    state: &SharedPlaybackState,
    context: super::identity::PlaybackContext,
    request_id: u64,
    source: PreparedStreamingSource,
    shared_buffer: SharedBuffer,
    duration: Duration,
    cache_path: Option<PathBuf>,
    fade_in: bool,
    track_gain: f32,
) -> bool {
    // Clear pending seek from previous track (important for correct display_position)
    state.set_pending_seek(None);

    // Reset buffer state from previous track before setting up new callback
    // This ensures UI shows fresh progress for the new track
    state.set_buffer_bytes(0, 0);

    // Set up buffer callback to send BufferDataAvailable command
    setup_buffer_callback(&shared_buffer, buffer_mailbox, &context);

    // Initialize buffer progress (may be 0 if HTTP response not yet received)
    let downloaded = shared_buffer.cached_bytes();
    let total = shared_buffer.total_size();
    state.set_buffer_bytes(downloaded, total);

    // Send initial progress event if we have data
    if total > 0 {
        let progress = downloaded as f32 / total as f32;
        let _ = event_tx.send(AudioEvent::BufferProgress {
            context: context.clone(),
            downloaded,
            total,
            progress,
        });
    }

    match player.play_prepared_streaming(source, duration, cache_path.clone(), fade_in, track_gain)
    {
        Ok(_) => {
            update_state_from_player(player, state);
            state.set_current_path(cache_path);
            let _ = event_tx.send(AudioEvent::Started {
                context: context.clone(),
                request_id,
                path: None,
            });
            true
        }
        Err(e) => {
            shared_buffer.clear_buffer_callback();
            let kind = super::player::classify_playback_error(&e);
            let _ = event_tx.send(AudioEvent::Error {
                context: context.clone(),
                request_id: Some(request_id),
                message: e,
                error_kind: Some(kind),
            });
            false
        }
    }
}

#[allow(clippy::too_many_arguments)] // Command handler mirrors the protocol payload.
fn handle_load_paused_streaming(
    player: &mut AudioPlayer,
    buffer_mailbox: &BufferDataMailbox,
    event_tx: &AudioEventSender,
    state: &SharedPlaybackState,
    context: super::identity::PlaybackContext,
    request_id: u64,
    source: PreparedStreamingSource,
    shared_buffer: SharedBuffer,
    duration: Duration,
    cache_path: Option<PathBuf>,
    position: Duration,
    track_gain: f32,
) -> bool {
    state.set_pending_seek(None);
    state.set_buffer_bytes(0, 0);

    setup_buffer_callback(&shared_buffer, buffer_mailbox, &context);

    let downloaded = shared_buffer.cached_bytes();
    let total = shared_buffer.total_size();
    state.set_buffer_bytes(downloaded, total);

    if total > 0 {
        let progress = downloaded as f32 / total as f32;
        let _ = event_tx.send(AudioEvent::BufferProgress {
            context: context.clone(),
            downloaded,
            total,
            progress,
        });
    }

    match player.load_prepared_streaming_paused(
        source,
        duration,
        cache_path.clone(),
        position,
        track_gain,
    ) {
        Ok(seek_error) => {
            update_state_from_player(player, state);
            state.set_current_path(cache_path);
            let _ = event_tx.send(AudioEvent::Paused {
                context: context.clone(),
                request_id: Some(request_id),
                position: player.get_info().position,
            });
            if let Some(error) = seek_error {
                let kind = super::player::classify_playback_error(&error);
                let _ = event_tx.send(AudioEvent::Error {
                    context: context.clone(),
                    request_id: Some(request_id),
                    message: error,
                    error_kind: Some(kind),
                });
            }
            true
        }
        Err(e) => {
            shared_buffer.clear_buffer_callback();
            let kind = super::player::classify_playback_error(&e);
            let _ = event_tx.send(AudioEvent::Error {
                context: context.clone(),
                request_id: Some(request_id),
                message: e,
                error_kind: Some(kind),
            });
            false
        }
    }
}

fn handle_seek(
    player: &mut AudioPlayer,
    event_tx: &AudioEventSender,
    state: &SharedPlaybackState,
    generation: &super::identity::PlaybackGenerationController,
    context: super::identity::PlaybackContext,
    nonce: super::identity::SeekNonce,
    position: Duration,
) {
    if !generation.accepts_seek(&context, nonce) {
        return;
    }
    let _ = event_tx.send(AudioEvent::SeekStarted {
        context: context.clone(),
        nonce,
        target_position: position,
    });

    match player.seek(position) {
        Ok(_) => {
            if !generation.accepts_seek(&context, nonce) {
                return;
            }
            state.set_position(position);
            state.set_pending_seek(None);
            let _ = event_tx.send(AudioEvent::SeekComplete {
                context: context.clone(),
                nonce,
                position,
            });
        }
        Err(e) => {
            if !generation.accepts_seek(&context, nonce) {
                return;
            }
            let _ = event_tx.send(AudioEvent::SeekFailed {
                context: context.clone(),
                nonce,
                error: e,
            });
        }
    }
}

fn handle_create_preload_sink(
    player: &AudioPlayer,
    event_tx: &AudioEventSender,
    preloaded_sinks: &mut HashMap<u64, PreloadedSink>,
    identity: super::identity::PreloadIdentity,
    path: PathBuf,
    track_gain: f32,
) {
    let request_id = identity.request_id;
    match player.create_preload_sink(&path, track_gain) {
        Ok((sink, duration, runtime)) => {
            // Store the sink for later playback
            preloaded_sinks.insert(
                request_id,
                PreloadedSink {
                    identity: identity.clone(),
                    sink,
                    duration,
                    path: path.clone(),
                    track_gain,
                    runtime,
                    is_streaming: false,
                    shared_buffer: None, // Local files don't have shared buffer
                },
            );
            tracing::debug!(
                "Preload sink created: request_id={}, path={:?}",
                request_id,
                path
            );
            let _ = event_tx.send(AudioEvent::PreloadReady {
                identity,
                duration,
                path,
            });
        }
        Err(e) => {
            let _ = event_tx.send(AudioEvent::PreloadFailed { identity, error: e });
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_create_preload_sink_streaming(
    player: &AudioPlayer,
    event_tx: &AudioEventSender,
    preloaded_sinks: &mut HashMap<u64, PreloadedSink>,
    identity: super::identity::PreloadIdentity,
    source: PreparedStreamingSource,
    shared_buffer: SharedBuffer,
    duration: Duration,
    track_gain: f32,
) {
    let request_id = identity.request_id;
    match player.create_preload_sink_prepared_streaming(source, duration, track_gain) {
        Ok((sink, actual_duration, runtime)) => {
            // For streaming, we don't have a real path, use a placeholder
            let path = PathBuf::from(format!("streaming://{}", request_id));
            preloaded_sinks.insert(
                request_id,
                PreloadedSink {
                    identity: identity.clone(),
                    sink,
                    duration: actual_duration,
                    path: path.clone(),
                    track_gain,
                    runtime,
                    is_streaming: true,
                    shared_buffer: Some(shared_buffer), // Save for callback setup on play
                },
            );
            tracing::debug!("Preload streaming sink created: request_id={}", request_id);
            let _ = event_tx.send(AudioEvent::PreloadReady {
                identity,
                duration: actual_duration,
                path,
            });
        }
        Err(e) => {
            let _ = event_tx.send(AudioEvent::PreloadFailed { identity, error: e });
        }
    }
}

#[allow(clippy::too_many_arguments)] // Command handler mirrors the protocol payload.
fn handle_play_preloaded(
    player: &mut AudioPlayer,
    buffer_mailbox: &BufferDataMailbox,
    event_tx: &AudioEventSender,
    state: &SharedPlaybackState,
    preloaded_sinks: &mut HashMap<u64, PreloadedSink>,
    context: super::identity::PlaybackContext,
    playback_request_id: u64,
    identity: super::identity::PreloadIdentity,
    fade_in: bool,
    transition: super::automix::TransitionDirective,
) {
    let request_id = identity.request_id;
    if let Some(preloaded) = preloaded_sinks.remove(&request_id) {
        if preloaded.identity != identity {
            if let Some(shared_buffer) = preloaded.shared_buffer {
                shared_buffer.clear_buffer_callback();
            }
            tracing::warn!(
                "PlayPreloaded: identity mismatch for request_id {}",
                request_id
            );
            let _ = event_tx.send(AudioEvent::Error {
                context,
                request_id: Some(playback_request_id),
                message: format!("Preloaded sink identity mismatch: {}", request_id),
                error_kind: None,
            });
            return;
        }

        let PreloadedSink {
            identity: _,
            sink,
            duration,
            path,
            track_gain,
            runtime,
            is_streaming,
            shared_buffer,
        } = preloaded;

        // Clear pending seek from previous track (important for correct display_position)
        state.set_pending_seek(None);

        // Set up buffer callback for streaming preloads
        if let Some(shared_buffer) = &shared_buffer {
            // Reset buffer progress for new track
            let downloaded = shared_buffer.downloaded();
            let total = shared_buffer.total_size();
            state.set_buffer_bytes(downloaded, total);

            // Set up callback to send BufferDataAvailable command (DRY: single callback setup point)
            setup_buffer_callback(shared_buffer, buffer_mailbox, &context);

            tracing::info!(
                "Preload streaming: set up buffer callback, downloaded={}/{}",
                downloaded,
                total
            );
        } else {
            state.set_buffer_bytes(1, 1);
        }

        match player.play_preloaded_sink(
            sink,
            duration,
            path.clone(),
            is_streaming,
            fade_in,
            track_gain,
            runtime,
            transition,
        ) {
            Ok(_) => {
                update_state_from_player(player, state);
                state.set_current_path(Some(path.clone()));
                let _ = event_tx.send(AudioEvent::Started {
                    context: context.clone(),
                    request_id: playback_request_id,
                    path: Some(path),
                });
            }
            Err(e) => {
                let kind = super::player::classify_playback_error(&e);
                let _ = event_tx.send(AudioEvent::Error {
                    context: context.clone(),
                    request_id: Some(playback_request_id),
                    message: e,
                    error_kind: Some(kind),
                });
            }
        }
    } else {
        tracing::warn!("PlayPreloaded: request_id {} not found", request_id);
        let _ = event_tx.send(AudioEvent::Error {
            context: context.clone(),
            request_id: Some(playback_request_id),
            message: format!("Preloaded sink not found: {}", request_id),
            error_kind: None,
        });
    }
}

fn handle_switch_device(
    player: &mut AudioPlayer,
    event_tx: &AudioEventSender,
    state: &SharedPlaybackState,
    device_name: Option<String>,
) {
    match player.switch_device(device_name.as_deref()) {
        Ok(restore_state) => {
            update_state_from_player(player, state);
            let _ = event_tx.send(AudioEvent::DeviceSwitched { restore_state });
        }
        Err(e) => {
            let _ = event_tx.send(AudioEvent::DeviceSwitchFailed { error: e });
        }
    }
}

// ============ Helpers ============

/// Set up buffer callback to send BufferDataAvailable command
///
/// This is the single callback setup point (DRY principle).
/// The callback only sends commands to Audio Thread, it does not modify state directly.
fn setup_buffer_callback(
    shared_buffer: &SharedBuffer,
    buffer_mailbox: &BufferDataMailbox,
    context: &super::identity::PlaybackContext,
) {
    let mailbox = buffer_mailbox.clone();
    let context = context.clone();
    shared_buffer.set_buffer_callback(move |event| {
        use super::streaming::BufferEvent;
        if let BufferEvent::DataAppended { downloaded, total } = event {
            mailbox.publish(BufferDataUpdate {
                context: context.clone(),
                downloaded,
                total,
            });
        }
    });
}

fn handle_buffer_data_update(
    player: &mut AudioPlayer,
    event_tx: &AudioEventSender,
    state: &SharedPlaybackState,
    generation: &super::identity::PlaybackGenerationController,
    current_buffer: Option<&SharedBuffer>,
    update: BufferDataUpdate,
) {
    let context = update.context;
    if context.cancellation.is_cancelled() || !generation.accepts(&context) {
        return;
    }

    let downloaded = update.downloaded;
    let total = update.total;
    state.set_buffer_bytes(downloaded, total);

    let progress = if total > 0 {
        downloaded as f32 / total as f32
    } else {
        0.0
    };
    let _ = event_tx.send(AudioEvent::BufferProgress {
        context: context.clone(),
        downloaded,
        total,
        progress,
    });

    if let Some(buf) = current_buffer
        && let PlaybackStatus::Buffering { .. } = state.get_info().status
    {
        let remaining_bytes = buf.buffered_ahead();
        if buf.is_complete() || buf.remote_eof_reached() || remaining_bytes >= HIGH_WATER_MARK_BYTES
        {
            exit_buffering(player, state, event_tx, generation, context);
        }
    }
}

/// Check buffer status and enter/exit Buffering state as needed
///
/// Called periodically from Tick handler for streaming playback.
///
/// Uses hysteresis (watermark) mechanism to prevent rapid state oscillation:
/// - Enter Buffering when decoder-visible data < LOW_WATER_MARK_BYTES
/// - Exit Buffering when decoder-visible data >= HIGH_WATER_MARK_BYTES
///
/// IMPORTANT: Uses state.get_info().status (SharedPlaybackState) instead of
/// player.get_info().status because enter_buffering/exit_buffering only update
/// SharedPlaybackState, not player's internal status. Using player's status
/// would cause repeated enter_buffering calls.
fn check_buffer_status(
    player: &mut AudioPlayer,
    state: &SharedPlaybackState,
    event_tx: &AudioEventSender,
    buffer: &SharedBuffer,
    generation: &super::identity::PlaybackGenerationController,
    context: super::identity::PlaybackContext,
) {
    // Use SharedPlaybackState for status check (single source of truth)
    let current_status = state.get_info().status;
    // Use player for the user-visible position only.
    let player_info = player.get_info();
    let remaining_bytes = buffer.buffered_ahead();
    let source_has_all_required_bytes = buffer.is_complete() || buffer.remote_eof_reached();

    match &current_status {
        PlaybackStatus::Playing
            if should_enter_buffering(remaining_bytes, source_has_all_required_bytes) =>
        {
            tracing::info!(
                "Buffer low: remaining {} bytes < {} (low water mark), entering Buffering",
                remaining_bytes,
                LOW_WATER_MARK_BYTES
            );
            enter_buffering(
                player,
                state,
                event_tx,
                player_info.position,
                context.clone(),
            );
        }
        PlaybackStatus::Buffering { .. } => {
            // Check if we can exit Buffering (HIGH water mark)
            // Exit Buffering if:
            // 1. Remaining data exceeds HIGH_WATER_MARK_BYTES, OR
            // 2. Download is complete (no more data coming)
            if source_has_all_required_bytes {
                tracing::info!("Streaming window reached EOF, exiting Buffering");
                exit_buffering(player, state, event_tx, generation, context.clone());
            } else if remaining_bytes >= HIGH_WATER_MARK_BYTES {
                tracing::info!(
                    "Buffer sufficient: remaining {} bytes > {} (high water mark), exiting Buffering",
                    remaining_bytes,
                    HIGH_WATER_MARK_BYTES
                );
                exit_buffering(player, state, event_tx, generation, context.clone());
            }
            // Otherwise, stay in Buffering state and wait for more data
        }
        _ => {}
    }
}

fn should_enter_buffering(remaining_bytes: u64, source_has_all_required_bytes: bool) -> bool {
    remaining_bytes < LOW_WATER_MARK_BYTES && !source_has_all_required_bytes
}

/// Enter Buffering state
///
/// Pauses the Sink and sets status to Buffering.
fn enter_buffering(
    player: &mut AudioPlayer,
    state: &SharedPlaybackState,
    event_tx: &AudioEventSender,
    position: Duration,
    context: super::identity::PlaybackContext,
) {
    // Pause sink without changing player's internal status
    player.pause_sink();

    let old_status = state.get_info().status;
    let new_status = PlaybackStatus::Buffering { position };
    state.set_status(new_status.clone());

    let _ = event_tx.send(AudioEvent::BufferingStarted {
        context: context.clone(),
        position,
    });
    let _ = event_tx.send(AudioEvent::StateChanged {
        context: context.clone(),
        old_status,
        new_status,
    });

    tracing::info!("Entered Buffering state at position {:?}", position);
}

/// Exit Buffering state
///
/// If there's a pending seek target, executes the seek first.
/// Then resumes the Sink and sets status to Playing.
fn exit_buffering(
    player: &mut AudioPlayer,
    state: &SharedPlaybackState,
    event_tx: &AudioEventSender,
    generation: &super::identity::PlaybackGenerationController,
    context: super::identity::PlaybackContext,
) {
    let old_status = state.get_info().status;

    if let Some(pending) = state.pending_seek() {
        if pending.context.generation != context.generation
            || pending.context.cancellation.is_cancelled()
            || !generation.accepts_seek(&pending.context, pending.nonce)
        {
            state.set_pending_seek(None);
        } else {
            let target_position = pending.target;
            tracing::info!(
                "exit_buffering: executing pending seek to {:?} (nonce {:?})",
                target_position,
                pending.nonce
            );
            state.set_pending_seek(None);
            match player.seek(target_position) {
                Ok(_) => {
                    state.set_position(target_position);
                    let _ = event_tx.send(AudioEvent::SeekComplete {
                        context: pending.context.clone(),
                        nonce: pending.nonce,
                        position: target_position,
                    });
                }
                Err(e) => {
                    tracing::error!("exit_buffering: seek failed: {}", e);
                    let _ = event_tx.send(AudioEvent::SeekFailed {
                        context: pending.context.clone(),
                        nonce: pending.nonce,
                        error: e,
                    });
                }
            }
        }
    }

    // Resume sink
    player.play_sink();

    state.set_status(PlaybackStatus::Playing);

    let _ = event_tx.send(AudioEvent::BufferingEnded {
        context: context.clone(),
    });
    let _ = event_tx.send(AudioEvent::StateChanged {
        context: context.clone(),
        old_status,
        new_status: PlaybackStatus::Playing,
    });

    tracing::info!("Exited Buffering state, resumed Playing");
}

/// Update shared state from player's current info
fn update_state_from_player(player: &AudioPlayer, state: &SharedPlaybackState) {
    let info = player.get_info();
    state.update_from_info(&info);
}

/// Check if playback finished and send event
///
/// For streaming playback, if sink is empty but download is not complete,
/// this indicates we've caught up with the download
fn check_playback_finished(
    player: &AudioPlayer,
    event_tx: &AudioEventSender,
    state: &SharedPlaybackState,
    current_buffer: Option<&SharedBuffer>,
    context: Option<&super::identity::PlaybackContext>,
    generation: &super::identity::PlaybackGenerationController,
    finished_guard: &mut FinishedGuard,
) {
    // For streaming playback, check if we should enter Buffering instead of finishing
    if let Some(buffer) = current_buffer
        && !buffer.is_complete()
        && !buffer.remote_eof_reached()
    {
        return;
    }

    if player.is_finished()
        && let Some(context) = context
        && generation.accepts(context)
        && finished_guard.try_mark(context.generation, FinishReason::Natural)
    {
        state.set_status(PlaybackStatus::Stopped);
        let _ = event_tx.send(AudioEvent::Finished {
            context: context.clone(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maintenance_deadline_wakes_without_a_critical_command() {
        let (critical_tx, mut critical_rx) = audio_command_channel();
        let (result_tx, mut result_rx) =
            tokio::sync::mpsc::channel(STREAMING_PREPARATION_RESULT_CAPACITY);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap();

        let event = runtime.block_on(next_control_event(
            &mut critical_rx,
            &mut result_rx,
            tokio::time::Instant::now() + Duration::from_millis(5),
        ));

        assert!(matches!(event, ControlEvent::Maintenance));
        assert!(critical_rx.try_recv().is_err());
        drop(critical_tx);
        drop(result_tx);
    }

    #[test]
    fn pause_intent_is_consumed_only_by_its_preparation_context() {
        let generation = crate::audio::identity::PlaybackGenerationController::new();
        let first = generation.activate_generation();
        let second = generation.activate_generation();
        let mut pending = Some(first.clone());

        assert!(!take_pending_preparation_pause(&mut pending, &second));
        assert_eq!(pending, Some(first.clone()));
        assert!(take_pending_preparation_pause(&mut pending, &first));
        assert!(pending.is_none());
    }

    #[test]
    fn stalled_streaming_preparation_does_not_consume_critical_capacity() {
        let (result_tx, _result_rx) =
            tokio::sync::mpsc::channel(STREAMING_PREPARATION_RESULT_CAPACITY);
        let pool = StreamingPreparationPool::new(result_tx).unwrap();
        let generation = crate::audio::identity::PlaybackGenerationController::new();
        let context = generation.activate_generation();
        let shared = SharedBuffer::new(1024);

        let started = std::time::Instant::now();
        pool.prepare_playback(PlaybackPreparationRequest {
            context: context.clone(),
            request_id: 1,
            buffer: StreamingBuffer::new(shared.clone()),
            duration: Duration::from_secs(1),
            cache_path: None,
            track_gain: 1.0,
            mode: PlaybackPreparationMode::Play { fade_in: false },
        })
        .unwrap();
        assert!(started.elapsed() < Duration::from_millis(100));

        let (critical_tx, mut critical_rx) = audio_command_channel();
        critical_tx
            .try_send(AudioCommand::Pause {
                context,
                fade_out: false,
            })
            .unwrap();
        assert!(matches!(
            critical_rx.try_recv().unwrap(),
            AudioCommand::Pause { .. }
        ));

        shared.cancel();
        drop(pool);
    }

    #[test]
    fn finished_guard_is_exactly_once_per_generation() {
        let mut guard = FinishedGuard::default();
        assert!(!guard.try_mark(
            crate::audio::identity::PlaybackGeneration(1),
            FinishReason::Stop
        ));
        assert!(!guard.try_mark(
            crate::audio::identity::PlaybackGeneration(1),
            FinishReason::SeekRebuild
        ));
        assert!(!guard.try_mark(
            crate::audio::identity::PlaybackGeneration(1),
            FinishReason::TransitionDisposed
        ));
        assert!(!guard.try_mark(
            crate::audio::identity::PlaybackGeneration(1),
            FinishReason::Stale
        ));
        assert!(guard.try_mark(
            crate::audio::identity::PlaybackGeneration(1),
            FinishReason::Natural
        ));
        assert!(!guard.try_mark(
            crate::audio::identity::PlaybackGeneration(1),
            FinishReason::Natural
        ));
        assert!(guard.try_mark(
            crate::audio::identity::PlaybackGeneration(2),
            FinishReason::Natural
        ));
        guard.reset();
        assert!(guard.try_mark(
            crate::audio::identity::PlaybackGeneration(2),
            FinishReason::Natural
        ));
    }

    #[test]
    fn resume_buffering_uses_low_water_mark_without_hysteresis_deadlock() {
        assert!(should_enter_buffering(LOW_WATER_MARK_BYTES - 1, false));
        assert!(!should_enter_buffering(LOW_WATER_MARK_BYTES, false));
        assert!(!should_enter_buffering(HIGH_WATER_MARK_BYTES - 1, false));
        assert!(!should_enter_buffering(0, true));
    }
}
