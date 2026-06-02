//! Canonical BCP 47 tag set + Sinitic Tier-2 disambiguation.
//!
//! ## Canonical tag set
//!
//! The library's full-pipeline tags are `en`, `fr`, `vi`, `ja`, `ko`,
//! `zh-Hans`, `zh-Hant`, `zh-Hant-HK`, `zh-Hant-TW`, `yue` (Cantonese),
//! `lzh` (Classical / Literary Chinese), `nan` (Min Nan), `hak` (Hakka),
//! `wuu` (Wu).
//!
//! `zh-Hant-HK` and `zh-Hant-TW` are the **only** locale subtags the
//! library emits — they are a deliberate narrow exception to the
//! otherwise bare-tag rule (`en` not `en-US`, `vi` not `vi-VN`). The
//! exception exists because Hong Kong and Taiwan traditional writing
//! differ in vocabulary and a handful of region-specific characters in
//! ways callers reliably need to distinguish.
//!
//! Layer 9 also surfaces ~180 additional ML-only tags routed through
//! [`crate::openlid`]; they are passed through `macro_of` as their own
//! macro (identity).
//!
//! ## `resolve_zh`
//!
//! [`resolve_zh`] is the Sinitic Tier-2 router. It is called from
//! [`crate::openlid::from_openlid_label`] when OpenLID emits `cmn_Hans`
//! or `cmn_Hant` — the model's coarse Mandarin label is then promoted
//! to the most specific BCP 47 tag the text supports (yue / lzh /
//! HK / TW / Hant / Hans).

use crate::layers::orthography;

// `zh` arrives from coarse classifiers (OpenLID emits `cmn_Hans` /
// `cmn_Hant` without HK/TW refinement and may mis-label written
// Cantonese as Mandarin). This router escalates to the most specific
// BCP 47 tag the input supports.
//
// Order matters:
//   1. Cantonese particles (嘅/嚟/唔/喺/啲/咁/哋) override everything —
//      these characters are diagnostic of written Cantonese.
//   2. Classical Chinese function-word density (之/乎/者/也/矣/焉/而/
//      於/以/哉) — fires only when the markers are dense enough that
//      the text is plausibly Classical rather than modern.
//   3. Hant-only chars present → route to zh-Hant, with HK/TW refinement
//      when the region markers fire.
//   4. Default → zh-Hans (modern simplified).
#[cfg_attr(not(feature = "ml-openlid"), allow(dead_code))]
pub(crate) fn resolve_zh(text: &str) -> &'static str {
    if orthography::has_yue_marker(text) {
        return "yue";
    }
    if orthography::is_classical_chinese(text) {
        return "lzh";
    }
    let any_hant = text.chars().any(orthography::is_hant_only_char);
    if any_hant {
        let hk = text.chars().any(orthography::is_hant_hk_marker_char);
        let tw = text.chars().any(orthography::is_hant_tw_marker_char);
        if hk && !tw {
            return "zh-Hant-HK";
        }
        if tw && !hk {
            return "zh-Hant-TW";
        }
        return "zh-Hant";
    }
    "zh-Hans"
}

/// Map a fine-grained BCP 47 tag to its consumer-facing macro language.
///
/// The library's internal pipeline tracks fine variants (`zh-Hant-HK`,
/// `yue`, `lzh`, …) so apps that care about the distinction can drill
/// down. For dictionary / language-learning UIs that only need the
/// big-picture language, [`macro_of`] collapses the variants into the
/// macro tag that gets returned in [`crate::IdentifyResult::candidates`].
///
/// Mapping:
/// - All Sinitic variants — `zh`, `zh-Hans`, `zh-Hant`, `zh-Hant-HK`,
///   `zh-Hant-TW`, `yue` (Cantonese), `lzh` (Classical),
///   `nan` (Min Nan), `hak` (Hakka), `wuu` (Wu) — collapse to `zh`.
///   This is a deliberate UX call: `yue`/`lzh`/`nan`/`hak`/`wuu` are
///   technically separate ISO 639-3 languages, but for dictionary-app
///   consumers they're surfaced under the `zh` macro alongside
///   Hans/Hant. The fine-grained tag is still available on
///   [`crate::IdentifyResult::primary_variant`] and
///   [`crate::Candidate::variants`].
/// - Every other supported tag (`en`, `fr`, `vi`, `ja`, `ko`, plus all
///   Layer 9 ML-only tags) is its own macro.
/// - Unknown tags pass through unchanged.
pub fn macro_of(tag: &str) -> &str {
    match tag {
        "zh" | "zh-Hans" | "zh-Hant" | "zh-Hant-HK" | "zh-Hant-TW" | "yue" | "lzh" | "nan"
        | "hak" | "wuu" => "zh",
        other => other,
    }
}

/// The canonical fine-grained tags the library knows how to emit under
/// a given macro language. Used to populate
/// [`crate::Candidate::variants`] so consumers can see *which* fine
/// tags a macro group represents — even before they show up in any
/// specific input.
///
/// Returns `&[]` for macros without enumerated variants — callers
/// should treat that as "the macro tag itself is the implicit single
/// variant" (this is how Layer 9 ML-only tags work without each
/// needing their own arm).
pub fn variants_of(macro_tag: &str) -> &'static [&'static str] {
    match macro_tag {
        "zh" => &[
            "zh-Hans",
            "zh-Hant",
            "zh-Hant-HK",
            "zh-Hant-TW",
            "yue",
            "lzh",
            "nan",
            "hak",
            "wuu",
        ],
        "en" => &["en"],
        "fr" => &["fr"],
        "vi" => &["vi"],
        "ja" => &["ja"],
        "ko" => &["ko"],
        _ => &[],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_zh_defaults_to_hans() {
        assert_eq!(resolve_zh("我们今天去学校"), "zh-Hans");
        assert_eq!(resolve_zh(""), "zh-Hans");
        assert_eq!(resolve_zh("Hello world"), "zh-Hans");
    }

    #[test]
    fn resolve_zh_routes_to_hant() {
        assert_eq!(resolve_zh("學生"), "zh-Hant");
        assert_eq!(resolve_zh("權限"), "zh-Hant");
    }

    #[test]
    fn resolve_zh_routes_to_yue_on_cantonese_markers() {
        // 嘅 is a Cantonese-only possessive particle.
        assert_eq!(resolve_zh("我嘅學生"), "yue");
        // 喺 (at/in) — Cantonese only.
        assert_eq!(resolve_zh("佢喺學校"), "yue");
    }

    #[test]
    fn resolve_zh_routes_to_lzh_on_classical_density() {
        // Dense classical function-word run.
        assert_eq!(resolve_zh("子曰：學而時習之，不亦說乎？"), "lzh");
    }

    #[test]
    fn resolve_zh_routes_to_hant_hk() {
        // 嘥 / 冚 / 嘜 — HK-specific written-Chinese characters.
        assert_eq!(resolve_zh("學生嘥時間"), "zh-Hant-HK");
    }

    #[test]
    fn resolve_zh_routes_to_hant_tw() {
        // 臺 — the TW-preferred form of 台. Combined with a Hant char.
        assert_eq!(resolve_zh("臺灣的學生"), "zh-Hant-TW");
    }

    #[test]
    fn macro_of_collapses_sinitic_variants_to_zh() {
        for fine in [
            "zh",
            "zh-Hans",
            "zh-Hant",
            "zh-Hant-HK",
            "zh-Hant-TW",
            "yue",
            "lzh",
            "nan",
            "hak",
            "wuu",
        ] {
            assert_eq!(macro_of(fine), "zh", "{fine} should collapse to zh");
        }
    }

    #[test]
    fn macro_of_is_identity_for_other_supported_tags() {
        for tag in ["en", "fr", "vi", "ja", "ko"] {
            assert_eq!(macro_of(tag), tag);
        }
    }

    #[test]
    fn macro_of_passes_unknown_tags_through() {
        assert_eq!(macro_of("de"), "de");
        assert_eq!(macro_of("es"), "es");
        assert_eq!(macro_of(""), "");
    }

    #[test]
    fn variants_of_zh_lists_every_fine_tag() {
        let v = variants_of("zh");
        for fine in [
            "zh-Hans",
            "zh-Hant",
            "zh-Hant-HK",
            "zh-Hant-TW",
            "yue",
            "lzh",
            "nan",
            "hak",
            "wuu",
        ] {
            assert!(v.contains(&fine), "variants_of(\"zh\") missing {fine}");
        }
    }

    #[test]
    fn variants_of_single_macro_languages_returns_self() {
        for tag in ["en", "fr", "vi", "ja", "ko"] {
            assert_eq!(variants_of(tag), &[tag]);
        }
    }

    #[test]
    fn variants_of_unknown_macro_is_empty() {
        // Callers treat an empty slice as "the macro is its own
        // implicit variant" — this keeps the table small while still
        // letting Layer 9 ML-only tags participate.
        assert_eq!(variants_of("de"), &[] as &[&str]);
    }
}
