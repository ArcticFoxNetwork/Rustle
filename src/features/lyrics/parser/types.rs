//! Lyrics data types
//!
//! Owned variants for easier use.

use serde::{Deserialize, Serialize};

/// Lyrics format enum
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LyricsFormat {
    /// Standard LRC format [mm:ss.xx]text
    Lrc,
    /// NetEase Cloud Music YRC format (word-level)
    Yrc,
    /// QQ Music QRC format (word-level)
    Qrc,
    /// Foobar2000 ESLyric format (word-level)
    EsLrc,
    /// Lyricify Syllable format
    Lys,
    /// Apple Music TTML format
    Ttml,
    /// Unknown format
    Unknown,
}

/// A single word in a lyric line
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricWordOwned {
    /// Start time in milliseconds
    pub start_time: u64,
    /// End time in milliseconds
    pub end_time: u64,
    /// The word text
    pub word: String,
    /// Romanized/phonetic version of the word
    pub roman_word: String,
}

impl LyricWordOwned {
    /// Check if the word is empty (whitespace only)
    pub fn is_empty(&self) -> bool {
        self.word.trim().is_empty()
    }
}

/// A single line of lyrics
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricLineOwned {
    /// Words in this line (for word-level sync)
    pub words: Vec<LyricWordOwned>,
    /// Translated lyric text
    pub translated_lyric: String,
    /// Romanized/phonetic lyric text
    pub roman_lyric: String,
    /// Whether this is a background vocal line
    #[serde(default, rename = "isBG")]
    pub is_bg: bool,
    /// Whether this is a duet line (different singer)
    #[serde(default)]
    pub is_duet: bool,
    /// Start time in milliseconds
    #[serde(default)]
    pub start_time: u64,
    /// End time in milliseconds
    #[serde(default)]
    pub end_time: u64,
}

impl LyricLineOwned {
    /// Get the full line text by joining all words
    pub fn to_line(&self) -> String {
        self.words
            .iter()
            .map(|w| w.word.as_str())
            .collect::<Vec<_>>()
            .join("")
    }

    /// Check if the line is empty
    pub fn is_empty(&self) -> bool {
        self.words.is_empty() || self.words.iter().all(|w| w.is_empty())
    }
}

/// Process lyrics lines: sort by time and clamp values
pub fn process_lyrics(lines: &mut [LyricLineOwned]) {
    // Sort by start time
    lines.sort_by(|a, b| {
        a.words
            .first()
            .map(|x| x.start_time)
            .cmp(&b.words.first().map(|x| x.start_time))
    });

    const MAX_TIME: u64 = 60039999; // 999:99.999

    for line in lines.iter_mut() {
        // Update start_time from first word
        line.start_time = line
            .words
            .first()
            .map(|x| x.start_time)
            .unwrap_or(line.start_time)
            .clamp(0, MAX_TIME);

        // Update end_time from last word, but only if word has valid end_time
        // This preserves end_time calculated by parsers (like LRC's next-line-start logic)
        let word_end_time = line.words.last().map(|x| x.end_time).unwrap_or(0);
        if word_end_time > 0 {
            line.end_time = word_end_time.clamp(0, MAX_TIME);
        } else if line.end_time == 0 {
            // If both are 0, we'll fix it in the next pass
            line.end_time = 0;
        } else {
            line.end_time = line.end_time.clamp(0, MAX_TIME);
        }

        for word in line.words.iter_mut() {
            word.start_time = word.start_time.clamp(0, MAX_TIME);
            word.end_time = word.end_time.clamp(0, MAX_TIME);
        }
    }

    // Second pass: fix any remaining 0 end_times using next line's start_time
    let len = lines.len();
    for i in 0..len {
        if lines[i].end_time == 0 {
            let next_start = if i + 1 < len {
                lines[i + 1].start_time
            } else {
                // Last line: add 5 seconds
                lines[i].start_time.saturating_add(5000)
            };
            lines[i].end_time = next_start.clamp(0, MAX_TIME);

            // Also fix word end_time
            if let Some(word) = lines[i].words.last_mut() {
                if word.end_time == 0 {
                    word.end_time = next_start.clamp(0, MAX_TIME);
                }
            }
        }
    }
}
