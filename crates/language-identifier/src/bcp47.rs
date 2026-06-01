//! Centralized ISO 639 → BCP 47 mapping.
//!
//! Single source of truth for translating the coarse ISO 639 language
//! codes produced by Layer 9 classifiers (e.g. fastText's `lid.176`,
//! which emits labels like `__label__en`, `__label__zh`, `__label__yue`)
//! into the library's canonical BCP 47 tag set.
//!
//! ## Two-tier pipeline
//!
//! 1. **Tier 1 — fast path.** A static lookup for ISO 639 codes whose
//!    BCP 47 translation is unambiguous (`en`, `vi`, `ja`, `ko`, plus
//!    the Sinitic siblings `yue`, `lzh`, `nan`, `hak`, `wuu`). Adding a
//!    new unambiguous language is a one-line change in [`tier1_direct`].
//!
//! 2. **Tier 2 — deep analysis.** A router for ambiguous macro
//!    languages whose correct variant depends on the text. Today only
//!    `zh` is routed; the router inspects the input and may escalate to
//!    one of `yue`, `lzh`, `zh-Hant-HK`, `zh-Hant-TW`, `zh-Hant`, or
//!    `zh-Hans`. Future macros — Arabic (`ar` → `ar-SA`/`ar-EG`),
//!    Serbian (`sr` → `sr-Cyrl`/`sr-Latn`), Norwegian (`no` → `nb`/`nn`)
//!    — plug in by adding a match arm in [`tier2_disambiguate`] and a
//!    `resolve_<lang>` helper.
//!
//! Tier 2 receives the **normalized** text — the same NFC-composed,
//! whitespace-collapsed form every other layer sees.
//!
//! ## Canonical tag set
//!
//! The supported set is `en`, `fr`, `vi`, `ja`, `ko`, `zh-Hans`,
//! `zh-Hant`, `zh-Hant-HK`, `zh-Hant-TW`, `yue` (Cantonese), `lzh`
//! (Classical / Literary Chinese), `nan` (Min Nan), `hak` (Hakka),
//! `wuu` (Wu).
//!
//! `zh-Hant-HK` and `zh-Hant-TW` are the **only** locale subtags the
//! library emits — they are a deliberate narrow exception to the
//! otherwise bare-tag rule (`en` not `en-US`, `vi` not `vi-VN`). The
//! exception exists because Hong Kong and Taiwan traditional writing
//! differ in vocabulary and a handful of region-specific characters in
//! ways callers reliably need to distinguish.
//!
//! ## `__label__` prefix
//!
//! Both [`from_iso639`] and [`is_supported_iso639`] tolerate fastText's
//! `__label__` prefix so callers can pass model output verbatim.

use crate::layers::orthography;

/// Map an ISO 639 language code to the library's canonical BCP 47 tag.
///
/// `text` is the normalized input the classifier saw. It is consulted
/// only by Tier 2 resolvers — Tier 1 mappings ignore it.
///
/// Returns `None` for codes outside the supported set.
pub fn from_iso639(iso: &str, text: &str) -> Option<&'static str> {
    let code = strip_label_prefix(iso);
    tier1_direct(code).or_else(|| tier2_disambiguate(code, text))
}

/// Whether `iso` is in the library's supported set — i.e. whether
/// [`from_iso639`] would return `Some` for some input.
///
/// Used by `MlClassifier::unsupported_signal` impls to decide when the
/// classifier's top label warrants short-circuiting the pipeline to
/// [`crate::Status::Unsupported`].
pub fn is_supported_iso639(iso: &str) -> bool {
    let code = strip_label_prefix(iso);
    matches!(
        code,
        "en" | "fr" | "vi" | "ja" | "ko" | "zh" | "yue" | "lzh" | "nan" | "hak" | "wuu"
    )
}

fn tier1_direct(code: &str) -> Option<&'static str> {
    match code {
        "en" => Some("en"),
        "fr" => Some("fr"),
        "vi" => Some("vi"),
        "ja" => Some("ja"),
        "ko" => Some("ko"),
        "yue" => Some("yue"),
        "lzh" => Some("lzh"),
        "nan" => Some("nan"),
        "hak" => Some("hak"),
        "wuu" => Some("wuu"),
        _ => None,
    }
}

fn tier2_disambiguate(code: &str, text: &str) -> Option<&'static str> {
    match code {
        "zh" => Some(resolve_zh(text)),
        _ => None,
    }
}

// `zh` arrives from coarse classifiers (lid.176 emits a single
// `__label__zh` without splitting variants). This router escalates to
// the most specific BCP 47 tag the input supports.
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
fn resolve_zh(text: &str) -> &'static str {
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

fn strip_label_prefix(s: &str) -> &str {
    s.strip_prefix("__label__").unwrap_or(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tier1_maps_unambiguous_codes() {
        assert_eq!(from_iso639("en", ""), Some("en"));
        assert_eq!(from_iso639("fr", "bonjour"), Some("fr"));
        assert_eq!(from_iso639("vi", "anything"), Some("vi"));
        assert_eq!(from_iso639("ja", ""), Some("ja"));
        assert_eq!(from_iso639("ko", ""), Some("ko"));
    }

    #[test]
    fn tier1_maps_sinitic_siblings() {
        assert_eq!(from_iso639("yue", ""), Some("yue"));
        assert_eq!(from_iso639("lzh", ""), Some("lzh"));
        assert_eq!(from_iso639("nan", ""), Some("nan"));
        assert_eq!(from_iso639("hak", ""), Some("hak"));
        assert_eq!(from_iso639("wuu", ""), Some("wuu"));
    }

    #[test]
    fn tier2_zh_defaults_to_hans() {
        assert_eq!(from_iso639("zh", "我们今天去学校"), Some("zh-Hans"));
        assert_eq!(from_iso639("zh", ""), Some("zh-Hans"));
        assert_eq!(from_iso639("zh", "Hello world"), Some("zh-Hans"));
    }

    #[test]
    fn tier2_zh_routes_to_hant() {
        assert_eq!(from_iso639("zh", "學生"), Some("zh-Hant"));
        assert_eq!(from_iso639("zh", "權限"), Some("zh-Hant"));
    }

    #[test]
    fn tier2_zh_routes_to_yue_on_cantonese_markers() {
        // 嘅 is a Cantonese-only possessive particle.
        assert_eq!(from_iso639("zh", "我嘅學生"), Some("yue"));
        // 喺 (at/in) — Cantonese only.
        assert_eq!(from_iso639("zh", "佢喺學校"), Some("yue"));
    }

    #[test]
    fn tier2_zh_routes_to_lzh_on_classical_density() {
        // Dense classical function-word run.
        assert_eq!(from_iso639("zh", "子曰：學而時習之，不亦說乎？"), Some("lzh"));
    }

    #[test]
    fn tier2_zh_routes_to_hant_hk() {
        // 嘥 / 冚 / 嘜 — HK-specific written-Chinese characters.
        // Use a Hant char + an HK-specific marker.
        assert_eq!(from_iso639("zh", "學生嘥時間"), Some("zh-Hant-HK"));
    }

    #[test]
    fn tier2_zh_routes_to_hant_tw() {
        // 臺 — the TW-preferred form of 台. Combined with a Hant char.
        assert_eq!(from_iso639("zh", "臺灣的學生"), Some("zh-Hant-TW"));
    }

    #[test]
    fn returns_none_for_unsupported_codes() {
        assert_eq!(from_iso639("de", ""), None);
        assert_eq!(from_iso639("es", "hola"), None);
        assert_eq!(from_iso639("it", ""), None);
        assert_eq!(from_iso639("", ""), None);
        assert_eq!(from_iso639("zh-cn", ""), None);
    }

    #[test]
    fn tolerates_fasttext_label_prefix() {
        assert_eq!(from_iso639("__label__en", ""), Some("en"));
        assert_eq!(from_iso639("__label__yue", ""), Some("yue"));
        assert_eq!(from_iso639("__label__zh", "學生"), Some("zh-Hant"));
        assert_eq!(from_iso639("__label__zh", ""), Some("zh-Hans"));
    }

    #[test]
    fn is_supported_covers_the_full_iso_set() {
        for code in ["en", "fr", "vi", "ja", "ko", "zh", "yue", "lzh", "nan", "hak", "wuu"] {
            assert!(is_supported_iso639(code), "expected {code} supported");
            assert!(
                is_supported_iso639(&format!("__label__{code}")),
                "expected __label__{code} supported"
            );
        }
    }

    #[test]
    fn is_supported_rejects_unsupported_codes() {
        for code in ["de", "es", "it", "ru", ""] {
            assert!(!is_supported_iso639(code), "expected {code} unsupported");
        }
    }
}
