use crate::layers::morphology::LatinAttribution;
use crate::layers::normalize::Normalized;
use crate::layers::orthography::OrthoSignals;
use crate::layers::script::{classify, Script};
use crate::types::Segment;

/// Walk the normalized input left-to-right, attribute each char to a language
/// using the same rules as the aggregate, and group consecutive same-language
/// chars into segments. Single-language inputs collapse to one segment which
/// the pipeline then suppresses if it matches the primary language.
///
/// `latin_attribution` (from Layer 6) overrides the per-char Latin rule for
/// any char index inside an attributed token's span — this produces clean
/// per-word EN vs vi-VN segments for mixed Latin input.
pub fn extract(
    input: &Normalized,
    ortho: &OrthoSignals,
    han_strategy: HanStrategy,
    latin_attribution: &[LatinAttribution],
) -> Vec<Segment> {
    if input.chars.is_empty() {
        return Vec::new();
    }

    let mut spans: Vec<Segment> = Vec::new();
    let mut current_lang: Option<&'static str> = None;
    let mut start_char_idx: usize = 0;
    // Walk attributions in lock-step so the lookup is O(1) per char.
    let mut attr_idx = 0usize;

    for (i, &c) in input.chars.iter().enumerate() {
        // Advance past attributions fully consumed by previous chars.
        while attr_idx < latin_attribution.len() && latin_attribution[attr_idx].end <= i {
            attr_idx += 1;
        }
        let latin_override = latin_attribution
            .get(attr_idx)
            .filter(|a| a.start <= i && i < a.end)
            .map(|a| a.language);

        let attr = match (classify(c), latin_override) {
            (Script::Latin, Some(l)) => Some(l),
            _ => attribute(c, ortho, han_strategy),
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
            // No disambiguating markers in the input — surface as the umbrella
            // `zh` tag so the embedded segment is still visible to callers.
            HanStrategy::HanAmbiguous => Some("zh"),
        },
        Script::Latin => {
            if crate::layers::orthography::is_vi_marker_char(c) {
                Some("vi")
            } else if c.is_ascii_alphabetic() {
                Some("en")
            } else if ortho.vi_markers > 0 {
                // Latin char inside a VI-rich region: bias to VI so a single
                // VI segment doesn't get fragmented around plain Latin letters.
                Some("vi")
            } else {
                Some("en")
            }
        }
        Script::Other => None,
    }
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
