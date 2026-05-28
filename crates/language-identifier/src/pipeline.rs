use crate::aggregate::{aggregate, AggregateInput};
use crate::calibration::calibrate;
use crate::layers::{ngram, normalize, orthography, script};
use crate::types::{IdentifyResult, Reason, Status};

pub fn run(input: &str) -> IdentifyResult {
    let normalized = normalize::normalize(input);
    if normalized.visible_chars == 0 {
        return IdentifyResult {
            candidates: vec![],
            primary_language: None,
            status: Status::Unknown,
            reason: Reason::Single("Empty input after normalization".into()),
        };
    }

    let counts = script::count_scripts(&normalized);
    let ortho = orthography::detect(&normalized);
    let ngram_sig = ngram::score(&normalized);

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

    let ranked = aggregate(AggregateInput {
        counts: &counts,
        ortho: &ortho,
        ngram: &ngram_sig,
    });

    let unsupported_only = counts.supported_total() == 0 && counts.other > 0;
    if unsupported_only {
        return IdentifyResult {
            candidates: vec![],
            primary_language: None,
            status: Status::Unsupported,
            reason: Reason::Single(
                "Input uses scripts that are not in the supported set".into(),
            ),
        };
    }

    let cal = calibrate(ranked, normalized.visible_chars);
    notes.push(cal.note);

    IdentifyResult {
        candidates: cal.candidates,
        primary_language: cal.primary_language,
        status: cal.status,
        reason: Reason::from_notes(notes),
    }
}
