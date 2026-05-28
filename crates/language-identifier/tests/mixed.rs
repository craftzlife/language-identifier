use language_identifier::{identify, Status};

#[test]
fn english_and_japanese_balanced_is_mixed() {
    let r = identify("Hello こんにちは world ありがとう");
    assert_eq!(r.status, Status::Mixed, "{r:?}");
    let langs: Vec<&str> = r.candidates.iter().map(|c| c.language.as_str()).collect();
    assert!(langs.contains(&"en-US") && langs.contains(&"ja"));
}

#[test]
fn english_and_korean_balanced_is_mixed() {
    // Cross-script mixing splits cleanly via script proportions.
    let r = identify("Hello world from the 선생님은 학생을 가르칩니다 today");
    assert_eq!(r.status, Status::Mixed, "{r:?}");
    let langs: Vec<&str> = r.candidates.iter().map(|c| c.language.as_str()).collect();
    assert!(langs.contains(&"en-US") && langs.contains(&"ko"));
}

#[test]
fn mostly_english_with_small_embedded_is_not_mixed() {
    // Embedded language should be resolved, not mixed.
    let r = identify(
        "The teacher asked the student a question and the student answered correctly with 先生",
    );
    assert_ne!(r.status, Status::Mixed, "{r:?}");
}

#[test]
fn mixed_status_serializes_lowercase() {
    let s: Status = Status::Mixed;
    assert_eq!(serde_json::to_string(&s).unwrap(), "\"mixed\"");
}

#[test]
fn mixed_short_input_falls_back_to_ambiguous() {
    // Three-char input cannot demonstrate "first-class multi-language content".
    let r = identify("先生");
    assert_ne!(r.status, Status::Mixed, "{r:?}");
}
