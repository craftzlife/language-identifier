//! Integration-level tests for Layer 4 (Function words / particles / stopwords).
//! Drives the public API; asserts behaviors influenced by the function-word bonus.

use language_identifier::{identify, Status};

#[test]
fn english_function_words_keep_english_resolved() {
    let r = identify("the and or but if then else of for");
    assert_eq!(r.status, Status::Resolved);
    assert_eq!(r.primary_language.as_deref(), Some("en"));
}

#[test]
fn vietnamese_function_words_keep_vi_resolved() {
    let r = identify("là của và có không được để một");
    assert_eq!(r.primary_language.as_deref(), Some("vi"));
}

#[test]
fn japanese_particles_strengthen_ja_signal() {
    // Very short kana-only input dominated by particles.
    let r = identify("のにてはがをでも");
    assert_eq!(r.primary_language.as_deref(), Some("ja"));
}

#[test]
fn chinese_particles_strengthen_zh_signal() {
    // 的 是 在 了 — top-frequency zh particles.
    let r = identify("的是在了");
    assert_eq!(r.primary_language.as_deref(), Some("zh"));
    assert!(matches!(
        r.primary_variant.as_deref(),
        Some("zh-Hans") | Some("zh-Hant")
    ));
}

#[test]
fn korean_particles_strengthen_ko_signal() {
    let r = identify("이는을하에가의");
    assert_eq!(r.primary_language.as_deref(), Some("ko"));
}

#[test]
fn function_word_bonus_is_capped() {
    // Repeating the same stopword many times should not blow the EN confidence past 1.0.
    let huge_input: String = "the ".repeat(500);
    let r = identify(&huge_input);
    for c in &r.candidates {
        assert!((0.0..=1.0).contains(&c.confidence), "{c:?}");
    }
}
