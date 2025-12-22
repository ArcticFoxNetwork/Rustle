//! MPRIS D-Bus integration using LocalPlayerInterface trait

use mpris_server::{
    LocalPlayerInterface, LocalRootInterface, LocalServer, LoopStatus, Metadata, PlaybackRate,
    PlaybackStatus, Property, Time, TrackId, Volume,
    zbus::{Result, fdo},
};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

// Track metadata for MPRIS
#[derive(Debug, Clone, Default)]
pub struct MprisMetadata {
    /// Track ID (unique identifier)
    pub track_id: Option<String>,
    /// Track title
    pub title: Option<String>,
    /// Artist(s)
    pub artists: Vec<String>,
    /// Album name
    pub album: Option<String>,
    /// Album artist(s)
    pub album_artists: Vec<String>,
    /// Track length in microseconds
    pub length_us: Option<i64>,
    /// Cover art URL (file:// or http://)
    pub art_url: Option<String>,
}

impl MprisMetadata {
    /// Convert to mpris-server Metadata
    pub fn to_metadata(&self) -> Metadata {
        let mut builder = Metadata::builder();

        if let Some(ref track_id) = self.track_id {
            builder = builder.trackid(
                TrackId::try_from(format!("/org/rustle/track/{}", track_id))
                    .unwrap_or_else(|_| TrackId::NO_TRACK),
            );
        }

        if let Some(ref title) = self.title {
            builder = builder.title(title);
        }

        if !self.artists.is_empty() {
            builder = builder.artist(self.artists.clone());
        }

        if let Some(ref album) = self.album {
            builder = builder.album(album);
        }

        if !self.album_artists.is_empty() {
            builder = builder.album_artist(self.album_artists.clone());
        }

        if let Some(length) = self.length_us {
            builder = builder.length(Time::from_micros(length));
        }

        if let Some(ref art_url) = self.art_url {
            builder = builder.art_url(art_url);
        }

        builder.build()
    }
}

/// Playback status for MPRIS
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MprisPlaybackStatus {
    Playing,
    Paused,
    Stopped,
}

impl From<MprisPlaybackStatus> for PlaybackStatus {
    fn from(status: MprisPlaybackStatus) -> Self {
        match status {
            MprisPlaybackStatus::Playing => PlaybackStatus::Playing,
            MprisPlaybackStatus::Paused => PlaybackStatus::Paused,
            MprisPlaybackStatus::Stopped => PlaybackStatus::Stopped,
        }
    }
}

/// MPRIS state
#[derive(Debug, Clone)]
pub struct MprisState {
    /// Current playback status
    pub status: MprisPlaybackStatus,
    /// Track metadata
    pub metadata: MprisMetadata,
    /// Current position in microseconds
    pub position_us: i64,
    /// Volume (0.0 to 1.0)
    pub volume: f64,
    /// Can go to next track
    pub can_go_next: bool,
    /// Can go to previous track
    pub can_go_previous: bool,
    /// Can play
    pub can_play: bool,
    /// Can pause
    pub can_pause: bool,
    /// Can seek
    pub can_seek: bool,
}

impl Default for MprisState {
    fn default() -> Self {
        Self {
            status: MprisPlaybackStatus::Stopped,
            metadata: MprisMetadata::default(),
            position_us: 0,
            volume: 1.0,
            can_go_next: false,
            can_go_previous: false,
            can_play: false,
            can_pause: false,
            can_seek: false,
        }
    }
}

// Shared state between the app and MPRIS
#[derive(Debug, Clone)]
pub struct MprisSharedState {
    pub status: Arc<Mutex<PlaybackStatus>>,
    pub metadata: Arc<Mutex<Metadata>>,
    pub volume: Arc<Mutex<f64>>,
    pub position: Arc<Mutex<Time>>,
    pub can_go_next: Arc<Mutex<bool>>,
    pub can_go_previous: Arc<Mutex<bool>>,
    pub can_play: Arc<Mutex<bool>>,
    pub can_pause: Arc<Mutex<bool>>,
    pub can_seek: Arc<Mutex<bool>>,
}

impl Default for MprisSharedState {
    fn default() -> Self {
        Self {
            status: Arc::new(Mutex::new(PlaybackStatus::Stopped)),
            metadata: Arc::new(Mutex::new(Metadata::default())),
            volume: Arc::new(Mutex::new(1.0)),
            position: Arc::new(Mutex::new(Time::ZERO)),
            can_go_next: Arc::new(Mutex::new(false)),
            can_go_previous: Arc::new(Mutex::new(false)),
            can_play: Arc::new(Mutex::new(false)),
            can_pause: Arc::new(Mutex::new(false)),
            can_seek: Arc::new(Mutex::new(false)),
        }
    }
}

pub struct MprisPlayer {
    state: MprisSharedState,
    cmd_tx: mpsc::UnboundedSender<MprisCommand>,
}

/// Commands that can be sent from MPRIS to the application
#[derive(Debug, Clone)]
pub enum MprisCommand {
    /// Play
    Play,
    /// Pause
    Pause,
    /// Toggle play/pause
    PlayPause,
    /// Stop
    Stop,
    /// Next track
    Next,
    /// Previous track
    Previous,
    /// Seek by offset (in microseconds)
    Seek(i64),
    /// Set absolute position (track_id, position_us)
    SetPosition(String, i64),
    /// Set volume (0.0 to 1.0)
    SetVolume(f64),
    /// Raise/show window
    Raise,
    /// Quit application
    Quit,
}

impl LocalRootInterface for MprisPlayer {
    async fn raise(&self) -> fdo::Result<()> {
        let _ = self.cmd_tx.send(MprisCommand::Raise);
        Ok(())
    }

    async fn quit(&self) -> fdo::Result<()> {
        let _ = self.cmd_tx.send(MprisCommand::Quit);
        Ok(())
    }

    async fn can_quit(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn fullscreen(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn set_fullscreen(&self, _fullscreen: bool) -> Result<()> {
        Ok(())
    }

    async fn can_set_fullscreen(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn can_raise(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn has_track_list(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn identity(&self) -> fdo::Result<String> {
        Ok("Rustle".to_string())
    }

    async fn desktop_entry(&self) -> fdo::Result<String> {
        Ok("rustle".to_string())
    }

    async fn supported_uri_schemes(&self) -> fdo::Result<Vec<String>> {
        Ok(vec!["file".to_string()])
    }

    async fn supported_mime_types(&self) -> fdo::Result<Vec<String>> {
        Ok(vec![
            "audio/mpeg".to_string(),
            "audio/ogg".to_string(),
            "audio/flac".to_string(),
        ])
    }
}

impl LocalPlayerInterface for MprisPlayer {
    async fn next(&self) -> fdo::Result<()> {
        let _ = self.cmd_tx.send(MprisCommand::Next);
        Ok(())
    }

    async fn previous(&self) -> fdo::Result<()> {
        let _ = self.cmd_tx.send(MprisCommand::Previous);
        Ok(())
    }

    async fn pause(&self) -> fdo::Result<()> {
        let _ = self.cmd_tx.send(MprisCommand::Pause);
        Ok(())
    }

    async fn play(&self) -> fdo::Result<()> {
        let _ = self.cmd_tx.send(MprisCommand::Play);
        Ok(())
    }

    async fn play_pause(&self) -> fdo::Result<()> {
        let _ = self.cmd_tx.send(MprisCommand::PlayPause);
        Ok(())
    }

    async fn stop(&self) -> fdo::Result<()> {
        let _ = self.cmd_tx.send(MprisCommand::Stop);
        Ok(())
    }

    async fn seek(&self, offset: Time) -> fdo::Result<()> {
        let _ = self.cmd_tx.send(MprisCommand::Seek(offset.as_micros()));
        Ok(())
    }

    async fn set_position(&self, track_id: TrackId, position: Time) -> fdo::Result<()> {
        let _ = self.cmd_tx.send(MprisCommand::SetPosition(
            track_id.to_string(),
            position.as_micros(),
        ));
        Ok(())
    }

    async fn open_uri(&self, _uri: String) -> fdo::Result<()> {
        // Not implemented
        Ok(())
    }

    async fn playback_status(&self) -> fdo::Result<PlaybackStatus> {
        let status = self.state.status.lock().unwrap();
        Ok(*status)
    }

    async fn loop_status(&self) -> fdo::Result<LoopStatus> {
        Ok(LoopStatus::None)
    }

    async fn set_loop_status(&self, _loop_status: LoopStatus) -> Result<()> {
        Ok(())
    }

    async fn rate(&self) -> fdo::Result<PlaybackRate> {
        Ok(PlaybackRate::default())
    }

    async fn set_rate(&self, _rate: PlaybackRate) -> Result<()> {
        Ok(())
    }

    async fn shuffle(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn set_shuffle(&self, _shuffle: bool) -> Result<()> {
        Ok(())
    }

    async fn metadata(&self) -> fdo::Result<Metadata> {
        let metadata = self.state.metadata.lock().unwrap();
        Ok(metadata.clone())
    }

    async fn volume(&self) -> fdo::Result<Volume> {
        let volume = self.state.volume.lock().unwrap();
        Ok(*volume)
    }

    async fn set_volume(&self, volume: Volume) -> Result<()> {
        let _ = self.cmd_tx.send(MprisCommand::SetVolume(volume));
        Ok(())
    }

    async fn position(&self) -> fdo::Result<Time> {
        let position = self.state.position.lock().unwrap();
        Ok(*position)
    }

    async fn minimum_rate(&self) -> fdo::Result<PlaybackRate> {
        Ok(PlaybackRate::default())
    }

    async fn maximum_rate(&self) -> fdo::Result<PlaybackRate> {
        Ok(PlaybackRate::default())
    }

    async fn can_go_next(&self) -> fdo::Result<bool> {
        let can_go_next = self.state.can_go_next.lock().unwrap();
        Ok(*can_go_next)
    }

    async fn can_go_previous(&self) -> fdo::Result<bool> {
        let can_go_previous = self.state.can_go_previous.lock().unwrap();
        Ok(*can_go_previous)
    }

    async fn can_play(&self) -> fdo::Result<bool> {
        let can_play = self.state.can_play.lock().unwrap();
        Ok(*can_play)
    }

    async fn can_pause(&self) -> fdo::Result<bool> {
        let can_pause = self.state.can_pause.lock().unwrap();
        Ok(*can_pause)
    }

    async fn can_seek(&self) -> fdo::Result<bool> {
        let can_seek = self.state.can_seek.lock().unwrap();
        Ok(*can_seek)
    }

    async fn can_control(&self) -> fdo::Result<bool> {
        Ok(true)
    }
}

#[derive(Debug, Clone)]
pub struct MprisHandle {
    state: MprisSharedState,
    state_tx: mpsc::UnboundedSender<MprisState>,
}

impl MprisHandle {
    /// Update MPRIS state and notify D-Bus clients
    pub fn update(&self, state: MprisState) {
        // Update shared state for on-demand queries
        *self.state.status.lock().unwrap() = state.status.into();
        *self.state.metadata.lock().unwrap() = state.metadata.to_metadata();
        *self.state.volume.lock().unwrap() = state.volume;
        *self.state.position.lock().unwrap() = Time::from_micros(state.position_us);
        *self.state.can_go_next.lock().unwrap() = state.can_go_next;
        *self.state.can_go_previous.lock().unwrap() = state.can_go_previous;
        *self.state.can_play.lock().unwrap() = state.can_play;
        *self.state.can_pause.lock().unwrap() = state.can_pause;
        *self.state.can_seek.lock().unwrap() = state.can_seek;

        // Send state to MPRIS thread for PropertiesChanged signal
        let _ = self.state_tx.send(state);
    }
}

/// Start MPRIS service
pub fn start_mpris() -> (MprisHandle, mpsc::UnboundedReceiver<MprisCommand>) {
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
    let (state_tx, mut state_rx) = mpsc::unbounded_channel::<MprisState>();
    let state = MprisSharedState::default();
    let player = MprisPlayer {
        state: state.clone(),
        cmd_tx: cmd_tx.clone(),
    };

    // Spawn MPRIS on a dedicated thread with its own runtime
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Failed to create runtime for MPRIS");

        let local = tokio::task::LocalSet::new();
        local.block_on(&rt, async move {
            // Start the MPRIS server
            let server = LocalServer::new("Rustle", player)
                .await
                .expect("Failed to create MPRIS server");

            // Run server and handle state updates concurrently
            tokio::select! {
                _ = server.run() => {}
                _ = async {
                    while let Some(state) = state_rx.recv().await {
                        // Send PropertiesChanged signal to notify clients like Waybar
                        let _ = server.properties_changed([
                            Property::PlaybackStatus(state.status.into()),
                            Property::Metadata(state.metadata.to_metadata()),
                            Property::CanGoNext(state.can_go_next),
                            Property::CanGoPrevious(state.can_go_previous),
                            Property::CanPlay(state.can_play),
                            Property::CanPause(state.can_pause),
                            Property::CanSeek(state.can_seek),
                        ]).await;
                    }
                } => {}
            }
        });
    });

    (MprisHandle { state, state_tx }, cmd_rx)
}
