use std::collections::BTreeMap;

use crate::layers::ngram::NgramSignal;
use crate::layers::orthography::OrthoSignals;
use crate::layers::script::ScriptCounts;
use crate::types::Candidate;

pub struct AggregateInput<'a> {
    pub counts: &'a ScriptCounts,
    pub ortho: &'a OrthoSignals,
    pub ngram: &'a NgramSignal,
}

pub fn aggregate(input: AggregateInput<'_>) -> Vec<Candidate> {
    let total = input.counts.supported_total() as f32;
    if total == 0.0 {
        return vec![];
    }

    let mut w: BTreeMap<String, f32> = BTreeMap::new();
    let add = |w: &mut BTreeMap<String, f32>, lang: &str, val: f32| {
        *w.entry(lang.to_string()).or_insert(0.0) += val;
    };

    let latin_w = input.counts.latin as f32 / total;
    let kana_w = input.counts.kana() as f32 / total;
    let han_w = input.counts.han as f32 / total;
    let hangul_w = input.counts.hangul as f32 / total;

    if hangul_w > 0.0 {
        add(&mut w, "ko", hangul_w);
    }

    // Latin: split between en-US and vi-VN based on VI markers.
    if latin_w > 0.0 {
        let latin_count = input.counts.latin as f32;
        let vi_density = input.ortho.vi_markers as f32 / latin_count;

        if vi_density >= 0.10 {
            // Strong VI signal across the Latin segment: claim it all for Vietnamese.
            add(&mut w, "vi-VN", latin_w);
        } else if input.ortho.vi_markers >= 1 {
            // Weak VI signal: a small slice of Latin belongs to vi as embedded content.
            let vi_share = (input.ortho.vi_markers as f32 / total).min(latin_w);
            add(&mut w, "vi-VN", vi_share);
            add(&mut w, "en-US", latin_w - vi_share);
        } else if input.ngram.vi_hits > input.ngram.en_hits * 3 {
            // n-gram tie-breaker: VI bigrams strongly outweigh EN bigrams.
            add(&mut w, "vi-VN", latin_w);
        } else {
            add(&mut w, "en-US", latin_w);
        }
    }

    // Kana + Han
    if kana_w > 0.0 {
        // Kana present: Japanese is the structural carrier. Kana proportion goes to ja.
        add(&mut w, "ja", kana_w);
        if han_w > 0.0 {
            // If Hans/Hant markers are present alongside kana, split some Han to that variant
            // (the diagram's mixed JA_ZH case).
            let han_count = input.counts.han as f32;
            if input.ortho.hans_markers > 0 && input.ortho.hant_markers == 0 {
                let frac =
                    (input.ortho.hans_markers as f32 / han_count).clamp(0.0, 0.5);
                add(&mut w, "zh-Hans", han_w * frac);
                add(&mut w, "ja", han_w * (1.0 - frac));
            } else if input.ortho.hant_markers > 0 && input.ortho.hans_markers == 0 {
                let frac =
                    (input.ortho.hant_markers as f32 / han_count).clamp(0.0, 0.5);
                add(&mut w, "zh-Hant", han_w * frac);
                add(&mut w, "ja", han_w * (1.0 - frac));
            } else {
                // No markers or conflicting markers: all Han goes to ja (kana spine wins).
                add(&mut w, "ja", han_w);
            }
        }
    } else if han_w > 0.0 {
        // No kana: this is Chinese or kanji-only Japanese.
        if input.ortho.hans_markers > 0 && input.ortho.hant_markers == 0 {
            add(&mut w, "zh-Hans", han_w);
        } else if input.ortho.hant_markers > 0 && input.ortho.hans_markers == 0 {
            add(&mut w, "zh-Hant", han_w);
        } else {
            // Mixed or no markers: ambiguous between ja (kanji-only) and zh (variant unclear).
            add(&mut w, "ja", han_w * 0.5);
            add(&mut w, "zh", han_w * 0.5);
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
    candidates.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal));
    candidates
}

fn round2(x: f32) -> f32 {
    (x * 100.0).round() / 100.0
}
