use std::collections::{BTreeMap, HashSet};
use std::sync::OnceLock;

use super::normalize::Normalized;
use super::script::{classify, Script};

const LEXICON_EN: &str = include_str!("../data/lexicon_en.txt");
const LEXICON_JA: &str = include_str!("../data/lexicon_ja.txt");
const LEXICON_ZH_HANS: &str = include_str!("../data/lexicon_zh_hans.txt");
const LEXICON_ZH_HANT: &str = include_str!("../data/lexicon_zh_hant.txt");
const LEXICON_VI: &str = include_str!("../data/lexicon_vi.txt");
const LEXICON_KO: &str = include_str!("../data/lexicon_ko.txt");

pub const LANGUAGES: &[&str] = &["en", "ja", "zh-Hans", "zh-Hant", "vi", "ko"];

/// Exposed for the function-words layer, which derives its tables from the
/// top N entries of each lexicon at load time.
pub fn raw_lexicon_for_function_words(lang: &str) -> &'static str {
    raw_for(lang)
}

fn raw_for(lang: &str) -> &'static str {
    match lang {
        "en" => LEXICON_EN,
        "ja" => LEXICON_JA,
        "zh-Hans" => LEXICON_ZH_HANS,
        "zh-Hant" => LEXICON_ZH_HANT,
        "vi" => LEXICON_VI,
        "ko" => LEXICON_KO,
        _ => "",
    }
}

/// Look up a token in a language's full 10K-entry lexicon. Returns true on hit.
/// Lazily loads the lexicon on first use per language.
pub fn contains(lang: &str, token: &str) -> bool {
    lexicon(lang).contains(token)
}

fn lexicon(lang: &str) -> &'static HashSet<&'static str> {
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
    cell.get_or_init(|| raw_for(lang).lines().filter(|l| !l.is_empty()).collect())
}

#[derive(Debug, Clone, Default)]
pub struct DictionarySignal {
    /// language → total recognized hits (token appeared in this language's lexicon).
    pub hits_per_language: BTreeMap<String, usize>,
    /// language → exclusive hits (token appeared in this language only).
    pub exclusive_per_language: BTreeMap<String, usize>,
    /// language → shared hits (token appeared here and in at least one other language).
    pub shared_per_language: BTreeMap<String, usize>,
    /// Distinct tokens recognized by at least one lexicon.
    pub recognized_tokens: usize,
    /// Distinct tokens recognized by at least two lexicons.
    pub ambiguous_tokens: usize,
}

pub fn score(input: &Normalized) -> DictionarySignal {
    let mut sig = DictionarySignal::default();
    if input.chars.is_empty() {
        return sig;
    }

    let mut tokens: Vec<String> = Vec::new();
    extract_latin_tokens(input, &mut tokens);
    extract_cjk_tokens(input, &mut tokens);

    let mut seen: HashSet<String> = HashSet::new();
    for tok in tokens {
        if !seen.insert(tok.clone()) {
            continue;
        }
        let lower = tok.to_lowercase();
        let mut langs: Vec<&'static str> = Vec::new();
        for &lang in LANGUAGES {
            let lex = lexicon(lang);
            let probe: &str = if lang == "en" || lang == "vi" {
                lower.as_str()
            } else {
                tok.as_str()
            };
            if lex.contains(probe) {
                langs.push(lang);
            }
        }

        if langs.is_empty() {
            continue;
        }
        sig.recognized_tokens += 1;
        if langs.len() >= 2 {
            sig.ambiguous_tokens += 1;
        }
        for lang in &langs {
            *sig.hits_per_language.entry((*lang).into()).or_insert(0) += 1;
            if langs.len() == 1 {
                *sig.exclusive_per_language
                    .entry((*lang).into())
                    .or_insert(0) += 1;
            } else {
                *sig.shared_per_language.entry((*lang).into()).or_insert(0) += 1;
            }
        }
    }
    sig
}

fn extract_latin_tokens(input: &Normalized, out: &mut Vec<String>) {
    let mut current = String::new();
    let flush = |current: &mut String, out: &mut Vec<String>| {
        if !current.is_empty() {
            out.push(std::mem::take(current));
        }
    };
    for &c in &input.chars {
        if classify(c) == Script::Latin {
            current.push(c);
        } else {
            flush(&mut current, out);
        }
    }
    flush(&mut current, out);
}

fn extract_cjk_tokens(input: &Normalized, out: &mut Vec<String>) {
    let cjk: Vec<char> = input
        .chars
        .iter()
        .copied()
        .filter(|&c| {
            matches!(
                classify(c),
                Script::Han | Script::Hiragana | Script::Katakana | Script::Hangul
            )
        })
        .collect();

    // 2-, 3-, and 4-char sliding windows. 1-char tokens are too noisy and
    // dominated by particles already handled in Layer 4.
    for window in 2..=4 {
        if cjk.len() < window {
            break;
        }
        for start in 0..=cjk.len() - window {
            let token: String = cjk[start..start + window].iter().collect();
            out.push(token);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layers::normalize::normalize;

    #[test]
    fn english_tokens_resolve_to_en_exclusively() {
        let n = normalize("teacher professor university student");
        let s = score(&n);
        assert!(s.exclusive_per_language.get("en").copied().unwrap_or(0) >= 2);
        assert_eq!(s.exclusive_per_language.get("ja").copied().unwrap_or(0), 0);
    }

    #[test]
    fn shared_cjk_token_marks_ambiguity() {
        // 先生 appears in both ja and zh lexicons.
        let n = normalize("先生");
        let s = score(&n);
        let in_ja = s.hits_per_language.get("ja").copied().unwrap_or(0) > 0;
        let in_zh = s.hits_per_language.get("zh-Hans").copied().unwrap_or(0) > 0
            || s.hits_per_language.get("zh-Hant").copied().unwrap_or(0) > 0;
        assert!(in_ja && in_zh, "expected 先生 in both ja and zh: {s:?}");
        assert!(s.ambiguous_tokens >= 1);
    }

    #[test]
    fn vietnamese_tokens_resolve_to_vi() {
        let n = normalize("giáo viên đại học");
        let s = score(&n);
        assert!(s.hits_per_language.get("vi").copied().unwrap_or(0) >= 1);
    }

    #[test]
    fn empty_input_yields_empty_signal() {
        let n = normalize("");
        let s = score(&n);
        assert_eq!(s.recognized_tokens, 0);
        assert!(s.hits_per_language.is_empty());
    }

    #[test]
    fn lexicons_load_with_expected_size() {
        for &lang in LANGUAGES {
            let lex = lexicon(lang);
            assert!(
                lex.len() >= 9_000,
                "{lang} lexicon too small: {}",
                lex.len()
            );
        }
    }
}
