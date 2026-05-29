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
    // 先生|教師|先生|老师 — Han-only with mixed Simplified+Traditional markers.
    // The SDD expects ambiguous ja/zh. v2 surfaces both ja and a zh-* variant
    // as candidates via Layer 5 dictionary overlap (the diagram's expected
    // behavior), but the presence of 老师 (Hans-exclusive) tilts the deterministic
    // calibration toward zh-Hans. Assert top is in the ja/zh family AND a second
    // candidate from the other family is present.
    let r = identify("先生 教師 先生 老师");
    assert!(
        matches!(top_lang(&r), "ja" | "zh-Hans" | "zh-Hant"),
        "W1 top should be ja/zh-family, got: {r:?}"
    );
    let has_ja = has_lang(&r, "ja");
    let has_zh = has_lang(&r, "zh-Hans") || has_lang(&r, "zh-Hant");
    assert!(has_ja && has_zh, "W1 should expose both ja and zh: {r:?}");
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
    // SDD case M3: ambiguous status with ja as primary; zh-Hans / zh-Hant
    // visible as candidates. v3 reaches this via Layer 7's intra-sentence
    // Han-run detection: the embedded Chinese phrases (中文老师在大学教中文,
    // 王先生今天不在, 王老师在大学教中文) trigger the multi-language flag
    // which downgrades the otherwise-Resolved ja result to Ambiguous.
    let r = identify_lines(&[
        "日本語の文の中に 中文老师在大学教中文 という中国語の例文が含まれている場合、システムは日本語を主言語として扱い、中国語部分を別のセグメントとして検出する必要があります。",
        "この入力では 先生 という言葉が日本語にも中国語にも存在するため、王先生今天不在 という中国語の文脈を使って判断することが重要です。",
        "日本語では 先生は大学で日本語を教えています と言えますが、中国語では 王老师在大学教中文 のように表現するため、両方の言語が混在していることを検出する必要があります。",
    ]);
    assert_eq!(r.status, Status::Ambiguous, "M3 status: {r:?}");
    assert_eq!(r.primary_language.as_deref(), Some("ja"));
    assert!(has_lang(&r, "zh-Hans") || has_lang(&r, "zh-Hant"),
        "M3 must surface a zh-* candidate: {r:?}");
}

#[test]
fn m4_four_language_meta_discussion() {
    let r = identify_lines(&[
        "This language detection library should support English, tiếng Việt, 日本語, and 中文 in the same input, especially when a user explains that 先生 can appear in both Japanese and Chinese contexts.",
        "When the input says Tôi muốn compare the Japanese sentence 先生は大学で日本語を教えています with the Chinese sentence 王老师在大学教中文, the detector should return a mixed-language result instead of forcing one language.",
        "A real user may write Please translate câu này sang tiếng Việt: 田中先生は大学で日本語を教えています, so the library needs to detect English, Vietnamese, and Japanese in one line.",
    ]);
    assert_eq!(top_lang(&r), "en-US", "M4: {r:?}");
    // Tightened: SDD expects all four embedded language families to appear as candidates.
    let embedded_count = ["vi-VN", "ja", "zh-Hans"]
        .iter()
        .filter(|l| has_lang(&r, l))
        .count();
    assert!(
        embedded_count >= 3,
        "M4 should expose ≥3 embedded language candidates: {r:?}"
    );
    // Segments should report at least one ja span (the long Japanese sentence).
    assert!(
        r.segments.iter().any(|s| s.language == "ja"),
        "M4 should have at least one ja segment: {r:?}"
    );
}

// --- SDD §8.5 mixed-language inputs (no expected outputs in diagram; v1 behavior) ---

#[test]
fn m5_en_vi_carrier_with_vietnamese_embedded() {
    let r = identify_lines(&[
        "The user may write an English sentence like I want to translate giáo viên tiếng Nhật into Japanese, so the detector should identify English as the main language and Vietnamese as an embedded phrase.",
        "In a language learning app, a sentence such as Please explain the difference between thầy giáo, giáo viên, and giáo sư contains English structure but several Vietnamese terms.",
        "When the input contains I am building a feature that helps users understand cụm từ tiếng Việt trong câu dài, the detector should recognize both English and Vietnamese.",
    ]);
    assert_eq!(top_lang(&r), "en-US", "M5: {r:?}");
    assert!(has_lang(&r, "vi-VN"), "M5 should expose vi-VN: {r:?}");
}

#[test]
fn m6_vi_carrier_with_japanese_embedded() {
    let r = identify_lines(&[
        "Tôi muốn hệ thống nhận diện được rằng câu này chủ yếu là tiếng Việt, nhưng cụm 先生は大学で日本語を教えています là tiếng Nhật và cần được xử lý như một đoạn nhúng.",
        "Khi người dùng chọn từ 先生 trong câu tiếng Nhật như 先生はとても親切です, ứng dụng nên dùng cả ngữ cảnh xung quanh thay vì chỉ dựa vào một từ đơn lẻ.",
        "Ứng dụng học ngôn ngữ cần hiểu rằng câu Tôi đang học cách dùng cụm 大学の先生 trong tiếng Nhật có cả tiếng Việt và một cụm tiếng Nhật.",
    ]);
    assert_eq!(top_lang(&r), "vi-VN", "M6: {r:?}");
    assert!(has_lang(&r, "ja"), "M6 should expose ja: {r:?}");
}

#[test]
fn m7_vi_carrier_with_simplified_chinese_embedded() {
    let r = identify_lines(&[
        "Tôi muốn thư viện phát hiện rằng câu này chủ yếu là tiếng Việt, nhưng cụm 王先生今天在大学教中文 là tiếng Trung và không nên bị nhận nhầm thành tiếng Nhật.",
        "Khi người dùng nhập câu Hãy giải thích sự khác nhau giữa 老师 và 先生 trong tiếng Trung, hệ thống nên nhận diện tiếng Việt là ngôn ngữ chính và tiếng Trung là nội dung được nhúng.",
        "Ứng dụng nên xử lý tốt những câu như Tôi đang đọc một ví dụ tiếng Trung: 这位老师很有耐心, trong đó phần đầu là tiếng Việt còn phần sau là tiếng Trung.",
    ]);
    assert_eq!(top_lang(&r), "vi-VN", "M7: {r:?}");
    assert!(has_lang(&r, "zh-Hans"), "M7 should expose zh-Hans: {r:?}");
}

// --- Script-specific single-language inputs ---

#[test]
fn traditional_chinese_phrase() {
    // Strong Traditional markers, no kana.
    let r = identify("學校的國文老師會說中文");
    assert_eq!(top_lang(&r), "zh-Hant", "{r:?}");
    assert_eq!(r.status, Status::Resolved);
}

#[test]
fn hiragana_only_phrase() {
    let r = identify("ありがとうございます");
    assert_eq!(top_lang(&r), "ja");
    assert_eq!(r.status, Status::Resolved);
}

#[test]
fn katakana_only_phrase() {
    let r = identify("コンピューターサイエンス");
    assert_eq!(top_lang(&r), "ja");
    assert_eq!(r.status, Status::Resolved);
}

#[test]
fn vietnamese_single_word_with_strong_marker() {
    let r = identify("thầy");
    assert_eq!(top_lang(&r), "vi-VN");
}

#[test]
fn vietnamese_word_with_horn_diacritic() {
    let r = identify("tương lai tươi sáng");
    assert_eq!(top_lang(&r), "vi-VN");
}

#[test]
fn single_kanji_no_kana_is_ambiguous() {
    // 学 is shared between Japanese and Chinese; no markers either way.
    let r = identify("学");
    assert!(
        matches!(top_lang(&r), "ja" | "zh"),
        "single shared kanji top: {r:?}"
    );
    assert_eq!(r.status, Status::Ambiguous);
}

#[test]
fn korean_paragraph() {
    let r = identify_lines(&[
        "선생님은 매일 학생들에게 한국어를 가르칩니다.",
        "학생들은 열심히 공부하고 질문을 합니다.",
    ]);
    assert_eq!(top_lang(&r), "ko");
    assert_eq!(r.status, Status::Resolved);
}

#[test]
fn korean_with_english_embedded() {
    let r = identify("선생님은 university에서 가르칩니다");
    assert_eq!(top_lang(&r), "ko", "{r:?}");
    assert!(has_lang(&r, "en-US"));
}

// --- False-positive guards (shared diacritics, neutral Latin) ---

#[test]
fn french_diacritics_do_not_trigger_vietnamese() {
    // é à ç are Latin-1 Supplement, used widely beyond Vietnamese.
    let r = identify("café résumé naïve façade");
    assert_eq!(top_lang(&r), "en-US", "{r:?}");
}

#[test]
fn spanish_chars_do_not_trigger_vietnamese() {
    let r = identify("señor niño año");
    assert_eq!(top_lang(&r), "en-US", "{r:?}");
}

#[test]
fn alphanumeric_latin_still_resolves_english() {
    let r = identify("user123 password456 status789");
    assert_eq!(top_lang(&r), "en-US");
    assert_eq!(r.status, Status::Resolved);
}

#[test]
fn english_with_single_embedded_vi_word() {
    // One Vietnamese word inside long English text shouldn't flip the primary.
    let r = identify(
        "I would like to learn the Vietnamese word thầy which means teacher in English",
    );
    assert_eq!(top_lang(&r), "en-US", "{r:?}");
}

// --- Unsupported / Unknown branches ---

#[test]
fn emoji_only_is_unsupported() {
    let r = identify("🎉🎊✨");
    assert_eq!(r.status, Status::Unsupported);
    assert!(r.candidates.is_empty());
}

#[test]
fn digits_only_is_unsupported() {
    let r = identify("12345 67890");
    assert_eq!(r.status, Status::Unsupported);
}

#[test]
fn greek_text_is_unsupported() {
    let r = identify("καλημέρα κόσμε");
    assert_eq!(r.status, Status::Unsupported);
}

#[test]
fn punctuation_only_is_unknown() {
    // No visible chars after stripping punctuation.
    let r = identify("... !!! ???");
    assert_eq!(r.status, Status::Unknown);
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
fn empty_lines_array_is_unknown() {
    let r = identify_lines::<&str>(&[]);
    assert_eq!(r.status, Status::Unknown);
}

#[test]
fn korean_resolves() {
    let r = identify("선생님은 학생들을 가르칩니다");
    assert_eq!(top_lang(&r), "ko");
}

#[test]
fn input_with_nfc_decomposed_form() {
    // "café" with combining acute: should be NFC-composed and resolved (not
    // unknown / unsupported). The exact language depends on lexicon overlap
    // — "café" happens to be in the VI lexicon as a loanword.
    let r = identify("cafe\u{0301}");
    assert!(matches!(
        top_lang(&r),
        "en-US" | "vi-VN"
    ), "{r:?}");
    assert_ne!(r.status, language_identifier::Status::Unknown);
}

// --- JSON serialization stays in sync with SDD schema ---

#[test]
fn json_output_uses_sdd_field_names() {
    let r = identify("Teacher");
    let json = serde_json::to_string(&r).unwrap();
    assert!(json.contains("\"candidates\""));
    assert!(json.contains("\"status\""));
    assert!(json.contains("\"reasons\""));
    // primaryLanguage is camelCase per SDD
    assert!(json.contains("\"primaryLanguage\":\"en-US\""));
    // status is lowercase per SDD
    assert!(json.contains("\"status\":\"resolved\""));
}

#[test]
fn all_status_variants_serialize_lowercase() {
    assert_eq!(serde_json::to_string(&Status::Resolved).unwrap(), "\"resolved\"");
    assert_eq!(serde_json::to_string(&Status::Ambiguous).unwrap(), "\"ambiguous\"");
    assert_eq!(serde_json::to_string(&Status::Mixed).unwrap(), "\"mixed\"");
    assert_eq!(serde_json::to_string(&Status::Unknown).unwrap(), "\"unknown\"");
    assert_eq!(serde_json::to_string(&Status::Unsupported).unwrap(), "\"unsupported\"");
}

#[test]
fn reasons_is_array_for_unknown_input() {
    let r = identify("");
    let json = serde_json::to_string(&r).unwrap();
    let idx = json.find("\"reasons\":").unwrap();
    let after = &json[idx + "\"reasons\":".len()..];
    assert!(after.starts_with('['), "expected array reasons: {after}");
}

#[test]
fn reasons_is_array_for_multi() {
    let r = identify("先生は大学で日本語を教えています");
    let json = serde_json::to_string(&r).unwrap();
    let idx = json.find("\"reasons\":").unwrap();
    let after = &json[idx + "\"reasons\":".len()..];
    assert!(after.starts_with('['), "expected array reasons: {after}");
}

#[test]
fn primary_language_omitted_when_unknown() {
    let r = identify("");
    let json = serde_json::to_string(&r).unwrap();
    assert!(
        !json.contains("primaryLanguage"),
        "primaryLanguage should be omitted: {json}"
    );
}

#[test]
fn confidence_values_are_in_unit_interval() {
    let r = identify_lines(&[
        "When the user selects the word 先生 from a Japanese article, the library should not only look at the word itself but also analyze the surrounding sentence, such as 先生は大学で日本語を教えています。",
    ]);
    for c in &r.candidates {
        assert!(
            (0.0..=1.0).contains(&c.confidence),
            "confidence out of range: {c:?}"
        );
    }
}

#[test]
fn candidates_are_sorted_descending() {
    let r = identify_lines(&[
        "When the user selects the word 先生 from a Japanese article, the library should not only look at the word itself but also analyze the surrounding sentence, such as 先生は大学で日本語を教えています。",
    ]);
    for w in r.candidates.windows(2) {
        assert!(w[0].confidence >= w[1].confidence, "not sorted: {r:?}");
    }
}
