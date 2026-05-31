use crate::aggregate::{aggregate, AggregateInput};
use crate::calibration::{calibrate, CalibrationHints};

/// Confidence at or above which a Layer 9 unsupported-language signal
/// short-circuits the pipeline to `Status::Unsupported`. Strong enough
/// to avoid false positives on mixed inputs (where lid.176 typically
/// splits 0.55 / 0.40 between two languages); paranoid models can pick
/// a higher threshold by post-processing their `UnsupportedSignal`.
const ML_UNSUPPORTED_THRESHOLD: f32 = 0.50;
use crate::layers::{
    context_window, dictionary, function_words, morphology, ngram, normalize, orthography, script,
};
use crate::options::IdentifyOptions;
use crate::segments::{self, HanStrategy};
use crate::types::{IdentifyResult, Status};

pub fn run(input: &str) -> IdentifyResult {
    run_with(input, &IdentifyOptions::default())
}

pub fn run_with(input: &str, opts: &IdentifyOptions<'_>) -> IdentifyResult {
    let normalized = normalize::normalize(input);
    if normalized.visible_chars == 0 {
        return IdentifyResult {
            candidates: vec![],
            primary_language: None,
            status: Status::Unknown,
            reasons: vec!["Empty input after normalization".into()],
            segments: vec![],
            normalized_text: normalized.text,
        };
    }

    let counts = script::count_scripts(&normalized);
    let ortho = orthography::detect(&normalized);
    let ngram_sig = ngram::score(&normalized);
    let fw = function_words::score(&normalized);
    let dict = dictionary::score(&normalized);
    let morph = morphology::score(&normalized);
    let context = context_window::score(&normalized);
    let ml_scores: Option<Vec<(String, f32)>> =
        opts.ml_classifier.map(|c| c.classify(&normalized.text));

    // Layer 9 unsupported-language short-circuit. When the classifier
    // is confident the input is in a language outside the supported
    // set (e.g. French through an en/vi/ja/ko/zh-only library), bail
    // to `Status::Unsupported` rather than let the deterministic Latin
    // path force the verdict onto en.
    if let Some(classifier) = opts.ml_classifier {
        if let Some(sig) = classifier.unsupported_signal(&normalized.text) {
            if sig.confidence >= ML_UNSUPPORTED_THRESHOLD {
                return IdentifyResult {
                    candidates: vec![],
                    primary_language: None,
                    status: Status::Unsupported,
                    reasons: vec![format!(
                        "Layer 9 (ML classifier) detected unsupported language '{}' (confidence {:.2}) — verdict downgraded to unsupported",
                        sig.label, sig.confidence
                    )],
                    segments: vec![],
                    normalized_text: normalized.text,
                };
            }
        }
    }

    let mut notes: Vec<String> = Vec::new();
    notes.push(format!(
        "Script counts — latin:{}, hiragana:{}, katakana:{}, han:{}, hangul:{}",
        counts.latin, counts.hiragana, counts.katakana, counts.han, counts.hangul
    ));
    if ortho.vi_markers > 0 {
        notes.push(format!(
            "Vietnamese diacritic markers detected: {}",
            ortho.vi_markers
        ));
    }
    if ortho.hans_markers > 0 {
        notes.push(format!(
            "Simplified-Chinese-only markers detected: {}",
            ortho.hans_markers
        ));
    }
    if ortho.hant_markers > 0 {
        notes.push(format!(
            "Traditional-Chinese-only markers detected: {}",
            ortho.hant_markers
        ));
    }
    if counts.kana() > 0 {
        let kana = counts.kana();
        let note = if ortho.hans_markers > kana || ortho.hant_markers > kana {
            "Japanese kana present, but Chinese-only markers outweigh kana — Han attributed proportionally"
        } else if ortho.hans_markers > 0 || ortho.hant_markers > 0 {
            "Japanese kana present alongside Chinese-only markers — Han split proportionally"
        } else {
            "Japanese kana present — Han characters interpreted as kanji"
        };
        notes.push(note.into());
    }
    if !fw.per_language.is_empty() {
        let mut parts: Vec<String> = fw
            .per_language
            .iter()
            .map(|(l, n)| format!("{l}:{n}"))
            .collect();
        parts.sort();
        notes.push(format!("Function-word hits — {}", parts.join(", ")));
    }
    if dict.recognized_tokens > 0 {
        notes.push(format!(
            "Dictionary — {} recognized tokens ({} ambiguous across lexicons)",
            dict.recognized_tokens, dict.ambiguous_tokens
        ));
    }
    if !morph.per_language.is_empty() || !morph.latin_attribution.is_empty() {
        let mut parts: Vec<String> = morph
            .per_language
            .iter()
            .map(|(l, n)| format!("{l}:{n}"))
            .collect();
        parts.sort();
        let attr_summary = if morph.latin_attribution.is_empty() {
            String::new()
        } else {
            format!(
                " ({} Latin tokens attributed)",
                morph.latin_attribution.len()
            )
        };
        notes.push(format!("Morphology — {}{}", parts.join(", "), attr_summary));
    }
    if !context.sentences.is_empty() {
        let mut parts: Vec<String> = context
            .per_language
            .iter()
            .map(|(l, n)| format!("{l}:{n}"))
            .collect();
        parts.sort();
        notes.push(format!(
            "Context window — {} sentences, dominant: {{{}}}{}",
            context.sentences.len(),
            parts.join(", "),
            if context.multi_language {
                "; multi-language paragraph"
            } else {
                ""
            }
        ));
    }
    if let Some(scores) = ml_scores.as_ref() {
        let mut parts: Vec<String> = scores.iter().map(|(l, c)| format!("{l}:{c:.2}")).collect();
        parts.sort();
        notes.push(format!("Layer 9 (ML classifier) — {}", parts.join(", ")));
    }

    let ranked = aggregate(AggregateInput {
        counts: &counts,
        ortho: &ortho,
        ngram: &ngram_sig,
        function_words: &fw,
        dictionary: &dict,
        morphology: &morph,
        context: &context,
        ml_scores: ml_scores.as_deref(),
    });

    let unsupported_only = counts.supported_total() == 0 && counts.other > 0;
    if unsupported_only {
        return IdentifyResult {
            candidates: vec![],
            primary_language: None,
            status: Status::Unsupported,
            reasons: vec!["Input uses scripts that are not in the supported set".into()],
            segments: vec![],
            normalized_text: normalized.text,
        };
    }

    let han_strategy = pick_han_strategy(&counts, &ortho);
    let segments = segments::extract(&normalized, &ortho, han_strategy, &morph.latin_attribution);

    let hints = CalibrationHints {
        context_multi_language: context.multi_language,
    };
    let cal = calibrate(ranked, normalized.visible_chars, hints);
    notes.push(cal.note);

    IdentifyResult {
        candidates: cal.candidates,
        primary_language: cal.primary_language,
        status: cal.status,
        reasons: notes.into_iter().filter(|n| !n.is_empty()).collect(),
        segments,
        normalized_text: normalized.text,
    }
}

fn pick_han_strategy(
    counts: &script::ScriptCounts,
    ortho: &orthography::OrthoSignals,
) -> HanStrategy {
    let kana = counts.kana();
    let hans_m = ortho.hans_markers;
    let hant_m = ortho.hant_markers;

    // Chinese-only markers can outweigh kana when they are the stronger
    // per-character signal — Chinese-dominant text with a small kana
    // embedding shouldn't have every kanji attributed to Japanese.
    if hans_m > kana && hant_m == 0 {
        return HanStrategy::HansPreferred;
    }
    if hant_m > kana && hans_m == 0 {
        return HanStrategy::HantPreferred;
    }
    if kana > 0 {
        return HanStrategy::JapaneseClaimsHan;
    }
    if hans_m > 0 && hant_m == 0 {
        return HanStrategy::HansPreferred;
    }
    if hant_m > 0 && hans_m == 0 {
        return HanStrategy::HantPreferred;
    }
    HanStrategy::HanAmbiguous
}
