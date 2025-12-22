//! Pages module
//! Full-page views for the music streaming application

pub mod audio_engine;
pub mod discover;
pub mod home;
pub mod lyrics;
pub mod playlist;
pub mod settings;

pub use lyrics::{LyricLine, LyricWord, find_current_line};
pub use playlist::{PlaylistSongView, PlaylistView}; // PlaylistSongView used by app when loading playlists
