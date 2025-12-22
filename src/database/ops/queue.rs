//! Queue operations

use anyhow::Result;
use sqlx::{Pool, Sqlite, SqliteConnection};

use crate::database::DbSong;

/// Clear the current queue
pub async fn clear_queue(pool: &Pool<Sqlite>) -> Result<()> {
    sqlx::query("DELETE FROM queue").execute(pool).await?;
    Ok(())
}

/// Clear the current queue (transaction version)
pub async fn clear_queue_tx(conn: &mut SqliteConnection) -> Result<()> {
    sqlx::query("DELETE FROM queue").execute(&mut *conn).await?;
    Ok(())
}

/// Add song to queue
pub async fn add_to_queue(
    pool: &Pool<Sqlite>,
    song_id: i64,
    source_playlist_id: Option<i64>,
) -> Result<()> {
    let max_pos: Option<i64> = sqlx::query_scalar("SELECT MAX(position) FROM queue")
        .fetch_one(pool)
        .await?;

    let position = max_pos.unwrap_or(-1) + 1;

    sqlx::query("INSERT INTO queue (song_id, position, source_playlist_id) VALUES (?, ?, ?)")
        .bind(song_id)
        .bind(position)
        .bind(source_playlist_id)
        .execute(pool)
        .await?;

    Ok(())
}

/// Set queue from a list of song ids (replaces current queue)
pub async fn set_queue(
    pool: &Pool<Sqlite>,
    song_ids: &[i64],
    source_playlist_id: Option<i64>,
) -> Result<()> {
    clear_queue(pool).await?;

    for (position, song_id) in song_ids.iter().enumerate() {
        sqlx::query("INSERT INTO queue (song_id, position, source_playlist_id) VALUES (?, ?, ?)")
            .bind(song_id)
            .bind(position as i64)
            .bind(source_playlist_id)
            .execute(pool)
            .await?;
    }

    Ok(())
}

/// Set queue from a list of song ids (transaction version)
pub async fn set_queue_tx(
    conn: &mut SqliteConnection,
    song_ids: &[i64],
    source_playlist_id: Option<i64>,
) -> Result<()> {
    clear_queue_tx(conn).await?;

    for (position, song_id) in song_ids.iter().enumerate() {
        sqlx::query("INSERT INTO queue (song_id, position, source_playlist_id) VALUES (?, ?, ?)")
            .bind(song_id)
            .bind(position as i64)
            .bind(source_playlist_id)
            .execute(&mut *conn)
            .await?;
    }

    Ok(())
}

/// Get current queue
/// For NCM songs (file_path starts with "ncm://"), the ID is converted to negative
pub async fn get_queue(pool: &Pool<Sqlite>) -> Result<Vec<DbSong>> {
    let mut songs = sqlx::query_as::<_, DbSong>(
        r#"
        SELECT s.* FROM songs s
        INNER JOIN queue q ON s.id = q.song_id
        ORDER BY q.position
        "#,
    )
    .fetch_all(pool)
    .await?;

    // Convert NCM song IDs to negative for consistency with the app
    for song in &mut songs {
        if song.file_path.starts_with("ncm://") {
            if let Ok(ncm_id) = song.file_path.trim_start_matches("ncm://").parse::<i64>() {
                song.id = -ncm_id;
            }
        }
    }

    Ok(songs)
}

/// Remove song from queue by position
pub async fn remove_from_queue(pool: &Pool<Sqlite>, position: i64) -> Result<()> {
    sqlx::query("DELETE FROM queue WHERE position = ?")
        .bind(position)
        .execute(pool)
        .await?;

    // Reorder remaining items
    sqlx::query("UPDATE queue SET position = position - 1 WHERE position > ?")
        .bind(position)
        .execute(pool)
        .await?;

    Ok(())
}
