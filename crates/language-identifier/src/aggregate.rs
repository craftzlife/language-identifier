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
        let latin_count = input.counts.latin;
        let attr = &input.morphology.latin_attribution;

        if !attr.is_empty() {
            // Use Layer 6 per-token attribution. Each attributed token contributes
            // its char count to the corresponding language's share. The
            // unattributed remainder falls back to v2's vi-density / ngram logic.
            let mut en_chars = 0usize;
            let mut fr_chars = 0usize;
            let mut vi_chars = 0usize;
            let mut attributed_chars = 0usize;
            for a in attr {
                let len = a.end - a.start;
                attributed_chars += len;
                match a.language {
                    "en" => en_chars += len,
                    "fr" => fr_chars += len,
                    "vi" => vi_chars += len,
                    _ => {}
                }
            }
            let remainder = latin_count.saturating_sub(attributed_chars);
            if en_chars > 0 {
                add(w, "en", en_chars as f32 / total);
            }
            if fr_chars > 0 {
                add(w, "fr", fr_chars as f32 / total);
            }
            if vi_chars > 0 {
                add(w, "vi", vi_chars as f32 / total);
            }
            if remainder > 0 {
                // Fall through to v2 logic on the unattributed slice. Use the
                // global vi-marker count as a proxy; this gives plain ASCII
                // tokens (numbers, unknown words) the same treatment as v2.
                let rem_w = remainder as f32 / total;
                if input.ortho.vi_markers >= 1
                    && (input.ortho.vi_markers as f32 / latin_count as f32) >= 0.10
                {
                    add(w, "vi", rem_w);
                } else if input.ortho.fr_markers >= 1 && fr_chars > 0 {
                    // FR context — let remainder follow the dominant FR
                    // attribution rather than defaulting to EN.
                    add(w, "fr", rem_w);
                } else {
                    add(w, "en", rem_w);
                }
            }
        } else {
            // No Layer 6 attribution — v2 path with fr-marker tier added.
            let vi_density = input.ortho.vi_markers as f32 / latin_count as f32;
            let fr_density = input.ortho.fr_markers as f32 / latin_count as f32;
            if vi_density >= 0.10 {
                add(w, "vi", latin_w);
            } else if fr_density >= 0.05 {
                // FR markers (ç, œ, æ) are rarer than VI tone marks, so
                // a lower density still warrants attributing the Latin
                // mass to fr.
                add(w, "fr", latin_w);
            } else if input.ortho.vi_markers >= 1 {
                let vi_share = (input.ortho.vi_markers as f32 / total).min(latin_w);
                add(w, "vi", vi_share);
                add(w, "en", latin_w - vi_share);
            } else if input.ortho.fr_markers >= 1 {
                let fr_share = (input.ortho.fr_markers as f32 / total).min(latin_w);
                add(w, "fr", fr_share);
                add(w, "en", latin_w - fr_share);
            } else if input.ngram.vi_hits > input.ngram.en_hits * 3 {
                add(w, "vi", latin_w);
            } else {
                add(w, "en", latin_w);
            }
        }
    }

    if kana_w > 0.0 {
        add(w, "ja", kana_w);
    }
    if han_w > 0.0 {
        for (lang, share) in split_han_mass(han_w, input.counts.kana(), input.counts.han, input.ortho) {
            add(w, lang, share);
        }
    }
}

// Pick the most specific Hant tag warranted by HK/TW markers. Used by
// both the kana-with-Chinese-markers branch and the no-kana branch.
fn pick_hant_tag(ortho: &OrthoSignals) -> &'static str {
    let hk = ortho.hant_hk_markers > 0;
    let tw = ortho.hant_tw_markers > 0;
    if hk && !tw {
        "zh-Hant-HK"
    } else if tw && !hk {
        "zh-Hant-TW"
    } else {
        "zh-Hant"
    }
}

// Classical Chinese gating — mirrors `orthography::is_classical_chinese`
// (kept inline here to avoid threading the normalized text through
// AggregateInput just for one bool).
fn is_lzh_active(ortho: &OrthoSignals, han_count: usize) -> bool {
    ortho.lzh_markers >= 3 && han_count >= 4
}

// Distribute Han-char mass across the candidate languages that can
// carry Han script. Returns (lang, share) pairs whose shares sum to
// `han_w`.
//
// Priority order:
//   1. Classical density (`lzh_active`) → all Han to `lzh`.
//   2. Cantonese particles (`yue_markers > 0`) → all Han to `yue`.
//      Written Cantonese uses Hant-style chars but the language is yue,
//      so the particle signal claims the script for yue rather than
//      splitting it across zh-Hant.
//   3. Otherwise: proportional split across ja / zh-Hans /
//      zh-Hant{,-HK,-TW} / nan / hak / wuu weighted by their respective
//      marker counts (or by kana count for ja).
//   4. When no marker fires at all, fall back to the v2 even three-way
//      split between ja and the two zh variants so dictionary / context
//      signals can tip the balance.
fn split_han_mass(
    han_w: f32,
    kana_count: usize,
    han_count: usize,
    ortho: &OrthoSignals,
) -> Vec<(&'static str, f32)> {
    if is_lzh_active(ortho, han_count) {
        return vec![("lzh", han_w)];
    }
    if ortho.yue_markers > 0 {
        return vec![("yue", han_w)];
    }

    let hant_tag = pick_hant_tag(ortho);
    let kana = kana_count as f32;
    let hans_m = ortho.hans_markers as f32;
    let hant_m = ortho.hant_markers as f32;
    let nan_m = ortho.nan_markers as f32;
    let hak_m = ortho.hak_markers as f32;
    let wuu_m = ortho.wuu_markers as f32;

    let proof = kana + hans_m + hant_m + nan_m + hak_m + wuu_m;
    let mut out: Vec<(&'static str, f32)> = Vec::new();
    if proof > 0.0 {
        if kana > 0.0 {
            out.push(("ja", han_w * (kana / proof)));
        }
        if hans_m > 0.0 {
            out.push(("zh-Hans", han_w * (hans_m / proof)));
        }
        if hant_m > 0.0 {
            out.push((hant_tag, han_w * (hant_m / proof)));
        }
        if nan_m > 0.0 {
            out.push(("nan", han_w * (nan_m / proof)));
        }
        if hak_m > 0.0 {
            out.push(("hak", han_w * (hak_m / proof)));
        }
        if wuu_m > 0.0 {
            out.push(("wuu", han_w * (wuu_m / proof)));
        }
        return out;
    }

    // No kana, no markers — dictionary signals (Layer 5) will tip the
    // balance. Distribute Han evenly across the three classic candidates.
    if kana > 0.0 {
        out.push(("ja", han_w));
    } else {
        out.push(("ja", han_w * 0.34));
        out.push(("zh-Hans", han_w * 0.33));
        out.push((hant_tag, han_w * 0.33));
    }
    out
}

fn round2(x: f32) -> f32 {
    (x * 100.0).round() / 100.0
}
