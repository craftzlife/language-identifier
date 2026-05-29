use std::collections::BTreeMap;

use super::normalize::Normalized;
use super::orthography;
use super::script::{classify, Script};

/// Cap the number of sentences Layer 7 inspects. Beyond this, the per-sentence
/// cost outweighs the benefit and the global script/orthography signals are
/// already informative enough.
const SENTENCE_CAP: usize = 50;

#[derive(Debug, Clone)]
#[allow(dead_code)] // fields exposed for tests / future use even when unread internally
pub struct SentenceClassification {
    /// Byte offset (into `Normalized.text`) where this sentence starts.
    pub start: usize,
    /// Byte offset (exclusive) where this sentence ends.
    pub end: usize,
    pub language: String,
}

#[derive(Debug, Clone, Default)]
pub struct ContextWindowSignal {
    pub sentences: Vec<SentenceClassification>,
    /// Language → number of sentences won.
    pub per_language: BTreeMap<String, usize>,
    /// True when ≥2 languages each win ≥1 sentence and the runner-up wins at
    /// least 1/5 of the top language's sentences, OR when intra-sentence
    /// embedded Han runs are detected inside ja-carrier sentences (SDD M3).
    pub multi_language: bool,
    /// Number of runs of >=5 consecutive Han characters inside a sentence whose
    /// carrier is not zh. These are embedded Chinese phrases that the
    /// per-char script tally folds into the carrier language.
    pub embedded_han_runs: usize,
}

pub fn score(input: &Normalized) -> ContextWindowSignal {
    let mut sig = ContextWindowSignal::default();
    if input.chars.is_empty() {
        return sig;
    }

    let sentences = split_sentences(input);
    let truncated: Vec<(usize, usize)> = sentences.into_iter().take(SENTENCE_CAP).collect();

    for (start_char, end_char) in truncated {
        if start_char == end_char {
            continue;
        }
        let slice: &[char] = &input.chars[start_char..end_char];
        if let Some(lang) = classify_sentence(slice) {
            let start_byte = input.char_byte_offsets[start_char];
            let end_byte = input.char_byte_offsets[end_char];
            sig.sentences.push(SentenceClassification {
                start: start_byte,
                end: end_byte,
                language: lang.into(),
            });
            *sig.per_language.entry(lang.into()).or_insert(0) += 1;

            // Count embedded Han runs inside non-zh carrier sentences.
            if lang != "zh-Hans" && lang != "zh-Hant" && lang != "zh" {
                sig.embedded_han_runs += count_embedded_han_runs(slice);
            }
        }
    }

    // Collapse same-macro variants (e.g. `zh` / `zh-Hans` / `zh-Hant`) into a
    // single bucket before deciding multi-language. A paragraph that mixes
    // Hans + Hant — or that contains a marker-less Han sentence that falls
    // back to the umbrella `zh` tag — is the same language, not two.
    let mut macro_counts: BTreeMap<&str, usize> = BTreeMap::new();
    for (lang, n) in &sig.per_language {
        *macro_counts.entry(macro_of(lang)).or_insert(0) += n;
    }
    let mut counts: Vec<(&str, usize)> = macro_counts.into_iter().collect();
    counts.sort_by_key(|b| std::cmp::Reverse(b.1));

    if counts.len() >= 2 {
        let top = counts[0].1;
        let second = counts[1].1;
        // Runner-up wins at least 1 sentence and is within factor 5 of the top.
        if second >= 1 && second * 5 >= top {
            sig.multi_language = true;
        }
    }
    // SDD case M3: every sentence is ja-carrier but contains embedded Chinese
    // phrases. Detect via intra-sentence Han runs.
    if sig.embedded_han_runs >= 2 {
        sig.multi_language = true;
    }

    sig
}

/// Strip any BCP 47 subtag suffix so language-variant siblings collapse to
/// their shared macro for the multi-language comparison.
fn macro_of(lang: &str) -> &str {
    lang.split('-').next().unwrap_or(lang)
}

/// Count runs of >=5 consecutive Han chars without intervening kana / hangul /
/// Latin in the given sentence. These are usually embedded Chinese phrases
/// when the sentence carrier is Japanese.
fn count_embedded_han_runs(chars: &[char]) -> usize {
    const MIN_HAN_RUN: usize = 5;
    let mut runs = 0usize;
    let mut current = 0usize;
    for &c in chars {
        if classify(c) == Script::Han {
            current += 1;
        } else {
            if current >= MIN_HAN_RUN {
                runs += 1;
            }
            current = 0;
        }
    }
    if current >= MIN_HAN_RUN {
        runs += 1;
    }
    runs
}

/// Split the normalized chars into sentence char-ranges using terminal
/// punctuation. Returns `(start_char_idx, end_char_idx)` pairs (end exclusive).
fn split_sentences(input: &Normalized) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut start = 0usize;
    for (i, &c) in input.chars.iter().enumerate() {
        if is_sentence_terminator(c) {
            // Sentence ends after the terminator.
            let end = i + 1;
            // Trim leading whitespace from `start` so empty/whitespace-only
            // sentences don't produce phantom entries.
            let mut s = start;
            while s < end && input.chars[s].is_whitespace() {
                s += 1;
            }
            if s < end {
                out.push((s, end));
            }
            start = end;
        }
    }
    if start < input.chars.len() {
        let mut s = start;
        while s < input.chars.len() && input.chars[s].is_whitespace() {
            s += 1;
        }
        if s < input.chars.len() {
            out.push((s, input.chars.len()));
        }
    }
    out
}

fn is_sentence_terminator(c: char) -> bool {
    // Note: Layer 0 collapses newlines to single spaces, so '\n' can never
    // appear in normalized text. Paragraph-array inputs (identify_lines) rely
    // on each line ending with a terminator like '.', '。', '!', '?'.
    matches!(
        c,
        '.' | '!' | '?'
            | '。'  // U+3002 CJK full stop
            | '！'  // U+FF01 fullwidth exclamation
            | '？' // U+FF1F fullwidth question mark
    )
}

/// Classify a single sentence's dominant language using fast script +
/// orthography signals only. Returns `None` if no supported script is present.
fn classify_sentence(chars: &[char]) -> Option<&'static str> {
    let mut latin = 0usize;
    let mut kana = 0usize;
    let mut han = 0usize;
    let mut hangul = 0usize;
    let mut vi_markers = 0usize;
    let mut hans_markers = 0usize;
    let mut hant_markers = 0usize;

    for &c in chars {
        match classify(c) {
            Script::Latin => {
                latin += 1;
                if orthography::is_vi_marker_char(c) {
                    vi_markers += 1;
                }
            }
            Script::Hiragana | Script::Katakana => kana += 1,
            Script::Han => {
                han += 1;
                if is_hans_only(c) {
                    hans_markers += 1;
                }
                if is_hant_only(c) {
                    hant_markers += 1;
                }
            }
            Script::Hangul => hangul += 1,
            Script::Other => {}
        }
    }

    if kana > 0 {
        return Some("ja");
    }
    if hangul > 0 {
        return Some("ko");
    }
    if han > 0 && latin == 0 {
        if hans_markers > 0 && hant_markers == 0 {
            return Some("zh-Hans");
        }
        if hant_markers > 0 && hans_markers == 0 {
            return Some("zh-Hant");
        }
        return Some("zh");
    }
    if latin > 0 {
        // VI density threshold: 5% within Latin chars (lower than the global
        // 10% switch because per-sentence the signal is more concentrated).
        if vi_markers > 0 && (vi_markers as f32 / latin as f32) >= 0.05 {
            return Some("vi");
        }
        // Mixed latin + han within one sentence with no kana → carrier is
        // probably the Latin script's language.
        return Some("en");
    }
    None
}

// Mirror the small Hans-only / Hant-only sets used in layers::orthography.
// Kept local so context_window has no cross-layer field dependency.
fn is_hans_only(c: char) -> bool {
    matches!(
        c,
        '师' | '时'
            | '这'
            | '让'
            | '给'
            | '们'
            | '经'
            | '还'
            | '实'
            | '进'
            | '见'
            | '问'
            | '谁'
            | '长'
            | '说'
            | '应'
            | '听'
            | '个'
            | '观'
            | '议'
            | '态'
            | '资'
            | '产'
            | '风'
            | '调'
            | '语'
            | '请'
            | '认'
            | '识'
            | '记'
    )
}

fn is_hant_only(c: char) -> bool {
    matches!(
        c,
        '學' | '國'
            | '來'
            | '們'
            | '經'
            | '這'
            | '實'
            | '會'
            | '說'
            | '應'
            | '讓'
            | '與'
            | '沒'
            | '聽'
            | '寫'
            | '認'
            | '識'
            | '觀'
            | '議'
            | '態'
            | '資'
            | '產'
            | '風'
            | '調'
            | '請'
            | '記'
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layers::normalize::normalize;

    #[test]
    fn pure_english_paragraph_one_winner() {
        let n = normalize("The teacher asked a question. The student answered.");
        let s = score(&n);
        assert_eq!(s.per_language.get("en").copied(), Some(2));
        assert!(!s.multi_language);
    }

    #[test]
    fn pure_japanese_paragraph_one_winner() {
        let n = normalize("先生は親切です。学生たちは熱心に勉強しています。");
        let s = score(&n);
        assert_eq!(s.per_language.get("ja").copied().unwrap_or(0), 2);
        assert!(!s.multi_language);
    }

    #[test]
    fn cjk_full_stop_splits_sentences() {
        let n = normalize("先生は親切です。学生は元気です。");
        let s = score(&n);
        assert_eq!(s.sentences.len(), 2);
    }

    #[test]
    fn period_splits_latin_sentences() {
        let n = normalize("Hello world. How are you?");
        let s = score(&n);
        assert_eq!(s.sentences.len(), 2);
    }

    #[test]
    fn normalize_collapses_newlines_so_one_logical_paragraph_yields_one_sentence() {
        // Normalize collapses '\n' to ' ', so Layer 7 sees no terminator and
        // returns a single sentence. Callers needing per-line sentences must
        // include explicit terminators (. ! ?) at the end of each line.
        let n = normalize("line one\nline two");
        let s = score(&n);
        assert_eq!(s.sentences.len(), 1);
    }

    #[test]
    fn mixed_en_ja_paragraph_is_multi_language() {
        let n = normalize("The teacher asked a question. 先生は親切です。");
        let s = score(&n);
        assert!(s.multi_language, "{s:?}");
    }

    #[test]
    fn empty_input_no_sentences() {
        let n = normalize("");
        let s = score(&n);
        assert!(s.sentences.is_empty());
    }

    #[test]
    fn single_sentence_no_terminator() {
        let n = normalize("hello world");
        let s = score(&n);
        assert_eq!(s.sentences.len(), 1);
    }

    #[test]
    fn sentence_boundaries_index_into_normalized_text() {
        let n = normalize("先生。学生。");
        let s = score(&n);
        for sent in &s.sentences {
            let slice = &n.text[sent.start..sent.end];
            assert!(!slice.is_empty());
        }
    }
}
