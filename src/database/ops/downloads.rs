use sqlx::{Pool, Sqlite};

use crate::database::DownloadRow;

pub async fn insert_download(
    pool: &Pool<Sqlite>,
    song_id: i64,
    ncm_id: u64,
    title: &str,
    artist: &str,
    file_path: &str,
    file_size: u64,
    quality: &str,
) -> Result<(), sqlx::Error> {
    let now = super::current_timestamp();
    sqlx::query(
        "INSERT INTO downloads (song_id, ncm_id, title, artist, file_path, file_size, quality, downloaded_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(song_id)
    .bind(ncm_id as i64)
    .bind(title)
    .bind(artist)
    .bind(file_path)
    .bind(file_size as i64)
    .bind(quality)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_all_downloads(pool: &Pool<Sqlite>) -> Result<Vec<DownloadRow>, sqlx::Error> {
    sqlx::query_as::<_, DownloadRow>(
        "SELECT song_id, ncm_id, title, artist, file_path, file_size, quality, downloaded_at FROM downloads ORDER BY downloaded_at DESC",
    )
    .fetch_all(pool)
    .await
}

pub async fn delete_download(pool: &Pool<Sqlite>, song_id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM downloads WHERE song_id = ?")
        .bind(song_id)
        .execute(pool)
        .await?;
    Ok(())
}
