use super::normalize::Normalized;
use super::script::{classify, Script};

#[derive(Debug, Clone, Default)]
pub struct NgramSignal {
    pub en_hits: usize,
    pub vi_hits: usize,
}

const EN_BIGRAMS: &[&str] = &["th", "he", "in", "er", "an", "ed", "ng", "ou"];
const EN_TRIGRAMS: &[&str] = &["the", "ing", "and", "ion", "ent"];
const VI_BIGRAMS: &[&str] = &["nh", "ng", "ch", "tr", "ph"];
const VI_TRIGRAMS: &[&str] = &["ngu", "trư", "ngh", "thư"];

pub fn score(input: &Normalized) -> NgramSignal {
    let latin_lower: String = input
        .chars
        .iter()
        .filter(|c| classify(**c) == Script::Latin || c.is_whitespace())
        .flat_map(|c| c.to_lowercase())
        .collect();

    if latin_lower.trim().is_empty() {
        return NgramSignal::default();
    }

    let mut sig = NgramSignal::default();
    for bg in EN_BIGRAMS {
        sig.en_hits += latin_lower.matches(bg).count();
    }
    for tg in EN_TRIGRAMS {
        sig.en_hits += latin_lower.matches(tg).count();
    }
    for bg in VI_BIGRAMS {
        sig.vi_hits += latin_lower.matches(bg).count();
    }
    for tg in VI_TRIGRAMS {
        sig.vi_hits += latin_lower.matches(tg).count();
    }
    sig
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layers::normalize::normalize;

    #[test]
    fn english_text_scores_more_en_hits() {
        let n = normalize("the teacher is in the classroom and teaching English");
        let s = score(&n);
        assert!(s.en_hits > s.vi_hits);
    }

    #[test]
    fn empty_on_non_latin() {
        let n = normalize("先生は大学で日本語を教えています");
        let s = score(&n);
        assert_eq!(s.en_hits, 0);
        assert_eq!(s.vi_hits, 0);
    }
}
