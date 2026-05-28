use std::collections::BTreeMap;

use crate::layers::dictionary::DictionarySignal;
use crate::layers::function_words::FunctionWordSignal;
use crate::layers::ngram::NgramSignal;
use crate::layers::orthography::OrthoSignals;
use crate::layers::script::ScriptCounts;
use crate::types::Candidate;

pub struct AggregateInput<'a> {
    pub counts: &'a ScriptCounts,
    pub ortho: &'a OrthoSignals,
    pub ngram: &'a NgramSignal,
    pub function_words: &'a FunctionWordSignal,
    pub dictionary: &'a DictionarySignal,
}

const FUNCTION_WORD_PER_HIT: f32 = 0.02;
const FUNCTION_WORD_CAP: f32 = 0.10;
const DICT_EXCLUSIVE_PER_HIT: f32 = 0.03;
const DICT_EXCLUSIVE_CAP: f32 = 0.20;
const DICT_SHARED_PER_HIT: f32 = 0.01;
const DICT_SHARED_CAP: f32 = 0.10;

pub fn aggregate(input: AggregateInput<'_>) -> Vec<Candidate> {
    let mut w: BTreeMap<String, f32> = BTreeMap::new();

    let total = input.counts.supported_total() as f32;
    if total > 0.0 {
        accumulate_script_weights(&mut w, &input);
    }

    // Layer 4 — function words.
    for (lang, n) in &input.function_words.per_language {
        let bonus = (*n as f32 * FUNCTION_WORD_PER_HIT).min(FUNCTION_WORD_CAP);
        *w.entry(lang.clone()).or_insert(0.0) += bonus;
    }

    // Layer 5 — dictionary.
    for (lang, n) in &input.dictionary.exclusive_per_language {
        let bonus = (*n as f32 * DICT_EXCLUSIVE_PER_HIT).min(DICT_EXCLUSIVE_CAP);
        *w.entry(lang.clone()).or_insert(0.0) += bonus;
    }
    for (lang, n) in &input.dictionary.shared_per_language {
        let bonus = (*n as f32 * DICT_SHARED_PER_HIT).min(DICT_SHARED_CAP);
        *w.entry(lang.clone()).or_insert(0.0) += bonus;
    }

    // Renormalize so weights sum to 1.0.
    let sum: f32 = w.values().sum();
    if sum > 0.0 {
        for v in w.values_mut() {
            *v /= sum;
        }
    }

    let mut candidates: Vec<Candidate> = w
        .into_iter()
        .filter(|(_, c)| *c > 0.0)
        .map(|(language, confidence)| Candidate {
            language,
            confidence: round2(confidence),
        })
        .collect();
    candidates.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    candidates
}

fn accumulate_script_weights(w: &mut BTreeMap<String, f32>, input: &AggregateInput<'_>) {
    let total = input.counts.supported_total() as f32;
    let add = |w: &mut BTreeMap<String, f32>, lang: &str, val: f32| {
        *w.entry(lang.to_string()).or_insert(0.0) += val;
    };

    let latin_w = input.counts.latin as f32 / total;
    let kana_w = input.counts.kana() as f32 / total;
    let han_w = input.counts.han as f32 / total;
    let hangul_w = input.counts.hangul as f32 / total;

    if hangul_w > 0.0 {
        add(w, "ko", hangul_w);
    }

    if latin_w > 0.0 {
        let latin_count = input.counts.latin as f32;
        let vi_density = input.ortho.vi_markers as f32 / latin_count;

        if vi_density >= 0.10 {
            add(w, "vi-VN", latin_w);
        } else if input.ortho.vi_markers >= 1 {
            let vi_share = (input.ortho.vi_markers as f32 / total).min(latin_w);
            add(w, "vi-VN", vi_share);
            add(w, "en-US", latin_w - vi_share);
        } else if input.ngram.vi_hits > input.ngram.en_hits * 3 {
            add(w, "vi-VN", latin_w);
        } else {
            add(w, "en-US", latin_w);
        }
    }

    if kana_w > 0.0 {
        add(w, "ja", kana_w);
        if han_w > 0.0 {
            let han_count = input.counts.han as f32;
            if input.ortho.hans_markers > 0 && input.ortho.hant_markers == 0 {
                let frac = (input.ortho.hans_markers as f32 / han_count).clamp(0.0, 0.5);
                add(w, "zh-Hans", han_w * frac);
                add(w, "ja", han_w * (1.0 - frac));
            } else if input.ortho.hant_markers > 0 && input.ortho.hans_markers == 0 {
                let frac = (input.ortho.hant_markers as f32 / han_count).clamp(0.0, 0.5);
                add(w, "zh-Hant", han_w * frac);
                add(w, "ja", han_w * (1.0 - frac));
            } else {
                add(w, "ja", han_w);
            }
        }
    } else if han_w > 0.0 {
        if input.ortho.hans_markers > 0 && input.ortho.hant_markers == 0 {
            add(w, "zh-Hans", han_w);
        } else if input.ortho.hant_markers > 0 && input.ortho.hans_markers == 0 {
            add(w, "zh-Hant", han_w);
        } else {
            // No kana, no markers — dictionary signals (Layer 5) will tip the
            // balance. Distribute Han evenly across the three possibilities to
            // surface them all as candidates.
            add(w, "ja", han_w * 0.34);
            add(w, "zh-Hans", han_w * 0.33);
            add(w, "zh-Hant", han_w * 0.33);
        }
    }
}

fn round2(x: f32) -> f32 {
    (x * 100.0).round() / 100.0
}
