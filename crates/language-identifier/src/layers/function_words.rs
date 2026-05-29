use std::collections::{BTreeMap, HashSet};
use std::sync::OnceLock;

use super::dictionary::{raw_lexicon_for_function_words, LANGUAGES};
use super::normalize::Normalized;
use super::script::{classify, Script};

/// How many top-ranked words to use as the function-word table for each language.
/// In a frequency-ranked list, the top ~50 entries are dominated by particles,
/// articles, conjunctions, copulas, and very-high-frequency verbs.
const TOP_N: usize = 50;

/// Maximum CJK function-word character length to scan. Most CJK particles are
/// 1–2 chars; capping prevents accidentally matching long words against a 50-word table.
const CJK_MAX_LEN: usize = 2;

#[derive(Debug, Clone, Default)]
pub struct FunctionWordSignal {
    /// language → number of function-word hits in the input.
    pub per_language: BTreeMap<String, usize>,
}

fn table_for(lang: &str) -> &'static HashSet<&'static str> {
    static EN: OnceLock<HashSet<&'static str>> = OnceLock::new();
    static JA: OnceLock<HashSet<&'static str>> = OnceLock::new();
    static ZH_HANS: OnceLock<HashSet<&'static str>> = OnceLock::new();
    static ZH_HANT: OnceLock<HashSet<&'static str>> = OnceLock::new();
    static VI: OnceLock<HashSet<&'static str>> = OnceLock::new();
    static KO: OnceLock<HashSet<&'static str>> = OnceLock::new();
    static EMPTY: OnceLock<HashSet<&'static str>> = OnceLock::new();

    let cell = match lang {
        "en" => &EN,
        "ja" => &JA,
        "zh-Hans" => &ZH_HANS,
        "zh-Hant" => &ZH_HANT,
        "vi" => &VI,
        "ko" => &KO,
        _ => &EMPTY,
    };
    cell.get_or_init(|| {
        raw_lexicon_for_function_words(lang)
            .lines()
            .take(TOP_N)
            .filter(|l| !l.is_empty())
            .collect()
    })
}

pub fn score(input: &Normalized) -> FunctionWordSignal {
    let mut sig = FunctionWordSignal::default();
    if input.chars.is_empty() {
        return sig;
    }

    // Latin: whitespace-tokenize, lowercase, intersect with en + vi tables.
    let mut current = String::new();
    let mut latin_tokens: Vec<String> = Vec::new();
    for &c in &input.chars {
        if classify(c) == Script::Latin {
            current.push(c);
        } else {
            if !current.is_empty() {
                latin_tokens.push(std::mem::take(&mut current));
            }
        }
    }
    if !current.is_empty() {
        latin_tokens.push(current);
    }

    for tok in &latin_tokens {
        let lower = tok.to_lowercase();
        for &lang in &["en", "vi"] {
            if table_for(lang).contains(lower.as_str()) {
                *sig.per_language.entry(lang.into()).or_insert(0) += 1;
            }
        }
    }

    // CJK / Hangul: sliding 1- and 2-char windows against each CJK table.
    for &lang in &["ja", "zh-Hans", "zh-Hant", "ko"] {
        let table = table_for(lang);
        if table.is_empty() {
            continue;
        }
        let mut count = 0;
        for (i, &c) in input.chars.iter().enumerate() {
            if !matches!(
                classify(c),
                Script::Han | Script::Hiragana | Script::Katakana | Script::Hangul
            ) {
                continue;
            }
            // 1-char window
            let mut buf = [0u8; 4];
            let one = c.encode_utf8(&mut buf);
            if table.contains(one) {
                count += 1;
            }
            // 2-char window
            if CJK_MAX_LEN >= 2 && i + 1 < input.chars.len() {
                let next = input.chars[i + 1];
                if matches!(
                    classify(next),
                    Script::Han | Script::Hiragana | Script::Katakana | Script::Hangul
                ) {
                    let pair: String = [c, next].iter().collect();
                    if table.contains(pair.as_str()) {
                        count += 1;
                    }
                }
            }
        }
        if count > 0 {
            sig.per_language.insert(lang.into(), count);
        }
    }

    let _ = LANGUAGES;
    sig
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layers::normalize::normalize;

    #[test]
    fn english_stopwords_counted() {
        let n = normalize("the teacher and the student of the school");
        let s = score(&n);
        assert!(s.per_language.get("en").copied().unwrap_or(0) >= 3);
    }

    #[test]
    fn japanese_particles_counted() {
        let n = normalize("先生は大学で日本語を教えています");
        let s = score(&n);
        assert!(s.per_language.get("ja").copied().unwrap_or(0) >= 2);
    }

    #[test]
    fn chinese_particles_counted() {
        let n = normalize("老师在大学教中文");
        let s = score(&n);
        let zh = s.per_language.get("zh-Hans").copied().unwrap_or(0)
            + s.per_language.get("zh-Hant").copied().unwrap_or(0);
        assert!(zh >= 1, "{s:?}");
    }

    #[test]
    fn vietnamese_stopwords_counted() {
        let n = normalize("là của và có không được");
        let s = score(&n);
        assert!(s.per_language.get("vi").copied().unwrap_or(0) >= 3);
    }

    #[test]
    fn korean_particles_counted() {
        let n = normalize("선생님은 학생을 가르치는 학교에서");
        let s = score(&n);
        assert!(s.per_language.get("ko").copied().unwrap_or(0) >= 1);
    }

    #[test]
    fn empty_input_yields_empty_signal() {
        let n = normalize("");
        let s = score(&n);
        assert!(s.per_language.is_empty());
    }
}
