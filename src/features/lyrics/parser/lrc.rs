//! Standard LRC format parser
//!
//! Supports the common [mm:ss.xx]text format with line-level synchronization.

use super::types::{LyricLineOwned, LyricWordOwned, process_lyrics};

/// Parse timestamp from LRC format: [mm:ss.xx] or [mm:ss:xx]
fn parse_time(src: &str) -> Option<(usize, u64)> {
    if !src.starts_with('[') {
        return None;
    }

    let end_bracket = src.find(']')?;
    let time_str = &src[1..end_bracket];

    // Skip metadata tags like [ar:Artist], [ti:Title]
    if time_str.contains(':') {
        if let Some(first_char) = time_str.chars().next() {
            if first_char.is_alphabetic() {
                return None;
            }
        }
    }

    // Parse mm:ss.xx or mm:ss:xx
    let parts: Vec<&str> = time_str.split(|c| c == ':' || c == '.').collect();

    let time_ms = match parts.len() {
        2 => {
            // mm:ss format
            let min: u64 = parts[0].parse().ok()?;
            let sec: u64 = parts[1].parse().ok()?;
            min * 60 * 1000 + sec * 1000
        }
        3 => {
            // mm:ss.xx or mm:ss:xx format
            let min: u64 = parts[0].parse().ok()?;
            let sec: u64 = parts[1].parse().ok()?;
            let ms_str = parts[2];
            let mut ms: u64 = ms_str.parse().ok()?;

            // Handle different precision: xx (centiseconds) vs xxx (milliseconds)
            match ms_str.len() {
                1 => ms *= 100,
                2 => ms *= 10,
                3 => {}
                _ => return None,
            }

            min * 60 * 1000 + sec * 1000 + ms
        }
        _ => return None,
    };

    Some((end_bracket + 1, time_ms))
}

/// Parse a single LRC line, which may have multiple timestamps
fn parse_line(line: &str) -> Vec<LyricLineOwned> {
    let mut results = Vec::new();
    let mut timestamps = Vec::new();
    let mut pos = 0;
    let line = line.trim();

    // Extract all timestamps at the beginning
    while pos < line.len() {
        if let Some((consumed, time)) = parse_time(&line[pos..]) {
            timestamps.push(time);
            pos += consumed;
        } else {
            break;
        }
    }

    if timestamps.is_empty() {
        return results;
    }

    // Get the text after all timestamps
    let text = line[pos..].trim().to_string();

    // Create a LyricLine for each timestamp
    for start_time in timestamps {
        results.push(LyricLineOwned {
            words: vec![LyricWordOwned {
                start_time,
                end_time: 0, // Will be calculated later
                word: text.clone(),
                roman_word: String::new(),
            }],
            start_time,
            end_time: 0,
            ..Default::default()
        });
    }

    results
}

/// Parse LRC content into lyric lines
pub fn parse_lrc(src: &str) -> Vec<LyricLineOwned> {
    let lines = src.lines();
    let mut result = Vec::with_capacity(lines.size_hint().1.unwrap_or(128).min(1024));

    for line in lines {
        let parsed = parse_line(line);
        result.extend(parsed);
    }

    // Sort by start time
    result.sort_unstable_by_key(|x| x.start_time);

    // Calculate end times based on next line's start time
    let mut last_end_time = u64::MAX;
    for line in result.iter_mut().rev() {
        line.end_time = last_end_time;
        if let Some(first_word) = line.words.first_mut() {
            first_word.end_time = last_end_time;
        }
        last_end_time = line.start_time;
    }

    process_lyrics(&mut result);

    result
}

/// Write timestamp in LRC format
pub fn write_timestamp(result: &mut String, time: u64) {
    use std::fmt::Write;
    let ms = time % 1000;
    let sec = (time / 1000) % 60;
    let min = time / 60000;
    write!(result, "[{:02}:{:02}.{:03}]", min, sec, ms).unwrap();
}

/// Convert lyrics to LRC format string
pub fn stringify_lrc(lines: &[LyricLineOwned]) -> String {
    let capacity: usize = lines
        .iter()
        .map(|x| x.words.iter().map(|y| y.word.len()).sum::<usize>() + 13)
        .sum();
    let mut result = String::with_capacity(capacity);

    for line in lines {
        if !line.words.is_empty() {
            write_timestamp(&mut result, line.words[0].start_time);
            for word in line.words.iter() {
                result.push_str(&word.word);
            }
            result.push('\n');
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_time() {
        assert_eq!(parse_time("[00:01.12]"), Some((10, 1120)));
        assert_eq!(parse_time("[00:10.254]"), Some((11, 10254)));
        assert_eq!(parse_time("[01:10.1]"), Some((9, 70100)));
        assert_eq!(parse_time("[00:00.00]"), Some((10, 0)));
    }

    #[test]
    fn test_parse_line() {
        let lines = parse_line("[00:01.12] test LyRiC");
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].start_time, 1120);
        assert_eq!(lines[0].words[0].word, "test LyRiC");
    }

    #[test]
    fn test_parse_multiple_timestamps() {
        let lines = parse_line("[00:12.50][01:30.00]Repeated line");
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].start_time, 12500);
        assert_eq!(lines[1].start_time, 90000);
    }

    #[test]
    fn test_parse_lrc() {
        let content = "[ti:Test Song]\n[ar:Test Artist]\n[00:00.00]First line\n[00:05.00]Second line\n[00:10.00]Third line";
        let lines = parse_lrc(content);
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].words[0].word, "First line");
        assert_eq!(lines[1].words[0].word, "Second line");
        assert_eq!(lines[2].words[0].word, "Third line");
    }

    #[test]
    fn test_stringify_lrc() {
        let lines = parse_lrc("[00:01.12] test LyRiC\n[00:10.254] sssxxx");
        let output = stringify_lrc(&lines);
        assert!(output.contains("[00:01.120]"));
        assert!(output.contains("[00:10.254]"));
    }
}
