//! Song CRUD operations

use anyhow::Result;
use sqlx::{Pool, Sqlite};

use super::current_timestamp;
use crate::database::{DbSong, NewSong};

/// Insert a new song, returns the new song id
pub async fn insert_song(pool: &Pool<Sqlite>, song: NewSong) -> Result<i64> {
    let now = current_timestamp();

    let result = sqlx::query(
        r#"
        INSERT INTO songs (file_path, title, artist, album, duration_secs, track_number, year, genre, cover_path, file_hash, file_size, format, last_modified, created_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&song.file_path)
    .bind(&song.title)
    .bind(&song.artist)
    .bind(&song.album)
    .bind(song.duration_secs)
    .bind(song.track_number)
    .bind(song.year)
    .bind(&song.genre)
    .bind(&song.cover_path)
    .bind(&song.file_hash)
    .bind(song.file_size)
    .bind(&song.format)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;

    Ok(result.last_insert_rowid())
}

/// Get song by id
pub async fn get_song(pool: &Pool<Sqlite>, id: i64) -> Result<Option<DbSong>> {
    let song = sqlx::query_as::<_, DbSong>("SELECT * FROM songs WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(song)
}

/// Get song by file path
pub async fn get_song_by_path(pool: &Pool<Sqlite>, path: &str) -> Result<Option<DbSong>> {
    let song = sqlx::query_as::<_, DbSong>("SELECT * FROM songs WHERE file_path = ?")
        .bind(path)
        .fetch_optional(pool)
        .await?;
    Ok(song)
}

/// Get all songs
pub async fn get_all_songs(pool: &Pool<Sqlite>) -> Result<Vec<DbSong>> {
    let songs =
        sqlx::query_as::<_, DbSong>("SELECT * FROM songs ORDER BY artist, album, track_number")
            .fetch_all(pool)
            .await?;
    Ok(songs)
}

/// Search songs by title, artist, or album
pub async fn search_songs(pool: &Pool<Sqlite>, query: &str) -> Result<Vec<DbSong>> {
    let pattern = format!("%{}%", query);
    let songs = sqlx::query_as::<_, DbSong>(
        "SELECT * FROM songs WHERE title LIKE ? OR artist LIKE ? OR album LIKE ? ORDER BY title",
    )
    .bind(&pattern)
    .bind(&pattern)
    .bind(&pattern)
    .fetch_all(pool)
    .await?;
    Ok(songs)
}

/// Delete song by id
pub async fn delete_song(pool: &Pool<Sqlite>, id: i64) -> Result<()> {
    sqlx::query("DELETE FROM songs WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Delete song by file path
pub async fn delete_song_by_path(pool: &Pool<Sqlite>, path: &str) -> Result<()> {
    sqlx::query("DELETE FROM songs WHERE file_path = ?")
        .bind(path)
        .execute(pool)
        .await?;
    Ok(())
}

/// Update song file path (for handling file renames)
pub async fn update_song_path(pool: &Pool<Sqlite>, old_path: &str, new_path: &str) -> Result<()> {
    let now = super::current_timestamp();
    sqlx::query("UPDATE songs SET file_path = ?, last_modified = ? WHERE file_path = ?")
        .bind(new_path)
        .bind(now)
        .bind(old_path)
        .execute(pool)
        .await?;
    Ok(())
}

/// Upsert NCM song (insert if not exists, update if exists)
/// Returns the database ID of the song
///
/// Note: cover_path should be a local file path, not a URL.
/// If cover_path starts with "http", it will be set to None.
pub async fn upsert_ncm_song(pool: &Pool<Sqlite>, song: &DbSong) -> Result<i64> {
    let now = super::current_timestamp();

    // Determine NCM ID from song.id (negative) or extract from file_path
    let ncm_id = if song.id < 0 {
        (-song.id) as u64
    } else if song.file_path.starts_with("ncm://") {
        song.file_path
            .trim_start_matches("ncm://")
            .parse()
            .unwrap_or(0)
    } else {
        song.id as u64
    };

    // NCM songs are identified by "ncm://<id>" in file_path for DB storage
    let file_path = format!("ncm://{}", ncm_id);

    // Only store local cover paths, not URLs
    let cover_path = song.cover_path.as_ref().and_then(|p| {
        if p.starts_with("http") {
            None
        } else {
            Some(p.clone())
        }
    });

    // Check if exists
    let existing = sqlx::query_scalar::<_, i64>("SELECT id FROM songs WHERE file_path = ?")
        .bind(&file_path)
        .fetch_optional(pool)
        .await?;

    if let Some(id) = existing {
        // Update - only update cover_path if we have a valid local path
        if cover_path.is_some() {
            sqlx::query(
                r#"
                UPDATE songs SET 
                    title = ?, artist = ?, album = ?, duration_secs = ?, 
                    cover_path = ?, last_modified = ?
                WHERE id = ?
                "#,
            )
            .bind(&song.title)
            .bind(&song.artist)
            .bind(&song.album)
            .bind(song.duration_secs)
            .bind(&cover_path)
            .bind(now)
            .bind(id)
            .execute(pool)
            .await?;
        } else {
            // Don't overwrite existing cover_path with None
            sqlx::query(
                r#"
                UPDATE songs SET 
                    title = ?, artist = ?, album = ?, duration_secs = ?, 
                    last_modified = ?
                WHERE id = ?
                "#,
            )
            .bind(&song.title)
            .bind(&song.artist)
            .bind(&song.album)
            .bind(song.duration_secs)
            .bind(now)
            .bind(id)
            .execute(pool)
            .await?;
        }

        Ok(id)
    } else {
        // Insert
        let result = sqlx::query(
            r#"
            INSERT INTO songs (
                file_path, title, artist, album, duration_secs, 
                cover_path, format, last_modified, created_at, play_count
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 0)
            "#,
        )
        .bind(&file_path)
        .bind(&song.title)
        .bind(&song.artist)
        .bind(&song.album)
        .bind(song.duration_secs)
        .bind(&cover_path)
        .bind("ncm")
        .bind(now)
        .bind(now)
        .execute(pool)
        .await?;

        Ok(result.last_insert_rowid())
    }
}

/// Upsert NCM song (transaction version for batch operations)
pub async fn upsert_ncm_song_tx(conn: &mut sqlx::SqliteConnection, song: &DbSong) -> Result<i64> {
    let now = super::current_timestamp();

    let ncm_id = if song.id < 0 {
        (-song.id) as u64
    } else if song.file_path.starts_with("ncm://") {
        song.file_path
            .trim_start_matches("ncm://")
            .parse()
            .unwrap_or(0)
    } else {
        song.id as u64
    };

    let file_path = format!("ncm://{}", ncm_id);

    // Keep cover_path as-is (can be URL or local path)
    // UI layer will handle URL vs local path distinction
    let cover_path = song.cover_path.clone();

    // Check if exists
    let existing = sqlx::query_scalar::<_, i64>("SELECT id FROM songs WHERE file_path = ?")
        .bind(&file_path)
        .fetch_optional(&mut *conn)
        .await?;

    if let Some(id) = existing {
        // Only update cover_path if we have a new value AND it's a local path
        // This prevents overwriting local path with URL
        let should_update_cover = cover_path
            .as_ref()
            .map(|p| !p.starts_with("http"))
            .unwrap_or(false);

        if should_update_cover {
            sqlx::query(
                r#"
                UPDATE songs SET 
                    title = ?, artist = ?, album = ?, duration_secs = ?, 
                    cover_path = ?, last_modified = ?
                WHERE id = ?
                "#,
            )
            .bind(&song.title)
            .bind(&song.artist)
            .bind(&song.album)
            .bind(song.duration_secs)
            .bind(&cover_path)
            .bind(now)
            .bind(id)
            .execute(&mut *conn)
            .await?;
        } else {
            sqlx::query(
                r#"
                UPDATE songs SET 
                    title = ?, artist = ?, album = ?, duration_secs = ?, 
                    last_modified = ?
                WHERE id = ?
                "#,
            )
            .bind(&song.title)
            .bind(&song.artist)
            .bind(&song.album)
            .bind(song.duration_secs)
            .bind(now)
            .bind(id)
            .execute(&mut *conn)
            .await?;
        }

        Ok(id)
    } else {
        let result = sqlx::query(
            r#"
            INSERT INTO songs (
                file_path, title, artist, album, duration_secs, 
                cover_path, format, last_modified, created_at, play_count
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 0)
            "#,
        )
        .bind(&file_path)
        .bind(&song.title)
        .bind(&song.artist)
        .bind(&song.album)
        .bind(song.duration_secs)
        .bind(&cover_path)
        .bind("ncm")
        .bind(now)
        .bind(now)
        .execute(&mut *conn)
        .await?;

        Ok(result.last_insert_rowid())
    }
}
