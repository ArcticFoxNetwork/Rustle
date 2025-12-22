//! Audio playback module
//!
//! This module provides audio playback with real-time processing:
//! - `AudioPlayer`: Playback control with preloading support
//! - `AudioProcessingChain`: Unified audio processing (preamp, EQ, analyzer)
//! - `AudioAnalysisData`: Real-time visualization data

pub mod analyzer;
mod chain;
mod equalizer;
mod player;

pub use analyzer::AudioAnalysisData;
pub use chain::AudioProcessingChain;
pub use player::{AudioPlayer, PlaybackStatus, get_audio_devices};
