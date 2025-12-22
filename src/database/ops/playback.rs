//! Playback state operations

use anyhow::Result;
use sqlx::{Pool, Sqlite};

use super::current_timestamp;
use crate::database::DbPlaybackState;

/// Get current playback state
pub async fn get_playback_state(pool: &Pool<Sqlite>) -> Result<DbPlaybackState> {
    let state = sqlx::query_as::<_, DbPlaybackState>("SELECT * FROM playback_state WHERE id = 1")
        .fetch_one(pool)
        .await?;
    Ok(state)
}

/// Update playback position
/// For NCM songs (negative ID), converts to database ID by looking up the song
pub async fn update_playback_position(
    pool: &Pool<Sqlite>,
    song_id: Option<i64>,
    queue_position: i64,
    position_secs: f64,
) -> Result<()> {
    let now = current_timestamp();

    // Convert NCM song ID (negative) to database ID
    let db_song_id = if let Some(id) = song_id {
        if id < 0 {
            // NCM song - look up by file_path
            let ncm_id = (-id) as u64;
            let file_path = format!("ncm://{}", ncm_id);
            sqlx::query_scalar::<_, i64>("SELECT id FROM songs WHERE file_path = ?")
                .bind(&file_path)
                .fetch_optional(pool)
                .await?
        } else {
            Some(id)
        }
    } else {
        None
    };

    sqlx::query(
        "UPDATE playback_state SET current_song_id = ?, queue_position = ?, position_secs = ?, updated_at = ? WHERE id = 1",
    )
    .bind(db_song_id)
    .bind(queue_position)
    .bind(position_secs)
    .bind(now)
    .execute(pool)
    .await?;

    Ok(())
}

/// Update volume
pub async fn update_volume(pool: &Pool<Sqlite>, volume: f64) -> Result<()> {
    let now = current_timestamp();

    sqlx::query("UPDATE playback_state SET volume = ?, updated_at = ? WHERE id = 1")
        .bind(volume)
        .bind(now)
        .execute(pool)
        .await?;

    Ok(())
}

/// Update shuffle mode
pub async fn update_shuffle(pool: &Pool<Sqlite>, shuffle: bool) -> Result<()> {
    let now = current_timestamp();

    sqlx::query("UPDATE playback_state SET shuffle = ?, updated_at = ? WHERE id = 1")
        .bind(shuffle)
        .bind(now)
        .execute(pool)
        .await?;

    Ok(())
}

/// Update repeat mode (0=off, 1=all, 2=one)
pub async fn update_repeat_mode(pool: &Pool<Sqlite>, mode: i64) -> Result<()> {
    let now = current_timestamp();

    sqlx::query("UPDATE playback_state SET repeat_mode = ?, updated_at = ? WHERE id = 1")
        .bind(mode)
        .bind(now)
        .execute(pool)
        .await?;

    Ok(())
}
