//! Cross-checks that segment-level tokenization preserves the
//! absolute-byte-offset invariant. The whole file is gated on the
//! `tokenize` feature: when it is off, `Segment.tokens` is always
//! empty (verified by the lone test below); when it is on, every
//! per-language backend is exercised.

use language_identifier::{identify, IdentifyResult, Segment};

#[cfg(not(feature = "tokenize"))]
#[test]
fn default_build_emits_no_tokens() {
    let r = identify("The teacher asked a question");
    for seg in &r.segments {
        assert!(
            seg.tokens.is_empty(),
            "expected empty tokens without the `tokenize` feature, got {:?}",
            seg.tokens
        );
    }
}

#[cfg(feature = "tokenize")]
fn assert_tokens_consistent(seg: &Segment, normalized: &str) {
    let mut last_end = seg.start;
    for tok in &seg.tokens {
        assert!(
            tok.start >= seg.start && tok.end <= seg.end,
            "token {tok:?} escapes segment [{}, {}]",
            seg.start,
            seg.end
        );
        assert!(tok.start < tok.end, "empty token range: {tok:?}");
        assert!(
            tok.start >= last_end,
            "tokens not monotonic at {tok:?} (last_end={last_end})"
        );
        // The exact slice of the normalized text must match the
        // token's reported `text` — this is the absolute-offset round
        // trip the whole feature hinges on.
        assert_eq!(
            &normalized[tok.start..tok.end],
            tok.text,
            "token text vs normalized_text mismatch at {tok:?}"
        );
        // No whitespace-only tokens.
        assert!(
            !tok.text.chars().all(char::is_whitespace),
            "whitespace-only token leaked: {tok:?}"
        );
        last_end = tok.end;
    }
}

#[cfg(feature = "tokenize")]
fn find_segment<'a>(r: &'a IdentifyResult, lang: &str) -> &'a Segment {
    r.segments
        .iter()
        .find(|s| s.language == lang)
        .unwrap_or_else(|| panic!("expected a {lang} segment, got {:?}", r.segments))
}

#[cfg(feature = "tokenize")]
#[test]
fn icu_tokenizes_english_segment() {
    let r = identify("The quick brown fox jumps over the lazy dog");
    let seg = find_segment(&r, "en");
    assert!(!seg.tokens.is_empty(), "expected ICU tokens, got none");
    assert_tokens_consistent(seg, &r.normalized_text);
    let texts: Vec<&str> = seg.tokens.iter().map(|t| t.text.as_str()).collect();
    assert!(texts.contains(&"quick"), "missing 'quick' in {texts:?}");
}

#[cfg(feature = "tokenize")]
#[test]
fn jieba_tokenizes_chinese_segment() {
    let r = identify("王老师在大学教中文");
    let seg = r
        .segments
        .iter()
        .find(|s| s.language.starts_with("zh") || s.language == "yue" || s.language == "lzh")
        .unwrap_or_else(|| panic!("expected a zh segment, got {:?}", r.segments));
    assert!(!seg.tokens.is_empty(), "expected jieba tokens, got none");
    assert_tokens_consistent(seg, &r.normalized_text);
    let texts: Vec<&str> = seg.tokens.iter().map(|t| t.text.as_str()).collect();
    // jieba may group "王老师" as a single name token; the structural
    // checks above guarantee the tokens are real partitions. We only
    // need a couple of stable single-word tokens to confirm jieba ran.
    assert!(texts.contains(&"大学"), "missing '大学' in {texts:?}");
    assert!(texts.contains(&"中文"), "missing '中文' in {texts:?}");
}

#[cfg(feature = "tokenize")]
#[test]
fn lindera_tokenizes_japanese_segment() {
    let r = identify("私は学校に行きます");
    let seg = find_segment(&r, "ja");
    assert!(!seg.tokens.is_empty(), "expected lindera tokens, got none");
    assert_tokens_consistent(seg, &r.normalized_text);
    let texts: Vec<String> = seg.tokens.iter().map(|t| t.text.clone()).collect();
    assert!(
        texts.iter().any(|t| t == "学校"),
        "missing '学校' in {texts:?}"
    );
}

#[cfg(feature = "tokenize")]
#[test]
fn icu_tokenizes_korean_segment() {
    let r = identify("안녕하세요 세계");
    let seg = find_segment(&r, "ko");
    assert!(!seg.tokens.is_empty(), "expected ICU tokens for ko, got none");
    assert_tokens_consistent(seg, &r.normalized_text);
}
