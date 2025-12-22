//! Word splitting and chunking
//!
//! This module implements word splitting and chunking algorithm which:
//! 1. Re-splits words that contain spaces into multiple words with interpolated timing
//! 2. Groups consecutive words without spaces into chunks for shared emphasis animation
//!
//! ## Example
//!
//! Input: `["Life", " ", "is", " a", " su", "gar so", "sweet"]`
//! Output: `["Life", " ", "is", " a", [" su", "gar"], "so", "sweet"]`
//!
//! The grouped words share emphasis animation timing.

use super::types::WordData;

/// Check if a string is entirely CJK characters (the CJKEXP regex)
///
/// Regex: /^[\p{Unified_Ideograph}\u0800-\u9FFC]+$/u
/// This matches CJK Unified Ideographs and the range 0x0800-0x9FFC
fn is_cjk_only(text: &str) -> bool {
    if text.is_empty() {
        return false;
    }
    text.chars().all(|c| {
        // Match the CJKEXP: Unified_Ideograph property + \u0800-\u9FFC range
        // CJK Unified Ideographs: U+4E00-U+9FFF
        // The range \u0800-\u9FFC covers:
        // - U+0800-U+0FFF: Various scripts (Samaritan, Mandaic, etc.)
        // - U+3000-U+303F: CJK Symbols and Punctuation
        // - U+3040-U+309F: Hiragana
        // - U+30A0-U+30FF: Katakana
        // - U+3400-U+4DBF: CJK Extension A
        // - U+4E00-U+9FFF: CJK Unified Ideographs
        matches!(c, '\u{4E00}'..='\u{9FFF}') || // CJK Unified Ideographs (Unified_Ideograph)
        matches!(c, '\u{3400}'..='\u{4DBF}') || // CJK Extension A (Unified_Ideograph)
        matches!(c, '\u{0800}'..='\u{9FFC}') // the explicit range
    })
}

/// Result of word chunking - either a single word or a group of words
#[derive(Debug, Clone)]
pub enum WordChunk {
    /// A single word (may be whitespace-only)
    Single(WordData),
    /// A group of consecutive words without spaces between them
    /// These words share emphasis animation
    Group(Vec<WordData>),
}

impl WordChunk {
    /// Check if this chunk should have emphasis effect
    pub fn should_emphasize(&self) -> bool {
        match self {
            WordChunk::Single(word) => word.should_emphasize(),
            WordChunk::Group(words) => {
                // For groups, check if any word qualifies OR if the merged word qualifies
                let any_qualifies = words.iter().any(|w| WordData::should_emphasize(w));
                if any_qualifies {
                    return true;
                }
                // Check merged word
                let merged = self.merged_word();
                merged.should_emphasize()
            }
        }
    }

    /// Get the merged word data for a group (for emphasis calculations)
    pub fn merged_word(&self) -> WordData {
        match self {
            WordChunk::Single(word) => word.clone(),
            WordChunk::Group(words) => {
                if words.is_empty() {
                    return WordData::default();
                }
                let text: String = words.iter().map(|w| w.text.as_str()).collect();
                let start_ms = words.iter().map(|w| w.start_ms).min().unwrap_or(0);
                let end_ms = words.iter().map(|w| w.end_ms).max().unwrap_or(0);
                // Inherit is_last_word from the last word in the group
                let is_last_word = words.last().map(|w| w.is_last_word).unwrap_or(false);
                WordData {
                    text,
                    start_ms,
                    end_ms,
                    roman_word: None,
                    emphasize: false,
                    x_start: words.first().map(|w| w.x_start).unwrap_or(0.0),
                    x_end: words.last().map(|w| w.x_end).unwrap_or(1.0),
                    is_last_word,
                }
            }
        }
    }

    /// Get all words in this chunk (flattened)
    pub fn words(&self) -> Vec<&WordData> {
        match self {
            WordChunk::Single(word) => vec![word],
            WordChunk::Group(words) => words.iter().collect(),
        }
    }

    /// Get total character count
    pub fn char_count(&self) -> usize {
        match self {
            WordChunk::Single(word) => word.text.chars().count(),
            WordChunk::Group(words) => words.iter().map(|w| w.text.chars().count()).sum(),
        }
    }
}

/// Re-split words that contain spaces into multiple words with interpolated timing
///
/// the algorithm:
/// 1. For each word, split by spaces
/// 2. Interpolate timing based on character position
/// 3. Add space words between split parts
fn resplit_words(words: &[WordData]) -> Vec<WordData> {
    let mut result = Vec::new();

    for word in words {
        let real_length = word.text.replace(|c: char| c.is_whitespace(), "").len();
        let parts: Vec<&str> = word.text.split(' ').filter(|s| !s.is_empty()).collect();

        if parts.len() > 1 {
            // Word contains spaces, need to split
            if word.text.starts_with(' ') {
                result.push(WordData {
                    text: " ".to_string(),
                    start_ms: 0,
                    end_ms: 0,
                    roman_word: None,
                    emphasize: false,
                    x_start: 0.0,
                    x_end: 0.0,
                    is_last_word: false,
                });
            }

            let mut char_pos = 0usize;
            let duration = word.end_ms.saturating_sub(word.start_ms);
            let is_last_part_count = parts.len();

            for (i, part) in parts.iter().enumerate() {
                // Calculate interpolated timing
                let part_start = if real_length > 0 {
                    word.start_ms + (char_pos as u64 * duration / real_length as u64)
                } else {
                    word.start_ms
                };
                let part_end = if real_length > 0 {
                    word.start_ms + ((char_pos + part.len()) as u64 * duration / real_length as u64)
                } else {
                    word.end_ms
                };

                // Only the last part of the last word inherits is_last_word
                let is_last = word.is_last_word && i == is_last_part_count - 1;

                result.push(WordData {
                    text: part.to_string(),
                    start_ms: part_start,
                    end_ms: part_end,
                    roman_word: None, // Roman word is not split
                    emphasize: false,
                    x_start: 0.0,
                    x_end: 0.0,
                    is_last_word: is_last,
                });

                // Add space after each part except the last
                if i < parts.len() - 1 {
                    result.push(WordData {
                        text: " ".to_string(),
                        start_ms: 0,
                        end_ms: 0,
                        roman_word: None,
                        emphasize: false,
                        x_start: 0.0,
                        x_end: 0.0,
                        is_last_word: false,
                    });
                }

                char_pos += part.len();
            }

            if word.text.ends_with(' ') && !parts.is_empty() {
                // Don't add trailing space if we already added one
            }
        } else {
            // No spaces in word, keep as-is
            result.push(word.clone());
        }
    }

    result
}

/// Chunk and split lyric words (the chunkAndSplitLyricWords)
///
/// This function:
/// 1. Re-splits words containing spaces
/// 2. Groups consecutive non-space words into chunks
/// 3. CJK words break the grouping (each CJK word is its own chunk)
///
/// ## Returns
///
/// A vector of `WordChunk` where:
/// - `WordChunk::Single` is a single word (including whitespace)
/// - `WordChunk::Group` is a group of consecutive words that share emphasis animation
pub fn chunk_and_split_words(words: &[WordData]) -> Vec<WordChunk> {
    // First, re-split words containing spaces
    let resplited = resplit_words(words);

    let mut result: Vec<WordChunk> = Vec::new();
    let mut word_chunk: Vec<String> = Vec::new();
    let mut w_chunk: Vec<WordData> = Vec::new();

    for word in resplited {
        let text = &word.text;
        let is_cjk = is_cjk_only(text);

        word_chunk.push(text.clone());
        w_chunk.push(word.clone());

        // Check if this is a whitespace-only word
        if !text.is_empty() && text.trim().is_empty() {
            // Pure whitespace - flush current chunk and add whitespace
            word_chunk.pop();
            w_chunk.pop();

            if w_chunk.len() == 1 {
                result.push(WordChunk::Single(w_chunk.remove(0)));
            } else if w_chunk.len() > 1 {
                result.push(WordChunk::Group(std::mem::take(&mut w_chunk)));
            }

            result.push(WordChunk::Single(word));
            word_chunk.clear();
            w_chunk.clear();
        } else if !is_single_word_pattern(&word_chunk.join("")) || is_cjk {
            // Not a single word pattern OR is CJK - break the chunk
            word_chunk.pop();
            w_chunk.pop();

            if w_chunk.len() == 1 {
                result.push(WordChunk::Single(w_chunk.remove(0)));
            } else if w_chunk.len() > 1 {
                result.push(WordChunk::Group(std::mem::take(&mut w_chunk)));
            }

            // For CJK words, output them immediately as single chunks
            // This ensures CJK words don't get grouped with following words
            if is_cjk {
                result.push(WordChunk::Single(word));
                word_chunk.clear();
                w_chunk.clear();
            } else {
                word_chunk = vec![text.clone()];
                w_chunk = vec![word];
            }
        }
    }

    // Flush remaining chunk
    if w_chunk.len() == 1 {
        result.push(WordChunk::Single(w_chunk.remove(0)));
    } else if !w_chunk.is_empty() {
        result.push(WordChunk::Group(w_chunk));
    }

    result
}

/// Check if text matches the single word pattern (optional whitespace + word + optional whitespace)
/// Regex: /^\s*[^\s]*\s*$/
fn is_single_word_pattern(text: &str) -> bool {
    let trimmed = text.trim();
    // A single word has no internal whitespace
    !trimmed.contains(char::is_whitespace)
}

/// Process words and mark emphasis based on chunking
///
/// This updates the `emphasize` field of words based on chunking logic.
/// Words in a group share emphasis if the merged word qualifies.
pub fn process_words_with_chunking(words: &mut [WordData]) {
    let chunks = chunk_and_split_words(words);

    // Build a map of word index to emphasis state
    let mut word_idx = 0;
    let mut emphasis_map: Vec<bool> = vec![false; words.len()];

    for chunk in &chunks {
        let should_emp = chunk.should_emphasize();
        match chunk {
            WordChunk::Single(_) => {
                if word_idx < emphasis_map.len() {
                    emphasis_map[word_idx] = should_emp;
                }
                word_idx += 1;
            }
            WordChunk::Group(group_words) => {
                for _ in group_words {
                    if word_idx < emphasis_map.len() {
                        emphasis_map[word_idx] = should_emp;
                    }
                    word_idx += 1;
                }
            }
        }
    }

    // Apply emphasis to original words
    // Note: The resplit may have changed word count, so we need to be careful
    // For now, we'll just mark emphasis on the original words based on their own criteria
    // plus the group logic
    for (i, word) in words.iter_mut().enumerate() {
        if i < emphasis_map.len() {
            word.emphasize = emphasis_map[i];
        } else {
            word.emphasize = word.should_emphasize();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_word(text: &str, start: u64, end: u64) -> WordData {
        WordData {
            text: text.to_string(),
            start_ms: start,
            end_ms: end,
            roman_word: None,
            emphasize: false,
            x_start: 0.0,
            x_end: 0.0,
            is_last_word: false,
        }
    }

    #[test]
    fn test_resplit_words() {
        let words = vec![
            make_word("Hello", 0, 500),
            make_word(" world test", 500, 1500),
        ];

        let result = resplit_words(&words);

        // "Hello" stays as-is
        // " world test" becomes " ", "world", " ", "test"
        assert!(result.len() >= 4);
        assert_eq!(result[0].text, "Hello");
    }

    #[test]
    fn test_chunk_simple() {
        let words = vec![
            make_word("Life", 0, 500),
            make_word(" ", 500, 500),
            make_word("is", 500, 1000),
        ];

        let chunks = chunk_and_split_words(&words);

        // Should be: Single("Life"), Single(" "), Single("is")
        assert_eq!(chunks.len(), 3);
    }

    #[test]
    fn test_chunk_group() {
        // Words without spaces between them should be grouped
        let words = vec![make_word("su", 0, 500), make_word("gar", 500, 1000)];

        let chunks = chunk_and_split_words(&words);

        // Should be grouped: Group(["su", "gar"])
        assert_eq!(chunks.len(), 1);
        match &chunks[0] {
            WordChunk::Group(g) => assert_eq!(g.len(), 2),
            _ => panic!("Expected group"),
        }
    }

    #[test]
    fn test_cjk_breaks_group() {
        let words = vec![
            make_word("test", 0, 500),
            make_word("你好", 500, 1000),
            make_word("world", 1000, 1500),
        ];

        let chunks = chunk_and_split_words(&words);

        // CJK should break grouping
        // Result: Single("test"), Single("你好"), Single("world")
        assert_eq!(chunks.len(), 3);
    }
}
