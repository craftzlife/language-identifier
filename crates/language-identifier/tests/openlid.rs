//! Integration tests for the `OpenLidClassifier` Layer 9 default impl.
//!
//! These tests require the OpenLID v3 model file (~few hundred MB). Set
//! `LANGUAGE_IDENTIFIER_MODEL_PATH=/path/to/openlid-v3.bin` to opt in.
//! When the env var is unset, each test prints a skip line and
//! short-circuits so CI without the model stays green.

#![cfg(feature = "ml-openlid")]

use language_identifier::{
    identify_with, IdentifyOptions, MlClassifier, OpenLidClassifier, Status,
};

fn try_load() -> Option<OpenLidClassifier> {
    let path = match std::env::var("LANGUAGE_IDENTIFIER_MODEL_PATH") {
        Ok(p) => p,
        Err(_) => {
            eprintln!("skip: LANGUAGE_IDENTIFIER_MODEL_PATH unset");
            return None;
        }
    };
    match OpenLidClassifier::builder().model_path(&path).build() {
        Ok(c) => Some(c),
        Err(e) => {
            eprintln!("skip: failed to load model from {path}: {e}");
            None
        }
    }
}

#[test]
fn classifier_is_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<OpenLidClassifier>();
}

#[test]
fn classifier_is_a_dyn_ml_classifier() {
    let Some(c) = try_load() else { return };
    // Smoke check that the trait-object plumbing compiles at the use
    // site, not just at the impl site.
    let _: &dyn MlClassifier = &c;
}

#[test]
fn english_paragraph_keeps_english_primary() {
    let Some(c) = try_load() else { return };
    let opts = IdentifyOptions {
        ml_classifier: Some(&c),
    };
    let r = identify_with("This is an English sentence about cats.", &opts);
    assert_eq!(r.primary_language.as_deref(), Some("en"), "{r:?}");
    assert!(
        r.reasons
            .iter()
            .any(|s| s.contains("Layer 9 (ML classifier)")),
        "expected Layer 9 reason: {:?}",
        r.reasons
    );
}

#[test]
fn vietnamese_paragraph_keeps_vietnamese_primary() {
    let Some(c) = try_load() else { return };
    let opts = IdentifyOptions {
        ml_classifier: Some(&c),
    };
    let r = identify_with("Đây là một câu tiếng Việt về mèo.", &opts);
    assert_eq!(r.primary_language.as_deref(), Some("vi"), "{r:?}");
}

#[test]
fn japanese_paragraph_keeps_japanese_primary() {
    let Some(c) = try_load() else { return };
    let opts = IdentifyOptions {
        ml_classifier: Some(&c),
    };
    let r = identify_with("これは日本語の文です。", &opts);
    assert_eq!(r.primary_language.as_deref(), Some("ja"), "{r:?}");
}

#[test]
fn korean_paragraph_keeps_korean_primary() {
    let Some(c) = try_load() else { return };
    let opts = IdentifyOptions {
        ml_classifier: Some(&c),
    };
    let r = identify_with("이것은 한국어 문장입니다.", &opts);
    assert_eq!(r.primary_language.as_deref(), Some("ko"), "{r:?}");
}

#[test]
fn simplified_chinese_routes_cmn_hans_to_zh_hans() {
    let Some(c) = try_load() else { return };
    let opts = IdentifyOptions {
        ml_classifier: Some(&c),
    };
    let r = identify_with("这是一个中文句子。", &opts);
    // Macro primary is `zh`; fine variant is `zh-Hans`.
    assert_eq!(r.primary_language.as_deref(), Some("zh"), "{r:?}");
    assert_eq!(r.primary_variant.as_deref(), Some("zh-Hans"), "{r:?}");
    let layer9 = r
        .reasons
        .iter()
        .find_map(|s| s.strip_prefix("Layer 9 (ML classifier) — "));
    if let Some(line) = layer9 {
        assert!(
            line.contains("zh-Hans:"),
            "expected Layer 9 to surface zh-Hans: {line}"
        );
        assert!(
            !line.contains("cmn"),
            "raw `cmn_*` labels must not appear in Layer 9 reason: {line}"
        );
    }
}

#[test]
fn traditional_chinese_routes_cmn_hant_to_zh_hant() {
    let Some(c) = try_load() else { return };
    let opts = IdentifyOptions {
        ml_classifier: Some(&c),
    };
    // `學` / `這` / `繁` / `體` — Hant-only markers per Layer 3.
    let r = identify_with("這是繁體中文的句子。", &opts);
    assert_eq!(r.primary_language.as_deref(), Some("zh"), "{r:?}");
    assert_eq!(r.primary_variant.as_deref(), Some("zh-Hant"), "{r:?}");
    let layer9 = r
        .reasons
        .iter()
        .find_map(|s| s.strip_prefix("Layer 9 (ML classifier) — "));
    if let Some(line) = layer9 {
        assert!(
            line.contains("zh-Hant:"),
            "expected Layer 9 to surface zh-Hant: {line}"
        );
    }
}

#[test]
fn french_input_resolves_to_fr_via_openlid() {
    // French is in OpenLID's supported set, so it isn't downgraded —
    // it lands on the full-pipeline `fr` tag.
    let Some(c) = try_load() else { return };
    let opts = IdentifyOptions {
        ml_classifier: Some(&c),
    };
    let r = identify_with("Bonjour le monde", &opts);
    assert_ne!(r.status, Status::Unsupported, "{r:?}");
    assert_eq!(r.primary_language.as_deref(), Some("fr"), "{r:?}");
}

#[test]
fn ml_only_spanish_surfaces_es_candidate() {
    // Spanish has no Tier 1 lexicon / morphology — only Layer 9.
    // The candidate should appear; status may be Ambiguous because the
    // ML bonus is capped (see SOFTWARE_DESIGN.md §6).
    let Some(c) = try_load() else { return };
    let opts = IdentifyOptions {
        ml_classifier: Some(&c),
    };
    let r = identify_with("Hola, ¿cómo estás?", &opts);
    assert!(
        r.candidates.iter().any(|c| c.language == "es"),
        "expected an `es` candidate: {r:?}"
    );
}

#[test]
fn classifier_reused_across_calls() {
    let Some(c) = try_load() else { return };
    let opts = IdentifyOptions {
        ml_classifier: Some(&c),
    };
    let a = identify_with("Hello there.", &opts);
    let b = identify_with("How are you today?", &opts);
    assert_eq!(a.primary_language.as_deref(), Some("en"));
    assert_eq!(b.primary_language.as_deref(), Some("en"));
    assert_eq!(a.status, Status::Resolved);
}
