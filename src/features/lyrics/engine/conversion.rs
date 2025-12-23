//! Conversion utilities for adapting existing lyric data to the new engine format
//!
//! This module provides conversion functions to transform various lyric formats
//! into the engine's internal `LyricLineData` format.
//!
//! ## Features
//!
//! The conversion process supports:
//! - Translation lines (translatedLyric)
//! - Romanized lines (romanLyric)
//! - Background vocals (isBG)
//! - Duet lines (isDuet)
//! - Emphasis detection for long words
//! - Word chunking for proper emphasis grouping

#![allow(dead_code)]

use super::types::{LyricLineData, WordData};
use super::word_splitter::{WordChunk, chunk_and_split_words};
use crate::ui::pages::lyrics::LyricLine as OldLyricLine;
use crate::ui::pages::lyrics::LyricWord as OldLyricWord;

/// Convert from old LyricLine format to new LyricLineData
pub fn convert_lyric_lines(old_lines: &[OldLyricLine]) -> Vec<LyricLineData> {
    old_lines
        .iter()
        .map(|old_line| {
            let words = convert_words(&old_line.words);
            let mut line = LyricLineData {
                text: old_line.text.clone(),
                words,
                translated: old_line.translated.clone(),
                romanized: None, // Not in old format
                start_ms: old_line.start_ms,
                end_ms: old_line.end_ms,
                is_duet: false, // Not in old format
                is_bg: false,   // Not in old format
                mask_animation: None,
            };
            // Compute mask animation after words are set
            line.compute_mask_animation();
            line
        })
        .collect()
}

/// Convert words from old format to new format
/// Uses Apple Music-style word chunking to determine emphasis
fn convert_words(old_words: &[OldLyricWord]) -> Vec<WordData> {
    let word_count = old_words.len();

    // First convert to WordData without emphasis
    let mut words: Vec<WordData> = old_words
        .iter()
        .enumerate()
        .map(|(i, old_word)| {
            WordData {
                text: old_word.word.clone(),
                start_ms: old_word.start_ms,
                end_ms: old_word.end_ms,
                roman_word: None, // Not in old format
                emphasize: false, // Will be calculated below
                x_start: 0.0,     // Will be calculated later
                x_end: 0.0,       // Will be calculated later
                // default: Last word gets emphasis boost (1.6x amount, 1.5x blur, 1.2x duration)
                is_last_word: i == word_count.saturating_sub(1),
            }
        })
        .collect();

    // Apply Apple Music-style word chunking to determine emphasis
    apply_chunk_emphasis(&mut words);

    words
}

/// Apply emphasis based on word chunking algorithm
///
/// Words in a group share emphasis if the merged word qualifies.
/// CJK words are always separate chunks.
fn apply_chunk_emphasis(words: &mut [WordData]) {
    let chunks = chunk_and_split_words(words);

    // Build emphasis map based on chunks
    // chunk_and_split_words may resplit words, so we need to map back
    // For simplicity, we'll iterate through original words and check each one
    for word in words.iter_mut() {
        // Find which chunk this word belongs to
        for chunk in &chunks {
            match chunk {
                WordChunk::Single(w) => {
                    if w.text == word.text && w.start_ms == word.start_ms {
                        word.emphasize = w.should_emphasize();
                        break;
                    }
                }
                WordChunk::Group(group) => {
                    // Check if this word is in the group
                    let in_group = group
                        .iter()
                        .any(|w| w.text == word.text && w.start_ms == word.start_ms);
                    if in_group {
                        // For groups, check if merged word qualifies
                        word.emphasize = chunk.should_emphasize();
                        break;
                    }
                }
            }
        }
    }
}

/// Convert from database song format to engine format
pub fn convert_from_db_lyrics(db_lyrics: &[OldLyricLine]) -> Vec<LyricLineData> {
    db_lyrics
        .iter()
        .map(|line| {
            let words = convert_words(&line.words);
            let mut line_data = LyricLineData {
                text: line.text.clone(),
                words,
                translated: line.translated.clone(),
                romanized: None, // Not in old format
                start_ms: line.start_ms,
                end_ms: line.end_ms,
                is_duet: false, // Not in old format
                is_bg: line.is_background,
                mask_animation: None,
            };
            // Compute mask animation after words are set
            line_data.compute_mask_animation();
            line_data
        })
        .collect()
}

/// Process lyrics to add advanced features
///
/// This function processes the lyrics to:
/// 1. Adjust start times (bring forward by up to 1 second)
/// 2. Sync background vocals with their parent lines
/// 3. Calculate word positions for rendering
pub fn process_lyrics_amll_style(lines: &mut [LyricLineData]) {
    if lines.is_empty() {
        return;
    }

    // Step 1: Bring line start times forward by up to 1 second
    // (This is done to make lyrics appear slightly before they're sung)
    for i in (0..lines.len()).rev() {
        if lines[i].is_bg {
            continue;
        }

        let prev_end = if i > 0 { lines[i - 1].end_ms } else { 0 };

        // Bring start time forward by up to 1 second, but not before previous line ends
        let new_start = lines[i].start_ms.saturating_sub(1000);
        lines[i].start_ms = new_start.max(prev_end).min(lines[i].start_ms);
    }

    // Step 2: Sync background vocals with their parent lines
    for i in 0..lines.len() {
        if lines[i].is_bg {
            continue;
        }

        // Check if next line is a background vocal
        if let Some(next) = lines.get(i + 1) {
            if next.is_bg {
                // Get the combined time range
                let bg_start = lines[i + 1]
                    .words
                    .iter()
                    .filter(|w| !w.text.trim().is_empty())
                    .map(|w| w.start_ms)
                    .min()
                    .unwrap_or(lines[i].start_ms);
                let bg_end = lines[i + 1]
                    .words
                    .iter()
                    .filter(|w| !w.text.trim().is_empty())
                    .map(|w| w.end_ms)
                    .max()
                    .unwrap_or(lines[i].end_ms);

                let start_time = bg_start.min(lines[i].start_ms);
                let end_time = bg_end.max(lines[i].end_ms);

                // Update both lines
                if i + 1 < lines.len() {
                    lines[i + 1].start_ms = start_time;
                    lines[i + 1].end_ms = end_time;
                }
            }
        }
    }
}

/// Check if a line has any words that should be emphasized
pub fn line_has_emphasis(line: &LyricLineData) -> bool {
    line.words.iter().any(|w| w.should_emphasize())
}

/// Get the total duration of a line in milliseconds
pub fn line_duration_ms(line: &LyricLineData) -> u64 {
    line.end_ms.saturating_sub(line.start_ms)
}
