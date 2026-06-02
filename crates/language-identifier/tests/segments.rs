use language_identifier::{identify, identify_lines, Segment};

fn find<'a>(segs: &'a [Segment], lang: &str) -> Option<&'a Segment> {
    segs.iter().find(|s| s.language == lang)
}

#[test]
fn pure_english_has_one_segment_matching_primary() {
    // Single-language input still emits one segment covering the whole text.
    let r = identify("The teacher asked a question");
    assert_eq!(r.segments.len(), 1, "{r:?}");
    assert_eq!(r.segments[0].language, "en");
}

#[test]
fn embedded_cjk_in_english_produces_cjk_segment() {
    // 先生 with no markers and no kana surfaces as the umbrella zh tag.
    // (Disambiguating to ja vs zh-Hans/Hant requires Layer 7+ context.)
    let r = identify("Please read 先生 carefully and slowly");
    assert_eq!(r.primary_language.as_deref(), Some("en"));
    let cjk = r
        .segments
        .iter()
        .find(|s| matches!(s.language.as_str(), "ja" | "zh" | "zh-Hans" | "zh-Hant"))
        .unwrap_or_else(|| panic!("missing CJK segment: {r:?}"));
    assert!(cjk.end > cjk.start);
}

#[test]
fn embedded_japanese_kana_text_produces_ja_segment() {
    // Kana presence in the embedded snippet should make it ja unambiguously.
    let r = identify("Please read 先生はとても親切です carefully");
    assert_eq!(r.primary_language.as_deref(), Some("en"));
    let ja = find(&r.segments, "ja").unwrap_or_else(|| panic!("missing ja: {r:?}"));
    assert!(ja.end > ja.start);
}

#[test]
fn embedded_chinese_in_english_produces_zh_segment() {
    let r = identify("Please explain the word 老师 in Chinese");
    assert_eq!(r.primary_language.as_deref(), Some("en"));
    let zh = find(&r.segments, "zh-Hans");
    assert!(zh.is_some(), "expected zh-Hans segment: {r:?}");
}

#[test]
fn segments_appear_in_left_to_right_order() {
    let r = identify("Hello こんにちは middle 先生 trailing");
    for w in r.segments.windows(2) {
        assert!(w[0].start <= w[1].start, "{r:?}");
        assert!(w[0].end <= w[1].start, "{r:?}");
    }
}

#[test]
fn segments_carry_valid_byte_offsets() {
    let r = identify("Read 田中先生 today");
    for s in &r.segments {
        assert!(s.end > s.start);
    }
}

#[test]
fn single_language_paragraph_has_all_segments_in_one_language() {
    let r = identify_lines(&[
        "The teacher asked a question.",
        "The student answered correctly.",
    ]);
    assert!(!r.segments.is_empty(), "{r:?}");
    assert!(r.segments.iter().all(|s| s.language == "en"), "{r:?}");
}

#[test]
fn segments_present_for_mixed_input() {
    let r = identify_lines(&[
        "When the user selects 先生 from a Japanese article, the library should analyze the surrounding sentence.",
        "This dictionary app should detect the embedded 日本語を勉強する phrase as Japanese.",
    ]);
    assert!(!r.segments.is_empty());
    assert!(find(&r.segments, "ja").is_some(), "{r:?}");
    assert!(find(&r.segments, "en").is_some(), "{r:?}");
}

#[test]
fn segment_text_matches_byte_range() {
    let r = identify("Please read 先生はとても親切です carefully");
    for s in &r.segments {
        // The reported `text` must equal the slice the byte range claims.
        // We can't slice the original input (it's been NFC-normalized), but
        // the segment's own text + offsets should be self-consistent.
        assert_eq!(s.text.len(), s.end - s.start, "{s:?}");
    }
}

#[test]
fn vi_segment_text_is_the_vi_words() {
    let r = identify("I want to translate giáo viên tiếng Nhật into Japanese");
    let vi = r
        .segments
        .iter()
        .find(|s| s.language == "vi")
        .expect("expected vi segment");
    assert_eq!(vi.text, "giáo viên tiếng Nhật");
}

#[test]
fn embedded_chinese_run_in_japanese_overrides_global_han_strategy() {
    // A contiguous Chinese-only Han run (含 Hans-only `师`) embedded
    // inside a kana-dominated Japanese paragraph should be re-attributed
    // to zh-Hans locally, instead of inheriting the global
    // JapaneseClaimsHan strategy and getting absorbed into the
    // surrounding ja segment.
    let r = identify(
        "日本語の文の中に 中文老师在大学教中文 という中国語の例文が含まれている場合、システムは日本語を主言語として扱う必要があります。",
    );
    assert_eq!(r.primary_language.as_deref(), Some("ja"), "{r:?}");
    let zh_hans = r
        .segments
        .iter()
        .find(|s| s.language == "zh-Hans")
        .unwrap_or_else(|| panic!("expected an embedded zh-Hans segment: {r:?}"));
    assert_eq!(zh_hans.text, "中文老师在大学教中文");
    // Surrounding kana stays in ja segments — they are not swallowed by
    // the override.
    assert!(
        r.segments.iter().any(|s| s.language == "ja"),
        "expected ja segments around the embedded zh run: {r:?}"
    );
}

#[test]
fn embedded_ambiguous_han_run_keeps_global_strategy() {
    // Han run with no Chinese-only marker (`先生` is shared with
    // Japanese) inside a kana-rich paragraph stays attributed to ja —
    // the override only fires when the run carries independent
    // Chinese-only evidence.
    let r = identify("日本語の文の中で 先生 という言葉は重要です。");
    assert_eq!(r.primary_language.as_deref(), Some("ja"), "{r:?}");
    assert!(
        r.segments.iter().all(|s| s.language == "ja"),
        "expected only ja segments: {r:?}"
    );
}

#[test]
fn segments_serialize_as_array() {
    // Use kana to anchor the embedded CJK as ja unambiguously.
    let r = identify("Please read 先生はとても親切です carefully");
    let json = serde_json::to_string(&r).unwrap();
    assert!(json.contains("\"segments\""));
    assert!(json.contains("\"language\":\"ja\""));
    assert!(json.contains("\"text\":"));
}
