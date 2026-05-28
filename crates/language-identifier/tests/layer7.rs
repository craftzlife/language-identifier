//! Integration tests for Layer 7 (Context window scoring).
//! Asserts observable behaviors through the public `identify_lines()` API:
//! the multi-language flag downgrades Resolved → Ambiguous, embedded
//! sentences are visible as candidates.

use language_identifier::{identify, identify_lines, Status};

fn has_lang(r: &language_identifier::IdentifyResult, lang: &str) -> bool {
    r.candidates.iter().any(|c| c.language == lang)
}

#[test]
fn pure_english_paragraph_stays_resolved() {
    let r = identify_lines(&[
        "The teacher asked a question.",
        "The student answered correctly.",
    ]);
    assert_eq!(r.status, Status::Resolved);
    assert_eq!(r.primary_language.as_deref(), Some("en-US"));
}

#[test]
fn pure_japanese_paragraph_stays_resolved() {
    let r = identify_lines(&[
        "先生は親切です。",
        "学生たちは熱心に勉強しています。",
    ]);
    assert_eq!(r.status, Status::Resolved);
    assert_eq!(r.primary_language.as_deref(), Some("ja"));
}

#[test]
fn single_embedded_sentence_does_not_trigger_multi_language_downgrade() {
    // One short embedded JA phrase in a long EN paragraph — shouldn't be
    // enough to make the whole paragraph ambiguous.
    let r = identify(
        "This is the first English sentence. The second English sentence is even longer than the first. The third sentence has 先生 in it but it is still mostly English.",
    );
    // Either Resolved or Mixed is acceptable here; Ambiguous would be a
    // regression of the v2 behavior on cleanly-English text.
    assert_ne!(r.status, Status::Ambiguous, "{r:?}");
}

#[test]
fn m3_paragraph_downgrades_to_ambiguous() {
    // SDD case M3: kana dominates per-char counts, but the paragraph also
    // contains complete Chinese sentences. Layer 7 should detect the multi-
    // language structure and downgrade Resolved → Ambiguous.
    let r = identify_lines(&[
        "日本語の文の中に 中文老师在大学教中文 という中国語の例文が含まれている場合、システムは日本語を主言語として扱い、中国語部分を別のセグメントとして検出する必要があります。",
        "この入力では 先生 という言葉が日本語にも中国語にも存在するため、王先生今天不在 という中国語の文脈を使って判断することが重要です。",
        "日本語では 先生は大学で日本語を教えています と言えますが、中国語では 王老师在大学教中文 のように表現するため、両方の言語が混在していることを検出する必要があります。",
    ]);
    assert_eq!(r.status, Status::Ambiguous, "M3: {r:?}");
    assert_eq!(r.primary_language.as_deref(), Some("ja"));
    assert!(has_lang(&r, "zh-Hans") || has_lang(&r, "zh-Hant"));
}

#[test]
fn alternating_en_and_ja_sentences_produces_multi_language() {
    let r = identify_lines(&[
        "The teacher asked a question.",
        "先生は親切です。",
        "The student answered correctly.",
        "学生は元気です。",
    ]);
    // At minimum, both ja and en-US should be candidates.
    assert!(has_lang(&r, "en-US"));
    assert!(has_lang(&r, "ja"));
}

#[test]
fn cjk_only_paragraph_one_winner() {
    let r = identify_lines(&[
        "王老师在大学教中文。",
        "学生们每天来上课。",
    ]);
    assert!(matches!(
        r.primary_language.as_deref(),
        Some("zh-Hans") | Some("zh-Hant")
    ));
    assert_eq!(r.status, Status::Resolved);
}

#[test]
fn vi_paragraph_one_winner() {
    let r = identify_lines(&[
        "Thầy giáo dạy tiếng Việt.",
        "Học sinh học rất chăm chỉ.",
    ]);
    assert_eq!(r.primary_language.as_deref(), Some("vi-VN"));
    assert_eq!(r.status, Status::Resolved);
}

#[test]
fn paragraph_without_terminators_is_one_sentence() {
    // No periods in the lines — Layer 7 sees a single super-sentence.
    let r = identify_lines(&[
        "the teacher asked a question",
        "先生は親切です",
    ]);
    // No assertion on multi_language because per-line splitting requires
    // terminators; this test just confirms no panic and reasonable output.
    assert!(!r.candidates.is_empty(), "{r:?}");
}

#[test]
fn empty_input_unknown() {
    let r = identify("");
    assert_eq!(r.status, Status::Unknown);
}

#[test]
fn many_short_sentences_do_not_explode_runtime() {
    // 30 short sentences — well below the SENTENCE_CAP of 50.
    let lines: Vec<String> = (0..30)
        .map(|i| format!("Sentence number {i}."))
        .collect();
    let refs: Vec<&str> = lines.iter().map(|s| s.as_str()).collect();
    let r = identify_lines(&refs);
    assert_eq!(r.primary_language.as_deref(), Some("en-US"));
}
