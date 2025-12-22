//! TTML (Apple Music) lyrics format parser

use std::io::BufRead;

use super::types::{LyricLineOwned, LyricWordOwned};

/// TTML lyrics with metadata
#[derive(Debug, Default, Clone)]
pub struct TTMLLyric {
    pub lines: Vec<LyricLineOwned>,
    pub metadata: Vec<(String, Vec<String>)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParseStatus {
    None,
    InTtml,
    InHead,
    InMetadata,
    InBody,
    InDiv,
    InP,
    InSpan,
    InTranslationSpan,
    InRomanSpan,
    InBackgroundSpan,
    InSpanInBackgroundSpan,
    InTranslationSpanInBackgroundSpan,
    InRomanSpanInBackgroundSpan,
}

/// Parse TTML format lyrics
pub fn parse_ttml(data: impl BufRead) -> Result<TTMLLyric, String> {
    use quick_xml::{Reader, events::Event};

    let mut reader = Reader::from_reader(data);
    let mut buf = Vec::with_capacity(256);
    let mut str_buf = String::with_capacity(256);
    let mut status = ParseStatus::None;
    let mut result = TTMLLyric::default();
    let mut main_agent: Vec<u8> = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let name = e.name();
                match name.as_ref() {
                    b"tt" => {
                        if status == ParseStatus::None {
                            status = ParseStatus::InTtml;
                        }
                    }
                    b"head" => {
                        if status == ParseStatus::InTtml {
                            status = ParseStatus::InHead;
                        }
                    }
                    b"metadata" => {
                        if status == ParseStatus::InHead {
                            status = ParseStatus::InMetadata;
                        }
                    }
                    b"ttm:agent" => {
                        if main_agent.is_empty() && status == ParseStatus::InMetadata {
                            let mut agent_type = Vec::new();
                            let mut agent_id = Vec::new();
                            for attr in e.attributes().flatten() {
                                match attr.key.as_ref() {
                                    b"type" => agent_type = attr.value.to_vec(),
                                    b"xml:id" => agent_id = attr.value.to_vec(),
                                    _ => {}
                                }
                            }
                            if agent_type == b"person" {
                                main_agent = agent_id;
                            }
                        }
                    }
                    b"amll:meta" => {
                        if status == ParseStatus::InMetadata {
                            let mut meta_key = String::new();
                            let mut meta_value = String::new();
                            for attr in e.attributes().flatten() {
                                match attr.key.as_ref() {
                                    b"key" => {
                                        meta_key = String::from_utf8_lossy(&attr.value).to_string();
                                    }
                                    b"value" => {
                                        meta_value =
                                            String::from_utf8_lossy(&attr.value).to_string();
                                    }
                                    _ => {}
                                }
                            }
                            if !meta_key.is_empty() {
                                if let Some(values) =
                                    result.metadata.iter_mut().find(|x| x.0 == meta_key)
                                {
                                    values.1.push(meta_value);
                                } else {
                                    result.metadata.push((meta_key, vec![meta_value]));
                                }
                            }
                        }
                    }
                    b"body" => {
                        if status == ParseStatus::InTtml {
                            status = ParseStatus::InBody;
                        }
                    }
                    b"div" => {
                        if status == ParseStatus::InBody {
                            status = ParseStatus::InDiv;
                        }
                    }
                    b"p" => {
                        if status == ParseStatus::InDiv {
                            status = ParseStatus::InP;
                            let mut new_line = LyricLineOwned::default();
                            configure_line(&e, &main_agent, &mut new_line);
                            result.lines.push(new_line);
                        }
                    }
                    b"span" => match status {
                        ParseStatus::InP => {
                            status = ParseStatus::InSpan;
                            for attr in e.attributes().flatten() {
                                if attr.key.as_ref() == b"ttm:role" {
                                    match attr.value.as_ref() {
                                        b"x-bg" => {
                                            status = ParseStatus::InBackgroundSpan;
                                            let mut new_bg_line = LyricLineOwned {
                                                is_bg: true,
                                                is_duet: result
                                                    .lines
                                                    .last()
                                                    .map(|l| l.is_duet)
                                                    .unwrap_or(false),
                                                ..Default::default()
                                            };
                                            configure_line(&e, &main_agent, &mut new_bg_line);
                                            result.lines.push(new_bg_line);
                                            break;
                                        }
                                        b"x-translation" => {
                                            status = ParseStatus::InTranslationSpan;
                                            break;
                                        }
                                        b"x-roman" => {
                                            status = ParseStatus::InRomanSpan;
                                            break;
                                        }
                                        _ => {}
                                    }
                                }
                            }
                            if status == ParseStatus::InSpan {
                                let mut new_word = LyricWordOwned::default();
                                configure_word(&e, &mut new_word);
                                if let Some(line) = result.lines.last_mut() {
                                    line.words.push(new_word);
                                }
                            }
                        }
                        ParseStatus::InBackgroundSpan => {
                            status = ParseStatus::InSpanInBackgroundSpan;
                            for attr in e.attributes().flatten() {
                                if attr.key.as_ref() == b"ttm:role" {
                                    match attr.value.as_ref() {
                                        b"x-translation" => {
                                            status = ParseStatus::InTranslationSpanInBackgroundSpan;
                                            break;
                                        }
                                        b"x-roman" => {
                                            status = ParseStatus::InRomanSpanInBackgroundSpan;
                                            break;
                                        }
                                        _ => {}
                                    }
                                }
                            }
                            if status == ParseStatus::InSpanInBackgroundSpan {
                                let mut new_word = LyricWordOwned::default();
                                configure_word(&e, &mut new_word);
                                if let Some(line) = result.lines.iter_mut().rev().find(|l| l.is_bg)
                                {
                                    line.words.push(new_word);
                                }
                            }
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
            Ok(Event::End(e)) => match e.name().as_ref() {
                b"tt" => status = ParseStatus::None,
                b"head" => {
                    if status == ParseStatus::InHead {
                        status = ParseStatus::InTtml;
                    }
                }
                b"metadata" => {
                    if status == ParseStatus::InMetadata {
                        status = ParseStatus::InHead;
                    }
                }
                b"body" => {
                    if status == ParseStatus::InBody {
                        status = ParseStatus::InTtml;
                    }
                }
                b"div" => {
                    if status == ParseStatus::InDiv {
                        status = ParseStatus::InBody;
                    }
                }
                b"p" => {
                    if status == ParseStatus::InP {
                        status = ParseStatus::InDiv;
                    }
                }
                b"span" => match status {
                    ParseStatus::InSpan => {
                        status = ParseStatus::InP;
                        if let Some(line) = result.lines.last_mut() {
                            if let Some(word) = line.words.last_mut() {
                                word.word = str_buf.clone();
                            }
                        }
                        str_buf.clear();
                    }
                    ParseStatus::InBackgroundSpan => {
                        status = ParseStatus::InP;
                        str_buf.clear();
                    }
                    ParseStatus::InSpanInBackgroundSpan => {
                        status = ParseStatus::InBackgroundSpan;
                        if let Some(line) = result.lines.iter_mut().rev().find(|l| l.is_bg) {
                            if let Some(word) = line.words.last_mut() {
                                word.word = str_buf.clone();
                            }
                        }
                        str_buf.clear();
                    }
                    ParseStatus::InTranslationSpan => {
                        status = ParseStatus::InP;
                        if let Some(line) = result.lines.iter_mut().rev().find(|l| !l.is_bg) {
                            if line.translated_lyric.is_empty() {
                                line.translated_lyric = str_buf.clone();
                            }
                        }
                        str_buf.clear();
                    }
                    ParseStatus::InRomanSpan => {
                        status = ParseStatus::InP;
                        if let Some(line) = result.lines.iter_mut().rev().find(|l| !l.is_bg) {
                            line.roman_lyric = str_buf.clone();
                        }
                        str_buf.clear();
                    }
                    ParseStatus::InTranslationSpanInBackgroundSpan => {
                        status = ParseStatus::InBackgroundSpan;
                        if let Some(line) = result.lines.iter_mut().rev().find(|l| l.is_bg) {
                            line.translated_lyric = str_buf.clone();
                        }
                        str_buf.clear();
                    }
                    ParseStatus::InRomanSpanInBackgroundSpan => {
                        status = ParseStatus::InBackgroundSpan;
                        if let Some(line) = result.lines.iter_mut().rev().find(|l| l.is_bg) {
                            line.roman_lyric = str_buf.clone();
                        }
                        str_buf.clear();
                    }
                    _ => {}
                },
                _ => {}
            },
            Ok(Event::Text(e)) => {
                if let Ok(txt) = e.unescape() {
                    match status {
                        ParseStatus::InP => {
                            if let Some(line) = result.lines.iter_mut().rev().find(|l| !l.is_bg) {
                                line.words.push(LyricWordOwned {
                                    word: txt.to_string(),
                                    ..Default::default()
                                });
                            }
                        }
                        ParseStatus::InBackgroundSpan => {
                            if let Some(line) = result.lines.iter_mut().rev().find(|l| l.is_bg) {
                                line.words.push(LyricWordOwned {
                                    word: txt.to_string(),
                                    ..Default::default()
                                });
                            }
                        }
                        ParseStatus::InSpan
                        | ParseStatus::InTranslationSpan
                        | ParseStatus::InRomanSpan
                        | ParseStatus::InSpanInBackgroundSpan
                        | ParseStatus::InTranslationSpanInBackgroundSpan
                        | ParseStatus::InRomanSpanInBackgroundSpan => {
                            str_buf.push_str(&txt);
                        }
                        _ => {}
                    }
                }
            }
            Err(e) => return Err(format!("XML parse error: {}", e)),
            _ => {}
        }
        buf.clear();
    }

    // Post-process: strip parentheses from background vocals
    for line in result.lines.iter_mut() {
        if line.is_bg {
            if let Some(first) = line.words.first_mut() {
                if let Some(stripped) = first.word.strip_prefix('(') {
                    first.word = stripped.to_string();
                }
            }
            if let Some(last) = line.words.last_mut() {
                if let Some(stripped) = last.word.strip_suffix(')') {
                    last.word = stripped.to_string();
                }
            }
        }
        // Update line timing from words
        if let Some(first) = line.words.first() {
            if line.start_time == 0 {
                line.start_time = first.start_time;
            }
        }
        if let Some(last) = line.words.last() {
            if line.end_time == 0 {
                line.end_time = last.end_time;
            }
        }
    }

    Ok(result)
}

fn configure_line(
    e: &quick_xml::events::BytesStart<'_>,
    main_agent: &[u8],
    line: &mut LyricLineOwned,
) {
    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"ttm:agent" => {
                line.is_duet = attr.value.as_ref() != main_agent;
            }
            b"begin" => {
                if let Some(time) = parse_timestamp(&attr.value) {
                    line.start_time = time;
                }
            }
            b"end" => {
                if let Some(time) = parse_timestamp(&attr.value) {
                    line.end_time = time;
                }
            }
            _ => {}
        }
    }
}

fn configure_word(e: &quick_xml::events::BytesStart<'_>, word: &mut LyricWordOwned) {
    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"begin" => {
                if let Some(time) = parse_timestamp(&attr.value) {
                    word.start_time = time;
                }
            }
            b"end" => {
                if let Some(time) = parse_timestamp(&attr.value) {
                    word.end_time = time;
                }
            }
            _ => {}
        }
    }
}

/// Parse TTML timestamp format (HH:MM:SS.MS or MM:SS.MS or SS.MS)
fn parse_timestamp(data: &[u8]) -> Option<u64> {
    let s = std::str::from_utf8(data).ok()?;
    let s = s.trim().trim_end_matches('s');

    let parts: Vec<&str> = s.split(':').collect();

    match parts.len() {
        1 => {
            // SS.MS format
            parse_seconds_ms(parts[0])
        }
        2 => {
            // MM:SS.MS format
            let min: u64 = parts[0].parse().ok()?;
            let sec_ms = parse_seconds_ms(parts[1])?;
            Some(min * 60 * 1000 + sec_ms)
        }
        3 => {
            // HH:MM:SS.MS format
            let hour: u64 = parts[0].parse().ok()?;
            let min: u64 = parts[1].parse().ok()?;
            let sec_ms = parse_seconds_ms(parts[2])?;
            Some(hour * 60 * 60 * 1000 + min * 60 * 1000 + sec_ms)
        }
        _ => None,
    }
}

fn parse_seconds_ms(s: &str) -> Option<u64> {
    if let Some(dot_pos) = s.find('.') {
        let sec: u64 = s[..dot_pos].parse().ok()?;
        let frac_str = &s[dot_pos + 1..];
        let ms = match frac_str.len() {
            0 => 0,
            1 => frac_str.parse::<u64>().ok()? * 100,
            2 => frac_str.parse::<u64>().ok()? * 10,
            3 => frac_str.parse::<u64>().ok()?,
            _ => frac_str[..3].parse::<u64>().ok()?,
        };
        Some(sec * 1000 + ms)
    } else {
        let sec: u64 = s.parse().ok()?;
        Some(sec * 1000)
    }
}

/// Stringify lyrics to TTML format
pub fn stringify_ttml(lines: &[LyricLineOwned]) -> String {
    let mut result = String::with_capacity(64 * 1024);

    result.push_str(r#"<tt xmlns="http://www.w3.org/ns/ttml" xmlns:ttm="http://www.w3.org/ns/ttml#metadata" xmlns:amll="http://www.example.com/ns/amll">"#);
    result.push_str("<head><metadata>");

    let has_duet = lines.iter().any(|l| l.is_duet);
    result.push_str(r#"<ttm:agent type="person" xml:id="v1"/>"#);
    if has_duet {
        result.push_str(r#"<ttm:agent type="other" xml:id="v2"/>"#);
    }

    result.push_str("</metadata></head>");

    let duration = lines
        .last()
        .map(|l| {
            l.end_time
                .max(l.words.last().map(|w| w.end_time).unwrap_or(0))
        })
        .unwrap_or(0);
    result.push_str(&format!(
        r#"<body dur="{}"><div>"#,
        ms_to_timestamp(duration)
    ));

    let mut line_i = 0;
    let mut iter = lines.iter().peekable();

    while let Some(line) = iter.next() {
        if line
            .words
            .iter()
            .map(|w| w.word.trim().len())
            .sum::<usize>()
            == 0
        {
            continue;
        }

        line_i += 1;
        let begin = ms_to_timestamp(line.start_time);
        let end = ms_to_timestamp(line.end_time);
        let agent = if line.is_duet { "v2" } else { "v1" };

        result.push_str(&format!(
            r#"<p begin="{}" end="{}" ttm:agent="{}" itunes:key="L{}">"#,
            begin, end, agent, line_i
        ));

        for word in &line.words {
            if word.word.trim().is_empty() {
                result.push_str(&word.word);
            } else {
                let wb = ms_to_timestamp(word.start_time);
                let we = ms_to_timestamp(word.end_time);
                result.push_str(&format!(
                    r#"<span begin="{}" end="{}">{}</span>"#,
                    wb,
                    we,
                    escape_xml(&word.word)
                ));
            }
        }

        // Handle background line
        if let Some(next) = iter.peek() {
            if next.is_bg {
                let bg = iter.next().unwrap();
                let bb = ms_to_timestamp(bg.start_time);
                let be = ms_to_timestamp(bg.end_time);
                result.push_str(&format!(
                    r#"<span ttm:role="x-bg" begin="{}" end="{}">"#,
                    bb, be
                ));

                for word in &bg.words {
                    if word.word.trim().is_empty() {
                        result.push_str(&word.word);
                    } else {
                        let wb = ms_to_timestamp(word.start_time);
                        let we = ms_to_timestamp(word.end_time);
                        result.push_str(&format!(
                            r#"<span begin="{}" end="{}">{}</span>"#,
                            wb,
                            we,
                            escape_xml(&word.word)
                        ));
                    }
                }

                if !bg.translated_lyric.is_empty() {
                    result.push_str(&format!(
                        r#"<span ttm:role="x-translation" xml:lang="zh-CN">{}</span>"#,
                        escape_xml(&bg.translated_lyric)
                    ));
                }
                if !bg.roman_lyric.is_empty() {
                    result.push_str(&format!(
                        r#"<span ttm:role="x-roman">{}</span>"#,
                        escape_xml(&bg.roman_lyric)
                    ));
                }

                result.push_str("</span>");
            }
        }

        if !line.translated_lyric.is_empty() {
            result.push_str(&format!(
                r#"<span ttm:role="x-translation" xml:lang="zh-CN">{}</span>"#,
                escape_xml(&line.translated_lyric)
            ));
        }
        if !line.roman_lyric.is_empty() {
            result.push_str(&format!(
                r#"<span ttm:role="x-roman">{}</span>"#,
                escape_xml(&line.roman_lyric)
            ));
        }

        result.push_str("</p>");
    }

    result.push_str("</div></body></tt>");
    result
}

fn ms_to_timestamp(time_ms: u64) -> String {
    if time_ms == 0 {
        return "00:00.000".to_string();
    }
    let ms = time_ms % 1000;
    let sec = (time_ms / 1000) % 60;
    let min = (time_ms / 60000) % 60;
    let hour = time_ms / 3600000;

    if hour > 0 {
        format!("{}:{:02}:{:02}.{:03}", hour, min, sec, ms)
    } else {
        format!("{:02}:{:02}.{:03}", min, sec, ms)
    }
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
