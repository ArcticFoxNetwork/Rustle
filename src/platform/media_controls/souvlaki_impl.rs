//! Windows/macOS media controls using souvlaki
//!
//! Provides system media controls integration for Windows (SMTC) and macOS (MPNowPlayingInfoCenter)

use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata as SouvlakiMetadata, MediaPlayback,
    MediaPosition, PlatformConfig, SeekDirection,
};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;

use super::{MediaCommand, MediaMetadata, MediaPlaybackStatus, MediaState};

/// Convert our MediaPlaybackStatus to souvlaki's MediaPlayback
fn to_souvlaki_playback(status: MediaPlaybackStatus, position_us: i64) -> MediaPlayback {
    match status {
        MediaPlaybackStatus::Playing => MediaPlayback::Playing {
            progress: Some(MediaPosition(Duration::from_micros(position_us as u64))),
        },
        MediaPlaybackStatus::Paused => MediaPlayback::Paused {
            progress: Some(MediaPosition(Duration::from_micros(position_us as u64))),
        },
        MediaPlaybackStatus::Stopped => MediaPlayback::Stopped,
    }
}

/// Shared state for thread-safe access
struct SharedState {
    controls: Mutex<Option<MediaControls>>,
}

/// Handle to control media controls from the application (Windows/macOS implementation)
#[derive(Clone)]
pub struct SouvlakiMediaHandle {
    state: Arc<SharedState>,
    // Keep owned strings for metadata lifetime
    metadata_cache: Arc<Mutex<MetadataCache>>,
}

/// Cache for metadata strings (souvlaki needs references with specific lifetimes)
#[derive(Default)]
struct MetadataCache {
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    cover_url: Option<String>,
    duration: Option<Duration>,
}

impl std::fmt::Debug for SouvlakiMediaHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SouvlakiMediaHandle").finish()
    }
}

impl SouvlakiMediaHandle {
    /// Update media controls state
    pub fn update(&self, state: MediaState) {
        // Update metadata cache
        {
            let mut cache = self.metadata_cache.lock().unwrap();
            cache.title = state.metadata.title.clone();
            cache.artist = if !state.metadata.artists.is_empty() {
                Some(state.metadata.artists.join(", "))
            } else {
                None
            };
            cache.album = state.metadata.album.clone();
            cache.cover_url = state.metadata.art_url.clone();
            cache.duration = state
                .metadata
                .length_us
                .map(|us| Duration::from_micros(us as u64));
        }

        // Update controls
        if let Ok(mut controls_guard) = self.state.controls.lock() {
            if let Some(ref mut controls) = *controls_guard {
                // Set playback status
                let playback = to_souvlaki_playback(state.status, state.position_us);
                let _ = controls.set_playback(playback);

                // Set metadata (need to borrow from cache)
                let cache = self.metadata_cache.lock().unwrap();
                let metadata = SouvlakiMetadata {
                    title: cache.title.as_deref(),
                    artist: cache.artist.as_deref(),
                    album: cache.album.as_deref(),
                    cover_url: cache.cover_url.as_deref(),
                    duration: cache.duration,
                };
                let _ = controls.set_metadata(metadata);
            }
        }
    }
}

/// Start media controls service using souvlaki
pub fn start() -> (SouvlakiMediaHandle, mpsc::UnboundedReceiver<MediaCommand>) {
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();

    // Create platform config
    #[cfg(target_os = "windows")]
    let hwnd = None; // Will be set later if needed

    #[cfg(target_os = "macos")]
    let hwnd = None;

    let config = PlatformConfig {
        dbus_name: "rustle", // Not used on Windows/macOS
        display_name: "Rustle",
        hwnd,
    };

    let state = Arc::new(SharedState {
        controls: Mutex::new(None),
    });

    let metadata_cache = Arc::new(Mutex::new(MetadataCache::default()));

    // Try to create media controls
    match MediaControls::new(config) {
        Ok(mut controls) => {
            // Attach event handler
            let tx = cmd_tx.clone();
            if let Err(e) = controls.attach(move |event: MediaControlEvent| {
                let cmd = match event {
                    MediaControlEvent::Play => Some(MediaCommand::Play),
                    MediaControlEvent::Pause => Some(MediaCommand::Pause),
                    MediaControlEvent::Toggle => Some(MediaCommand::PlayPause),
                    MediaControlEvent::Next => Some(MediaCommand::Next),
                    MediaControlEvent::Previous => Some(MediaCommand::Previous),
                    MediaControlEvent::Stop => Some(MediaCommand::Stop),
                    MediaControlEvent::Seek(direction) => {
                        // Seek by 10 seconds
                        let offset = match direction {
                            SeekDirection::Forward => 10_000_000i64, // 10 seconds in microseconds
                            SeekDirection::Backward => -10_000_000i64,
                        };
                        Some(MediaCommand::Seek(offset))
                    }
                    MediaControlEvent::SeekBy(direction, duration) => {
                        let micros = duration.as_micros() as i64;
                        let offset = match direction {
                            SeekDirection::Forward => micros,
                            SeekDirection::Backward => -micros,
                        };
                        Some(MediaCommand::Seek(offset))
                    }
                    MediaControlEvent::SetPosition(pos) => Some(MediaCommand::SetPosition(
                        String::new(),
                        pos.0.as_micros() as i64,
                    )),
                    MediaControlEvent::SetVolume(volume) => Some(MediaCommand::SetVolume(volume)),
                    MediaControlEvent::Raise => Some(MediaCommand::Raise),
                    MediaControlEvent::Quit => Some(MediaCommand::Quit),
                    MediaControlEvent::OpenUri(_) => None, // Not supported
                };

                if let Some(cmd) = cmd {
                    let _ = tx.send(cmd);
                }
            }) {
                tracing::warn!("Failed to attach media controls event handler: {:?}", e);
            }

            *state.controls.lock().unwrap() = Some(controls);
            tracing::info!("Media controls (souvlaki) initialized successfully");
        }
        Err(e) => {
            tracing::warn!("Failed to create media controls: {:?}", e);
        }
    }

    (
        SouvlakiMediaHandle {
            state,
            metadata_cache,
        },
        cmd_rx,
    )
}
