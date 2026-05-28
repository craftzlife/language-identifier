use language_identifier::{identify, identify_lines, Segment};

fn find<'a>(segs: &'a [Segment], lang: &str) -> Option<&'a Segment> {
    segs.iter().find(|s| s.language == lang)
}

#[test]
fn pure_english_has_no_segments() {
    // present_segments() suppresses single-segment matches with the primary lang.
    let r = identify("The teacher asked a question");
    assert!(r.segments.is_empty(), "{r:?}");
}

#[test]
fn embedded_cjk_in_english_produces_cjk_segment() {
    // 先生 with no markers and no kana surfaces as the umbrella zh tag.
    // (Disambiguating to ja vs zh-Hans/Hant requires Layer 7+ context.)
    let r = identify("Please read 先生 carefully and slowly");
    assert_eq!(r.primary_language.as_deref(), Some("en-US"));
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
    assert_eq!(r.primary_language.as_deref(), Some("en-US"));
    let ja = find(&r.segments, "ja").unwrap_or_else(|| panic!("missing ja: {r:?}"));
    assert!(ja.end > ja.start);
}

#[test]
fn embedded_chinese_in_english_produces_zh_segment() {
    let r = identify("Please explain the word 老师 in Chinese");
    assert_eq!(r.primary_language.as_deref(), Some("en-US"));
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
fn segments_omitted_for_single_language_paragraph() {
    let r = identify_lines(&[
        "The teacher asked a question.",
        "The student answered correctly.",
    ]);
    assert!(r.segments.is_empty(), "{r:?}");
}

#[test]
fn segments_present_for_mixed_input() {
    let r = identify_lines(&[
        "When the user selects 先生 from a Japanese article, the library should analyze the surrounding sentence.",
        "This dictionary app should detect the embedded 日本語を勉強する phrase as Japanese.",
    ]);
    assert!(!r.segments.is_empty());
    assert!(find(&r.segments, "ja").is_some(), "{r:?}");
    assert!(find(&r.segments, "en-US").is_some(), "{r:?}");
}

#[test]
fn segments_serialize_as_array() {
    // Use kana to anchor the embedded CJK as ja unambiguously.
    let r = identify("Please read 先生はとても親切です carefully");
    let json = serde_json::to_string(&r).unwrap();
    assert!(json.contains("\"segments\""));
    assert!(json.contains("\"language\":\"ja\""));
}
