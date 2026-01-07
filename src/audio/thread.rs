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
use std::time::Duration;

use rodio::Sink;

use super::PlaybackStatus;
use super::chain::AudioProcessingChain;
use super::events::{
    AudioCommand, AudioCommandReceiver, AudioEvent, AudioEventSender, SharedPlaybackState,
    audio_command_channel, audio_event_channel,
};
use super::handle::AudioHandle;
use super::player::AudioPlayer;
use super::streaming::{HIGH_WATER_MARK_BYTES, LOW_WATER_MARK_BYTES, SharedBuffer};

/// Preloaded sink with metadata
struct PreloadedSink {
    sink: Sink,
    duration: Duration,
    #[allow(dead_code)]
    path: PathBuf,
    is_streaming: bool,
    shared_buffer: Option<SharedBuffer>,
}

pub struct AudioThreadHandle {
    pub handle: AudioHandle,
    pub event_rx: Option<super::events::AudioEventReceiver>,
    #[allow(dead_code)]
    thread_handle: Option<JoinHandle<()>>,
}

impl AudioThreadHandle {
    pub fn take_event_rx(&mut self) -> Option<super::events::AudioEventReceiver> {
        self.event_rx.take()
    }

    #[allow(dead_code)]
    pub fn join(mut self, timeout: Duration) -> Result<(), String> {
        if let Some(handle) = self.thread_handle.take() {
            self.handle.stop();

            let start = std::time::Instant::now();
            loop {
                if handle.is_finished() {
                    let _ = handle.join();
                    return Ok(());
                }
                if start.elapsed() > timeout {
                    return Err("Audio thread did not exit in time".to_string());
                }
                thread::sleep(Duration::from_millis(10));
            }
        }
        Ok(())
    }
}

impl Drop for AudioThreadHandle {
    fn drop(&mut self) {
        self.handle.stop();
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

    // Create shared state
    let state = SharedPlaybackState::new();
    let state_clone = state.clone();

    // Clone command_tx for use in buffer callbacks (DRY: single callback setup point)
    let command_tx_for_callbacks = command_tx.clone();

    // Create handle for UI
    let handle = AudioHandle::new(command_tx, state);

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
                    audio_thread_main(
                        player,
                        command_rx,
                        command_tx_for_callbacks,
                        event_tx,
                        state_clone,
                    );
                }
                Err(e) => {
                    tracing::error!("Failed to create audio player: {}", e);
                    let _ = event_tx.send(AudioEvent::Error { message: e });
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
/// This function blocks on `command_rx.blocking_recv()` and may also
/// block on audio operations (e.g., streaming seek).
fn audio_thread_main(
    mut player: AudioPlayer,
    mut command_rx: AudioCommandReceiver,
    command_tx: super::events::AudioCommandSender,
    event_tx: AudioEventSender,
    state: SharedPlaybackState,
) {
    tracing::info!("Audio thread started");

    // Storage for preloaded sinks (request_id -> PreloadedSink)
    let mut preloaded_sinks: HashMap<u64, PreloadedSink> = HashMap::new();

    // Current streaming buffer reference (for data availability checks)
    // Set when PlayStreaming command is processed, cleared on Play/Stop
    let mut current_buffer: Option<SharedBuffer> = None;

    // Process commands until channel closes
    while let Some(cmd) = command_rx.blocking_recv() {
        match cmd {
            AudioCommand::Play { path, fade_in } => {
                if let Some(ref old_buffer) = current_buffer {
                    old_buffer.clear_buffer_callback();
                }
                // Local file playback
                current_buffer = None;
                handle_play(&mut player, &event_tx, &state, path, fade_in);
            }

            AudioCommand::PlayStreaming {
                buffer,
                duration,
                cache_path,
            } => {
                if let Some(ref old_buffer) = current_buffer {
                    old_buffer.clear_buffer_callback();
                }
                // Store buffer reference for data availability checks
                let shared_buffer = buffer.shared().clone();
                handle_play_streaming(
                    &mut player,
                    &command_tx,
                    &event_tx,
                    &state,
                    buffer,
                    duration,
                    cache_path,
                );
                current_buffer = Some(shared_buffer);
            }

            AudioCommand::Pause { fade_out } => {
                if fade_out {
                    player.pause_with_fade(true);
                } else {
                    player.pause();
                }
                update_state_from_player(&player, &state);
                let pos = player.get_info().position;
                let _ = event_tx.send(AudioEvent::Paused { position: pos });
            }

            AudioCommand::Resume { fade_in } => {
                // Check data availability before resuming
                // Use HIGH water mark to ensure smooth playback after resume
                if let Some(ref buf) = current_buffer {
                    let info = player.get_info();
                    let byte_pos =
                        estimate_byte_position(info.position, buf.total_size(), info.duration);
                    let downloaded = buf.downloaded();
                    let remaining_bytes = downloaded.saturating_sub(byte_pos);

                    // Require HIGH water mark worth of data before resuming
                    // This prevents immediate re-buffering after resume
                    if remaining_bytes < HIGH_WATER_MARK_BYTES && !buf.is_complete() {
                        // Not enough data, enter Buffering instead of Playing
                        tracing::info!(
                            "Resume: remaining {} bytes < {} (high water mark), entering Buffering",
                            remaining_bytes,
                            HIGH_WATER_MARK_BYTES
                        );
                        enter_buffering(&mut player, &state, &event_tx, info.position);
                        continue;
                    }
                }

                if fade_in {
                    player.resume_with_fade(true);
                } else {
                    player.resume();
                }
                update_state_from_player(&player, &state);
                let _ = event_tx.send(AudioEvent::Resumed);
            }

            AudioCommand::Stop => {
                if let Some(ref old_buffer) = current_buffer {
                    old_buffer.clear_buffer_callback();
                }
                current_buffer = None;
                player.stop();
                update_state_from_player(&player, &state);
                state.set_current_path(None);
                let _ = event_tx.send(AudioEvent::Stopped);
            }

            AudioCommand::Seek { position } => {
                handle_seek(
                    &mut player,
                    &event_tx,
                    &state,
                    position,
                    current_buffer.as_ref(),
                );
            }

            AudioCommand::SetVolume { volume } => {
                player.set_volume(volume);
                state.set_volume(volume);
            }

            AudioCommand::SetTrackGain { gain } => {
                player.set_track_gain(gain);
            }

            AudioCommand::CreatePreloadSink { path, request_id } => {
                handle_create_preload_sink(
                    &player,
                    &event_tx,
                    &mut preloaded_sinks,
                    path,
                    request_id,
                );
            }

            AudioCommand::CreatePreloadSinkStreaming {
                buffer,
                duration,
                request_id,
            } => {
                handle_create_preload_sink_streaming(
                    &player,
                    &event_tx,
                    &mut preloaded_sinks,
                    buffer,
                    duration,
                    request_id,
                );
            }

            AudioCommand::PlayPreloaded { request_id, path } => {
                if let Some(ref old_buffer) = current_buffer {
                    old_buffer.clear_buffer_callback();
                }
                // Check if this is a streaming preload and store buffer reference
                if let Some(preloaded) = preloaded_sinks.get(&request_id) {
                    current_buffer = preloaded.shared_buffer.clone();
                }
                handle_play_preloaded_by_id(
                    &mut player,
                    &command_tx,
                    &event_tx,
                    &state,
                    &mut preloaded_sinks,
                    request_id,
                    path,
                );
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

            AudioCommand::Tick => {
                // Check buffer status for streaming playback
                if let Some(ref buf) = current_buffer {
                    check_buffer_status(&mut player, &state, &event_tx, buf);
                }

                // Update position in shared state
                let info = player.get_info();
                state.set_position(info.position);

                // Status sync with protection for "intent" states:
                // - Buffering: managed by check_buffer_status(), player doesn't know about it
                // - Paused: user intent, should only be changed by Resume command
                let current_status = state.get_info().status;
                if !matches!(
                    current_status,
                    PlaybackStatus::Buffering { .. } | PlaybackStatus::Paused
                ) {
                    state.set_status(info.status);
                }
            }

            AudioCommand::UpdatePausedPosition { position } => {
                player.update_paused_position(position);
                state.set_position(position);
            }

            AudioCommand::BufferDataAvailable { downloaded, total } => {
                // Update buffer progress in shared state
                state.set_buffer_bytes(downloaded, total);

                // Send progress event to UI
                let progress = if total > 0 {
                    downloaded as f32 / total as f32
                } else {
                    0.0
                };
                let _ = event_tx.send(AudioEvent::BufferProgress {
                    downloaded,
                    total,
                    progress,
                });

                // Check if we can exit Buffering (handles both normal buffering and pending seek)
                if let Some(ref buf) = current_buffer {
                    let current_status = state.get_info().status;

                    if let PlaybackStatus::Buffering { .. } = current_status {
                        let player_info = player.get_info();

                        // Determine which position to check buffer for:
                        // - If there's a pending seek, check buffer at target position
                        // - Otherwise, check buffer at current position
                        let check_position =
                            state.pending_seek_target().unwrap_or(player_info.position);
                        let byte_pos =
                            estimate_byte_position(check_position, total, player_info.duration);
                        let remaining_bytes = downloaded.saturating_sub(byte_pos);

                        if buf.is_complete() || remaining_bytes > HIGH_WATER_MARK_BYTES {
                            tracing::info!(
                                "BufferDataAvailable: {} bytes buffered after position {:?} (need {}), exiting Buffering",
                                remaining_bytes,
                                check_position,
                                HIGH_WATER_MARK_BYTES
                            );
                            exit_buffering(&mut player, &state, &event_tx);
                        }
                    }
                }
            }
        }

        // Check if playback finished after each command
        check_playback_finished(&player, &event_tx, &state, current_buffer.as_ref());
    }

    tracing::info!("Audio thread exiting (command channel closed)");
}

// ============ Command Handlers ============

fn handle_play(
    player: &mut AudioPlayer,
    event_tx: &AudioEventSender,
    state: &SharedPlaybackState,
    path: PathBuf,
    fade_in: bool,
) {
    state.set_pending_seek(None);
    state.set_buffer_bytes(1, 1);

    match player.play_with_fade(path.clone(), fade_in) {
        Ok(_) => {
            update_state_from_player(player, state);
            state.set_current_path(Some(path.clone()));
            let _ = event_tx.send(AudioEvent::Started { path: Some(path) });
        }
        Err(e) => {
            let _ = event_tx.send(AudioEvent::Error { message: e });
        }
    }
}

fn handle_play_streaming(
    player: &mut AudioPlayer,
    command_tx: &super::events::AudioCommandSender,
    event_tx: &AudioEventSender,
    state: &SharedPlaybackState,
    buffer: super::streaming::StreamingBuffer,
    duration: Duration,
    cache_path: Option<PathBuf>,
) {
    // Clear pending seek from previous track (important for correct display_position)
    state.set_pending_seek(None);

    // Reset buffer state from previous track before setting up new callback
    // This ensures UI shows fresh progress for the new track
    state.set_buffer_bytes(0, 0);

    // Get shared buffer reference for progress tracking
    let shared_buffer = buffer.shared().clone();

    // Set up buffer callback to send BufferDataAvailable command
    setup_buffer_callback(&shared_buffer, command_tx);

    // Initialize buffer progress (may be 0 if HTTP response not yet received)
    let downloaded = shared_buffer.downloaded();
    let total = shared_buffer.total_size();
    state.set_buffer_bytes(downloaded, total);

    // Send initial progress event if we have data
    if total > 0 {
        let progress = downloaded as f32 / total as f32;
        let _ = event_tx.send(AudioEvent::BufferProgress {
            downloaded,
            total,
            progress,
        });
    }

    match player.play_streaming(buffer, duration, cache_path.clone()) {
        Ok(_) => {
            update_state_from_player(player, state);
            state.set_current_path(cache_path);
            let _ = event_tx.send(AudioEvent::Started { path: None });
        }
        Err(e) => {
            let _ = event_tx.send(AudioEvent::Error { message: e });
        }
    }
}

fn handle_seek(
    player: &mut AudioPlayer,
    event_tx: &AudioEventSender,
    state: &SharedPlaybackState,
    position: Duration,
    current_buffer: Option<&SharedBuffer>,
) {
    // For streaming playback, check if target position has enough buffered data
    let is_streaming = player.is_streaming();

    if is_streaming {
        if let Some(buffer) = current_buffer {
            let current_info = player.get_info();
            let target_byte_pos =
                estimate_byte_position(position, buffer.total_size(), current_info.duration);
            let downloaded = buffer.downloaded();
            let buffered_after_target = downloaded.saturating_sub(target_byte_pos);

            // Use HIGH_WATER_MARK to ensure smooth playback after seek
            let has_enough_buffer =
                buffered_after_target >= HIGH_WATER_MARK_BYTES || buffer.is_complete();

            if has_enough_buffer {
                // Enough data buffered
                tracing::debug!(
                    "Seek to position {:?} (byte {}): buffered {} bytes after target, executing immediately",
                    position,
                    target_byte_pos,
                    buffered_after_target
                );

                let _ = event_tx.send(AudioEvent::SeekStarted {
                    target_position: position,
                });

                // Data is in memory, this will complete instantly (no blocking)
                match player.seek(position) {
                    Ok(_) => {
                        state.set_position(position);

                        // If we were in Buffering state with a pending seek, clear it and exit Buffering
                        let current_status = state.get_info().status;
                        if matches!(current_status, PlaybackStatus::Buffering { .. }) {
                            state.set_pending_seek(None);
                            // Resume playback since we successfully seeked to buffered position
                            player.play_sink();
                            state.set_status(PlaybackStatus::Playing);
                            let _ = event_tx.send(AudioEvent::BufferingEnded);
                            tracing::info!(
                                "Seek during Buffering to buffered position, exiting Buffering"
                            );
                        }

                        let _ = event_tx.send(AudioEvent::SeekComplete { position });
                    }
                    Err(e) => {
                        let _ = event_tx.send(AudioEvent::SeekFailed { error: e });
                    }
                }
                return;
            }

            tracing::info!(
                "Seek to position {:?} (byte {}): only {} bytes buffered after target (need {}), entering Buffering with pending seek",
                position,
                target_byte_pos,
                buffered_after_target,
                HIGH_WATER_MARK_BYTES
            );

            // Store pending seek target
            state.set_pending_seek(Some(position));

            // Emit seek started event
            let _ = event_tx.send(AudioEvent::SeekStarted {
                target_position: position,
            });

            // Enter Buffering state (reuse existing mechanism)
            // Use current position as the buffering position (we haven't seeked yet)
            enter_buffering(player, state, event_tx, current_info.position);

            return;
        }
    }

    // Local file playback
    let _ = event_tx.send(AudioEvent::SeekStarted {
        target_position: position,
    });

    match player.seek(position) {
        Ok(_) => {
            state.set_position(position);
            let _ = event_tx.send(AudioEvent::SeekComplete { position });
        }
        Err(e) => {
            let _ = event_tx.send(AudioEvent::SeekFailed { error: e });
        }
    }
}

fn handle_create_preload_sink(
    player: &AudioPlayer,
    event_tx: &AudioEventSender,
    preloaded_sinks: &mut HashMap<u64, PreloadedSink>,
    path: PathBuf,
    request_id: u64,
) {
    match player.create_preload_sink(&path) {
        Ok((sink, duration)) => {
            // Store the sink for later playback
            preloaded_sinks.insert(
                request_id,
                PreloadedSink {
                    sink,
                    duration,
                    path: path.clone(),
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
                request_id,
                duration,
                path,
            });
        }
        Err(e) => {
            let _ = event_tx.send(AudioEvent::PreloadFailed {
                request_id,
                error: e,
            });
        }
    }
}

fn handle_create_preload_sink_streaming(
    player: &AudioPlayer,
    event_tx: &AudioEventSender,
    preloaded_sinks: &mut HashMap<u64, PreloadedSink>,
    buffer: super::streaming::StreamingBuffer,
    duration: Duration,
    request_id: u64,
) {
    // Clone shared buffer before passing to decoder (for later callback setup)
    let shared_buffer = buffer.shared().clone();

    // This may block waiting for streaming data
    match player.create_preload_sink_streaming(buffer, duration) {
        Ok((sink, actual_duration)) => {
            // For streaming, we don't have a real path, use a placeholder
            let path = PathBuf::from(format!("streaming://{}", request_id));
            preloaded_sinks.insert(
                request_id,
                PreloadedSink {
                    sink,
                    duration: actual_duration,
                    path: path.clone(),
                    is_streaming: true,
                    shared_buffer: Some(shared_buffer), // Save for callback setup on play
                },
            );
            tracing::debug!("Preload streaming sink created: request_id={}", request_id);
            let _ = event_tx.send(AudioEvent::PreloadReady {
                request_id,
                duration: actual_duration,
                path,
            });
        }
        Err(e) => {
            let _ = event_tx.send(AudioEvent::PreloadFailed {
                request_id,
                error: e,
            });
        }
    }
}

fn handle_play_preloaded_by_id(
    player: &mut AudioPlayer,
    command_tx: &super::events::AudioCommandSender,
    event_tx: &AudioEventSender,
    state: &SharedPlaybackState,
    preloaded_sinks: &mut HashMap<u64, PreloadedSink>,
    request_id: u64,
    path: PathBuf,
) {
    if let Some(preloaded) = preloaded_sinks.remove(&request_id) {
        // Clear pending seek from previous track (important for correct display_position)
        state.set_pending_seek(None);

        // Set up buffer callback for streaming preloads
        if let Some(shared_buffer) = &preloaded.shared_buffer {
            // Reset buffer progress for new track
            let downloaded = shared_buffer.downloaded();
            let total = shared_buffer.total_size();
            state.set_buffer_bytes(downloaded, total);

            // Set up callback to send BufferDataAvailable command (DRY: single callback setup point)
            setup_buffer_callback(shared_buffer, command_tx);

            tracing::info!(
                "Preload streaming: set up buffer callback, downloaded={}/{}",
                downloaded,
                total
            );
        } else {
            state.set_buffer_bytes(1, 1);
        }

        match player.play_preloaded_sink(
            preloaded.sink,
            preloaded.duration,
            path.clone(),
            preloaded.is_streaming,
        ) {
            Ok(_) => {
                update_state_from_player(player, state);
                state.set_current_path(Some(path.clone()));
                let _ = event_tx.send(AudioEvent::Started { path: Some(path) });
            }
            Err(e) => {
                let _ = event_tx.send(AudioEvent::Error { message: e });
            }
        }
    } else {
        tracing::warn!("PlayPreloaded: request_id {} not found", request_id);
        let _ = event_tx.send(AudioEvent::Error {
            message: format!("Preloaded sink not found: {}", request_id),
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
    command_tx: &super::events::AudioCommandSender,
) {
    let command_tx = command_tx.clone();
    shared_buffer.set_buffer_callback(move |event| {
        use super::streaming::BufferEvent;
        match event {
            BufferEvent::DataAppended { downloaded, total } => {
                // Send command to Audio Thread for state update
                let _ = command_tx.send(AudioCommand::BufferDataAvailable { downloaded, total });
            }
            BufferEvent::Complete => {
                // Completion is detected via buffer.is_complete() in check_buffer_status()
            }
        }
    });
}

/// Estimate byte position from time position
///
/// This is an approximation assuming constant bitrate.
/// For VBR files, this may not be accurate, but it's good enough for buffering decisions.
fn estimate_byte_position(time_pos: Duration, total_bytes: u64, duration: Duration) -> u64 {
    if duration.as_secs_f64() <= 0.0 || total_bytes == 0 {
        return 0;
    }
    let progress = time_pos.as_secs_f64() / duration.as_secs_f64();
    (total_bytes as f64 * progress) as u64
}

/// Check buffer status and enter/exit Buffering state as needed
///
/// Called periodically from Tick handler for streaming playback.
///
/// Uses hysteresis (watermark) mechanism to prevent rapid state oscillation:
/// - Enter Buffering when remaining data < LOW_WATER_MARK_BYTES (~1 second)
/// - Exit Buffering when remaining data > HIGH_WATER_MARK_BYTES (~10 seconds)
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
) {
    // Use SharedPlaybackState for status check (single source of truth)
    let current_status = state.get_info().status;
    // Use player for position and duration (playback info)
    let player_info = player.get_info();

    // CRITICAL: If there's a pending seek, we must check buffer at the SEEK TARGET position,
    // not the current playback position. Otherwise, we might incorrectly exit Buffering
    // because the old position has enough data, then execute seek to unbuffered position
    // which would block the audio thread.
    let check_position = state.pending_seek_target().unwrap_or(player_info.position);

    // Calculate byte position and remaining buffered bytes at check_position
    let byte_pos =
        estimate_byte_position(check_position, buffer.total_size(), player_info.duration);
    let downloaded = buffer.downloaded();
    let remaining_bytes = downloaded.saturating_sub(byte_pos);

    match &current_status {
        PlaybackStatus::Playing => {
            // Check if we need to enter Buffering (LOW water mark)
            // Only enter Buffering if:
            // 1. Remaining data is below LOW_WATER_MARK_BYTES, AND
            // 2. Download is not complete
            if remaining_bytes < LOW_WATER_MARK_BYTES && !buffer.is_complete() {
                tracing::info!(
                    "Buffer low: remaining {} bytes < {} (low water mark), entering Buffering",
                    remaining_bytes,
                    LOW_WATER_MARK_BYTES
                );
                enter_buffering(player, state, event_tx, player_info.position);
            }
        }
        PlaybackStatus::Buffering { .. } => {
            // Check if we can exit Buffering (HIGH water mark)
            // Exit Buffering if:
            // 1. Remaining data exceeds HIGH_WATER_MARK_BYTES, OR
            // 2. Download is complete (no more data coming)
            if buffer.is_complete() {
                tracing::info!("Download complete, exiting Buffering");
                exit_buffering(player, state, event_tx);
            } else if remaining_bytes > HIGH_WATER_MARK_BYTES {
                tracing::info!(
                    "Buffer sufficient: remaining {} bytes > {} (high water mark), exiting Buffering",
                    remaining_bytes,
                    HIGH_WATER_MARK_BYTES
                );
                exit_buffering(player, state, event_tx);
            }
            // Otherwise, stay in Buffering state and wait for more data
        }
        _ => {}
    }
}

/// Enter Buffering state
///
/// Pauses the Sink and sets status to Buffering.
fn enter_buffering(
    player: &mut AudioPlayer,
    state: &SharedPlaybackState,
    event_tx: &AudioEventSender,
    position: Duration,
) {
    // Pause sink without changing player's internal status
    player.pause_sink();

    let old_status = state.get_info().status;
    let new_status = PlaybackStatus::Buffering { position };
    state.set_status(new_status.clone());

    let _ = event_tx.send(AudioEvent::BufferingStarted { position });
    let _ = event_tx.send(AudioEvent::StateChanged {
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
) {
    let old_status = state.get_info().status;

    // Check for pending seek
    if let Some(target_position) = state.pending_seek_target() {
        tracing::info!(
            "exit_buffering: executing pending seek to {:?}",
            target_position
        );

        // Clear pending seek first (before attempting seek)
        state.set_pending_seek(None);

        // Execute the seek
        match player.seek(target_position) {
            Ok(_) => {
                state.set_position(target_position);
                let _ = event_tx.send(AudioEvent::SeekComplete {
                    position: target_position,
                });
            }
            Err(e) => {
                tracing::error!("exit_buffering: seek failed: {}", e);
                let _ = event_tx.send(AudioEvent::SeekFailed { error: e });
                // Continue to resume playback even if seek failed
            }
        }
    }

    // Resume sink
    player.play_sink();

    state.set_status(PlaybackStatus::Playing);

    let _ = event_tx.send(AudioEvent::BufferingEnded);
    let _ = event_tx.send(AudioEvent::StateChanged {
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
) {
    // For streaming playback, check if we should enter Buffering instead of finishing
    if let Some(buffer) = current_buffer {
        if !buffer.is_complete() {
            return;
        }
    }

    if player.is_finished() {
        state.set_status(PlaybackStatus::Stopped);
        let _ = event_tx.send(AudioEvent::Finished);
    }
}
