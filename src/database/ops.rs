//! Database operations organized by entity type

mod history;
mod playback;
mod playlists;
mod queue;
mod songs;

pub use history::*;
pub use playback::*;
pub use playlists::*;
pub use queue::*;
pub use songs::*;

use std::time::{SystemTime, UNIX_EPOCH};

/// Get current Unix timestamp
pub fn current_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}
