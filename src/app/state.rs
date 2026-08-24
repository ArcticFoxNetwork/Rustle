// src/app/state.rs
//! Application state definitions

use iced::time::Instant;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crate::api::{AlbumSummary, ArtistSummary, Banner, NcmClient, PlaylistSummary, Track};
use crate::app::SettingsSection;
use crate::audio::{AudioAnalysisData, AudioProcessingChain, PlaybackInfo, PlaybackStatus};
use crate::database::{Database, DbPlaybackState, DbPlaylist, DbSong, DbWatchedFolder};
use crate::features::import::{CoverCache, FolderWatcher, ScanHandle, ScanProgress, ScanState};
use crate::i18n::Locale;
use crate::platform::discord::DiscordPresence;
use crate::platform::media_controls::{MediaCommand, MediaHandle};
use crate::ui::animation::{HoverAnimations, SingleHoverAnimation};
use crate::ui::components::{ImportingPlaylist, NavItem};
use crate::ui::effects::background::LyricsBackgroundProgram;
use crate::ui::effects::textured_background::TexturedBackgroundProgram;
use crate::ui::overlay::OverlayEntry;
use crate::ui::pages;
use crate::ui::widgets::Toast;
use crate::utils::Source;

// ---------------------------------------------------------------------------
// Unified image-handle cache
// ---------------------------------------------------------------------------

use std::collections::HashMap as StateMap;

/// Pre-loaded image data keyed by `(ImageKind, id)`.
///
/// Populated by the unified image pipeline (`handle_image`) so that views
/// never touch disk — they only borrow an `Option<&image::Handle>`.
#[derive(Debug, Default)]
pub struct ImageState {
    pub entries: StateMap<(crate::image::ImageKind, u64), ImageEntry>,
    pub inflight: std::collections::HashSet<(crate::image::ImageKind, u64)>,
    pub pending: VecDeque<ImageRequest>,
    pub queued: std::collections::HashSet<(crate::image::ImageKind, u64)>,
}

#[derive(Debug, Clone)]
pub struct ImageEntry {
    pub path: PathBuf,
    pub handle: iced::widget::image::Handle,
    pub dimensions: Option<(u32, u32)>,
}

#[derive(Debug, Clone)]
pub struct ImageRequest {
    pub kind: crate::image::ImageKind,
    pub id: u64,
    pub url: String,
}

impl ImageState {
    pub fn get(
        &self,
        kind: crate::image::ImageKind,
        id: u64,
    ) -> Option<&iced::widget::image::Handle> {
        self.entries.get(&(kind, id)).map(|entry| &entry.handle)
    }

    pub fn image_data(
        &self,
        kind: crate::image::ImageKind,
        id: u64,
    ) -> Option<(&PathBuf, u32, u32)> {
        self.entries.get(&(kind, id)).map(|entry| {
            let (width, height) = entry.dimensions.unwrap_or((0, 0));
            (&entry.path, width, height)
        })
    }

    pub fn insert_path(&mut self, kind: crate::image::ImageKind, id: u64, path: PathBuf) {
        let dimensions = image_dimensions(&path);
        let handle = iced::widget::image::Handle::from_path(path.clone());
        self.entries.insert(
            (kind, id),
            ImageEntry {
                path,
                handle,
                dimensions,
            },
        );
        self.inflight.remove(&(kind, id));
        self.queued.remove(&(kind, id));
    }

    pub fn mark_inflight(&mut self, kind: crate::image::ImageKind, id: u64) {
        self.queued.remove(&(kind, id));
        self.inflight.insert((kind, id));
    }

    pub fn clear_inflight(&mut self, kind: crate::image::ImageKind, id: u64) {
        self.inflight.remove(&(kind, id));
    }

    pub fn is_inflight(&self, kind: crate::image::ImageKind, id: u64) -> bool {
        self.inflight.contains(&(kind, id))
    }

    pub fn is_queued(&self, kind: crate::image::ImageKind, id: u64) -> bool {
        self.queued.contains(&(kind, id))
    }

    pub fn enqueue(&mut self, kind: crate::image::ImageKind, id: u64, url: String) -> bool {
        let key = (kind, id);
        if self.entries.contains_key(&key)
            || self.inflight.contains(&key)
            || self.queued.contains(&key)
            || url.is_empty()
        {
            return false;
        }

        self.pending.push_back(ImageRequest { kind, id, url });
        self.queued.insert(key);
        true
    }

    pub fn pop_pending(&mut self) -> Option<ImageRequest> {
        let request = self.pending.pop_front()?;
        self.queued.remove(&(request.kind, request.id));
        Some(request)
    }
}

fn image_dimensions(path: &std::path::Path) -> Option<(u32, u32)> {
    image::ImageReader::open(path)
        .and_then(|reader| reader.with_guessed_format())
        .ok()
        .and_then(|reader| reader.into_dimensions().ok())
}

/// Main application state
pub struct App {
    /// Core infrastructure (Settings, DB, Audio, System integrations)
    pub core: CoreState,
    /// Library/business data (Songs, Playlists, Import state)
    pub library: LibraryState,
    /// Playback session state (Queue, current track, preload, resume snapshot)
    pub playback: PlaybackSessionState,
    /// UI state (Navigation, Page states, Animations)
    pub ui: UiState,
}

/// Core Infrastructure & Services
pub struct CoreState {
    pub db: Option<Arc<Database>>,
    pub db_error: Option<String>,
    /// Download manager for offline downloads
    pub download_manager: crate::download::DownloadManager,
    /// Owns the audio thread lifetime.
    audio_thread: Option<crate::audio::AudioThreadHandle>,
    /// Audio handle for non-blocking audio control
    audio: Option<crate::audio::AudioHandle>,
    /// Audio processing chain (preamp, EQ, analyzer) - shared with AudioPlayer
    audio_chain: AudioProcessingChain,
    pub volume_before_mute: Option<f32>,
    pub settings: crate::features::Settings,
    pub locale: Locale,
    pub is_logged_in: bool,

    // NCM API Client
    pub ncm_client: Option<NcmClient>,
    pub user_info: Option<UserInfo>,

    // System Integrations
    pub cover_cache: Option<Arc<CoverCache>>,
    pub mpris_handle: Option<MediaHandle>,
    pub mpris_rx:
        Option<Arc<tokio::sync::Mutex<tokio::sync::mpsc::UnboundedReceiver<MediaCommand>>>>,
    pub discord_presence: DiscordPresence,
    pub global_hotkeys: Option<crate::platform::global_hotkeys::GlobalHotkeyService>,
    pub window_restore_mode: iced::window::Mode,
    pub window_visibility: WindowVisibilityState,
    pub window_focused: bool,
    pub window_operation_pending: bool,
    pub window_width: f32,
    pub window_height: f32,
    /// Current mouse Y position for drag area detection
    pub mouse_position: iced::Point,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowVisibilityState {
    Visible,
    Hiding,
    Hidden,
    Showing,
}

impl CoreState {
    pub fn is_window_visible(&self) -> bool {
        self.window_visibility == WindowVisibilityState::Visible
    }

    pub fn is_window_hidden(&self) -> bool {
        !self.is_window_visible()
    }

    /// Initialize core services with loaded settings
    pub fn new(
        settings: crate::features::Settings,
        locale: Locale,
        audio_thread: Option<crate::audio::AudioThreadHandle>,
        audio: Option<crate::audio::AudioHandle>,
        audio_chain: AudioProcessingChain,
    ) -> Self {
        let discord_enabled = settings.system.discord_enabled;
        Self {
            db: None,
            db_error: None,
            download_manager: Default::default(),
            audio_thread,
            audio,
            audio_chain,
            volume_before_mute: None,
            settings,
            locale,
            is_logged_in: false,
            ncm_client: None,
            user_info: None,
            cover_cache: None,
            mpris_handle: None,
            mpris_rx: None,
            discord_presence: DiscordPresence::new(discord_enabled),
            global_hotkeys: None,
            window_restore_mode: iced::window::Mode::Windowed,
            window_visibility: WindowVisibilityState::Visible,
            window_focused: true,
            window_operation_pending: false,
            window_width: 1280.0,
            window_height: 720.0,
            mouse_position: iced::Point::ORIGIN,
        }
    }
}

impl Drop for CoreState {
    fn drop(&mut self) {
        self.audio = None;
        let _ = self.audio_thread.take();
    }
}

#[derive(Clone)]
pub struct PlaybackRuntimeState {
    pub info: PlaybackInfo,
    pub display_position: Duration,
    pub buffer_progress: Option<f32>,
    pub has_loaded_audio: bool,
    pub analysis: AudioAnalysisData,
}

fn status_with_pause_intent(
    backend_status: PlaybackStatus,
    pause_requested: bool,
) -> PlaybackStatus {
    if pause_requested
        && matches!(
            backend_status,
            PlaybackStatus::Playing | PlaybackStatus::Buffering { .. }
        )
    {
        PlaybackStatus::Paused
    } else {
        backend_status
    }
}

impl Default for PlaybackRuntimeState {
    fn default() -> Self {
        Self {
            info: PlaybackInfo::default(),
            display_position: Duration::ZERO,
            buffer_progress: None,
            has_loaded_audio: false,
            analysis: AudioAnalysisData::new(),
        }
    }
}

impl PlaybackRuntimeState {
    pub fn is_playing(&self) -> bool {
        matches!(
            self.info.status,
            PlaybackStatus::Playing | PlaybackStatus::Buffering { .. }
        )
    }

    pub fn is_buffering(&self) -> bool {
        matches!(self.info.status, PlaybackStatus::Buffering { .. })
    }

    pub fn can_seek(&self) -> bool {
        self.has_loaded_audio || !self.info.duration.is_zero()
    }
}

#[cfg(test)]
mod playback_intent_tests {
    use super::*;

    #[test]
    fn pause_intent_overrides_playing_and_buffering_until_backend_confirms() {
        assert_eq!(
            status_with_pause_intent(PlaybackStatus::Playing, true),
            PlaybackStatus::Paused
        );
        assert_eq!(
            status_with_pause_intent(
                PlaybackStatus::Buffering {
                    position: Duration::from_secs(3),
                },
                true,
            ),
            PlaybackStatus::Paused
        );
    }

    #[test]
    fn pause_intent_does_not_relabel_backend_terminal_states() {
        assert_eq!(
            status_with_pause_intent(PlaybackStatus::Paused, true),
            PlaybackStatus::Paused
        );
        assert_eq!(
            status_with_pause_intent(PlaybackStatus::Stopped, true),
            PlaybackStatus::Stopped
        );
        assert_eq!(
            status_with_pause_intent(PlaybackStatus::Playing, false),
            PlaybackStatus::Playing
        );
    }

    #[test]
    fn pause_intent_can_be_reversed_before_the_backend_pauses() {
        let backend_status = PlaybackStatus::Playing;
        assert_eq!(
            status_with_pause_intent(backend_status.clone(), true),
            PlaybackStatus::Paused
        );
        assert_eq!(
            status_with_pause_intent(backend_status, false),
            PlaybackStatus::Playing
        );
    }
}

impl App {
    pub(crate) fn begin_audio_resolution_context(
        &self,
    ) -> Result<crate::audio::identity::PlaybackContext, String> {
        self.require_audio_handle()?.begin_playback_resolution()
    }

    pub(crate) fn current_audio_context(&self) -> Option<crate::audio::identity::PlaybackContext> {
        self.audio_handle()
            .and_then(|audio| audio.current_context())
    }

    pub(crate) fn accepts_audio_preload_identity(
        &self,
        identity: &crate::audio::identity::PreloadIdentity,
    ) -> bool {
        self.core.audio.as_ref().is_some_and(|audio| {
            audio.current_context().is_some_and(|context| {
                context.generation == identity.generation
                    && context.cancellation == identity.cancellation
                    && !identity.cancellation.is_cancelled()
                    && audio.accepts_context(&context)
                    && self
                        .playback
                        .audio_preload_manager
                        .accepts_identity(identity)
            })
        })
    }
    pub(crate) fn accepts_audio_context(
        &self,
        context: &crate::audio::identity::PlaybackContext,
    ) -> bool {
        self.core
            .audio
            .as_ref()
            .is_some_and(|audio| audio.accepts_context(context))
    }

    pub(crate) fn accepts_audio_seek(
        &self,
        context: &crate::audio::identity::PlaybackContext,
        nonce: crate::audio::identity::SeekNonce,
    ) -> bool {
        self.core
            .audio
            .as_ref()
            .is_some_and(|audio| audio.accepts_seek(context, nonce))
    }

    fn audio_handle(&self) -> Option<&crate::audio::AudioHandle> {
        self.core.audio.as_ref()
    }

    fn require_audio_handle(&self) -> Result<&crate::audio::AudioHandle, String> {
        self.audio_handle()
            .ok_or_else(|| "No audio player".to_string())
    }

    pub(crate) fn refresh_playback_runtime(&mut self) {
        let analysis = self.core.audio_chain.analysis();
        let pause_requested = self.playback.pause_requested;

        if let Some(audio) = self.audio_handle() {
            let mut info = audio.get_info();
            info.status = status_with_pause_intent(info.status, pause_requested);
            self.playback.runtime = PlaybackRuntimeState {
                info,
                display_position: audio.display_position(),
                buffer_progress: audio.buffer_progress(),
                has_loaded_audio: !audio.is_empty(),
                analysis,
            };
            return;
        }

        let mut runtime = PlaybackRuntimeState {
            analysis,
            ..PlaybackRuntimeState::default()
        };
        if let Some(saved_state) = &self.playback.saved_state {
            runtime.info.volume = saved_state.volume as f32;
        }
        self.playback.runtime = runtime;
    }

    pub(crate) fn playback_runtime(&self) -> &PlaybackRuntimeState {
        &self.playback.runtime
    }

    pub(crate) fn playback_info(&self) -> &PlaybackInfo {
        &self.playback.runtime.info
    }

    pub(crate) fn playback_status(&self) -> PlaybackStatus {
        self.playback.runtime.info.status.clone()
    }

    pub(crate) fn playback_is_playing(&self) -> bool {
        self.playback.runtime.is_playing()
    }

    pub(crate) fn playback_is_buffering(&self) -> bool {
        self.playback.runtime.is_buffering()
    }

    pub(crate) fn playback_output_available(&self) -> bool {
        self.core.audio.is_some()
    }

    pub(crate) fn playback_can_seek(&self) -> bool {
        self.playback.runtime.can_seek()
    }

    pub(crate) fn playback_buffer_progress(&self) -> Option<f32> {
        self.playback.runtime.buffer_progress
    }

    pub(crate) fn playback_analysis_data(&self) -> AudioAnalysisData {
        self.playback.runtime.analysis.clone()
    }

    pub(crate) fn play_audio_file(
        &self,
        path: PathBuf,
        fade_in: bool,
        track_gain: f32,
    ) -> Result<u64, String> {
        let audio = self.require_audio_handle()?;
        audio.play_with_fade(path, fade_in, track_gain)
    }

    pub(crate) fn play_audio_file_in_context(
        &self,
        context: &crate::audio::identity::PlaybackContext,
        path: PathBuf,
        fade_in: bool,
        track_gain: f32,
    ) -> Result<u64, String> {
        self.require_audio_handle()?
            .play_with_fade_in_context(context, path, fade_in, track_gain)
    }

    pub(crate) fn play_audio_file_at_position(
        &self,
        path: PathBuf,
        position: Duration,
        fade_in: bool,
        track_gain: f32,
    ) -> Result<u64, String> {
        let audio = self.require_audio_handle()?;
        audio.play_from_position_with_fade(path, position, fade_in, track_gain)
    }

    pub(crate) fn play_audio_file_at_position_in_context(
        &self,
        context: &crate::audio::identity::PlaybackContext,
        path: PathBuf,
        position: Duration,
        fade_in: bool,
        track_gain: f32,
    ) -> Result<u64, String> {
        self.require_audio_handle()?
            .play_from_position_with_fade_in_context(context, path, position, fade_in, track_gain)
    }

    pub(crate) fn play_streaming_audio(
        &self,
        buffer: crate::audio::SharedBuffer,
        duration: Duration,
        cache_path: Option<PathBuf>,
        fade_in: bool,
        track_gain: f32,
    ) -> Result<u64, String> {
        let audio = self.require_audio_handle()?;
        let streaming_buffer = crate::audio::StreamingBuffer::new(buffer);
        audio.play_streaming(streaming_buffer, duration, cache_path, fade_in, track_gain)
    }

    pub(crate) fn play_streaming_audio_in_context(
        &self,
        context: &crate::audio::identity::PlaybackContext,
        buffer: crate::audio::SharedBuffer,
        duration: Duration,
        cache_path: Option<PathBuf>,
        fade_in: bool,
        track_gain: f32,
    ) -> Result<u64, String> {
        let streaming_buffer = crate::audio::StreamingBuffer::new(buffer);
        self.require_audio_handle()?.play_streaming_in_context(
            context,
            streaming_buffer,
            duration,
            cache_path,
            fade_in,
            track_gain,
        )
    }

    pub(crate) fn play_preloaded_audio(
        &self,
        identity: crate::audio::identity::PreloadIdentity,
        fade_in: bool,
        transition: Option<crate::audio::automix::TransitionDirective>,
    ) -> Result<u64, String> {
        let audio = self.require_audio_handle()?;
        audio.play_preloaded(identity, fade_in, transition)
    }

    pub(crate) fn schedule_preloaded_audio_transition(
        &self,
        identity: crate::audio::identity::PreloadIdentity,
        trigger_at: Duration,
        fade_in: bool,
        transition: crate::audio::automix::TransitionDirective,
    ) -> Result<u64, String> {
        self.require_audio_handle()?
            .schedule_preloaded_transition(identity, trigger_at, fade_in, transition)
    }

    pub(crate) fn cancel_scheduled_audio_transition(&self) {
        if let Some(audio) = self.audio_handle()
            && let Err(error) = audio.cancel_scheduled_transition()
        {
            tracing::warn!("Failed to cancel scheduled audio transition: {error}");
        }
    }

    pub(crate) fn clear_scheduled_transition_state(&mut self) {
        if self.playback.scheduled_transition_request_id.is_some() {
            self.cancel_scheduled_audio_transition();
        }
        if let Some(buffer) = self.playback.scheduled_transition_buffer.take() {
            buffer.cancel();
        }
        if let Some(request_id) = self.playback.scheduled_transition_request_id.take()
            && self
                .playback
                .pending_playback_request
                .as_ref()
                .is_some_and(|pending| pending.request_id == request_id)
        {
            self.playback.pending_playback_request = None;
        }
        self.playback.pending_transition = None;
        self.playback.pending_transition_trigger = None;
        self.playback.crossfade_triggered = false;
    }

    pub(crate) fn load_audio_file_paused(
        &self,
        path: PathBuf,
        position: Duration,
        track_gain: f32,
    ) -> Result<u64, String> {
        let audio = self.require_audio_handle()?;
        audio.load_paused(path, position, track_gain)
    }

    pub(crate) fn load_audio_file_paused_in_context(
        &self,
        context: &crate::audio::identity::PlaybackContext,
        path: PathBuf,
        position: Duration,
        track_gain: f32,
    ) -> Result<u64, String> {
        self.require_audio_handle()?
            .load_paused_in_context(context, path, position, track_gain)
    }

    pub(crate) fn load_streaming_audio_paused_in_context(
        &self,
        context: &crate::audio::identity::PlaybackContext,
        buffer: crate::audio::SharedBuffer,
        duration: Duration,
        cache_path: Option<PathBuf>,
        position: Duration,
        track_gain: f32,
    ) -> Result<u64, String> {
        let streaming_buffer = crate::audio::StreamingBuffer::new(buffer);
        self.require_audio_handle()?
            .load_streaming_paused_in_context(
                context,
                streaming_buffer,
                duration,
                cache_path,
                position,
                track_gain,
            )
    }

    pub(crate) fn reserve_preload_identity(
        &self,
    ) -> Result<crate::audio::identity::PreloadIdentity, String> {
        self.require_audio_handle()?
            .reserve_preload_identity()
            .ok_or_else(|| "Preload requires active playback generation".to_string())
    }

    pub(crate) fn reserve_preload_handoff(
        &self,
        parent: &crate::audio::identity::PreloadIdentity,
    ) -> Result<crate::audio::identity::PreloadIdentity, String> {
        self.require_audio_handle()?
            .reserve_preload_handoff(parent)
            .ok_or_else(|| "Preload handoff identity is stale or cancelled".to_string())
    }

    pub(crate) fn create_preload_sink_for_file(
        &self,
        identity: crate::audio::identity::PreloadIdentity,
        path: PathBuf,
        track_gain: f32,
    ) -> Result<(), String> {
        let audio = self.require_audio_handle()?;
        audio.create_preload_sink(identity, path, track_gain)
    }

    pub(crate) fn create_preload_sink_for_stream(
        &self,
        identity: crate::audio::identity::PreloadIdentity,
        buffer: crate::audio::SharedBuffer,
        duration: Duration,
        track_gain: f32,
    ) -> Result<(), String> {
        let audio = self.require_audio_handle()?;
        let streaming_buffer = crate::audio::StreamingBuffer::new(buffer);
        audio.create_preload_sink_streaming(identity, streaming_buffer, duration, track_gain)
    }

    pub(crate) fn release_preload_request(
        &self,
        identity: crate::audio::identity::PreloadIdentity,
    ) {
        if let Some(audio) = self.audio_handle()
            && let Err(error) = audio.release_preload(identity)
        {
            tracing::warn!("Failed to release audio preload: {error}");
        }
    }

    pub(crate) fn release_preload_requests<I>(&self, request_ids: I)
    where
        I: IntoIterator<Item = crate::audio::identity::PreloadIdentity>,
    {
        if let Some(audio) = self.audio_handle() {
            for identity in request_ids {
                if let Err(error) = audio.release_preload(identity) {
                    tracing::warn!("Failed to release audio preload: {error}");
                }
            }
        }
    }

    pub(crate) fn pause_audio_output_with_fade(&self, fade_out: bool) -> Result<(), String> {
        self.require_audio_handle()?.pause_with_fade(fade_out)
    }

    pub(crate) fn resume_audio_output_with_fade(&self, fade_in: bool) -> Result<(), String> {
        self.require_audio_handle()?.resume_with_fade(fade_in)
    }

    pub(crate) fn stop_audio_backend(&self) {
        if let Some(audio) = self.audio_handle()
            && let Err(error) = audio.stop()
        {
            tracing::warn!("Failed to enqueue stop: {error}");
        }
    }

    pub(crate) fn seek_audio_output(&mut self, position: Duration) {
        if let Some(audio) = self.audio_handle()
            && audio.seek(position).is_ok()
        {
            self.refresh_playback_runtime();
        }
    }

    pub(crate) fn tick_audio_output(&mut self) {
        if let Some(audio) = self.audio_handle() {
            audio.tick();
        }
        self.refresh_playback_runtime();
    }

    pub(crate) fn apply_output_volume(&mut self, volume: f32) {
        if let Some(audio) = self.audio_handle() {
            audio.set_volume(volume);
        }
        self.playback.runtime.info.volume = volume;
    }

    pub(crate) fn persist_output_volume(&self, volume: f32) {
        if let Some(db) = &self.core.db {
            let db = db.clone();
            tokio::spawn(async move {
                let _ = db.update_volume(volume as f64).await;
            });
        }
    }

    pub(crate) fn persist_queue_snapshot(&self) {
        if let Some(db) = &self.core.db {
            let db = db.clone();
            let queue_snapshot = self.playback.queue.clone();
            tokio::spawn(async move {
                if let Err(err) = db.save_queue_with_songs(&queue_snapshot, None).await {
                    tracing::warn!("Failed to persist queue snapshot: {}", err);
                }
            });
        }
    }

    pub(crate) fn set_output_volume(&mut self, volume: f32, persist: bool) {
        self.apply_output_volume(volume);
        if persist {
            self.persist_output_volume(volume);
        }
    }

    pub(crate) fn switch_audio_output_device(&mut self, device_name: Option<String>) {
        self.clear_scheduled_transition_state();
        if let Some(audio) = self.audio_handle()
            && let Err(error) = audio.switch_device(device_name)
        {
            tracing::warn!("Failed to enqueue audio device switch: {error}");
        }
    }

    pub(crate) fn audio_output_devices(&self) -> Vec<(String, String)> {
        crate::audio::get_audio_devices()
            .into_iter()
            .map(|device| (device.name, device.description))
            .collect()
    }

    /// List all installed font families available for lyrics rendering.
    pub(crate) fn lyrics_font_families(&self) -> Vec<String> {
        self.ui
            .lyrics
            .shared_font_system
            .as_ref()
            .map_or_else(Vec::new, |fs| {
                let font_system = fs.lock();
                crate::platform::theme::list_installed_font_families(font_system.db())
            })
    }

    pub(crate) fn set_audio_analysis_enabled(&self, enabled: bool) {
        self.core.audio_chain.set_analysis_enabled(enabled);
    }

    pub(crate) fn set_audio_analysis_decay(&self, decay: f32) {
        self.core.audio_chain.set_analysis_decay(decay);
    }

    pub(crate) fn set_audio_equalizer_enabled(&self, enabled: bool) {
        self.core.audio_chain.set_equalizer_enabled(enabled);
    }

    pub(crate) fn set_audio_equalizer_gains(&self, gains: [f32; 10]) {
        self.core.audio_chain.set_equalizer_gains(gains);
    }

    pub(crate) fn set_audio_preamp(&self, preamp_db: f32) {
        self.core.audio_chain.set_preamp(preamp_db);
    }
}

/// User information from NCM
#[derive(Debug, Clone)]
pub struct UserInfo {
    pub user_id: u64,
    pub nickname: String,
    pub vip_type: i32,
    pub like_songs: HashSet<u64>,
}

impl UserInfo {
    pub fn new(user_id: u64, nickname: String) -> Self {
        Self {
            user_id,
            nickname,
            vip_type: 0,
            like_songs: HashSet::new(),
        }
    }
}

/// Business Logic Data
#[derive(Default)]
pub struct LibraryState {
    pub db_songs: Vec<DbSong>,
    pub playlists: Vec<DbPlaylist>,
    pub recently_played: Vec<DbSong>,

    // Import State
    pub scan_state: Option<Arc<ScanState>>,
    pub scan_handle: Option<ScanHandle>,
    pub scan_progress: Option<ScanProgress>,
    pub folder_watcher: Option<FolderWatcher>,
    pub watched_folders: Vec<DbWatchedFolder>,
}

/// Playback-owned session state.
///
/// This is kept separate from `LibraryState` so playback coordination has a
/// single home for queue ownership, preload state, and resume bookkeeping.
#[derive(Default)]
pub struct PlaybackSessionState {
    /// Current playing song reflected in UI/system integrations.
    pub current_song: Option<DbSong>,
    /// Current playing song's artist id when available from NCM metadata.
    pub current_artist_id: Option<u64>,
    /// Last saved playback snapshot loaded from the database.
    pub saved_state: Option<DbPlaybackState>,
    /// Active playback queue.
    pub queue: Vec<DbSong>,
    /// Current position within the active queue.
    pub current_index: Option<usize>,
    /// Whether personal FM playback mode is active.
    pub personal_fm_mode: bool,
    /// Source id used when reporting NCM scrobble events for the active queue.
    pub ncm_scrobble_source_id: Option<u64>,
    /// Queue navigation cache used as the single source of truth for shuffle order.
    pub shuffle_cache: crate::app::update::queue_navigator::ShuffleCache,
    /// Audio preload state machine for adjacent tracks.
    pub audio_preload_manager: crate::app::update::audio_preload_manager::AudioPreloadManager,
    /// Background lyrics preload state.
    pub lyrics_preload_manager: crate::app::update::lyrics_preload_manager::LyricsPreloadManager,
    /// Per-song lyrics render cache (engine lines, shaped lines).
    pub lyrics_render_manager: crate::app::update::lyrics_render_manager::LyricsRenderManager,
    /// Central preload coordinator - owns the preload window lifecycle.
    pub preload_coordinator: crate::app::update::preload_coordinator::PreloadCoordinator,
    /// Track index currently being resolved before playback starts.
    pub pending_resolution_index: Option<usize>,
    /// Playback request waiting for audio thread confirmation.
    pub pending_playback_request: Option<PendingPlaybackRequest>,
    /// Active streaming buffer shared with the audio thread.
    pub active_streaming_buffer: Option<crate::audio::SharedBuffer>,
    /// Cached runtime snapshot consumed by UI/system integrations.
    pub runtime: PlaybackRuntimeState,
    /// Logical pause intent exposed immediately while the backend finishes its
    /// sample-driven Fade and physical Sink pause.
    pub pause_requested: bool,
    /// Consecutive playback failure counter.
    pub consecutive_failures: u8,
    /// Prevents repeated natural Crossfade triggers while the next generation
    /// is being promoted.
    pub crossfade_triggered: bool,
    /// One-shot transition intent consumed only by the next preloaded handoff.
    pub pending_transition: Option<crate::audio::automix::TransitionDirective>,
    /// Audio-clock deadline paired with `pending_transition` for natural handoff.
    pub pending_transition_trigger: Option<Duration>,
    /// Request whose UI commit is deferred until the scheduled handoff starts.
    pub scheduled_transition_request_id: Option<u64>,
    /// Incoming streaming buffer retained without cancelling the outgoing source.
    pub scheduled_transition_buffer: Option<crate::audio::SharedBuffer>,
    /// Throttles finalized-cache discovery for streaming Automix analysis.
    pub automix_analysis_poll_at: Option<std::time::Instant>,
    /// Startup restore coordination state.
    pub startup_restore: StartupRestoreState,
}

#[derive(Debug, Clone, Default)]
pub struct StartupRestoreState {
    pub playback_state_loaded: bool,
    pub queue_loaded: bool,
    pub songs_loaded: bool,
    pub in_progress: bool,
    pub completed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingPlaybackKind {
    StartPlayingTrack,
    LoadPausedTrack,
    RestartCurrentTrack,
}

#[derive(Debug, Clone)]
pub struct PendingPlaybackRequest {
    pub request_id: u64,
    pub queue_index: Option<usize>,
    pub song: DbSong,
    pub kind: PendingPlaybackKind,
}

/// Unified route model for page rendering and navigation history
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    Home,
    Discover(DiscoverViewMode),
    Radio,
    Downloads,
    Settings(SettingsSection),
    AudioEngine,
    Playlist(i64),
    NcmPlaylist(u64),
    User(u64),
    Artist(u64),
    Album(u64),
    RecentlyPlayed,
    Search {
        keyword: String,
        tab: SearchTab,
        page: u32,
    },
}

impl Route {
    pub fn nav_item(&self) -> Option<NavItem> {
        match self {
            Self::Home => Some(NavItem::Home),
            Self::Discover(_) => Some(NavItem::Discover),
            Self::Radio => Some(NavItem::Radio),
            Self::Downloads => Some(NavItem::Downloads),
            Self::Settings(_) => Some(NavItem::Settings),
            Self::AudioEngine => Some(NavItem::AudioEngine),
            Self::Playlist(_)
            | Self::NcmPlaylist(_)
            | Self::User(_)
            | Self::Artist(_)
            | Self::Album(_)
            | Self::RecentlyPlayed
            | Self::Search { .. } => None,
        }
    }
}

/// Navigation history entry
#[derive(Debug, Clone, PartialEq)]
pub enum NavigationEntry {
    Route(Route),
}

/// Navigation history for back/forward functionality
#[derive(Debug, Default)]
pub struct NavigationHistory {
    /// History stack
    pub entries: Vec<NavigationEntry>,
    /// Current position in history (index)
    pub current_index: Option<usize>,
}

impl NavigationHistory {
    /// Push a new entry to history, clearing forward history
    pub fn push(&mut self, entry: NavigationEntry) {
        // Don't push if it's the same as current
        if let Some(idx) = self.current_index {
            if idx < self.entries.len() && self.entries[idx] == entry {
                return;
            }
            // Clear forward history
            self.entries.truncate(idx + 1);
        }
        self.entries.push(entry);
        self.current_index = Some(self.entries.len() - 1);
    }

    /// Replace the current history entry without changing stack length
    pub fn replace_current(&mut self, entry: NavigationEntry) {
        if let Some(idx) = self.current_index
            && idx < self.entries.len()
        {
            self.entries[idx] = entry;
            return;
        }

        self.push(entry);
    }

    /// Go back in history, returns the entry to navigate to
    pub fn go_back(&mut self) -> Option<NavigationEntry> {
        if let Some(idx) = self.current_index
            && idx > 0
        {
            self.current_index = Some(idx - 1);
            return self.entries.get(idx - 1).cloned();
        }
        None
    }

    /// Go forward in history, returns the entry to navigate to
    pub fn go_forward(&mut self) -> Option<NavigationEntry> {
        if let Some(idx) = self.current_index
            && idx + 1 < self.entries.len()
        {
            self.current_index = Some(idx + 1);
            return self.entries.get(idx + 1).cloned();
        }
        None
    }

    /// Check if can go back
    pub fn can_go_back(&self) -> bool {
        self.current_index.map(|idx| idx > 0).unwrap_or(false)
    }

    /// Check if can go forward
    pub fn can_go_forward(&self) -> bool {
        self.current_index
            .map(|idx| idx + 1 < self.entries.len())
            .unwrap_or(false)
    }
}

/// UI View State
pub struct UiState {
    pub current_route: Route,
    pub search_query: String,
    pub toast: Option<Toast>,
    pub toast_visible: bool,

    /// Overlay stack — LIFO: last = topmost rendered overlay
    pub overlay_stack: Vec<OverlayEntry>,

    /// Navigation history for back/forward
    pub nav_history: NavigationHistory,

    // Sub-modules
    pub playlist_page: PlaylistPageState,
    pub lyrics: LyricsState,
    pub dialogs: DialogState,
    pub home: HomePageState,
    pub discover: DiscoverPageState,
    pub search: SearchPageState,

    // Global UI Layout
    pub active_settings_section: SettingsSection,
    /// Runtime-measured Y positions of each settings section (for scroll→tab mapping)
    pub section_positions: Vec<(SettingsSection, f32)>,
    /// Whether section positions have been measured from the actual widget tree
    pub positions_measured: bool,
    /// Whether the mouse is hovering over the volume slider area
    pub is_volume_slider_hovered: bool,
    pub editing_keybinding: Option<(crate::features::Action, crate::features::ShortcutScope)>,
    /// Native event already observed for the key currently being recorded.
    pub global_hotkey_seen_while_recording: Option<u32>,
    /// A native event expected just after recording finishes. The deadline
    /// prevents a missing event from suppressing a later real key press.
    pub suppressed_recording_hotkey: Option<(u32, Instant)>,
    pub queue_visible: bool,

    // Playback Controls UI
    pub seek_preview_position: Option<f32>,
    pub save_position_counter: u32,
    pub last_mpris_sync: Option<Instant>,

    // Sidebar
    pub importing_playlist: Option<ImportingPlaylist>,
    pub sidebar_animations: HoverAnimations<crate::app::message::SidebarId>,
    /// Sidebar width in pixels (draggable)
    pub sidebar_width: f32,
    /// Whether the sidebar resize handle is being dragged
    pub sidebar_dragging: bool,

    // Cache statistics
    pub cache_stats: Option<crate::cache::CacheStats>,

    // Context menu
    pub context_menu: Option<ContextMenuState>,

    // Song edit form state (visibility managed by overlay_stack)
    pub song_edit_dialog: Option<SongEditDialogState>,

    // Download panel tab
    pub download_tab: DownloadTab,

    // Unified image-handle cache (pre-loaded handles, no disk I/O in views)
    pub image_state: ImageState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DownloadTab {
    #[default]
    Active,
    Completed,
    Failed,
}

/// Context menu state
#[derive(Debug, Clone)]
pub struct ContextMenuState {
    pub song_id: i64,
    pub x: f32,
    pub y: f32,
    /// Song source origin (Local / Cached / Online)
    pub source: Source,
    /// Whether this song is liked/favorited by the current user
    pub is_liked: bool,
}

/// Song edit dialog state
#[derive(Debug, Clone)]
pub struct SongEditDialogState {
    pub song_id: i64,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub track_number: Option<u32>,
    pub year: Option<u32>,
    pub genre: String,
    pub cover_path: Option<PathBuf>,
}

impl UiState {
    pub fn new() -> Self {
        Self {
            current_route: Route::Home,
            search_query: String::new(),
            toast: None,
            toast_visible: false,
            overlay_stack: Vec::new(),
            nav_history: {
                let mut history = NavigationHistory::default();
                history.push(NavigationEntry::Route(Route::Home));
                history
            },
            active_settings_section: SettingsSection::Account,
            section_positions: vec![
                (SettingsSection::Account, 0.0),
                (SettingsSection::Playback, 150.0),
                (SettingsSection::Display, 500.0),
                (SettingsSection::System, 850.0),
                (SettingsSection::Network, 1000.0),
                (SettingsSection::Storage, 1150.0),
                (SettingsSection::Shortcuts, 1390.0),
                (SettingsSection::About, 1965.0),
            ],
            positions_measured: false,
            is_volume_slider_hovered: false,
            editing_keybinding: None,
            global_hotkey_seen_while_recording: None,
            suppressed_recording_hotkey: None,
            queue_visible: false,
            seek_preview_position: None,
            save_position_counter: 0,
            last_mpris_sync: None,
            importing_playlist: None,
            sidebar_animations: Default::default(),
            sidebar_width: 280.0,
            sidebar_dragging: false,
            cache_stats: None,
            context_menu: None,
            song_edit_dialog: None,
            download_tab: Default::default(),
            image_state: ImageState::default(),

            playlist_page: PlaylistPageState {
                current: None,
                viewing_recently_played: false,
                song_animations: Default::default(),
                icon_animations: Default::default(),
                search_expanded: false,
                search_query: String::new(),
                search_animation: Default::default(),
                scroll_state: std::rc::Rc::new(std::cell::RefCell::new(
                    crate::ui::widgets::VirtualListState::default(),
                )),
                load_state: Default::default(),
                content_width: 904.0,
                description_expanded: false,
            },

            lyrics: LyricsState {
                is_open: false,
                animation: Default::default(),
                displayed_song_id: None,
                pending_song_id: None,
                lines: Vec::new(),
                current_line_idx: None,
                last_update: None,
                bg_colors: crate::utils::DominantColors::dark_default(),
                // Initialized directly as requested
                bg_shader: LyricsBackgroundProgram::new(),
                textured_bg_shader: TexturedBackgroundProgram::new(),
                // Engine will be created lazily when FontSystem is ready
                // This avoids blocking app startup with FontSystem::new()
                engine: None,
                shader_start_time: None,
                cached_engine_lines: None,
                cached_shaped_lines: None,
                // FontSystem will be created asynchronously
                shared_font_system: None,
                viewport_width: 800.0,  // Default, will be updated from view
                viewport_height: 600.0, // Default, will be updated from view
                viewport_initialized: false,
                pending_viewport_size: None,
                shaped_content_width: 0.0,
                shaped_font_size: 0.0,
                shape_generation: 0,
                pending_shape_song_id: None,
                pending_shape_generation: 0,
                pending_shape_content_width: 0.0,
                pending_shape_font_size: 0.0,
                is_loading: false,
                load_error: None,
            },

            dialogs: DialogState { import_open: false },

            home: HomePageState {
                banners: Vec::new(),
                current_banner: 0,
                top_picks: Vec::new(),
                trending_songs: Vec::new(),
                login_popup_open: false,
                qr_code_path: None,
                qr_unikey: None,
                qr_status: None,
                user_playlists: Vec::new(),
                current_ncm_playlist_songs: Vec::new(),
                song_hover_animations: Default::default(),
                last_banner: 0,
                carousel_animation: iced::animation::Animation::new(false),
                carousel_direction: 1,
            },

            discover: DiscoverPageState::default(),

            search: SearchPageState {
                scroll_state: std::rc::Rc::new(std::cell::RefCell::new(
                    crate::ui::widgets::VirtualListState::default(),
                )),
                ..Default::default()
            },
        }
    }

    /// Check if any global or submodule animation is currently active
    /// Optimized: O(1) check for hover animations, only checks active/fading states
    pub fn has_active_animations(&self, _now: Instant) -> bool {
        // Hover animations are now O(1) - they only track active + fading
        // iced_anim doesn't need Instant - it uses internal timing
        self.sidebar_animations.is_animating()
            || self.playlist_page.song_animations.is_animating()
            || self.playlist_page.icon_animations.is_animating()
            || self.playlist_page.search_animation.is_animating()
            || self.lyrics.animation.is_animating()
            || self.home.carousel_animation.is_animating(_now)
            || self.home.song_hover_animations.is_animating()
            || self.discover.card_animations.is_animating()
            || self.search.song_animations.is_animating()
            || self.search.card_animations.is_animating()
    }

    /// Clean up completed animations to prevent memory leaks
    /// Call this periodically (e.g., on AnimationTick)
    pub fn cleanup_animations(&mut self, now: Instant) {
        // Tick all animations to advance time
        self.sidebar_animations.tick(now);
        self.playlist_page.song_animations.tick(now);
        self.playlist_page.icon_animations.tick(now);
        self.playlist_page.search_animation.tick(now);
        self.lyrics.animation.tick(now);
        self.home.song_hover_animations.tick(now);
        self.discover.card_animations.tick(now);
        self.search.song_animations.tick(now);
        self.search.card_animations.tick(now);

        // Clean up completed fade-out animations
        self.sidebar_animations.cleanup_completed();
        self.playlist_page.song_animations.cleanup_completed();
        self.playlist_page.icon_animations.cleanup_completed();
        self.home.song_hover_animations.cleanup_completed();
        self.discover.card_animations.cleanup_completed();
        self.search.song_animations.cleanup_completed();
        self.search.card_animations.cleanup_completed();
    }

    /// Clear all playlist-related animations when navigating away
    pub fn clear_playlist_animations(&mut self) {
        self.playlist_page.song_animations.clear();
        self.playlist_page.icon_animations.clear();
    }
}

pub struct PlaylistPageState {
    pub current: Option<pages::PlaylistView>,
    pub viewing_recently_played: bool,
    pub song_animations: HoverAnimations<i64>,
    pub icon_animations: HoverAnimations<crate::app::message::IconId>,
    pub search_expanded: bool,
    pub search_query: String,
    pub search_animation: SingleHoverAnimation,
    /// Virtual list scroll state for efficient rendering
    pub scroll_state: std::rc::Rc<std::cell::RefCell<crate::ui::widgets::VirtualListState>>,
    /// Loading state for async playlist loading
    pub load_state: crate::app::update::page_loader::PlaylistLoadState,
    /// Main content width for responsive grid layouts
    pub content_width: f32,
    /// Whether the playlist description is expanded (vs clamped to 2 lines)
    pub description_expanded: bool,
}

impl Default for PlaylistPageState {
    fn default() -> Self {
        Self {
            current: None,
            viewing_recently_played: false,
            song_animations: Default::default(),
            icon_animations: Default::default(),
            search_expanded: false,
            search_query: String::new(),
            search_animation: Default::default(),
            scroll_state: std::rc::Rc::new(std::cell::RefCell::new(
                crate::ui::widgets::VirtualListState::default(),
            )),
            load_state: Default::default(),
            content_width: 904.0,
            description_expanded: false,
        }
    }
}

pub struct LyricsState {
    pub is_open: bool,
    pub animation: SingleHoverAnimation,
    /// Song currently displayed in the lyrics page.
    pub displayed_song_id: Option<i64>,
    /// Song currently being loaded for display.
    pub pending_song_id: Option<i64>,
    pub lines: Vec<crate::ui::pages::LyricLine>,
    pub current_line_idx: Option<usize>,
    pub last_update: Option<Instant>,

    // Visuals & Shaders
    pub bg_colors: crate::utils::DominantColors,
    pub bg_shader: LyricsBackgroundProgram,
    pub textured_bg_shader: TexturedBackgroundProgram,
    /// 歌词引擎 (RefCell 用于 view() 中的内部可变性)
    pub engine: Option<std::cell::RefCell<crate::features::lyrics::engine::LyricsEngine>>,
    pub shader_start_time: Option<Instant>,
    /// Cached engine lines to avoid recreating every frame
    /// Using Arc for O(1) clone in view function (thread-safe for iced Primitive)
    pub cached_engine_lines:
        Option<std::sync::Arc<Vec<crate::features::lyrics::engine::LyricLineData>>>,
    /// Cached shaped lines (pre-computed in background thread)
    /// 文本布局的唯一数据源
    pub cached_shaped_lines:
        Option<std::sync::Arc<Vec<crate::features::lyrics::engine::CachedShapedLine>>>,
    /// Shared font system for async text shaping (created asynchronously at app startup)
    pub shared_font_system: Option<crate::features::lyrics::engine::SharedFontSystem>,

    // Viewport info for line height calculations
    /// Last known viewport width (in logical pixels)
    pub viewport_width: f32,
    /// Last known viewport height (in logical pixels)
    pub viewport_height: f32,
    /// Whether viewport metrics have been initialized from a real layout/window event.
    pub viewport_initialized: bool,
    /// Last viewport size reported while the lyrics page transition was animating.
    pub pending_viewport_size: Option<iced::Size>,
    /// Content width used by the latest accepted shaped lines.
    pub shaped_content_width: f32,
    /// Main font size used by the latest accepted shaped lines.
    pub shaped_font_size: f32,
    /// Monotonic generation for async lyrics shaping requests.
    pub shape_generation: u64,
    /// Current in-flight shaping request for the displayed song.
    pub pending_shape_song_id: Option<i64>,
    pub pending_shape_generation: u64,
    pub pending_shape_content_width: f32,
    pub pending_shape_font_size: f32,

    // Display loading state
    /// Whether lyrics for the current display target are currently being loaded.
    pub is_loading: bool,
    /// Error message for the current display target.
    pub load_error: Option<String>,
}

pub struct DialogState {
    pub import_open: bool,
}

/// Discover page view mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DiscoverViewMode {
    /// Default view showing both sections with limited items
    #[default]
    Overview,
    /// Full view of recommended playlists
    AllRecommended,
    /// Full view of hot playlists with infinite scroll
    AllHot,
}

/// Search tab types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SearchTab {
    #[default]
    Songs,
    Artists,
    Albums,
    Playlists,
}

impl SearchTab {
    /// Get the NCM API search type code
    pub fn to_search_type(&self) -> crate::api::SearchType {
        match self {
            SearchTab::Songs => crate::api::SearchType::Songs,
            SearchTab::Artists => crate::api::SearchType::Artists,
            SearchTab::Albums => crate::api::SearchType::Albums,
            SearchTab::Playlists => crate::api::SearchType::Playlists,
        }
    }
}

/// Search page state
pub struct SearchPageState {
    /// Current search keyword
    pub keyword: String,
    /// Active search tab
    pub active_tab: SearchTab,
    /// Song search results
    pub tracks: Vec<Track>,
    /// Album search results
    pub albums: Vec<AlbumSummary>,
    /// Artist search results
    pub artists: Vec<ArtistSummary>,
    /// Playlist search results
    pub playlists: Vec<PlaylistSummary>,
    /// Total count for pagination
    pub total_count: u32,
    /// Current page (0-indexed)
    pub current_page: u32,
    /// Loading state
    pub loading: bool,
    /// Virtual list scroll state for efficient rendering of search results
    pub scroll_state: std::rc::Rc<std::cell::RefCell<crate::ui::widgets::VirtualListState>>,
    /// Hover animations for song list
    pub song_animations: HoverAnimations<u64>,
    /// Hover animations for grid cards
    pub card_animations: HoverAnimations<u64>,
    /// Content area width for responsive grid layouts
    pub content_width: f32,
}

impl Default for SearchPageState {
    fn default() -> Self {
        Self {
            keyword: String::new(),
            active_tab: SearchTab::default(),
            tracks: Vec::new(),
            albums: Vec::new(),
            artists: Vec::new(),
            playlists: Vec::new(),
            total_count: 0,
            current_page: 0,
            loading: false,
            scroll_state: std::rc::Rc::new(std::cell::RefCell::new(
                crate::ui::widgets::VirtualListState::default(),
            )),
            song_animations: Default::default(),
            card_animations: Default::default(),
            content_width: 936.0,
        }
    }
}

/// Discover page state for browsing playlists
pub struct DiscoverPageState {
    /// Current view mode
    pub view_mode: DiscoverViewMode,
    /// Recommended playlists (for logged-in users)
    pub recommended_playlists: Vec<PlaylistSummary>,
    /// Hot playlists (for all users)
    pub hot_playlists: Vec<PlaylistSummary>,
    /// Hover animations for playlist cards
    pub card_animations: HoverAnimations<u64>,
    /// Loading state for recommended playlists
    pub recommended_loading: bool,
    /// Loading state for hot playlists
    pub hot_loading: bool,
    /// Pagination offset for hot playlists
    pub hot_offset: u16,
    /// Whether more hot playlists are available
    pub hot_has_more: bool,
    /// Whether data has been loaded (to avoid re-fetching)
    pub data_loaded: bool,
    /// Content area width for dynamic grid column calculation
    pub content_width: f32,
}

impl Default for DiscoverPageState {
    fn default() -> Self {
        Self {
            view_mode: DiscoverViewMode::default(),
            recommended_playlists: Vec::new(),
            hot_playlists: Vec::new(),
            card_animations: Default::default(),
            recommended_loading: false,
            hot_loading: false,
            hot_offset: 0,
            hot_has_more: true,
            data_loaded: false,
            // Default width, will be updated from WindowResized/Sensor
            // Assumes window width ~1280, sidebar 280, padding 64
            content_width: 936.0,
        }
    }
}

/// Homepage state for NCM data
pub struct HomePageState {
    // Carousel banners
    pub banners: Vec<Banner>,
    pub current_banner: usize,

    // Content sections
    pub top_picks: Vec<PlaylistSummary>,
    pub trending_songs: Vec<Track>,

    // Login popup
    pub login_popup_open: bool,
    pub qr_code_path: Option<PathBuf>,
    pub qr_unikey: Option<String>,
    pub qr_status: Option<String>,

    pub user_playlists: Vec<PlaylistSummary>,
    /// Current NCM playlist songs (for playback)
    pub current_ncm_playlist_songs: Vec<Track>,

    // Hover animations for song list
    pub song_hover_animations: HoverAnimations<u64>,

    // Carousel animation
    pub last_banner: usize,
    pub carousel_animation: iced::animation::Animation<bool>,
    pub carousel_direction: i32,
}
