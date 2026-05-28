use language_identifier::{identify, identify_lines, Status};

fn top_lang(r: &language_identifier::IdentifyResult) -> &str {
    r.candidates
        .first()
        .map(|c| c.language.as_str())
        .unwrap_or("")
}

fn has_lang(r: &language_identifier::IdentifyResult, lang: &str) -> bool {
    r.candidates.iter().any(|c| c.language == lang)
}

// --- Single language, word examples ---

#[test]
fn w2_english_latin_word() {
    let r = identify("Teacher Profesor");
    assert_eq!(r.status, Status::Resolved, "W2 status: {r:?}");
    assert_eq!(top_lang(&r), "en-US", "W2 top: {r:?}");
}

#[test]
fn w1_ambiguous_han_word() {
    // 先生|教師|先生|老师 — Han-only mixed Simplified+Traditional markers.
    // v1 (no dictionary) can resolve to a Chinese variant; full match with the diagram
    // requires Layer 5+. Assert only that we don't return en-US.
    let r = identify("先生 教師 先生 老师");
    assert_ne!(top_lang(&r), "en-US");
    assert!(
        matches!(top_lang(&r), "ja" | "zh" | "zh-Hans" | "zh-Hant"),
        "W1 top should be ja/zh-family, got: {r:?}"
    );
}

// --- Single language, phrase examples ---

#[test]
fn p1_japanese_phrases() {
    let r = identify("大学の先生 日本語の教師 高校の先生 英語を教える先生 尊敬される教授");
    assert_eq!(r.status, Status::Resolved, "P1: {r:?}");
    assert_eq!(top_lang(&r), "ja");
}

#[test]
fn p2_simplified_chinese_phrases() {
    let r = identify("大学老师 中文老师 高中老师 教英语的老师 受人尊敬的教授");
    assert_eq!(r.status, Status::Resolved, "P2: {r:?}");
    assert_eq!(top_lang(&r), "zh-Hans");
}

#[test]
fn p3_english_phrases() {
    let r = identify(
        "university teacher Japanese language teacher high school teacher teacher of English respected professor",
    );
    assert_eq!(r.status, Status::Resolved, "P3: {r:?}");
    assert_eq!(top_lang(&r), "en-US");
}

#[test]
fn p4_vietnamese_phrases() {
    let r = identify(
        "giáo viên đại học giáo viên tiếng Nhật giáo viên trung học thầy giáo dạy tiếng Anh giáo sư được kính trọng",
    );
    assert_eq!(r.status, Status::Resolved, "P4: {r:?}");
    assert_eq!(top_lang(&r), "vi-VN");
}

// --- Paragraph examples ---

#[test]
fn g1_japanese_paragraph() {
    let r = identify_lines(&[
        "田中先生は大学で日本語を教えています。",
        "学生たちは毎日授業に参加し、新しい言葉や文法を学んでいます。",
        "先生はとても親切で、難しい質問にも丁寧に答えてくれます。",
    ]);
    assert_eq!(r.status, Status::Resolved, "G1: {r:?}");
    assert_eq!(top_lang(&r), "ja");
}

#[test]
fn g2_simplified_chinese_paragraph() {
    let r = identify_lines(&[
        "王老师在大学教中文。",
        "学生们每天来上课，学习新的词语和语法。",
        "老师很有耐心，也会认真回答学生的问题。",
    ]);
    assert_eq!(r.status, Status::Resolved, "G2: {r:?}");
    assert_eq!(top_lang(&r), "zh-Hans");
}

#[test]
fn g3_english_paragraph() {
    let r = identify_lines(&[
        "Mr. Tanaka is a teacher at the university.",
        "His students attend class every day and learn new vocabulary and grammar.",
        "He is very kind and always answers difficult questions carefully.",
    ]);
    assert_eq!(r.status, Status::Resolved, "G3: {r:?}");
    assert_eq!(top_lang(&r), "en-US");
}

#[test]
fn g4_vietnamese_paragraph() {
    let r = identify_lines(&[
        "Thầy Tanaka là giáo viên ở trường đại học.",
        "Các sinh viên tham gia lớp học mỗi ngày và học thêm từ vựng cũng như ngữ pháp mới.",
        "Thầy rất tận tâm và luôn trả lời cẩn thận những câu hỏi khó của sinh viên.",
    ]);
    assert_eq!(r.status, Status::Resolved, "G4: {r:?}");
    assert_eq!(top_lang(&r), "vi-VN");
}

// --- Mixed language paragraphs ---

#[test]
fn m1_en_carrier_with_japanese_embedded() {
    let r = identify_lines(&[
        "When the user selects the word 先生 from a Japanese article, the library should not only look at the word itself but also analyze the surrounding sentence, such as 先生は大学で日本語を教えています。",
        "This dictionary app should detect that the main sentence is English, while the embedded phrase 日本語を勉強する belongs to Japanese and provides useful context for the selected word.",
        "If a user writes I want to understand the meaning of 大学の先生 in this sentence, the detector should recognize that English is the main language and Japanese appears as an embedded phrase.",
    ]);
    assert_eq!(top_lang(&r), "en-US", "M1: {r:?}");
    assert_eq!(r.primary_language.as_deref(), Some("en-US"));
    assert!(has_lang(&r, "ja"), "M1 should expose ja as embedded: {r:?}");
}

#[test]
fn m2_en_carrier_with_simplified_chinese_embedded() {
    let r = identify_lines(&[
        "When the user selects the word 先生 from a Chinese article, the library should inspect the full sentence, such as 王先生今天在大学教中文, before deciding whether the text is Chinese or Japanese.",
        "This app should understand that the sentence is mainly English, but the phrase 中文老师在大学上课 is Chinese and should be treated as an embedded language segment.",
        "If the input says Please explain why 老师 and 先生 can both refer to a teacher or a respectful title in Chinese, the detector should mark English as the main language and Chinese as embedded content.",
    ]);
    assert_eq!(top_lang(&r), "en-US", "M2: {r:?}");
    assert_eq!(r.primary_language.as_deref(), Some("en-US"));
    assert!(
        has_lang(&r, "zh-Hans") || has_lang(&r, "zh"),
        "M2 should expose zh-Hans (or zh) as embedded: {r:?}"
    );
}

#[test]
fn m3_japanese_carrier_with_chinese_examples() {
    // The SDD case expects ambiguous status with LLM picking ja as primary.
    // v1 (no LLM) will pick ja as primary deterministically; status may be resolved
    // because kana dominates. We only assert top is ja.
    let r = identify_lines(&[
        "日本語の文の中に 中文老师在大学教中文 という中国語の例文が含まれている場合、システムは日本語を主言語として扱い、中国語部分を別のセグメントとして検出する必要があります。",
        "この入力では 先生 という言葉が日本語にも中国語にも存在するため、王先生今天不在 という中国語の文脈を使って判断することが重要です。",
        "日本語では 先生は大学で日本語を教えています と言えますが、中国語では 王老师在大学教中文 のように表現するため、両方の言語が混在していることを検出する必要があります。",
    ]);
    assert_eq!(top_lang(&r), "ja", "M3: {r:?}");
}

#[test]
fn m4_four_language_meta_discussion() {
    let r = identify_lines(&[
        "This language detection library should support English, tiếng Việt, 日本語, and 中文 in the same input, especially when a user explains that 先生 can appear in both Japanese and Chinese contexts.",
        "When the input says Tôi muốn compare the Japanese sentence 先生は大学で日本語を教えています with the Chinese sentence 王老师在大学教中文, the detector should return a mixed-language result instead of forcing one language.",
        "A real user may write Please translate câu này sang tiếng Việt: 田中先生は大学で日本語を教えています, so the library needs to detect English, Vietnamese, and Japanese in one line.",
    ]);
    assert_eq!(top_lang(&r), "en-US", "M4: {r:?}");
    // Should see at least two of: vi-VN, ja, zh-Hans as low-confidence candidates.
    let embedded_count = ["vi-VN", "ja", "zh-Hans"]
        .iter()
        .filter(|l| has_lang(&r, l))
        .count();
    assert!(
        embedded_count >= 2,
        "M4 should expose multiple embedded languages: {r:?}"
    );
}

// --- Edge cases ---

#[test]
fn empty_input_is_unknown() {
    let r = identify("");
    assert_eq!(r.status, Status::Unknown);
}

#[test]
fn whitespace_only_is_unknown() {
    let r = identify("   \t  ");
    assert_eq!(r.status, Status::Unknown);
}

#[test]
fn korean_resolves() {
    let r = identify("선생님은 학생들을 가르칩니다");
    assert_eq!(top_lang(&r), "ko");
}

// --- JSON serialization stays in sync with SDD schema ---

#[test]
fn json_output_uses_sdd_field_names() {
    let r = identify("Teacher");
    let json = serde_json::to_string(&r).unwrap();
    assert!(json.contains("\"candidates\""));
    assert!(json.contains("\"status\""));
    assert!(json.contains("\"reason\""));
    // primaryLanguage is camelCase per SDD
    assert!(json.contains("\"primaryLanguage\":\"en-US\""));
    // status is lowercase per SDD
    assert!(json.contains("\"status\":\"resolved\""));
}
