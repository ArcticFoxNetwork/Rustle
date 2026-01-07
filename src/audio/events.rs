//! Audio thread communication types
//!
//! This module provides commands and events for the audio thread architecture:
//! - `AudioCommand` - Commands sent from UI thread to audio thread
//! - `AudioEvent` - Events sent from audio thread to UI thread
//! - `SharedPlaybackState` - Thread-safe state for non-blocking UI reads
//!
//! For streaming download events, see `crate::audio::streaming::StreamingEvent`.
//!
//! ## Architecture
//! ```text
//! UI Thread (AudioHandle) --[AudioCommand]--> Audio Thread (AudioPlayer)
//! UI Thread              <--[AudioEvent]---- Audio Thread
//! UI Thread              <--[SharedState]--- Audio Thread (non-blocking reads)
//! ```

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use parking_lot::RwLock;

use super::PlaybackStatus;
use super::streaming::StreamingBuffer;

// ============ Commands (UI -> Audio Thread) ============

/// Commands sent from UI thread to audio thread
///
/// All commands are processed asynchronously - the UI thread sends and returns immediately.
/// Results are communicated back via `AudioEvent`.
pub enum AudioCommand {
    /// Play a local file
    Play { path: PathBuf, fade_in: bool },
    /// Play from streaming buffer (for NCM songs)
    PlayStreaming {
        buffer: StreamingBuffer,
        duration: Duration,
        cache_path: Option<PathBuf>,
    },
    /// Pause playback
    Pause { fade_out: bool },
    /// Resume playback  
    Resume { fade_in: bool },
    /// Stop playback
    Stop,
    /// Seek to position
    Seek { position: Duration },
    /// Set volume (0.0 - 1.0)
    SetVolume { volume: f32 },
    /// Set track gain for normalization
    SetTrackGain { gain: f32 },
    /// Create preload sink for a local file (async, returns via PreloadReady event)
    CreatePreloadSink { path: PathBuf, request_id: u64 },
    /// Create preload sink for streaming (async, returns via PreloadReady event)
    CreatePreloadSinkStreaming {
        buffer: StreamingBuffer,
        duration: Duration,
        request_id: u64,
    },
    /// Play a preloaded sink by request_id
    PlayPreloaded { request_id: u64, path: PathBuf },
    /// Switch audio output device
    SwitchDevice { device_name: Option<String> },
    /// Periodic tick for buffer status checks and position sync
    Tick,
    /// Update paused position cache
    UpdatePausedPosition { position: Duration },
    /// Buffer data available notification (from download callback)
    /// Used by Audio Thread to update buffer progress and check if buffering can end
    BufferDataAvailable { downloaded: u64, total: u64 },
}

impl std::fmt::Debug for AudioCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Play { path, fade_in } => f
                .debug_struct("Play")
                .field("path", path)
                .field("fade_in", fade_in)
                .finish(),
            Self::PlayStreaming {
                duration,
                cache_path,
                ..
            } => f
                .debug_struct("PlayStreaming")
                .field("duration", duration)
                .field("cache_path", cache_path)
                .finish_non_exhaustive(),
            Self::Pause { fade_out } => {
                f.debug_struct("Pause").field("fade_out", fade_out).finish()
            }
            Self::Resume { fade_in } => f.debug_struct("Resume").field("fade_in", fade_in).finish(),
            Self::Stop => write!(f, "Stop"),
            Self::Seek { position } => f.debug_struct("Seek").field("position", position).finish(),
            Self::SetVolume { volume } => {
                f.debug_struct("SetVolume").field("volume", volume).finish()
            }
            Self::SetTrackGain { gain } => {
                f.debug_struct("SetTrackGain").field("gain", gain).finish()
            }
            Self::CreatePreloadSink { path, request_id } => f
                .debug_struct("CreatePreloadSink")
                .field("path", path)
                .field("request_id", request_id)
                .finish(),
            Self::CreatePreloadSinkStreaming {
                duration,
                request_id,
                ..
            } => f
                .debug_struct("CreatePreloadSinkStreaming")
                .field("duration", duration)
                .field("request_id", request_id)
                .finish_non_exhaustive(),
            Self::PlayPreloaded { request_id, path } => f
                .debug_struct("PlayPreloaded")
                .field("request_id", request_id)
                .field("path", path)
                .finish(),
            Self::SwitchDevice { device_name } => f
                .debug_struct("SwitchDevice")
                .field("device_name", device_name)
                .finish(),
            Self::Tick => write!(f, "Tick"),
            Self::UpdatePausedPosition { position } => f
                .debug_struct("UpdatePausedPosition")
                .field("position", position)
                .finish(),
            Self::BufferDataAvailable { downloaded, total } => f
                .debug_struct("BufferDataAvailable")
                .field("downloaded", downloaded)
                .field("total", total)
                .finish(),
        }
    }
}

// ============ Events (Audio Thread -> UI) ============

/// Events emitted by the audio thread
///
/// These events notify the UI of state changes and operation results.
/// The UI should handle these in its message loop.
#[derive(Debug, Clone)]
pub enum AudioEvent {
    /// Playback started for a track
    Started { path: Option<PathBuf> },
    /// Playback paused
    Paused { position: Duration },
    /// Playback resumed
    Resumed,
    /// Playback stopped
    Stopped,
    /// Seek completed successfully
    SeekComplete { position: Duration },
    /// Seek failed
    SeekFailed { error: String },
    /// Seek started
    SeekStarted { target_position: Duration },
    /// State changed
    StateChanged {
        old_status: PlaybackStatus,
        new_status: PlaybackStatus,
    },
    /// Buffer progress update
    BufferProgress {
        downloaded: u64,
        total: u64,
        progress: f32,
    },
    /// Entered buffering state
    BufferingStarted { position: Duration },
    /// Buffering ended, playback resumed
    BufferingEnded,
    /// Preload sink ready
    PreloadReady {
        request_id: u64,
        duration: Duration,
        path: PathBuf,
    },
    /// Preload failed
    PreloadFailed { request_id: u64, error: String },
    /// Device switched successfully
    DeviceSwitched {
        /// State to restore: (path, position, was_playing)
        restore_state: Option<(PathBuf, Duration, bool)>,
    },
    /// Device switch failed
    DeviceSwitchFailed { error: String },
    /// Playback finished (track ended)
    Finished,
    /// Error occurred
    Error { message: String },
}

// ============ Shared State ============

use super::player::EffectiveStatus;

/// Inner state protected by RwLock
#[derive(Debug, Clone)]
struct PlaybackStateInner {
    /// Current playback status
    pub status: PlaybackStatus,
    /// Current playback position
    pub position: Duration,
    /// Total audio duration
    pub duration: Duration,
    /// Volume (0.0 - 1.0)
    pub volume: f32,
    /// Current playing file path
    pub current_path: Option<PathBuf>,
    /// Buffer progress (0.0 - 1.0), None for local files
    /// Updated by set_buffer_bytes() in audio thread
    pub buffer_progress: Option<f32>,
    /// Buffered bytes count
    pub buffered_bytes: u64,
    /// Total bytes count
    pub total_bytes: u64,
    /// Pending seek target position
    ///
    /// When seeking to an unbuffered position, we store the target here
    /// and enter Buffering state. When buffer is ready, exit_buffering()
    /// checks this field and executes the seek before resuming playback.
    pub pending_seek_target: Option<Duration>,
}

impl Default for PlaybackStateInner {
    fn default() -> Self {
        Self {
            status: PlaybackStatus::Stopped,
            position: Duration::ZERO,
            duration: Duration::ZERO,
            volume: 1.0,
            current_path: None,
            buffer_progress: None,
            buffered_bytes: 0,
            total_bytes: 0,
            pending_seek_target: None,
        }
    }
}

/// UI rendering snapshot of playback state
#[derive(Debug, Clone)]
#[allow(dead_code)] // Reserved for future UI snapshot API
pub struct PlaybackStateSnapshot {
    /// Effective status for UI icon display
    pub effective_status: EffectiveStatus,
    /// Whether loading indicator should be shown
    pub is_loading: bool,
    /// Display position (target position during seek)
    pub display_position: Duration,
    /// Actual playback position
    pub actual_position: Duration,
    /// Total duration
    pub duration: Duration,
    /// Volume
    pub volume: f32,
    /// Buffer progress (None for local files)
    pub buffer_progress: Option<f32>,
}

/// Thread-safe shared playback state
///
/// UI thread reads this without blocking.
/// Audio thread updates it after each operation.
#[derive(Clone)]
pub struct SharedPlaybackState {
    inner: Arc<RwLock<PlaybackStateInner>>,
}

impl std::fmt::Debug for SharedPlaybackState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let inner = self.inner.read();
        f.debug_struct("SharedPlaybackState")
            .field("status", &inner.status)
            .field("position", &inner.position)
            .field("duration", &inner.duration)
            .field("volume", &inner.volume)
            .finish()
    }
}

impl Default for SharedPlaybackState {
    fn default() -> Self {
        Self::new()
    }
}

impl SharedPlaybackState {
    /// Create new shared state
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(PlaybackStateInner::default())),
        }
    }

    /// Get current playback info
    pub fn get_info(&self) -> super::PlaybackInfo {
        let inner = self.inner.read();
        super::PlaybackInfo {
            status: inner.status.clone(),
            position: inner.position,
            duration: inner.duration,
            volume: inner.volume,
        }
    }

    /// Check if currently playing
    ///
    /// Uses effective_status() so that Buffering returns true.
    pub fn is_playing(&self) -> bool {
        let inner = self.inner.read();
        inner.status.effective_status() == super::player::EffectiveStatus::Playing
    }

    /// Check if stopped
    pub fn is_stopped(&self) -> bool {
        let inner = self.inner.read();
        inner.status == PlaybackStatus::Stopped
    }

    /// Check if in loading state
    pub fn is_loading(&self) -> bool {
        let inner = self.inner.read();
        inner.status.is_loading()
    }

    /// Get display positio
    pub fn display_position(&self) -> Duration {
        let inner = self.inner.read();
        // If there's a pending seek, show the target position
        if let Some(target) = inner.pending_seek_target {
            return target;
        }
        inner.position
    }

    /// Get buffer progress
    ///
    /// This is the single source of truth for buffer progress.
    /// Updated by audio thread via set_buffer_bytes().
    pub fn buffer_progress(&self) -> Option<f32> {
        self.inner.read().buffer_progress
    }

    /// Get current path
    pub fn current_path(&self) -> Option<PathBuf> {
        self.inner.read().current_path.clone()
    }

    // ---- Update methods (called by audio thread) ----
    /// Update status
    pub fn set_status(&self, status: PlaybackStatus) {
        self.inner.write().status = status;
    }

    /// Update position
    pub fn set_position(&self, position: Duration) {
        self.inner.write().position = position;
    }

    /// Update duration
    #[allow(dead_code)] // Reserved for future use
    pub fn set_duration(&self, duration: Duration) {
        self.inner.write().duration = duration;
    }

    /// Update volume
    pub fn set_volume(&self, volume: f32) {
        self.inner.write().volume = volume;
    }

    /// Update current path
    pub fn set_current_path(&self, path: Option<PathBuf>) {
        self.inner.write().current_path = path;
    }

    /// Update buffer bytes info
    pub fn set_buffer_bytes(&self, buffered: u64, total: u64) {
        let mut inner = self.inner.write();
        inner.buffered_bytes = buffered;
        inner.total_bytes = total;
        if total > 0 {
            inner.buffer_progress = Some(buffered as f32 / total as f32);
        } else {
            inner.buffer_progress = None;
        }
    }

    /// Set pending seek target
    ///
    /// Called when seeking to unbuffered position. The target is stored
    /// and will be executed when buffer is ready.
    pub fn set_pending_seek(&self, target: Option<Duration>) {
        self.inner.write().pending_seek_target = target;
    }

    /// Get pending seek target
    pub fn pending_seek_target(&self) -> Option<Duration> {
        self.inner.read().pending_seek_target
    }

    /// Update from PlaybackInfo
    pub fn update_from_info(&self, info: &super::PlaybackInfo) {
        let mut inner = self.inner.write();
        inner.status = info.status.clone();
        inner.position = info.position;
        inner.duration = info.duration;
        inner.volume = info.volume;
    }
}

// ============ Channel Types ============

/// Sender for audio commands (held by AudioHandle)
pub type AudioCommandSender = tokio::sync::mpsc::UnboundedSender<AudioCommand>;

/// Receiver for audio commands (held by audio thread)
pub type AudioCommandReceiver = tokio::sync::mpsc::UnboundedReceiver<AudioCommand>;

/// Sender for audio events (held by audio thread)
pub type AudioEventSender = tokio::sync::mpsc::UnboundedSender<AudioEvent>;

/// Receiver for audio events (held by App)
pub type AudioEventReceiver = tokio::sync::mpsc::UnboundedReceiver<AudioEvent>;

/// Create a new audio command channel
pub fn audio_command_channel() -> (AudioCommandSender, AudioCommandReceiver) {
    tokio::sync::mpsc::unbounded_channel()
}

/// Create a new audio event channel
pub fn audio_event_channel() -> (AudioEventSender, AudioEventReceiver) {
    tokio::sync::mpsc::unbounded_channel()
}
