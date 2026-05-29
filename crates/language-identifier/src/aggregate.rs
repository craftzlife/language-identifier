use std::collections::BTreeMap;

use crate::layers::context_window::ContextWindowSignal;
use crate::layers::dictionary::DictionarySignal;
use crate::layers::function_words::FunctionWordSignal;
use crate::layers::morphology::MorphologySignal;
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
    pub morphology: &'a MorphologySignal,
    pub context: &'a ContextWindowSignal,
    /// Layer 9 — per-language confidence from an `MlClassifier`. `None`
    /// when the caller didn't provide a classifier (the v3 default).
    pub ml_scores: Option<&'a [(String, f32)]>,
}

const FUNCTION_WORD_PER_HIT: f32 = 0.02;
const FUNCTION_WORD_CAP: f32 = 0.10;
const DICT_EXCLUSIVE_PER_HIT: f32 = 0.03;
const DICT_EXCLUSIVE_CAP: f32 = 0.20;
const DICT_SHARED_PER_HIT: f32 = 0.01;
const DICT_SHARED_CAP: f32 = 0.10;
const MORPH_PER_HIT: f32 = 0.02;
const MORPH_CAP: f32 = 0.10;
const CONTEXT_PER_SENTENCE: f32 = 0.05;
const CONTEXT_CAP: f32 = 0.15;
// Layer 9 carries the most weight among non-script layers but cannot
// single-handedly override a strong script signal.
const ML_PER_CONFIDENCE: f32 = 0.30;
const ML_CAP: f32 = 0.30;

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

    // Layer 6 — morphology hits (CJK endings + Latin token attributions
    // already absorbed into the script split above).
    for (lang, n) in &input.morphology.per_language {
        let bonus = (*n as f32 * MORPH_PER_HIT).min(MORPH_CAP);
        *w.entry(lang.clone()).or_insert(0.0) += bonus;
    }

    // Layer 7 — per-sentence winners. Non-top languages that win at least one
    // sentence get a small bump so embedded sentences register in the final
    // candidate distribution.
    for (lang, n) in &input.context.per_language {
        let bonus = (*n as f32 * CONTEXT_PER_SENTENCE).min(CONTEXT_CAP);
        *w.entry(lang.clone()).or_insert(0.0) += bonus;
    }

    // Layer 9 — ML classifier scores (if a classifier was provided).
    if let Some(scores) = input.ml_scores {
        for (lang, conf) in scores {
            let bonus = (*conf * ML_PER_CONFIDENCE).min(ML_CAP);
            *w.entry(lang.clone()).or_insert(0.0) += bonus;
        }
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
        let latin_count = input.counts.latin as usize;
        let attr = &input.morphology.latin_attribution;

        if !attr.is_empty() {
            // Use Layer 6 per-token attribution. Each attributed token contributes
            // its char count to the corresponding language's share. The
            // unattributed remainder falls back to v2's vi-density / ngram logic.
            let mut en_chars = 0usize;
            let mut vi_chars = 0usize;
            let mut attributed_chars = 0usize;
            for a in attr {
                let len = a.end - a.start;
                attributed_chars += len;
                match a.language {
                    "en-US" => en_chars += len,
                    "vi-VN" => vi_chars += len,
                    _ => {}
                }
            }
            let remainder = latin_count.saturating_sub(attributed_chars);
            if en_chars > 0 {
                add(w, "en-US", en_chars as f32 / total);
            }
            if vi_chars > 0 {
                add(w, "vi-VN", vi_chars as f32 / total);
            }
            if remainder > 0 {
                // Fall through to v2 logic on the unattributed slice. Use the
                // global vi-marker count as a proxy; this gives plain ASCII
                // tokens (numbers, unknown words) the same treatment as v2.
                let rem_w = remainder as f32 / total;
                if input.ortho.vi_markers >= 1
                    && (input.ortho.vi_markers as f32 / latin_count as f32) >= 0.10
                {
                    add(w, "vi-VN", rem_w);
                } else {
                    add(w, "en-US", rem_w);
                }
            }
        } else {
            // No Layer 6 attribution — v2 path.
            let vi_density = input.ortho.vi_markers as f32 / latin_count as f32;
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
