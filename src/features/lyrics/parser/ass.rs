//! ASS 字幕导出
//!
//! 导出精度 10ms 以下会丢失
//!
//! 主唱名称为 `v1`，对唱为 `v2`
//! Background lyrics get `-bg` suffix
//! Translation gets `-trans` suffix
//! Romanization gets `-roman` suffix

use std::fmt::Write;

use super::types::LyricLineOwned;

fn write_timestamp(result: &mut String, time: u64) {
    let ms = time % 1000;
    let sec = (time / 1000) % 60;
    let min = (time / 60000) % 60;
    let hour = time / 3600000;

    write!(result, "{}:{:02}:{:02}.{:02}", hour, min, sec, ms / 10).unwrap()
}

/// Convert lyrics to ASS subtitle format
pub fn stringify_ass(lines: &[LyricLineOwned]) -> String {
    let mut result = String::with_capacity(
        lines
            .iter()
            .map(|x| x.words.iter().map(|w| w.word.len() + 20).sum::<usize>())
            .sum(),
    );

    result.push_str("[Script Info]\n");
    result.push_str("[Events]\n");
    result.push_str(
        "Format: Marked, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n",
    );

    for line in lines {
        // Filter words with valid timing
        let timed_words: Vec<_> = line
            .words
            .iter()
            .filter(|word| word.end_time > word.start_time)
            .collect();

        let start_time = timed_words.iter().map(|x| x.start_time).min();
        let end_time = timed_words.iter().map(|x| x.end_time).max();

        if start_time.is_none() || end_time.is_none() {
            continue;
        }

        let start_time = start_time.unwrap();
        let end_time = end_time.unwrap();

        result.push_str("Dialogue: 0,");
        write_timestamp(&mut result, start_time);
        result.push(',');
        write_timestamp(&mut result, end_time);
        result.push_str(",Default,");

        if line.is_duet {
            result.push_str("v2");
        } else {
            result.push_str("v1");
        }
        if line.is_bg {
            result.push_str("-bg");
        }
        result.push_str(",0,0,0,,");

        let mut previous_word_end_time = start_time;

        for word in &line.words {
            if word.start_time >= word.end_time {
                result.push_str(&word.word);
                continue;
            }

            if word.start_time > previous_word_end_time {
                let gap_duration_cs =
                    (word.start_time.saturating_sub(previous_word_end_time) + 5) / 10;
                if gap_duration_cs > 0 {
                    write!(&mut result, "{{\\k{}}}", gap_duration_cs).unwrap();
                }
            }

            let word_duration_cs = (word.end_time.saturating_sub(word.start_time) + 5) / 10;
            if word_duration_cs > 0 {
                write!(&mut result, "{{\\k{}}}", word_duration_cs).unwrap();
            }

            result.push_str(&word.word);
            previous_word_end_time = word.end_time;
        }
        result.push('\n');

        // Translation line
        if !line.translated_lyric.is_empty() {
            result.push_str("Dialogue: 0,");
            write_timestamp(&mut result, start_time);
            result.push(',');
            write_timestamp(&mut result, end_time);
            result.push_str(",Default,");
            if line.is_duet {
                result.push_str("v2");
            } else {
                result.push_str("v1");
            }
            if line.is_bg {
                result.push_str("-bg");
            }
            result.push_str("-trans,0,0,0,,");
            result.push_str(&line.translated_lyric);
            result.push('\n');
        }

        // Romanization line
        if !line.roman_lyric.is_empty() {
            result.push_str("Dialogue: 0,");
            write_timestamp(&mut result, start_time);
            result.push(',');
            write_timestamp(&mut result, end_time);
            result.push_str(",Default,");
            if line.is_duet {
                result.push_str("v2");
            } else {
                result.push_str("v1");
            }
            if line.is_bg {
                result.push_str("-bg");
            }
            result.push_str("-roman,0,0,0,,");
            result.push_str(&line.roman_lyric);
            result.push('\n');
        }
    }

    result
}
