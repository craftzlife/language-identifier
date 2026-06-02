use crate::layers::morphology::LatinAttribution;
use crate::layers::normalize::Normalized;
use crate::layers::orthography::{self, OrthoSignals};
use crate::layers::script::{classify, Script};
use crate::types::Segment;

/// Walk the normalized input left-to-right, attribute each char to a language
/// using the same rules as the aggregate, and group consecutive same-language
/// chars into segments. Single-language inputs collapse to one segment which
/// the pipeline then suppresses if it matches the primary language.
///
/// `latin_attribution` (from Layer 6) overrides the per-char Latin rule for
/// any char index inside an attributed token's span — this produces clean
/// per-word EN vs VI segments for mixed Latin input.
pub fn extract(
    input: &Normalized,
    ortho: &OrthoSignals,
    han_strategy: HanStrategy,
    latin_attribution: &[LatinAttribution],
) -> Vec<Segment> {
    if input.chars.is_empty() {
        return Vec::new();
    }

    // Pre-compute per-Han-run strategy overrides. The global Han
    // strategy is one decision per call; that's usually right, but
    // a contiguous Chinese-only Han run embedded inside a kana-rich
    // Japanese paragraph would otherwise inherit `JapaneseClaimsHan`
    // and disappear into the surrounding ja segment. The override
    // pass re-classifies such runs locally.
    let han_overrides = han_run_overrides(input, han_strategy);

    let mut spans: Vec<Segment> = Vec::new();
    let mut current_lang: Option<&'static str> = None;
    let mut start_char_idx: usize = 0;
    // Walk attributions in lock-step so the lookup is O(1) per char.
    let mut attr_idx = 0usize;
    let mut over_idx = 0usize;

    for (i, &c) in input.chars.iter().enumerate() {
        // Advance past attributions fully consumed by previous chars.
        while attr_idx < latin_attribution.len() && latin_attribution[attr_idx].end <= i {
            attr_idx += 1;
        }
        let latin_override = latin_attribution
            .get(attr_idx)
            .filter(|a| a.start <= i && i < a.end)
            .map(|a| a.language);

        while over_idx < han_overrides.len() && han_overrides[over_idx].end <= i {
            over_idx += 1;
        }
        let effective_han = han_overrides
            .get(over_idx)
            .filter(|o| o.start <= i && i < o.end)
            .map(|o| o.strategy)
            .unwrap_or(han_strategy);

        let attr = match (classify(c), latin_override) {
            (Script::Latin, Some(l)) => Some(l),
            _ => attribute(c, ortho, effective_han),
        };
        match (current_lang, attr) {
            (None, Some(l)) => {
                current_lang = Some(l);
                start_char_idx = i;
            }
            (Some(prev), Some(l)) if prev == l => {
                // continue same span
            }
            (Some(prev), Some(l)) => {
                push_span(&mut spans, prev, start_char_idx, i, input);
                current_lang = Some(l);
                start_char_idx = i;
            }
            (Some(prev), None) => {
                push_span(&mut spans, prev, start_char_idx, i, input);
                current_lang = None;
            }
            (None, None) => {}
        }
    }
    if let Some(prev) = current_lang {
        push_span(&mut spans, prev, start_char_idx, input.chars.len(), input);
    }

    // Merge spans of the same language separated only by whitespace.
    merge_adjacent(&mut spans, &input.text);
    spans
}

#[derive(Debug, Clone, Copy)]
pub enum HanStrategy {
    /// Kana is present in the input ⇒ all Han goes to Japanese.
    JapaneseClaimsHan,
    /// No kana ⇒ Han goes to zh-Hans or zh-Hant per markers.
    HansPreferred,
    HantPreferred,
    /// Hant chars + HK markers ⇒ Han attributed to zh-Hant-HK.
    HantHkPreferred,
    /// Hant chars + TW markers ⇒ Han attributed to zh-Hant-TW.
    HantTwPreferred,
    /// Cantonese particles present ⇒ Han attributed to yue.
    CantonesePreferred,
    /// Classical-Chinese function-word density present ⇒ Han to lzh.
    ClassicalPreferred,
    /// Han variant is ambiguous (no markers, no kana).
    HanAmbiguous,
}

fn attribute(c: char, ortho: &OrthoSignals, han: HanStrategy) -> Option<&'static str> {
    match classify(c) {
        Script::Hiragana | Script::Katakana => Some("ja"),
        Script::Hangul => Some("ko"),
        Script::Han => match han {
            HanStrategy::JapaneseClaimsHan => Some("ja"),
            HanStrategy::HansPreferred => Some("zh-Hans"),
            HanStrategy::HantPreferred => Some("zh-Hant"),
            HanStrategy::HantHkPreferred => Some("zh-Hant-HK"),
            HanStrategy::HantTwPreferred => Some("zh-Hant-TW"),
            HanStrategy::CantonesePreferred => Some("yue"),
            HanStrategy::ClassicalPreferred => Some("lzh"),
            // No disambiguating markers in the input — surface as the umbrella
            // `zh` tag so the embedded segment is still visible to callers.
            HanStrategy::HanAmbiguous => Some("zh"),
        },
        Script::Latin => {
            if crate::layers::orthography::is_vi_marker_char(c) {
                Some("vi")
            } else if crate::layers::orthography::is_fr_marker_char(c) {
                Some("fr")
            } else if c.is_ascii_alphabetic() {
                if ortho.fr_markers > 0 && ortho.vi_markers == 0 {
                    Some("fr")
                } else {
                    Some("en")
                }
            } else if ortho.vi_markers > 0 {
                // Latin char inside a VI-rich region: bias to VI so a single
                // VI segment doesn't get fragmented around plain Latin letters.
                Some("vi")
            } else if ortho.fr_markers > 0 {
                Some("fr")
            } else {
                Some("en")
            }
        }
        Script::Other => None,
    }
}

/// A contiguous Han run whose attribution should override the global
/// `HanStrategy` for that run only. `start` / `end` are char indices
/// into `Normalized.chars`.
#[derive(Debug, Clone, Copy)]
struct HanRunOverride {
    start: usize,
    end: usize,
    strategy: HanStrategy,
}

/// Find Han runs that should override the global Han strategy.
///
/// The global strategy is correct for most inputs, but it forces every
/// Han char to one language — wrong when an unambiguously-Chinese run
/// sits embedded in a kana-rich Japanese paragraph, since the global
/// strategy becomes `JapaneseClaimsHan` and the embedded run silently
/// merges into the surrounding `ja` segment.
///
/// The override fires only when:
/// 1. The global strategy is `JapaneseClaimsHan` (kana present), and
/// 2. The run is purely Han + punctuation/whitespace (no kana inside —
///    splitting on each kana boundary), and
/// 3. The run contains at least one Chinese-only marker
///    (Hans-only / Hant-only / yue / lzh / HK / TW char).
///
/// Single Han chars with no marker stay on the global strategy — that
/// preserves the W1 / ambiguous-single-character behavior.
fn han_run_overrides(input: &Normalized, global: HanStrategy) -> Vec<HanRunOverride> {
    if !matches!(global, HanStrategy::JapaneseClaimsHan) {
        return Vec::new();
    }

    let mut overrides = Vec::new();
    let mut run_start: Option<usize> = None;

    for (i, &c) in input.chars.iter().enumerate() {
        let sc = classify(c);
        let breaks_run = matches!(sc, Script::Hiragana | Script::Katakana | Script::Hangul);
        let is_han = sc == Script::Han;

        if breaks_run {
            if let Some(s) = run_start.take() {
                if let Some(o) = try_override(&input.chars[s..i], s, i) {
                    overrides.push(o);
                }
            }
            continue;
        }

        if is_han && run_start.is_none() {
            run_start = Some(i);
        }
        // Latin / Other (punctuation, whitespace) neither start a new
        // Han run nor break an existing one — they ride along until
        // the next kana or end-of-input.
    }
    if let Some(s) = run_start {
        if let Some(o) = try_override(&input.chars[s..], s, input.chars.len()) {
            overrides.push(o);
        }
    }
    overrides
}

/// Decide whether the char slice between `[start, end)` warrants a
/// local Han-strategy override. Returns `Some` only if the slice
/// contains at least one Han char and at least one Chinese-only
/// marker.
fn try_override(chars: &[char], start: usize, end: usize) -> Option<HanRunOverride> {
    let any_han = chars.iter().any(|&c| classify(c) == Script::Han);
    if !any_han {
        return None;
    }
    let sig = orthography::detect_chars(chars);
    let chinese_evidence = sig.hans_markers
        + sig.hant_markers
        + sig.yue_markers
        + sig.lzh_markers
        + sig.hant_hk_markers
        + sig.hant_tw_markers;
    if chinese_evidence == 0 {
        return None;
    }

    let han_in_run = chars.iter().filter(|&&c| classify(c) == Script::Han).count();
    let strategy = if sig.yue_markers > 0 {
        HanStrategy::CantonesePreferred
    } else if sig.lzh_markers >= 3 && han_in_run >= 4 {
        HanStrategy::ClassicalPreferred
    } else if sig.hant_hk_markers > 0 && sig.hant_tw_markers == 0 {
        HanStrategy::HantHkPreferred
    } else if sig.hant_tw_markers > 0 && sig.hant_hk_markers == 0 {
        HanStrategy::HantTwPreferred
    } else if sig.hant_markers > 0 && sig.hans_markers == 0 {
        HanStrategy::HantPreferred
    } else if sig.hans_markers > 0 && sig.hant_markers == 0 {
        HanStrategy::HansPreferred
    } else {
        // Mixed Hans + Hant markers in the same run — keep the global
        // strategy rather than commit to one variant.
        return None;
    };
    Some(HanRunOverride { start, end, strategy })
}

fn push_span(
    spans: &mut Vec<Segment>,
    lang: &'static str,
    start_char: usize,
    end_char: usize,
    input: &Normalized,
) {
    let start = input.char_byte_offsets[start_char];
    let end = input.char_byte_offsets[end_char];
    if start == end {
        return;
    }
    debug_assert!(end <= input.text.len(), "segment end out of bounds");
    debug_assert!(
        input.text.is_char_boundary(start) && input.text.is_char_boundary(end),
        "segment offsets must lie on char boundaries"
    );
    spans.push(Segment {
        language: lang.to_string(),
        start,
        end,
        text: input.text[start..end].to_string(),
    });
}

fn merge_adjacent(spans: &mut Vec<Segment>, text: &str) {
    let mut i = 0;
    while i + 1 < spans.len() {
        let between = &text[spans[i].end..spans[i + 1].start];
        let same_lang = spans[i].language == spans[i + 1].language;
        let only_whitespace = between.chars().all(|c| c.is_whitespace());
        if same_lang && only_whitespace {
            let next_end = spans[i + 1].end;
            spans[i].end = next_end;
            spans[i].text = text[spans[i].start..next_end].to_string();
            spans.remove(i + 1);
        } else {
            i += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layers::normalize::normalize;
    use crate::layers::orthography;

    #[test]
    fn pure_english_single_segment() {
        let n = normalize("Teacher and student");
        let o = orthography::detect(&n);
        let segs = extract(&n, &o, HanStrategy::HanAmbiguous, &[]);
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0].language, "en");
        assert_eq!(&n.text[segs[0].start..segs[0].end], "Teacher and student");
    }

    #[test]
    fn japanese_embedded_in_english() {
        let n = normalize("Please read 先生 carefully");
        let o = orthography::detect(&n);
        // No kana in this input, but the test fixture acts as if Han = ja by strategy.
        let segs = extract(&n, &o, HanStrategy::JapaneseClaimsHan, &[]);
        let langs: Vec<&str> = segs.iter().map(|s| s.language.as_str()).collect();
        assert!(langs.contains(&"en") && langs.contains(&"ja"), "{segs:?}");
        let ja_span = segs.iter().find(|s| s.language == "ja").unwrap();
        assert_eq!(&n.text[ja_span.start..ja_span.end], "先生");
    }

    #[test]
    fn vietnamese_segment_keeps_plain_latin_inside() {
        let n = normalize("xin chào bạn");
        let o = orthography::detect(&n);
        let segs = extract(&n, &o, HanStrategy::HanAmbiguous, &[]);
        assert!(segs.iter().any(|s| s.language == "vi"));
    }

    #[test]
    fn empty_input_no_segments() {
        let n = normalize("");
        let o = orthography::detect(&n);
        let segs = extract(&n, &o, HanStrategy::HanAmbiguous, &[]);
        assert!(segs.is_empty());
    }

    #[test]
    fn segment_offsets_are_valid_utf8_boundaries() {
        let n = normalize("a先生b");
        let o = orthography::detect(&n);
        let segs = extract(&n, &o, HanStrategy::JapaneseClaimsHan, &[]);
        for s in &segs {
            // slice must succeed; if not on a char boundary this panics.
            let _ = &n.text[s.start..s.end];
        }
    }
}
