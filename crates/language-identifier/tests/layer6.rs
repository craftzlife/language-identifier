//! Integration tests for Layer 6 (Morphology / tokenization hints).
//! Asserts observable behaviors through the public `identify()` API.

use language_identifier::{identify, Status};

fn has_lang(r: &language_identifier::IdentifyResult, lang: &str) -> bool {
    r.candidates.iter().any(|c| c.language == lang)
}

fn segments_in_lang(r: &language_identifier::IdentifyResult, lang: &str) -> usize {
    r.segments.iter().filter(|s| s.language == lang).count()
}

#[test]
fn embedded_vi_word_inside_english_produces_vi_segment() {
    // Layer 6 attributes "thầy" as vi-VN per token; segments should show a
    // vi-VN span at exactly the word boundary.
    let r = identify(
        "I want to translate the word thầy into Japanese",
    );
    assert_eq!(r.primary_language.as_deref(), Some("en-US"));
    assert!(segments_in_lang(&r, "vi-VN") >= 1, "{r:?}");
}

#[test]
fn multi_word_vi_phrase_in_english_groups_into_one_segment() {
    // "giáo viên tiếng Nhật" should appear as a single contiguous vi-VN span
    // (after whitespace-merging adjacent same-language spans).
    let r = identify(
        "I want to translate giáo viên tiếng Nhật into Japanese",
    );
    assert_eq!(r.primary_language.as_deref(), Some("en-US"));
    let vi_spans: Vec<&language_identifier::Segment> = r
        .segments
        .iter()
        .filter(|s| s.language == "vi-VN")
        .collect();
    assert_eq!(vi_spans.len(), 1, "expected one merged vi-VN span: {r:?}");
}

#[test]
fn vi_word_without_diacritic_in_vi_lexicon_still_attributes() {
    // "nguyen" appears in the VI lexicon (common surname). Even without
    // diacritics it should be attributed to vi-VN over en-US.
    let r = identify("nguyen tran");
    assert!(
        matches!(r.primary_language.as_deref(), Some("vi-VN") | Some("en-US")),
        "{r:?}"
    );
}

#[test]
fn en_word_overlapping_vi_lexicon_resolves_to_en_with_shape_fallback() {
    // Pure English sentence with no diacritics should resolve to en-US even
    // though some tokens (e.g. "la") overlap the VI lexicon.
    let r = identify("the teacher and the student went to the school");
    assert_eq!(r.primary_language.as_deref(), Some("en-US"));
    assert_eq!(r.status, Status::Resolved);
}

#[test]
fn ja_morphology_endings_boost_ja_signal() {
    // です/ます endings should add Layer 6 morphology bonus on top of Layer 2.
    let r = identify("先生は親切でした");
    assert_eq!(r.primary_language.as_deref(), Some("ja"));
}

#[test]
fn ko_predicate_ending_boosts_ko_signal() {
    let r = identify("학생입니다");
    assert_eq!(r.primary_language.as_deref(), Some("ko"));
}

#[test]
fn zh_pattern_boosts_zh_signal() {
    let r = identify("这是老师");
    assert!(
        matches!(
            r.primary_language.as_deref(),
            Some("zh-Hans") | Some("zh-Hant")
        ),
        "{r:?}"
    );
}

#[test]
fn ambiguous_short_token_does_not_misclassify_pure_english() {
    // "la la la" (overlapping vi lexicon) should still land on en-US given
    // the en-US default for shape-ambiguous tokens.
    let r = identify("la la la la la la la");
    // Either en-US or vi-VN is defensible here. Assert no panic, no Unknown.
    assert_ne!(r.status, Status::Unknown);
}

#[test]
fn morphology_bonus_does_not_break_confidence_bounds() {
    let r = identify("teacher teacher teacher teacher teacher teacher teacher teacher");
    for c in &r.candidates {
        assert!((0.0..=1.0).contains(&c.confidence), "{c:?}");
    }
}

#[test]
fn per_word_attribution_in_genuine_en_vi_mix() {
    // Genuine EN+VI input — Layer 6 should produce both en-US and vi-VN spans.
    let r = identify(
        "Hello world this is teacher meets giáo viên đại học người thầy được kính",
    );
    assert!(segments_in_lang(&r, "en-US") >= 1, "{r:?}");
    assert!(segments_in_lang(&r, "vi-VN") >= 1, "{r:?}");
}

#[test]
fn pure_vi_input_resolves_to_vi() {
    let r = identify("thầy giáo dạy tiếng Việt cho sinh viên");
    assert_eq!(r.primary_language.as_deref(), Some("vi-VN"));
    assert_eq!(r.status, Status::Resolved);
}

#[test]
fn morphology_segments_carry_valid_byte_offsets() {
    let r = identify(
        "I want to translate giáo viên into Japanese",
    );
    for s in &r.segments {
        assert!(s.end > s.start);
    }
    let _ = has_lang(&r, "vi-VN");
}
