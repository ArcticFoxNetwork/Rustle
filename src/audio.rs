//! Audio playback module
//!
//! This module provides audio playback with real-time processing:
//! - `AudioPlayer`: Playback control with preloading support
//! - `AudioProcessingChain`: Unified audio processing (preamp, EQ, analyzer)
//! - `AudioAnalysisData`: Real-time visualization data
//! - `streaming`: Streaming buffer and download utilities
//! - `events`: Event-driven playback state (replaces polling)

pub mod analyzer;
mod chain;
mod equalizer;
pub mod events;
mod player;
pub mod streaming;

pub use analyzer::AudioAnalysisData;
pub use chain::AudioProcessingChain;
pub use events::{PlayerEvent, PlayerEventReceiver, PlayerEventSender, player_event_channel};
pub use player::{AudioPlayer, PlaybackStatus, get_audio_devices};
pub use streaming::{SharedBuffer, StreamingBuffer};
