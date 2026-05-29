//! Centralized ISO 639 → BCP 47 mapping.
//!
//! Single source of truth for translating the coarse ISO 639 language
//! codes produced by Layer 9 classifiers (e.g. fastText's `lid.176`,
//! which emits labels like `__label__en`, `__label__zh`) into the
//! library's canonical BCP 47 tag set.
//!
//! ## Two-tier pipeline
//!
//! 1. **Tier 1 — fast path.** A static lookup for ISO 639 codes whose
//!    BCP 47 translation is unambiguous (`en`, `vi`, `ja`, `ko`).
//!    Adding a new unambiguous language is a one-line change in
//!    [`tier1_direct`].
//!
//! 2. **Tier 2 — deep analysis.** A router for ambiguous macro
//!    languages whose correct variant depends on the text. Today only
//!    `zh` is routed (Hans vs Hant by Traditional-only character
//!    scan). Future macros — Arabic (`ar` → `ar-SA`/`ar-EG`), Serbian
//!    (`sr` → `sr-Cyrl`/`sr-Latn`), Norwegian (`no` → `nb`/`nn`) —
//!    plug in by adding a match arm in [`tier2_disambiguate`] and a
//!    `resolve_<lang>` helper.
//!
//! Tier 2 receives the **normalized** text — the same NFC-composed,
//! whitespace-collapsed form every other layer sees.
//!
//! ## Canonical tag set
//!
//! The current supported set is `en`, `vi`, `ja`, `ko`, `zh-Hans`,
//! `zh-Hant`. Locale subtags (`en-US`, `vi-VN`, …) are deliberately
//! deferred to a later milestone — when they land, only this module
//! changes; every call site already routes through it.
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
    matches!(code, "en" | "vi" | "ja" | "ko" | "zh")
}

fn tier1_direct(code: &str) -> Option<&'static str> {
    match code {
        "en" => Some("en"),
        "vi" => Some("vi"),
        "ja" => Some("ja"),
        "ko" => Some("ko"),
        _ => None,
    }
}

fn tier2_disambiguate(code: &str, text: &str) -> Option<&'static str> {
    match code {
        "zh" => Some(resolve_zh(text)),
        // Future ambiguous macros plug in here:
        //   "ar" => Some(resolve_ar(text)),
        //   "sr" => Some(resolve_sr(text)),
        //   "no" => Some(resolve_no(text)),
        _ => None,
    }
}

// Pick `zh-Hant` if the text contains any Traditional-only character
// from the Layer 3 orthography table; otherwise default to `zh-Hans`
// (the modern default). lid.176 emits a single `__label__zh` without
// splitting Hans vs Hant — this is the disambiguation hook that
// recovers the variant. Layer 3's marker-based path remains
// authoritative for inputs with explicit Hans-only signals.
fn resolve_zh(text: &str) -> &'static str {
    if text.chars().any(orthography::is_hant_only_char) {
        "zh-Hant"
    } else {
        "zh-Hans"
    }
}

fn strip_label_prefix(s: &str) -> &str {
    s.strip_prefix("__label__").unwrap_or(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tier1_maps_unambiguous_codes() {
        // Tier 1 ignores text — pass empty and arbitrary strings.
        assert_eq!(from_iso639("en", ""), Some("en"));
        assert_eq!(from_iso639("vi", "anything"), Some("vi"));
        assert_eq!(from_iso639("ja", ""), Some("ja"));
        assert_eq!(from_iso639("ko", ""), Some("ko"));
    }

    #[test]
    fn tier2_zh_defaults_to_hans() {
        // No Hant-only character present — default to the modern Hans.
        assert_eq!(from_iso639("zh", "我们今天去学校"), Some("zh-Hans"));
        assert_eq!(from_iso639("zh", ""), Some("zh-Hans"));
        assert_eq!(from_iso639("zh", "Hello world"), Some("zh-Hans"));
    }

    #[test]
    fn tier2_zh_routes_to_hant() {
        // `學` is in the base Hant-only table.
        assert_eq!(from_iso639("zh", "學生"), Some("zh-Hant"));
        // `權` is from the v4.1 extension — guards against the
        // extended table regressing.
        assert_eq!(from_iso639("zh", "權限"), Some("zh-Hant"));
    }

    #[test]
    fn returns_none_for_unsupported_codes() {
        assert_eq!(from_iso639("fr", ""), None);
        assert_eq!(from_iso639("de", ""), None);
        assert_eq!(from_iso639("es", "hola"), None);
        assert_eq!(from_iso639("", ""), None);
        // Already-tagged BCP 47 strings are not ISO 639 codes.
        assert_eq!(from_iso639("zh-cn", ""), None);
    }

    #[test]
    fn tolerates_fasttext_label_prefix() {
        assert_eq!(from_iso639("__label__en", ""), Some("en"));
        assert_eq!(from_iso639("__label__zh", "學生"), Some("zh-Hant"));
        assert_eq!(from_iso639("__label__zh", ""), Some("zh-Hans"));
    }

    #[test]
    fn is_supported_covers_the_full_iso_set() {
        for code in ["en", "vi", "ja", "ko", "zh"] {
            assert!(is_supported_iso639(code), "expected {code} supported");
            assert!(
                is_supported_iso639(&format!("__label__{code}")),
                "expected __label__{code} supported"
            );
        }
    }

    #[test]
    fn is_supported_rejects_unsupported_codes() {
        for code in ["fr", "de", "es", ""] {
            assert!(!is_supported_iso639(code), "expected {code} unsupported");
        }
    }
}
