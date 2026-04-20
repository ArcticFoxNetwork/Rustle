use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LyricsCacheStatus {
    Fetching,
    Ready,
    Failed,
}

#[derive(Debug, Clone)]
pub struct LyricsCacheEntry {
    pub ncm_id: u64,
    pub status: LyricsCacheStatus,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayFetchAction {
    StartFetch,
    AwaitExisting,
    UseCache,
}

#[derive(Debug, Default, Clone)]
pub struct LyricsCacheManagerState {
    pub entries: HashMap<i64, LyricsCacheEntry>,
}

impl LyricsCacheManagerState {
    pub fn should_schedule_warmup(&mut self, song_id: i64, ncm_id: u64) -> bool {
        if crate::features::lyrics::is_lyrics_cached(ncm_id) {
            self.mark_ready(song_id, ncm_id);
            return false;
        }

        if self
            .entries
            .get(&song_id)
            .is_some_and(|entry| entry.status == LyricsCacheStatus::Ready)
        {
            self.entries.remove(&song_id);
            return true;
        }

        !matches!(
            self.entries.get(&song_id).map(|entry| entry.status),
            Some(LyricsCacheStatus::Fetching) | Some(LyricsCacheStatus::Failed)
        )
    }

    pub fn begin_warmup(&mut self, song_id: i64, ncm_id: u64) -> bool {
        if crate::features::lyrics::is_lyrics_cached(ncm_id) {
            self.mark_ready(song_id, ncm_id);
            return false;
        }

        if self
            .entries
            .get(&song_id)
            .is_some_and(|entry| entry.status == LyricsCacheStatus::Ready)
        {
            self.entries.remove(&song_id);
        }

        if matches!(
            self.entries.get(&song_id).map(|entry| entry.status),
            Some(LyricsCacheStatus::Fetching)
        ) {
            return false;
        }

        self.entries.insert(
            song_id,
            LyricsCacheEntry {
                ncm_id,
                status: LyricsCacheStatus::Fetching,
                last_error: None,
            },
        );
        true
    }

    pub fn register_display_fetch(&mut self, song_id: i64, ncm_id: u64) -> DisplayFetchAction {
        if crate::features::lyrics::is_lyrics_cached(ncm_id) {
            self.mark_ready(song_id, ncm_id);
            return DisplayFetchAction::UseCache;
        }

        if matches!(
            self.entries.get(&song_id).map(|entry| entry.status),
            Some(LyricsCacheStatus::Fetching)
        ) {
            return DisplayFetchAction::AwaitExisting;
        }

        self.entries.insert(
            song_id,
            LyricsCacheEntry {
                ncm_id,
                status: LyricsCacheStatus::Fetching,
                last_error: None,
            },
        );
        DisplayFetchAction::StartFetch
    }

    pub fn finish_warmup(&mut self, song_id: i64, result: Result<(), String>) {
        let Some(entry) = self.entries.get_mut(&song_id) else {
            return;
        };

        match result {
            Ok(()) => {
                if crate::features::lyrics::is_lyrics_cached(entry.ncm_id) {
                    entry.status = LyricsCacheStatus::Ready;
                    entry.last_error = None;
                } else {
                    entry.status = LyricsCacheStatus::Failed;
                    entry.last_error = Some("Lyrics fetched but cache file was not created".into());
                }
            }
            Err(error) => {
                entry.status = LyricsCacheStatus::Failed;
                entry.last_error = Some(error);
            }
        }
    }

    pub fn mark_ready(&mut self, song_id: i64, ncm_id: u64) {
        self.entries.insert(
            song_id,
            LyricsCacheEntry {
                ncm_id,
                status: LyricsCacheStatus::Ready,
                last_error: None,
            },
        );
    }
}
