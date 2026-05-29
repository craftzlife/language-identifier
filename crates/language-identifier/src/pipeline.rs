use crate::aggregate::{aggregate, AggregateInput};
use crate::calibration::{calibrate, CalibrationHints};
use crate::layers::{
    context_window, dictionary, function_words, morphology, ngram, normalize, orthography, script,
};
use crate::segments::{self, HanStrategy};
use crate::types::{IdentifyResult, Status};

pub fn run(input: &str) -> IdentifyResult {
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
        notes.push("Japanese kana present — Han characters interpreted as kanji".into());
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
            format!(" ({} Latin tokens attributed)", morph.latin_attribution.len())
        };
        notes.push(format!(
            "Morphology — {}{}",
            parts.join(", "),
            attr_summary
        ));
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

    let ranked = aggregate(AggregateInput {
        counts: &counts,
        ortho: &ortho,
        ngram: &ngram_sig,
        function_words: &fw,
        dictionary: &dict,
        morphology: &morph,
        context: &context,
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

fn pick_han_strategy(counts: &script::ScriptCounts, ortho: &orthography::OrthoSignals) -> HanStrategy {
    if counts.kana() > 0 {
        HanStrategy::JapaneseClaimsHan
    } else if ortho.hans_markers > 0 && ortho.hant_markers == 0 {
        HanStrategy::HansPreferred
    } else if ortho.hant_markers > 0 && ortho.hans_markers == 0 {
        HanStrategy::HantPreferred
    } else {
        HanStrategy::HanAmbiguous
    }
}

