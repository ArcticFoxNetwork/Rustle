//! Database module for persistent storage
//! Uses SQLite via sqlx for storing playlists, songs, and playback state

mod models;
mod ops;
mod repository;
mod schema;

pub use models::*;
pub use repository::Database;
