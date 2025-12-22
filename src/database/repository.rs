//! Database repository - main entry point
//! Delegates to ops modules for actual operations

use anyhow::Result;
use sqlx::{Pool, Sqlite, sqlite::SqlitePoolOptions};
use std::path::Path;

use super::{models::*, ops, schema};

/// Database connection pool wrapper
#[derive(Debug)]
pub struct Database {
    pool: Pool<Sqlite>,
}

impl Database {
    /// Create and initialize database at the given path
    pub async fn new(db_path: &Path) -> Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let db_url = format!("sqlite:{}?mode=rwc", db_path.display());

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&db_url)
            .await?;

        // Enable WAL mode for better concurrent read/write performance
        // This prevents UI reads from being blocked by background writes
        sqlx::query("PRAGMA journal_mode = WAL")
            .execute(&pool)
            .await?;

        // Optimize SQLite for better performance
        sqlx::query("PRAGMA synchronous = NORMAL")
            .execute(&pool)
            .await?;

        // Increase cache size (default is 2000 pages = ~8MB, set to ~32MB)
        sqlx::query("PRAGMA cache_size = -32000")
            .execute(&pool)
            .await?;

        schema::run_migrations(&pool).await?;

        Ok(Self { pool })
    }

    // ============ Song Operations ============

    pub async fn insert_song(&self, song: NewSong) -> Result<i64> {
        ops::insert_song(&self.pool, song).await
    }

    pub async fn get_song(&self, id: i64) -> Result<Option<DbSong>> {
        ops::get_song(&self.pool, id).await
    }

    pub async fn get_song_by_path(&self, path: &str) -> Result<Option<DbSong>> {
        ops::get_song_by_path(&self.pool, path).await
    }

    pub async fn get_all_songs(&self) -> Result<Vec<DbSong>> {
        ops::get_all_songs(&self.pool).await
    }

    pub async fn search_songs(&self, query: &str) -> Result<Vec<DbSong>> {
        ops::search_songs(&self.pool, query).await
    }

    pub async fn delete_song(&self, id: i64) -> Result<()> {
        ops::delete_song(&self.pool, id).await
    }

    pub async fn delete_song_by_path(&self, path: &str) -> Result<()> {
        ops::delete_song_by_path(&self.pool, path).await
    }

    pub async fn update_song_path(&self, old_path: &str, new_path: &str) -> Result<()> {
        ops::update_song_path(&self.pool, old_path, new_path).await
    }

    pub async fn upsert_ncm_song(&self, song: &DbSong) -> Result<i64> {
        ops::upsert_ncm_song(&self.pool, song).await
    }

    // ============ Playlist Operations ============

    pub async fn create_playlist(&self, playlist: NewPlaylist) -> Result<i64> {
        ops::create_playlist(&self.pool, playlist).await
    }

    pub async fn get_all_playlists(&self) -> Result<Vec<DbPlaylist>> {
        ops::get_all_playlists(&self.pool).await
    }

    pub async fn get_playlist(&self, id: i64) -> Result<Option<DbPlaylist>> {
        ops::get_playlist(&self.pool, id).await
    }

    pub async fn add_song_to_playlist(&self, playlist_id: i64, song_id: i64) -> Result<()> {
        ops::add_song_to_playlist(&self.pool, playlist_id, song_id).await
    }

    pub async fn get_playlist_songs(&self, playlist_id: i64) -> Result<Vec<DbSong>> {
        ops::get_playlist_songs(&self.pool, playlist_id).await
    }

    pub async fn get_playlist_songs_with_date(
        &self,
        playlist_id: i64,
    ) -> Result<Vec<crate::database::DbPlaylistSongWithDate>> {
        ops::get_playlist_songs_with_date(&self.pool, playlist_id).await
    }

    pub async fn remove_song_from_playlist(&self, playlist_id: i64, song_id: i64) -> Result<()> {
        ops::remove_song_from_playlist(&self.pool, playlist_id, song_id).await
    }

    pub async fn delete_playlist(&self, id: i64) -> Result<()> {
        ops::delete_playlist(&self.pool, id).await
    }

    pub async fn update_playlist(
        &self,
        id: i64,
        name: &str,
        description: Option<&str>,
    ) -> Result<()> {
        ops::update_playlist(&self.pool, id, name, description).await
    }

    pub async fn update_playlist_full(
        &self,
        id: i64,
        name: &str,
        description: Option<&str>,
        cover_path: Option<&str>,
    ) -> Result<()> {
        ops::update_playlist_full(&self.pool, id, name, description, cover_path).await
    }

    // ============ Queue Operations ============

    pub async fn clear_queue(&self) -> Result<()> {
        ops::clear_queue(&self.pool).await
    }

    pub async fn add_to_queue(&self, song_id: i64, source_playlist_id: Option<i64>) -> Result<()> {
        ops::add_to_queue(&self.pool, song_id, source_playlist_id).await
    }

    pub async fn set_queue(&self, song_ids: &[i64], source_playlist_id: Option<i64>) -> Result<()> {
        ops::set_queue(&self.pool, song_ids, source_playlist_id).await
    }

    /// Save queue with full song data, handling NCM songs properly
    /// NCM songs (negative ID) will be upserted to the database first
    /// Uses a transaction for better performance
    pub async fn save_queue_with_songs(
        &self,
        songs: &[DbSong],
        source_playlist_id: Option<i64>,
    ) -> Result<()> {
        use sqlx::Acquire;

        let mut conn = self.pool.acquire().await?;
        let mut tx = conn.begin().await?;

        let mut db_song_ids = Vec::with_capacity(songs.len());

        for song in songs {
            if song.id < 0 || song.file_path.starts_with("ncm://") {
                // NCM song - upsert to database and get the real ID
                let db_id = ops::upsert_ncm_song_tx(&mut *tx, song).await?;
                db_song_ids.push(db_id);
            } else {
                // Local song - use existing ID
                db_song_ids.push(song.id);
            }
        }

        ops::set_queue_tx(&mut *tx, &db_song_ids, source_playlist_id).await?;

        tx.commit().await?;
        Ok(())
    }

    pub async fn get_queue(&self) -> Result<Vec<DbSong>> {
        ops::get_queue(&self.pool).await
    }

    pub async fn remove_from_queue(&self, position: i64) -> Result<()> {
        ops::remove_from_queue(&self.pool, position).await
    }

    // ============ Playback State Operations ============

    pub async fn get_playback_state(&self) -> Result<DbPlaybackState> {
        ops::get_playback_state(&self.pool).await
    }

    pub async fn update_playback_position(
        &self,
        song_id: Option<i64>,
        queue_position: i64,
        position_secs: f64,
    ) -> Result<()> {
        ops::update_playback_position(&self.pool, song_id, queue_position, position_secs).await
    }

    pub async fn update_volume(&self, volume: f64) -> Result<()> {
        ops::update_volume(&self.pool, volume).await
    }

    pub async fn update_shuffle(&self, shuffle: bool) -> Result<()> {
        ops::update_shuffle(&self.pool, shuffle).await
    }

    pub async fn update_repeat_mode(&self, mode: i64) -> Result<()> {
        ops::update_repeat_mode(&self.pool, mode).await
    }

    // ============ Play History Operations ============

    pub async fn record_play(
        &self,
        song_id: i64,
        listened_secs: i64,
        completed: bool,
    ) -> Result<()> {
        ops::record_play(&self.pool, song_id, listened_secs, completed).await
    }

    pub async fn get_recently_played(&self, limit: i64) -> Result<Vec<DbSong>> {
        ops::get_recently_played(&self.pool, limit).await
    }

    pub async fn get_play_count(&self, song_id: i64) -> Result<i64> {
        ops::get_play_count(&self.pool, song_id).await
    }
}
