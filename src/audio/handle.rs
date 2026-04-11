//! Audio handle for non-blocking audio control from UI thread
//!
//! `AudioHandle` provides a non-blocking interface to control audio playback.
//! All methods send commands to the audio thread and return immediately.
//! State is read from `SharedPlaybackState` without blocking.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use super::PlaybackInfo;
use super::events::{AudioCommand, AudioCommandSender, SharedPlaybackState};
use super::streaming::StreamingBuffer;

/// Counter for generating unique preload request IDs
static PRELOAD_REQUEST_COUNTER: AtomicU64 = AtomicU64::new(0);
/// Counter for generating unique playback request IDs
static PLAYBACK_REQUEST_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Handle for controlling audio from UI thread
///
/// All methods are non-blocking - they send commands to the audio thread
/// and return immediately. Results are communicated via `AudioEvent`.
///
/// State queries (get_info, is_playing, etc.) read from shared state
/// without blocking, even if the audio thread is busy.
#[derive(Clone)]
pub struct AudioHandle {
    command_tx: AudioCommandSender,
    state: SharedPlaybackState,
}

impl std::fmt::Debug for AudioHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioHandle")
            .field("state", &self.state)
            .finish()
    }
}

impl AudioHandle {
    fn next_playback_request_id() -> u64 {
        PLAYBACK_REQUEST_COUNTER.fetch_add(1, Ordering::Relaxed)
    }

    /// Create a new audio handle
    pub fn new(command_tx: AudioCommandSender, state: SharedPlaybackState) -> Self {
        Self { command_tx, state }
    }

    // ============ Playback Control ============

    /// Play a local file with optional fade in
    pub fn play_with_fade(&self, path: PathBuf, fade_in: bool, track_gain: f32) -> u64 {
        let request_id = Self::next_playback_request_id();
        let _ = self.command_tx.send(AudioCommand::Play {
            request_id,
            path,
            fade_in,
            track_gain,
        });
        request_id
    }

    /// Load a local file in paused state at a target position
    pub fn load_paused(&self, path: PathBuf, position: Duration, track_gain: f32) -> u64 {
        let request_id = Self::next_playback_request_id();
        let _ = self.command_tx.send(AudioCommand::LoadPaused {
            request_id,
            path,
            position,
            track_gain,
        });
        request_id
    }

    /// Load a streaming source in paused state at a target position
    pub fn load_streaming_paused(
        &self,
        buffer: StreamingBuffer,
        duration: Duration,
        cache_path: Option<PathBuf>,
        position: Duration,
        track_gain: f32,
    ) -> u64 {
        let request_id = Self::next_playback_request_id();
        let _ = self.command_tx.send(AudioCommand::LoadPausedStreaming {
            request_id,
            buffer,
            duration,
            cache_path,
            position,
            track_gain,
        });
        request_id
    }

    /// Play a local file from a target position
    pub fn play_from_position_with_fade(
        &self,
        path: PathBuf,
        position: Duration,
        fade_in: bool,
        track_gain: f32,
    ) -> u64 {
        let request_id = Self::next_playback_request_id();
        let _ = self.command_tx.send(AudioCommand::PlayAt {
            request_id,
            path,
            position,
            fade_in,
            track_gain,
        });
        request_id
    }

    /// Play from streaming buffer
    ///
    /// For NCM songs that stream from network.
    pub fn play_streaming(
        &self,
        buffer: StreamingBuffer,
        duration: Duration,
        cache_path: Option<PathBuf>,
        fade_in: bool,
        track_gain: f32,
    ) -> u64 {
        let request_id = Self::next_playback_request_id();
        let _ = self.command_tx.send(AudioCommand::PlayStreaming {
            request_id,
            buffer,
            duration,
            cache_path,
            fade_in,
            track_gain,
        });
        request_id
    }

    /// Pause playback with optional fade out
    ///
    /// Sends Pause command to audio thread.
    /// Note: Audio Thread will pause Sink before data runs out, so no interrupt needed.
    pub fn pause_with_fade(&self, fade_out: bool) {
        let _ = self.command_tx.send(AudioCommand::Pause { fade_out });
    }

    /// Resume playback
    #[allow(dead_code)] // Public API, used via resume_with_fade
    pub fn resume(&self) {
        self.resume_with_fade(false);
    }

    /// Resume playback with optional fade in
    pub fn resume_with_fade(&self, fade_in: bool) {
        let _ = self.command_tx.send(AudioCommand::Resume { fade_in });
    }

    /// Stop playback
    ///
    /// Sends Stop command to audio thread.
    pub fn stop(&self) {
        let _ = self.command_tx.send(AudioCommand::Stop);
    }

    /// Seek to position
    ///
    /// Sends Seek command and returns immediately.
    /// Listen for `AudioEvent::SeekComplete` or `AudioEvent::SeekFailed`.
    ///
    /// The shared state position is updated immediately to the target position,
    /// so UI shows the target position while seek is in progress (prevents
    /// "bounce back" effect during buffering).
    pub fn seek(&self, position: Duration) {
        // Update position immediately so UI shows target position during seek
        // This prevents the progress bar from "bouncing back" while audio thread
        // is blocked waiting for streaming data
        self.state.set_position(position);
        let _ = self.command_tx.send(AudioCommand::Seek { position });
    }

    /// Set volume
    pub fn set_volume(&self, volume: f32) {
        let _ = self.command_tx.send(AudioCommand::SetVolume { volume });
    }

    /// Tick handler - checks buffer status and syncs position
    pub fn tick(&self) {
        let _ = self.command_tx.send(AudioCommand::Tick);
    }
    // ============ Preloading ============

    /// Request creation of a preload sink for a local file
    ///
    /// Returns a request ID. Listen for `AudioEvent::PreloadReady` or
    /// `AudioEvent::PreloadFailed` with matching request_id.
    pub fn create_preload_sink(&self, path: PathBuf, track_gain: f32) -> u64 {
        let request_id = PRELOAD_REQUEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        let _ = self.command_tx.send(AudioCommand::CreatePreloadSink {
            path,
            request_id,
            track_gain,
        });
        request_id
    }

    /// Request creation of a preload sink for streaming
    ///
    /// Returns a request ID. Listen for `AudioEvent::PreloadReady` or
    /// `AudioEvent::PreloadFailed` with matching request_id.
    pub fn create_preload_sink_streaming(
        &self,
        buffer: StreamingBuffer,
        duration: Duration,
        track_gain: f32,
    ) -> u64 {
        let request_id = PRELOAD_REQUEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        let _ = self
            .command_tx
            .send(AudioCommand::CreatePreloadSinkStreaming {
                buffer,
                duration,
                request_id,
                track_gain,
            });
        request_id
    }

    /// Play a preloaded sink by request_id
    ///
    /// The sink must have been created via `create_preload_sink` or
    /// `create_preload_sink_streaming` and received `AudioEvent::PreloadReady`.
    pub fn play_preloaded(&self, request_id: u64, path: PathBuf, fade_in: bool) -> u64 {
        let playback_request_id = Self::next_playback_request_id();
        let _ = self.command_tx.send(AudioCommand::PlayPreloaded {
            request_id,
            playback_request_id,
            path,
            fade_in,
        });
        playback_request_id
    }

    /// Release a preloaded sink by request_id without playing it
    pub fn release_preload(&self, request_id: u64) {
        let _ = self
            .command_tx
            .send(AudioCommand::ReleasePreload { request_id });
    }

    // ============ Device Control ============

    /// Switch audio output device
    ///
    /// Listen for `AudioEvent::DeviceSwitched` or `AudioEvent::DeviceSwitchFailed`.
    pub fn switch_device(&self, device_name: Option<String>) {
        let _ = self
            .command_tx
            .send(AudioCommand::SwitchDevice { device_name });
    }

    // ============ State Queries (non-blocking reads) ============

    /// Get current playback info
    ///
    /// Reads from shared state, does not communicate with audio thread.
    pub fn get_info(&self) -> PlaybackInfo {
        self.state.get_info()
    }

    /// Get display position
    ///
    /// Returns target position during pending seek, otherwise actual position.
    /// Use this for UI display to show immediate feedback during seek.
    pub fn display_position(&self) -> Duration {
        self.state.display_position()
    }

    /// Check if player has no loaded audio
    pub fn is_empty(&self) -> bool {
        self.state.current_path().is_none() && self.state.is_stopped()
    }

    /// Get buffer progress for streaming playback
    ///
    /// Returns None for local files, Some(0.0-1.0) for streaming.
    /// This is the single source of truth for buffer progress in UI.
    pub fn buffer_progress(&self) -> Option<f32> {
        self.state.buffer_progress()
    }
}
