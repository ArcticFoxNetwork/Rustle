//! Unified page loading system
//!
//! This module provides:
//! - "Already on page" detection to prevent redundant loads
//! - Unified handling for local and NCM playlists

use crate::app::state::App;

/// Loading state for playlist pages
#[derive(Debug, Clone, Default)]
pub enum PlaylistLoadState {
    /// No playlist being loaded
    #[default]
    Idle,
    /// Loading playlist (shows skeleton)
    Loading,
    /// Fully loaded
    Ready,
}

impl App {
    /// Check if we're already viewing the specified playlist
    /// Returns true if we should skip loading
    pub fn is_viewing_playlist(&self, playlist_id: i64) -> bool {
        if let Some(current) = &self.ui.playlist_page.current {
            current.id == playlist_id
        } else {
            false
        }
    }

    /// Check if we're already viewing the specified NCM playlist
    /// NCM playlists use negative IDs internally
    pub fn is_viewing_ncm_playlist(&self, ncm_playlist_id: u64) -> bool {
        let internal_id = -(ncm_playlist_id as i64);
        self.is_viewing_playlist(internal_id)
    }
}

/// Convert NCM songs to PlaylistSongView with pre-checked cover paths
pub fn convert_ncm_songs_to_views(
    songs: &[crate::api::SongInfo],
    cover_paths: &[(u64, Option<String>)],
) -> Vec<crate::ui::pages::PlaylistSongView> {
    let cover_map: std::collections::HashMap<u64, Option<String>> =
        cover_paths.iter().cloned().collect();

    songs
        .iter()
        .enumerate()
        .map(|(i, song)| {
            let duration_secs = song.duration / 1000;
            let mins = duration_secs / 60;
            let secs = duration_secs % 60;

            let cover_path = cover_map.get(&song.id).cloned().flatten();
            let pic_url = if cover_path.is_none() && !song.pic_url.is_empty() {
                Some(song.pic_url.clone())
            } else {
                None
            };

            crate::ui::components::playlist_view::SongItem::with_pic_url(
                -(song.id as i64),
                i + 1,
                song.name.clone(),
                if song.singer.is_empty() {
                    "未知艺术家".to_string()
                } else {
                    song.singer.clone()
                },
                if song.album.is_empty() {
                    "未知专辑".to_string()
                } else {
                    song.album.clone()
                },
                format!("{}:{:02}", mins, secs),
                String::new(),
                cover_path,
                pic_url,
                false,
            )
        })
        .collect()
}
