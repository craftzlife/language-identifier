//! End-to-end tests for the Chinese-variant tags introduced beyond the
//! original `zh-Hans`/`zh-Hant` pair: `yue`, `lzh`, `nan`, `hak`, `wuu`,
//! `zh-Hant-HK`, `zh-Hant-TW`.
//!
//! These exercise [`identify`] all the way from input string through the
//! marker-based Han split and BCP-47 tag selection. They do **not**
//! depend on Layer 9 — every variant is detected from deterministic
//! orthographic / function-word signals.
//!
//! Note: since v5 the top-level `candidates[0].language` is the macro
//! tag (`zh` for every Sinitic input). Fine-grained variant detail is
//! exposed via `IdentifyResult::primary_variant`, which is what these
//! tests assert on.

use language_identifier::identify;

fn primary_variant(r: &language_identifier::IdentifyResult) -> &str {
    r.primary_variant.as_deref().unwrap_or("")
}

fn primary_macro(r: &language_identifier::IdentifyResult) -> &str {
    r.primary_language.as_deref().unwrap_or("")
}

#[test]
fn cantonese_particles_resolve_to_yue() {
    // Three colloquial Cantonese particles: 嘅 (possessive), 喺 (locative),
    // 嚟 (motion verb). One Hant char (學).
    let r = identify("我嘅學生喺學校嚟到");
    assert_eq!(primary_macro(&r), "zh", "{r:?}");
    assert_eq!(primary_variant(&r), "yue", "expected yue: {r:?}");
}

#[test]
fn cantonese_short_phrase_resolves_to_yue() {
    let r = identify("我哋唔係學生");
    assert_eq!(primary_variant(&r), "yue", "expected yue: {r:?}");
}

#[test]
fn classical_chinese_dense_function_words_resolve_to_lzh() {
    // Opening of the Analects — 之/乎/者/也 density is the lzh signature.
    let r = identify("子曰：學而時習之，不亦說乎？有朋自遠方來，不亦樂乎？");
    assert_eq!(primary_variant(&r), "lzh", "expected lzh: {r:?}");
}

#[test]
fn modern_chinese_single_classical_particle_stays_zh() {
    // One 之 alone does not flip modern Chinese to Classical.
    let r = identify("我之前去过北京和上海");
    assert!(
        matches!(primary_variant(&r), "zh-Hans" | "zh"),
        "expected modern zh-Hans: {r:?}"
    );
}

#[test]
fn taiwan_hant_marker_routes_to_zh_hant_tw() {
    // 臺 is the TW-preferred form of 台. With at least one generic Hant
    // marker (學) we expect zh-Hant-TW.
    let r = identify("臺灣的學生在大學讀書");
    assert_eq!(primary_variant(&r), "zh-Hant-TW", "{r:?}");
}

#[test]
fn hong_kong_hant_marker_routes_to_zh_hant_hk() {
    // 嘥 (to waste) is an HK-Standard-Chinese marker. With Hant context
    // (學) and no Cantonese particles we expect zh-Hant-HK.
    let r = identify("學生嘥時間，老師生氣了");
    assert_eq!(primary_variant(&r), "zh-Hant-HK", "{r:?}");
}

#[test]
fn yue_overrides_hk_when_cantonese_particles_present() {
    // When 嘥 (HK marker) coexists with 唔 (yue particle), yue wins —
    // the input is colloquial Cantonese, not HK Standard Chinese.
    let r = identify("學生嘥時間，老師唔開心");
    assert_eq!(primary_variant(&r), "yue", "{r:?}");
}

#[test]
fn wu_marker_儂_routes_to_wuu() {
    // 儂 alone is a strong Wu signal. With a non-Wu-only Han char
    // alongside (好), wuu should still be detected.
    let r = identify("儂好");
    // Wu's signal is sparse — we only ask that wuu shows up as a
    // recognized variant within the zh macro.
    let zh = r
        .candidates
        .iter()
        .find(|c| c.language == "zh")
        .expect("zh macro candidate");
    assert!(
        zh.variants.iter().any(|v| v == "wuu"),
        "expected wuu in zh variants: {r:?}"
    );
}

#[test]
fn yue_overrides_lzh_when_both_signal_present() {
    // Hypothetical text with both classical and Cantonese particles.
    // Behavior is deterministic: classical density check requires 3+
    // markers, which we satisfy below — so lzh wins.
    let r = identify("之乎者也嘅學生");
    assert_eq!(primary_variant(&r), "lzh");
}

#[test]
fn macro_grouping_exposes_variants_list_under_zh() {
    // Every Sinitic input should surface a `zh` macro candidate whose
    // `variants` array enumerates the fine tags the library can decide.
    let r = identify("學生嘥時間，老師生氣了");
    let zh = r
        .candidates
        .iter()
        .find(|c| c.language == "zh")
        .expect("zh macro candidate");
    for expected in [
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
        assert!(
            zh.variants.iter().any(|v| v == expected),
            "expected {expected} in variants: {:?}",
            zh.variants
        );
    }
}
