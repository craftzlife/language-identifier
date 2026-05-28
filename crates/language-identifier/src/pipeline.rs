use crate::aggregate::{aggregate, AggregateInput};
use crate::calibration::calibrate;
use crate::layers::{dictionary, function_words, ngram, normalize, orthography, script};
use crate::segments::{self, HanStrategy};
use crate::types::{IdentifyResult, Reason, Segment, Status};

pub fn run(input: &str) -> IdentifyResult {
    let normalized = normalize::normalize(input);
    if normalized.visible_chars == 0 {
        return IdentifyResult {
            candidates: vec![],
            primary_language: None,
            status: Status::Unknown,
            reason: Reason::Single("Empty input after normalization".into()),
            segments: vec![],
        };
    }

    let counts = script::count_scripts(&normalized);
    let ortho = orthography::detect(&normalized);
    let ngram_sig = ngram::score(&normalized);
    let fw = function_words::score(&normalized);
    let dict = dictionary::score(&normalized);

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

    let ranked = aggregate(AggregateInput {
        counts: &counts,
        ortho: &ortho,
        ngram: &ngram_sig,
        function_words: &fw,
        dictionary: &dict,
    });

    let unsupported_only = counts.supported_total() == 0 && counts.other > 0;
    if unsupported_only {
        return IdentifyResult {
            candidates: vec![],
            primary_language: None,
            status: Status::Unsupported,
            reason: Reason::Single("Input uses scripts that are not in the supported set".into()),
            segments: vec![],
        };
    }

    let han_strategy = pick_han_strategy(&counts, &ortho);
    let raw_segments = segments::extract(&normalized, &ortho, han_strategy);

    let cal = calibrate(ranked, normalized.visible_chars);
    notes.push(cal.note);

    let presented_segments = present_segments(raw_segments, cal.primary_language.as_deref());

    IdentifyResult {
        candidates: cal.candidates,
        primary_language: cal.primary_language,
        status: cal.status,
        reason: Reason::from_notes(notes),
        segments: presented_segments,
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

/// Suppress the segment list when every span matches the primary language —
/// no embedded content worth surfacing. Otherwise return as-is.
fn present_segments(segs: Vec<Segment>, primary: Option<&str>) -> Vec<Segment> {
    if segs.is_empty() {
        return segs;
    }
    if let Some(p) = primary {
        if segs.iter().all(|s| s.language == p) {
            return Vec::new();
        }
    }
    segs
}
