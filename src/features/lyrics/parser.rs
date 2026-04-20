//! Lyrics parsing module
//!
//! Supports multiple lyrics formats:
//! - LRC: Standard line-level lyrics [mm:ss.xx]text
//! - LQE: Lyricify Quick Export container (LYS + LRC attributes)
//! - YRC: NetEase Cloud Music word-level lyrics
//! - QRC: QQ Music word-level lyrics
//! - ESLrc: Foobar2000 ESLyric word-level format
//! - LYS: Lyricify Syllable format
//! - TTML: Apple Music lyrics format (XML)
//! - ASS: Subtitle export format

mod ass;
mod eslrc;
mod lqe;
mod lrc;
mod lys;
mod online;
mod qrc;
mod ttml;
mod types;
mod yrc;

pub use online::*;
pub use types::*;

fn split_bracket_prefix(line: &str) -> Option<(&str, &str)> {
    if !line.starts_with('[') {
        return None;
    }

    let bracket_end = line.find(']')?;
    Some((&line[1..bracket_end], &line[bracket_end + 1..]))
}

fn is_word_timed_line_header(header: &str) -> bool {
    let (start, duration) = match header.split_once(',') {
        Some(parts) => parts,
        None => return false,
    };

    !start.is_empty()
        && !duration.is_empty()
        && start.chars().all(|ch| ch.is_ascii_digit())
        && duration.chars().all(|ch| ch.is_ascii_digit())
}

/// Detect lyrics format from content
pub fn detect_format(content: &str) -> LyricsFormat {
    let trimmed = content.trim();

    if trimmed.starts_with("[Lyricify Quick Export]")
        || trimmed.contains("[lyrics: format@Lyricify Syllable]")
    {
        return LyricsFormat::Lqe;
    }

    // TTML format: XML with <tt> root element
    if trimmed.starts_with("<?xml") || trimmed.starts_with("<tt") {
        return LyricsFormat::Ttml;
    }

    if trimmed.starts_with('[') {
        if let Some(first_line) = trimmed.lines().find(|line| !line.trim().is_empty()) {
            if let Some((header, after_bracket)) = split_bracket_prefix(first_line.trim()) {
                let after_bracket = after_bracket.trim_start();
                if is_word_timed_line_header(header) {
                    // YRC uses a `[start,duration]` line header followed by word markers
                    // in the form `(start,duration,0)text`.
                    if after_bracket.starts_with('(') && after_bracket.contains(",0)") {
                        return LyricsFormat::Yrc;
                    }

                    // QRC also uses `[start,duration]`, but the word timing marker trails
                    // the text as `word(start,duration)`.
                    if after_bracket.contains('(')
                        && after_bracket.contains(')')
                        && !after_bracket.contains(",0)")
                    {
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
    let mut lines = match format {
        LyricsFormat::Lrc => lrc::parse_lrc(content),
        LyricsFormat::Lqe => lqe::parse_lqe(content),
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
    };

    optimize_lyrics_lines(&mut lines);
    lines
}

/// Parse sidecar LRC attributes like `.tlrc` using raw line timestamps.
pub fn parse_lrc_sidecar(content: &str) -> Vec<LyricLineOwned> {
    lrc::parse_lrc(content)
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
                    word: normalize_lyric_text(&w.word),
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
                    Some(normalize_lyric_text(&line.translated_lyric))
                },
                romanized: if line.roman_lyric.is_empty() {
                    None
                } else {
                    Some(normalize_lyric_text(&line.roman_lyric))
                },
                is_background: line.is_bg,
                is_duet: line.is_duet,
            }
        })
        .collect()
}

/// Merge translation lyrics into main lyrics
pub fn merge_translation(main: &mut [LyricLineOwned], translation: &[LyricLineOwned]) {
    merge_lrc_attr(main, translation, LyricAttr::Translation);
}

/// Merge romanized lyrics into main lyrics.
pub fn merge_romanization(main: &mut [LyricLineOwned], romanization: &[LyricLineOwned]) {
    merge_lrc_attr(main, romanization, LyricAttr::Romanization);
}

#[derive(Clone, Copy)]
enum LyricAttr {
    Translation,
    Romanization,
}

fn line_anchor_time(line: &LyricLineOwned) -> u64 {
    line.words
        .first()
        .map(|word| word.start_time)
        .unwrap_or(line.start_time)
}

fn merge_lrc_attr(main: &mut [LyricLineOwned], attr_lines: &[LyricLineOwned], attr: LyricAttr) {
    let mut attr_index = 0usize;

    for main_line in main.iter_mut() {
        let Some(attr_line) = attr_lines.get(attr_index) else {
            break;
        };

        if line_anchor_time(attr_line) != line_anchor_time(main_line) {
            continue;
        }

        let text = attr_line
            .words
            .iter()
            .map(|w| w.word.as_str())
            .collect::<Vec<_>>()
            .join("");

        if !text.is_empty() {
            match attr {
                LyricAttr::Translation if main_line.translated_lyric.is_empty() => {
                    main_line.translated_lyric = text
                }
                LyricAttr::Romanization if main_line.roman_lyric.is_empty() => {
                    main_line.roman_lyric = text
                }
                _ => {}
            }
        }

        attr_index += 1;
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
    fn test_detect_lqe() {
        let content = "[Lyricify Quick Export]\n[lyrics: format@Lyricify Syllable]\n[0]Hi(0,500)";
        assert_eq!(detect_format(content), LyricsFormat::Lqe);
    }

    #[test]
    fn test_detect_parenthesized_lrc_stays_lrc() {
        let content = "[00:01.000](Se-no! Ah Ah Ah Ah)\n[00:03.000]Next";
        assert_eq!(detect_format(content), LyricsFormat::Lrc);
    }

    #[test]
    fn test_detect_lrc_with_inline_parentheses_stays_lrc() {
        let content = "[00:01.000]Hello (world)\n[00:03.000]Next";
        assert_eq!(detect_format(content), LyricsFormat::Lrc);
    }

    #[test]
    fn test_parse_lrc() {
        let content = "[00:01.12]First line\n[00:05.00]Second line";
        let lines = parse_lyrics(content);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].start_time, 1120);
    }

    #[test]
    fn test_parse_parenthesized_lrc_through_auto_detect_marks_background() {
        let content = "[00:01.000](Hello, world!）";
        let lines = parse_lyrics(content);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].is_bg);
        assert_eq!(lines[0].words[0].word, "Hello, world!");
    }

    #[test]
    fn test_merge_translation_matches_duplicate_timestamps_sequentially() {
        let mut main = vec![
            LyricLineOwned {
                start_time: 1000,
                words: vec![LyricWordOwned {
                    start_time: 1000,
                    end_time: 2000,
                    word: "Main A".into(),
                    roman_word: String::new(),
                }],
                ..Default::default()
            },
            LyricLineOwned {
                start_time: 1000,
                words: vec![LyricWordOwned {
                    start_time: 1000,
                    end_time: 2000,
                    word: "Main B".into(),
                    roman_word: String::new(),
                }],
                ..Default::default()
            },
        ];
        let translation = vec![LyricLineOwned {
            start_time: 1000,
            words: vec![LyricWordOwned {
                start_time: 1000,
                end_time: 2000,
                word: "Trans".into(),
                roman_word: String::new(),
            }],
            ..Default::default()
        }];

        merge_translation(&mut main, &translation);

        assert_eq!(main[0].translated_lyric, "Trans");
        assert!(main[1].translated_lyric.is_empty());
    }

    #[test]
    fn test_merge_translation_preserves_existing_inline_text() {
        let mut main = vec![
            LyricLineOwned {
                start_time: 1000,
                translated_lyric: "Inline".into(),
                words: vec![LyricWordOwned {
                    start_time: 1000,
                    end_time: 2000,
                    word: "Main A".into(),
                    roman_word: String::new(),
                }],
                ..Default::default()
            },
            LyricLineOwned {
                start_time: 2000,
                words: vec![LyricWordOwned {
                    start_time: 2000,
                    end_time: 3000,
                    word: "Main B".into(),
                    roman_word: String::new(),
                }],
                ..Default::default()
            },
        ];
        let translation = vec![
            LyricLineOwned {
                start_time: 1000,
                words: vec![LyricWordOwned {
                    start_time: 1000,
                    end_time: 2000,
                    word: "Sidecar A".into(),
                    roman_word: String::new(),
                }],
                ..Default::default()
            },
            LyricLineOwned {
                start_time: 2000,
                words: vec![LyricWordOwned {
                    start_time: 2000,
                    end_time: 3000,
                    word: "Sidecar B".into(),
                    roman_word: String::new(),
                }],
                ..Default::default()
            },
        ];

        merge_translation(&mut main, &translation);

        assert_eq!(main[0].translated_lyric, "Inline");
        assert_eq!(main[1].translated_lyric, "Sidecar B");
    }
}
