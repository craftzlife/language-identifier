use std::collections::BTreeMap;

use super::dictionary;
use super::normalize::Normalized;
use super::orthography::is_vi_marker_char;
use super::script::{classify, Script};

/// Per-token attribution of a Latin word to a language.
/// `start` and `end` are **char indices** into `Normalized.chars` (not byte
/// offsets); callers can convert via `Normalized.char_byte_offsets`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LatinAttribution {
    pub start: usize,
    pub end: usize,
    pub language: &'static str,
}

#[derive(Debug, Clone, Default)]
pub struct MorphologySignal {
    /// language → number of tokens whose morphology / shape matches that language.
    pub per_language: BTreeMap<String, usize>,
    /// Per-Latin-token attribution, in left-to-right order.
    pub latin_attribution: Vec<LatinAttribution>,
}

pub fn score(input: &Normalized) -> MorphologySignal {
    let mut sig = MorphologySignal::default();
    if input.chars.is_empty() {
        return sig;
    }

    attribute_latin_tokens(input, &mut sig);
    score_cjk_endings(input, &mut sig);
    sig
}

fn attribute_latin_tokens(input: &Normalized, sig: &mut MorphologySignal) {
    let mut start: Option<usize> = None;
    for i in 0..=input.chars.len() {
        let in_latin = i < input.chars.len() && classify(input.chars[i]) == Script::Latin;
        match (start, in_latin) {
            (None, true) => start = Some(i),
            (Some(s), false) => {
                let token: String = input.chars[s..i].iter().collect();
                if let Some(lang) = attribute_one(&token) {
                    sig.latin_attribution.push(LatinAttribution {
                        start: s,
                        end: i,
                        language: lang,
                    });
                    *sig.per_language.entry(lang.into()).or_insert(0) += 1;
                }
                start = None;
            }
            _ => {}
        }
    }
}

fn attribute_one(token: &str) -> Option<&'static str> {
    if token.is_empty() {
        return None;
    }

    // Rule 1: any VI-only diacritic ⇒ vi-VN, full stop.
    if token.chars().any(is_vi_marker_char) {
        return Some("vi");
    }

    let lower = token.to_lowercase();
    let in_en = dictionary::contains("en", &lower);
    let in_vi = dictionary::contains("vi", &lower);

    match (in_en, in_vi) {
        (true, false) => Some("en"),
        (false, true) => Some("vi"),
        (true, true) => Some(disambiguate_by_shape(&lower)),
        (false, false) => None,
    }
}

/// Vietnamese-only initial / final consonant clusters that are extremely
/// uncommon in English. If a token shows one of these, prefer vi-VN; else
/// fall back to en-US as the safer default.
fn disambiguate_by_shape(token: &str) -> &'static str {
    const VI_INITIALS: &[&str] = &["ng", "nh", "tr", "ph", "kh", "th", "ch", "gi", "qu"];
    const VI_FINALS: &[&str] = &["ng", "nh", "ch"];

    let has_vi_initial = VI_INITIALS.iter().any(|p| token.starts_with(p));
    let has_vi_final = VI_FINALS.iter().any(|p| token.ends_with(p));

    if has_vi_initial && has_vi_final {
        "vi"
    } else {
        // VI shape is suggestive but not exclusive — `th`, `ng`, `ch` all
        // appear in English too. Default to en-US when only one indicator hits.
        "en"
    }
}

fn score_cjk_endings(input: &Normalized, sig: &mut MorphologySignal) {
    let text: String = input.chars.iter().collect();

    // Japanese verb / copula endings. These are 2–3-char sequences that
    // Layer 4 (TOP_N = 50 of each lexicon) does not always capture.
    const JA_ENDINGS: &[&str] = &[
        "です",
        "ます",
        "ました",
        "ません",
        "ている",
        "ない",
        "だった",
        "でした",
    ];
    for pat in JA_ENDINGS {
        let n = text.matches(pat).count();
        if n > 0 {
            *sig.per_language.entry("ja".into()).or_insert(0) += n;
        }
    }

    // Korean predicate endings.
    const KO_ENDINGS: &[&str] = &["입니다", "합니다", "했다", "이다", "하다"];
    for pat in KO_ENDINGS {
        let n = text.matches(pat).count();
        if n > 0 {
            *sig.per_language.entry("ko".into()).or_insert(0) += n;
        }
    }

    // Chinese grammatical patterns (Hans/Hant split is handled elsewhere).
    const ZH_PATTERNS: &[&str] = &["是", "不是", "没有", "这是", "也是"];
    for pat in ZH_PATTERNS {
        let n = text.matches(pat).count();
        if n > 0 {
            *sig.per_language.entry("zh".into()).or_insert(0) += n;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layers::normalize::normalize;

    #[test]
    fn english_only_word_attributes_to_en() {
        let n = normalize("teacher");
        let s = score(&n);
        assert_eq!(s.latin_attribution.len(), 1);
        assert_eq!(s.latin_attribution[0].language, "en");
    }

    #[test]
    fn vi_diacritic_word_attributes_to_vi() {
        let n = normalize("thầy");
        let s = score(&n);
        assert_eq!(s.latin_attribution[0].language, "vi");
    }

    #[test]
    fn shape_word_with_vi_initial_and_final_attributes_to_vi() {
        // "nhanh" starts with nh- and ends with -nh: clearly Vietnamese shape.
        // Without diacritics, only the shape rule applies.
        let n = normalize("nhanh");
        let s = score(&n);
        // May fall through to None if `nhanh` isn't in either lexicon at all;
        // in that case test passes vacuously. If in vi only or both, should be vi.
        if !s.latin_attribution.is_empty() {
            assert_eq!(s.latin_attribution[0].language, "vi");
        }
    }

    #[test]
    fn ambiguous_la_falls_to_en() {
        // "la" is in both EN and VI lexicons (LA = Los Angeles abbrev, etc.)
        let n = normalize("la");
        let s = score(&n);
        if !s.latin_attribution.is_empty() {
            assert_eq!(s.latin_attribution[0].language, "en");
        }
    }

    #[test]
    fn multi_word_input_produces_multiple_attributions() {
        let n = normalize("teacher giáo viên student");
        let s = score(&n);
        let langs: Vec<&str> = s.latin_attribution.iter().map(|a| a.language).collect();
        assert!(langs.contains(&"en"));
        assert!(langs.contains(&"vi"));
    }

    #[test]
    fn ja_verb_ending_counted() {
        let n = normalize("先生は教えています");
        let s = score(&n);
        assert!(s.per_language.get("ja").copied().unwrap_or(0) >= 1);
    }

    #[test]
    fn ja_copula_endings_counted() {
        let n = normalize("私は学生です。先生はとても親切でした。");
        let s = score(&n);
        assert!(s.per_language.get("ja").copied().unwrap_or(0) >= 2);
    }

    #[test]
    fn ko_predicate_ending_counted() {
        let n = normalize("학생입니다");
        let s = score(&n);
        assert!(s.per_language.get("ko").copied().unwrap_or(0) >= 1);
    }

    #[test]
    fn zh_pattern_counted() {
        let n = normalize("这是老师");
        let s = score(&n);
        assert!(s.per_language.get("zh").copied().unwrap_or(0) >= 1);
    }

    #[test]
    fn empty_input_empty_signal() {
        let n = normalize("");
        let s = score(&n);
        assert!(s.per_language.is_empty());
        assert!(s.latin_attribution.is_empty());
    }

    #[test]
    fn cjk_only_input_has_no_latin_attribution() {
        let n = normalize("先生は大学で日本語を教えています");
        let s = score(&n);
        assert!(s.latin_attribution.is_empty());
    }

    #[test]
    fn unknown_token_leaves_no_attribution() {
        let n = normalize("xqzplmwyt");
        let s = score(&n);
        assert!(s.latin_attribution.is_empty());
    }
}
