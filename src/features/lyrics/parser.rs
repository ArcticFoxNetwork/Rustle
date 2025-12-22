//! Lyrics parsing module
//!
//! Supports multiple lyrics formats:
//! - LRC: Standard line-level lyrics [mm:ss.xx]text
//! - YRC: NetEase Cloud Music word-level lyrics
//! - QRC: QQ Music word-level lyrics
//! - ESLrc: Foobar2000 ESLyric word-level format
//! - LYS: Lyricify Syllable format
//! - TTML: Apple Music lyrics format (XML)
//! - ASS: Subtitle export format

mod ass;
mod eslrc;
mod lrc;
mod lys;
mod online;
mod qrc;
mod ttml;
mod types;
mod yrc;

pub use online::*;
pub use types::*;

use std::path::Path;

/// Detect lyrics format from content
pub fn detect_format(content: &str) -> LyricsFormat {
    let trimmed = content.trim();

    // TTML format: XML with <tt> root element
    if trimmed.starts_with("<?xml") || trimmed.starts_with("<tt") {
        return LyricsFormat::Ttml;
    }

    // YRC format: starts with [timestamp,duration]
    if trimmed.starts_with('[') {
        // Check for YRC pattern: [start,duration](word_start,word_duration,0)
        if let Some(first_line) = trimmed.lines().next() {
            // YRC has pattern like [0,1000](0,500,0)word
            if first_line.contains("](") && first_line.contains(",0)") {
                return LyricsFormat::Yrc;
            }
            // QRC has pattern like [0,1000]word(0,500)
            if first_line.contains("](")
                || (first_line.contains("(")
                    && first_line.contains(")")
                    && !first_line.contains(",0)"))
            {
                // Check if it's QRC (word before timestamp) or YRC (timestamp before word)
                if let Some(bracket_end) = first_line.find(']') {
                    let after_bracket = &first_line[bracket_end + 1..];
                    // QRC: text(time,duration)
                    // YRC: (time,duration,0)text
                    if after_bracket.starts_with('(') {
                        return LyricsFormat::Yrc;
                    } else if after_bracket.contains('(') {
                        return LyricsFormat::Qrc;
                    }
                }
            }
            // LYS format: starts with [digit] property marker
            if first_line.len() >= 3 && first_line.starts_with('[') {
                if let Some(c) = first_line.chars().nth(1) {
                    if c.is_ascii_digit() {
                        if let Some(c2) = first_line.chars().nth(2) {
                            if c2 == ']' {
                                return LyricsFormat::Lys;
                            }
                        }
                    }
                }
            }
            // ESLrc: [mm:ss.xx]text[mm:ss.xx]
            // Check if there are multiple timestamps in a single line
            let timestamp_count = first_line.matches('[').count();
            if timestamp_count >= 2 {
                // Check if it's ESLrc pattern (timestamps interleaved with text)
                let parts: Vec<&str> = first_line.split('[').collect();
                if parts.len() >= 3 {
                    // ESLrc has pattern: [time]word[time]word[time]
                    let mut is_eslrc = true;
                    for part in parts.iter().skip(1) {
                        if let Some(bracket_pos) = part.find(']') {
                            let after = &part[bracket_pos + 1..];
                            // In ESLrc, text comes after each timestamp
                            if after.is_empty() && part != parts.last().unwrap() {
                                is_eslrc = false;
                                break;
                            }
                        }
                    }
                    if is_eslrc {
                        return LyricsFormat::EsLrc;
                    }
                }
            }
        }
        return LyricsFormat::Lrc;
    }

    LyricsFormat::Unknown
}

/// Parse lyrics from string content
pub fn parse_lyrics(content: &str) -> Vec<LyricLineOwned> {
    let format = detect_format(content);
    parse_lyrics_with_format(content, format)
}

/// Parse lyrics with specified format
pub fn parse_lyrics_with_format(content: &str, format: LyricsFormat) -> Vec<LyricLineOwned> {
    match format {
        LyricsFormat::Lrc => lrc::parse_lrc(content),
        LyricsFormat::Yrc => yrc::parse_yrc(content),
        LyricsFormat::Qrc => qrc::parse_qrc(content),
        LyricsFormat::EsLrc => eslrc::parse_eslrc(content),
        LyricsFormat::Lys => lys::parse_lys(content),
        LyricsFormat::Ttml => match ttml::parse_ttml(content.as_bytes()) {
            Ok(ttml_lyric) => ttml_lyric.lines,
            Err(_) => Vec::new(),
        },
        LyricsFormat::Unknown => {
            // Try LRC as fallback
            lrc::parse_lrc(content)
        }
    }
}

/// Parse lyrics from file
#[allow(dead_code)]
pub fn parse_lyrics_file(path: &Path) -> Option<Vec<LyricLineOwned>> {
    let content = std::fs::read_to_string(path).ok()?;
    let lines = parse_lyrics(&content);
    if lines.is_empty() { None } else { Some(lines) }
}

/// Convert parsed lyrics to UI format
pub fn to_ui_lyrics(lines: Vec<LyricLineOwned>) -> Vec<crate::ui::pages::LyricLine> {
    lines
        .into_iter()
        .map(|line| {
            let words: Vec<crate::ui::pages::LyricWord> = line
                .words
                .into_iter()
                .map(|w| crate::ui::pages::LyricWord {
                    start_ms: w.start_time,
                    end_ms: w.end_time,
                    word: w.word,
                })
                .collect();

            let text = if words.is_empty() {
                String::new()
            } else {
                words
                    .iter()
                    .map(|w| w.word.as_str())
                    .collect::<Vec<_>>()
                    .join("")
            };

            crate::ui::pages::LyricLine {
                start_ms: line.start_time,
                end_ms: line.end_time,
                text,
                words,
                translated: if line.translated_lyric.is_empty() {
                    None
                } else {
                    Some(line.translated_lyric)
                },
                romanized: if line.roman_lyric.is_empty() {
                    None
                } else {
                    Some(line.roman_lyric)
                },
                is_background: line.is_bg,
                is_duet: line.is_duet,
            }
        })
        .collect()
}

/// Merge translation lyrics into main lyrics
pub fn merge_translation(main: &mut [LyricLineOwned], translation: &[LyricLineOwned]) {
    for main_line in main.iter_mut() {
        // Find matching translation line by start time
        if let Some(trans_line) = translation
            .iter()
            .find(|t| t.start_time == main_line.start_time)
        {
            if !trans_line.words.is_empty() {
                main_line.translated_lyric = trans_line
                    .words
                    .iter()
                    .map(|w| w.word.as_str())
                    .collect::<Vec<_>>()
                    .join("");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_lrc() {
        let content = "[00:01.12]First line\n[00:05.00]Second line";
        assert_eq!(detect_format(content), LyricsFormat::Lrc);
    }

    #[test]
    fn test_detect_yrc() {
        let content = "[0,1000](0,500,0)Hello(500,500,0)World";
        assert_eq!(detect_format(content), LyricsFormat::Yrc);
    }

    #[test]
    fn test_parse_lrc() {
        let content = "[00:01.12]First line\n[00:05.00]Second line";
        let lines = parse_lyrics(content);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].start_time, 1120);
    }
}
