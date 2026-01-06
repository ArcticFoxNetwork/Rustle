//! Player events for event-driven playback control
//!
//! This module provides events for AudioPlayer state changes.
//! For streaming download events, see `crate::audio::streaming::StreamingEvent`.
//!
//! ## Architecture
//! - `PlayerEvent` - Playback state events (started only, finish detection via polling)
//! - `StreamingEvent` - Download state events (playable, progress, complete, buffering)
//!
//! The separation allows:
//! - AudioPlayer to focus on playback state
//! - StreamingDownloader to focus on download state
//! - App to handle both independently

use std::path::PathBuf;

/// Events emitted by the audio player for playback state changes
/// 
/// Note: Finish detection is done via polling `is_finished()` rather than events,
/// because rodio's Sink doesn't provide reliable finish callbacks.
#[derive(Debug, Clone)]
pub enum PlayerEvent {
    /// Playback started for a track
    Started { path: PathBuf },
    /// Streaming playback started (no file path)
    StreamingStarted,
}

/// Sender for player events (held by AudioPlayer)
pub type PlayerEventSender = tokio::sync::mpsc::UnboundedSender<PlayerEvent>;

/// Receiver for player events (held by App)
pub type PlayerEventReceiver = tokio::sync::mpsc::UnboundedReceiver<PlayerEvent>;

/// Create a new player event channel
pub fn player_event_channel() -> (PlayerEventSender, PlayerEventReceiver) {
    tokio::sync::mpsc::unbounded_channel()
}
