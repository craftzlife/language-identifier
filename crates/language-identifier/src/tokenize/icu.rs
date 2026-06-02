//! Fallback tokenization via ICU4X word segmentation. Covers
//! Latin-script languages (`en`, `fr`, `vi`) and any future macro the
//! pipeline starts emitting that isn't handled by a CJK-specific
//! tokenizer above.

#[cfg(feature = "tokenize")]
pub fn tokenize(text: &str) -> Vec<(String, usize, usize)> {
    use icu_segmenter::options::WordBreakInvariantOptions;
    use icu_segmenter::WordSegmenter;

    // `WordSegmenter::new_auto` returns a `WordSegmenterBorrowed<'static>`
    // that holds compiled-in static data — construction is effectively
    // free, so no need to cache the handle across calls.
    let segmenter = WordSegmenter::new_auto(WordBreakInvariantOptions::default());

    let mut iter = segmenter.segment_str(text);
    let mut prev = match iter.next() {
        Some(p) => p,
        None => return Vec::new(),
    };
    let mut out = Vec::new();
    for next in iter {
        if next <= prev || next > text.len() {
            prev = next;
            continue;
        }
        let span = &text[prev..next];
        if !span.chars().all(char::is_whitespace) {
            out.push((span.to_string(), prev, next));
        }
        prev = next;
    }
    out
}

#[cfg(not(feature = "tokenize"))]
pub fn tokenize(_text: &str) -> Vec<(String, usize, usize)> {
    Vec::new()
}
