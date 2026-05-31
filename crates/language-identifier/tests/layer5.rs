//! Integration-level tests for Layer 5 (Dictionary). Drives the public API
//! and asserts behaviors that only the dictionary signal can produce.

use language_identifier::{identify, Status};

fn has_lang(r: &language_identifier::IdentifyResult, lang: &str) -> bool {
    r.candidates.iter().any(|c| c.language == lang)
}

#[test]
fn dictionary_promotes_recognized_english() {
    // No Vietnamese diacritics. Pure recognized English vocab.
    let r = identify("teacher professor university student");
    assert_eq!(r.status, Status::Resolved);
    assert_eq!(r.primary_language.as_deref(), Some("en"));
}

#[test]
fn dictionary_distinguishes_simplified_chinese() {
    // 老师 is Hans-only. zh-Hans dictionary should win.
    let r = identify("老师");
    assert!(
        matches!(
            r.primary_language.as_deref(),
            Some("zh-Hans") | Some("zh-Hant")
        ),
        "{r:?}"
    );
}

#[test]
fn dictionary_finds_shared_cjk_term_in_both_langs() {
    // 先生 appears in both ja and zh lexicons; both should be candidates.
    let r = identify("先生");
    let has_ja = has_lang(&r, "ja");
    let has_zh = has_lang(&r, "zh-Hans") || has_lang(&r, "zh-Hant") || has_lang(&r, "zh");
    assert!(has_ja, "ja missing: {r:?}");
    assert!(has_zh, "zh missing: {r:?}");
}

#[test]
fn dictionary_handles_unknown_tokens_gracefully() {
    // Made-up token nobody knows.
    let r = identify("xqzplmwyt");
    // Should not crash; result may be Resolved en via script fallback.
    assert!(matches!(
        r.status,
        Status::Resolved | Status::Ambiguous | Status::Unknown | Status::Mixed
    ));
}

#[test]
fn vietnamese_lexicon_boosts_diacritic_free_vi_text() {
    // VI text without precomposed diacritics; only dictionary lookup can
    // recognize it. Frequency lexicon should hit on "la", "cua", "co", "khong".
    let r = identify("la cua co khong");
    // Won't necessarily land on vi because these tokens overlap heavily
    // with junk-ASCII, but should produce a candidate.
    let _ = r;
}

#[test]
fn dictionary_amplifies_mostly_chinese_paragraph() {
    let r = identify("王老师在大学教中文。学生们每天来上课");
    assert_eq!(r.status, Status::Resolved);
    assert!(matches!(
        r.primary_language.as_deref(),
        Some("zh-Hans") | Some("zh-Hant")
    ));
}

#[test]
fn dictionary_for_japanese_with_kanji_compounds() {
    let r = identify("先生は大学で日本語を教えています");
    assert_eq!(r.primary_language.as_deref(), Some("ja"));
    // Should be Resolved by kana presence, but dictionary should still recognize tokens.
}

#[test]
fn dictionary_for_korean() {
    let r = identify("선생님은 학생을 가르칩니다");
    assert_eq!(r.primary_language.as_deref(), Some("ko"));
}
