//! Unified image pipeline handler.
//!
//! Queues remote image requests, runs a bounded number of downloads, then stores
//! the resulting handle in `ImageState` on `ImageDownloadReady`.

use iced::Task;

use crate::app::state::ImageRequest;
use crate::app::{App, Message};
use crate::image::{ImageKind, ImageResult};

const MAX_IMAGE_DOWNLOADS: usize = 6;

impl App {
    // ── Main handler ──

    /// Handle unified image-pipeline messages.
    pub fn handle_image(&mut self, message: &Message) -> Option<Task<Message>> {
        match message {
            Message::ImageDownloadReady(kind, id, path) => {
                self.store_image_handle(*kind, *id, path.clone());
                Some(Task::batch([
                    self.after_image_ready(*kind, *id, path),
                    self.pump_image_downloads(),
                ]))
            }

            Message::ImageDownloadFailed(kind, id) => {
                self.ui.image_state.clear_inflight(*kind, *id);
                Some(self.pump_image_downloads())
            }

            _ => None,
        }
    }

    /// Schedule all image work implied by the handled message and current app state.
    pub(super) fn collect_image_tasks_after_message(&mut self, message: &Message) -> Task<Message> {
        if matches!(
            message,
            Message::ImageDownloadReady(..) | Message::ImageDownloadFailed(..)
        ) {
            return Task::none();
        }

        Task::batch([
            self.collect_image_tasks_for_message(message),
            self.collect_current_song_image_task(),
        ])
    }

    /// Collect image references carried by a business message.
    fn collect_image_tasks_for_message(&mut self, message: &Message) -> Task<Message> {
        let mut refs = Vec::new();

        match message {
            Message::AutoLoginResult(Some(login_info), _) | Message::LoginSuccess(login_info) => {
                refs.push(RemoteImage::new(
                    ImageKind::UserAvatar,
                    login_info.user_id,
                    &login_info.avatar_url,
                ));
            }
            Message::BannersLoaded(banners) => {
                refs.extend(banners.iter().enumerate().map(|(index, banner)| {
                    RemoteImage::new(ImageKind::Banner, index as u64, &banner.image_url)
                }));
            }
            Message::TopPicksLoaded(playlists)
            | Message::UserPlaylistsLoaded(playlists)
            | Message::RecommendedPlaylistsLoaded(playlists) => {
                refs.extend(remote_playlist_covers(playlists));
            }
            Message::HotPlaylistsLoaded(playlists, _) => {
                refs.extend(remote_playlist_covers(playlists));
            }
            Message::TrendingSongsLoaded(songs)
            | Message::AddNcmPlaylist(songs, _)
            | Message::AddNcmPlaylistWithSource(songs, _, _) => {
                refs.extend(remote_track_covers(songs));
            }
            Message::UserArtistDetailLoaded(
                _,
                crate::api::ArtistDetail {
                    top_tracks: tracks, ..
                },
            ) => {
                refs.extend(remote_track_covers(tracks));
            }
            Message::PlayNcmSong(song) => {
                refs.push(RemoteImage::new(
                    ImageKind::SongCover,
                    song.id,
                    song.cover_url(),
                ));
            }
            Message::SearchResultsLoaded(payload) => match payload.tab {
                crate::app::state::SearchTab::Songs => {
                    refs.extend(remote_track_covers(&payload.tracks))
                }
                crate::app::state::SearchTab::Albums => {
                    refs.extend(remote_album_covers(&payload.albums));
                }
                crate::app::state::SearchTab::Artists => {
                    refs.extend(remote_artist_covers(&payload.artists));
                }
                crate::app::state::SearchTab::Playlists => {
                    refs.extend(remote_playlist_covers(&payload.playlists));
                }
            },
            Message::NcmPlaylistDetailLoaded(detail) => {
                refs.push(RemoteImage::new(
                    ImageKind::PlaylistCover,
                    detail.id,
                    &detail.cover_url,
                ));
                if detail.creator.id != 0 {
                    refs.push(RemoteImage::new(
                        ImageKind::UserAvatar,
                        detail.creator.id,
                        &detail.creator.avatar_url,
                    ));
                }
                refs.extend(remote_track_covers(&detail.tracks));
            }
            Message::AlbumDetailLoaded(detail) => {
                refs.push(RemoteImage::new(
                    ImageKind::AlbumCover,
                    detail.id,
                    &detail.image_url,
                ));
                if let Some(artist) = detail.primary_artist() {
                    refs.push(RemoteImage::new(
                        ImageKind::ArtistCover,
                        artist.id,
                        &artist.image_url,
                    ));
                }
                refs.extend(remote_track_covers(&detail.tracks));
            }
            Message::ArtistDetailLoaded(detail) => {
                refs.push(RemoteImage::new(
                    ImageKind::ArtistCover,
                    detail.id,
                    &detail.image_url,
                ));
                refs.extend(remote_track_covers(&detail.top_tracks));
            }
            Message::ArtistAlbumsLoaded(_, albums) => {
                refs.extend(remote_album_covers(albums));
            }
            Message::UserPageDetailLoaded(_, detail) => {
                refs.push(RemoteImage::new(
                    ImageKind::UserAvatar,
                    detail.user_id,
                    &detail.avatar_url,
                ));
                if detail.artist_id != 0 {
                    refs.push(RemoteImage::new(
                        ImageKind::ArtistCover,
                        detail.artist_id,
                        &detail.background_url,
                    ));
                }
            }
            Message::UserPagePlaylistsLoaded(_, playlists) => {
                refs.extend(remote_playlist_covers(playlists));
            }
            Message::PlaylistCreatorDetailLoaded(_, detail) => {
                refs.push(RemoteImage::new(
                    ImageKind::UserAvatar,
                    detail.user_id,
                    &detail.avatar_url,
                ));
            }
            Message::DownloadBatchEnqueue(items) => {
                refs.extend(items.iter().filter_map(|(_, ncm_id, _, metadata)| {
                    if let Some(crate::metadata::CoverSource::Url(url)) = &metadata.cover {
                        Some(RemoteImage::new(ImageKind::SongCover, *ncm_id, url))
                    } else {
                        None
                    }
                }));
            }
            Message::DownloadUrlResolved(song_id, ncm_id, _, metadata) => {
                if let Some(crate::metadata::CoverSource::Url(url)) = &metadata.cover {
                    let id = if *song_id < 0 {
                        *ncm_id
                    } else {
                        *song_id as u64
                    };
                    refs.push(RemoteImage::new(ImageKind::SongCover, id, url));
                }
            }
            _ => {}
        }

        let mut tasks = refs
            .into_iter()
            .map(|image| self.enqueue_image_download(image.kind, image.id, &image.url))
            .collect::<Vec<_>>();
        tasks.push(self.pump_image_downloads());

        Task::batch(tasks)
    }

    fn collect_current_song_image_task(&mut self) -> Task<Message> {
        let Some(song) = self.playback.current_song.clone() else {
            return Task::none();
        };
        let Some((kind, id)) = crate::image::song_cover_key(song.id) else {
            return Task::none();
        };

        if let Some(local_path) = self.resolved_song_cover_local_path(&song) {
            if self.ui.image_state.get(kind, id).is_none() {
                self.store_image_path(kind, id, local_path.clone());
            }

            if !cover_path_matches(song.cover_path.as_deref(), &local_path) {
                return self.after_image_ready(kind, id, &local_path);
            }

            return Task::none();
        }

        if let Some(path_or_url) = song.cover_path.as_deref() {
            if crate::image::is_remote_url(path_or_url) {
                let enqueue_task = self.enqueue_image_download(kind, id, path_or_url);
                return Task::batch([enqueue_task, self.pump_image_downloads()]);
            }
        }

        self.register_cached_image(kind, id)
            .unwrap_or_else(Task::none)
    }

    /// Return the already available local cover file for a song, using the
    /// unified image state first and the on-disk image cache second.
    pub(super) fn cached_song_cover_local_path(&self, song_id: i64) -> Option<std::path::PathBuf> {
        let (kind, id) = crate::image::song_cover_key(song_id)?;

        if let Some((path, _, _)) = self.ui.image_state.image_data(kind, id)
            && path.exists()
        {
            return Some(path.clone());
        }

        crate::image::resolve_cached(kind, id)
    }

    /// Resolve the local cover file for a song without starting network work.
    pub(super) fn resolved_song_cover_local_path(
        &self,
        song: &crate::database::DbSong,
    ) -> Option<std::path::PathBuf> {
        if let Some(path) = song.cover_path.as_deref()
            && crate::image::is_valid_local_path(path)
        {
            return Some(std::path::PathBuf::from(path));
        }

        self.cached_song_cover_local_path(song.id)
    }

    // ── Internal ──

    /// If the given `(kind, id)` is already in memory or on disk, populate
    /// `ImageState` immediately. Otherwise queue it for the bounded downloader.
    fn enqueue_image_download(&mut self, kind: ImageKind, id: u64, url: &str) -> Task<Message> {
        if url.is_empty() {
            return Task::none();
        }
        if self.ui.image_state.get(kind, id).is_some() {
            return Task::none();
        }
        if let Some(task) = self.register_cached_image(kind, id) {
            return task;
        }
        if self.ui.image_state.is_inflight(kind, id) || self.ui.image_state.is_queued(kind, id) {
            return Task::none();
        }
        if self.core.ncm_client.is_none() {
            return Task::none();
        }

        self.ui.image_state.enqueue(kind, id, url.to_string());
        Task::none()
    }

    fn pump_image_downloads(&mut self) -> Task<Message> {
        let Some(client) = self.core.ncm_client.as_ref().cloned() else {
            return Task::none();
        };

        let mut tasks = Vec::new();

        while self.ui.image_state.inflight.len() < MAX_IMAGE_DOWNLOADS {
            let Some(request) = self.ui.image_state.pop_pending() else {
                break;
            };

            if self.ui.image_state.get(request.kind, request.id).is_some()
                || self.ui.image_state.is_inflight(request.kind, request.id)
            {
                continue;
            }

            if let Some(path) = crate::image::resolve_cached(request.kind, request.id) {
                self.store_image_handle(request.kind, request.id, path.clone());
                tasks.push(self.after_image_ready(request.kind, request.id, &path));
                continue;
            }

            self.ui.image_state.mark_inflight(request.kind, request.id);
            tasks.push(start_image_download(client.clone(), request));
        }

        Task::batch(tasks)
    }

    fn register_cached_image(&mut self, kind: ImageKind, id: u64) -> Option<Task<Message>> {
        if self.ui.image_state.get(kind, id).is_some() {
            return None;
        }
        let path = crate::image::resolve_cached(kind, id)?;
        self.store_image_handle(kind, id, path.clone());
        Some(self.after_image_ready(kind, id, &path))
    }

    fn store_image_handle(&mut self, kind: ImageKind, id: u64, path: std::path::PathBuf) {
        self.ui.image_state.insert_path(kind, id, path.clone());
        self.sync_loaded_image_to_current_page(kind, id, &path);
    }

    fn after_image_ready(
        &mut self,
        kind: ImageKind,
        id: u64,
        path: &std::path::Path,
    ) -> Task<Message> {
        let song_id = match kind {
            ImageKind::SongCover => {
                let Some(id) = i64::try_from(id).ok().and_then(|id| id.checked_neg()) else {
                    return Task::none();
                };
                id
            }
            ImageKind::LocalSongCover => {
                let Some(id) = i64::try_from(id).ok() else {
                    return Task::none();
                };
                id
            }
            _ => return Task::none(),
        };

        let path_string = path.to_string_lossy().to_string();

        if let Some(current) = &mut self.playback.current_song
            && current.id == song_id
        {
            current.cover_path = Some(path_string.clone());
        }

        if let Some(idx) = self.playback.current_index
            && let Some(queue_song) = self.playback.queue.get_mut(idx)
            && queue_song.id == song_id
        {
            queue_song.cover_path = Some(path_string.clone());

            if let Some(db) = &self.core.db {
                let db = db.clone();
                match kind {
                    ImageKind::SongCover => {
                        let song_clone = queue_song.clone();
                        tokio::spawn(async move {
                            if let Err(e) = db.upsert_ncm_song(&song_clone).await {
                                tracing::warn!("Failed to update cover path in database: {}", e);
                            }
                        });
                    }
                    ImageKind::LocalSongCover => {
                        let cover_path = path_string.clone();
                        tokio::spawn(async move {
                            if let Err(e) = db.update_song_cover(song_id, &cover_path).await {
                                tracing::warn!(
                                    "Failed to update local cover path in database: {}",
                                    e
                                );
                            }
                        });
                    }
                    _ => {}
                }
            }
        }

        if self.ui.lyrics.is_open
            && let Some(song) = self.playback.current_song.clone()
            && song.id == song_id
        {
            return self.update_lyrics_background(&song);
        }

        if self.ui.lyrics.is_open
            && self
                .playback
                .preload_coordinator
                .window()
                .contains_song(song_id)
        {
            return self
                .prepare_lyrics_background_for_cover_path(song_id, std::path::PathBuf::from(path));
        }

        Task::none()
    }

    pub(crate) fn store_image_path(&mut self, kind: ImageKind, id: u64, path: std::path::PathBuf) {
        if !path.exists() {
            return;
        }
        self.store_image_handle(kind, id, path);
    }

    pub(crate) fn store_image_paths<I>(&mut self, images: I)
    where
        I: IntoIterator<Item = (ImageKind, u64, std::path::PathBuf)>,
    {
        for (kind, id, path) in images {
            self.store_image_path(kind, id, path);
        }
    }

    pub(crate) fn store_db_song_cover_paths(&mut self, songs: &[crate::database::DbSong]) {
        let images = songs.iter().filter_map(|song| {
            let path = song.cover_path.as_deref()?;
            if !crate::image::is_valid_local_path(path) {
                return None;
            }
            let (kind, id) = crate::image::song_cover_key(song.id)?;
            Some((kind, id, std::path::PathBuf::from(path)))
        });
        self.store_image_paths(images);
    }

    pub(crate) fn store_local_playlist_cover_paths(
        &mut self,
        playlists: &[crate::database::DbPlaylist],
    ) {
        let images = playlists.iter().filter_map(|playlist| {
            let id = u64::try_from(playlist.id).ok()?;
            let path = playlist.cover_path.as_deref()?;
            if !crate::image::is_valid_local_path(path) {
                return None;
            }
            Some((
                ImageKind::LocalPlaylistCover,
                id,
                std::path::PathBuf::from(path),
            ))
        });
        self.store_image_paths(images);
    }

    fn sync_loaded_image_to_current_page(
        &mut self,
        kind: ImageKind,
        id: u64,
        path: &std::path::Path,
    ) {
        let Some(page) = self.ui.playlist_page.current.as_mut() else {
            return;
        };

        let path_string = path.to_string_lossy().to_string();
        match kind {
            ImageKind::PlaylistCover
                if page.kind == crate::ui::pages::playlist::DetailPageKind::Playlist
                    && page.id == ncm_playlist_page_id(id) =>
            {
                set_detail_cover(page, path, path_string);
            }
            ImageKind::LocalPlaylistCover
                if page.kind == crate::ui::pages::playlist::DetailPageKind::Playlist
                    && page.id == id as i64 =>
            {
                set_detail_cover(page, path, path_string);
            }
            ImageKind::AlbumCover
                if page.kind == crate::ui::pages::playlist::DetailPageKind::Album
                    && page.id == album_page_id(id) =>
            {
                set_detail_cover(page, path, path_string);
            }
            ImageKind::ArtistCover
                if page.kind == crate::ui::pages::playlist::DetailPageKind::Artist
                    && page.id == artist_page_id(id) =>
            {
                set_detail_cover(page, path, path_string);
            }
            ImageKind::ArtistCover
                if page.kind == crate::ui::pages::playlist::DetailPageKind::User
                    && page.owner_artist_id == Some(id) =>
            {
                set_detail_cover(page, path, path_string);
            }
            ImageKind::ArtistCover if page.owner_artist_id == Some(id) => {
                page.owner_avatar_path = Some(path_string);
            }
            ImageKind::UserAvatar if page.creator_id == id => {
                if page.kind == crate::ui::pages::playlist::DetailPageKind::User
                    && page.cover_path.is_none()
                {
                    set_detail_cover(page, path, path_string);
                } else {
                    page.owner_avatar_path = Some(path_string);
                }
            }
            _ => {}
        }
    }
}

fn start_image_download(client: crate::api::NcmClient, request: ImageRequest) -> Task<Message> {
    let ImageRequest { kind, id, url } = request;
    let (width, height): (u16, u16) = match kind {
        ImageKind::Banner => (800, 280),
        ImageKind::SongCover | ImageKind::LocalSongCover => (200, 200),
        ImageKind::UserAvatar => (200, 200),
        _ => (300, 300),
    };

    Task::perform(
        async move {
            let base_path = kind.cache_dir().join(format!("{}.jpg", kind.file_stem(id)));
            crate::utils::download_img(&client, &url, base_path, width, height)
                .await
                .map(|path| ImageResult { kind, id, path })
        },
        move |result| match result {
            Some(r) => Message::ImageDownloadReady(r.kind, r.id, r.path),
            None => Message::ImageDownloadFailed(kind, id),
        },
    )
}

struct RemoteImage {
    kind: ImageKind,
    id: u64,
    url: String,
}

impl RemoteImage {
    fn new(kind: ImageKind, id: u64, url: &str) -> Self {
        Self {
            kind,
            id,
            url: url.to_string(),
        }
    }
}

fn remote_track_covers(tracks: &[crate::api::Track]) -> impl Iterator<Item = RemoteImage> + '_ {
    tracks
        .iter()
        .map(|track| RemoteImage::new(ImageKind::SongCover, track.id, track.cover_url()))
}

fn remote_playlist_covers(
    playlists: &[crate::api::PlaylistSummary],
) -> impl Iterator<Item = RemoteImage> + '_ {
    playlists
        .iter()
        .map(|item| RemoteImage::new(ImageKind::PlaylistCover, item.id, &item.cover_url))
}

fn remote_album_covers(
    albums: &[crate::api::AlbumSummary],
) -> impl Iterator<Item = RemoteImage> + '_ {
    albums
        .iter()
        .map(|item| RemoteImage::new(ImageKind::AlbumCover, item.id, &item.image_url))
}

fn remote_artist_covers(
    artists: &[crate::api::ArtistSummary],
) -> impl Iterator<Item = RemoteImage> + '_ {
    artists
        .iter()
        .map(|item| RemoteImage::new(ImageKind::ArtistCover, item.id, &item.image_url))
}

fn cover_path_matches(path_or_url: Option<&str>, local_path: &std::path::Path) -> bool {
    path_or_url.is_some_and(|path| {
        crate::image::is_valid_local_path(path) && std::path::Path::new(path) == local_path
    })
}

fn ncm_playlist_page_id(id: u64) -> i64 {
    -(id as i64)
}

fn artist_page_id(id: u64) -> i64 {
    i64::MIN + id as i64
}

fn album_page_id(id: u64) -> i64 {
    (i64::MIN / 4) + id as i64
}

fn set_detail_cover(
    page: &mut crate::ui::pages::PlaylistView,
    path: &std::path::Path,
    path_string: String,
) {
    page.cover_path = Some(path_string);
    page.palette = crate::utils::ColorPalette::from_image_path(path);
}
