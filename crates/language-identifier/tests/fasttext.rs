//! Integration tests for the `FastTextClassifier` Layer 9 default impl.
//!
//! These tests require the `lid.176.bin` model file (~126 MB). Set
//! `LANGUAGE_IDENTIFIER_MODEL_PATH=/path/to/lid.176.bin` to opt in.
//! When the env var is unset, each test prints a skip line and
//! short-circuits so CI without the model stays green.

#![cfg(feature = "ml-fasttext")]

use language_identifier::{
    identify_with, FastTextClassifier, IdentifyOptions, MlClassifier, Status,
};

fn try_load() -> Option<FastTextClassifier> {
    let path = match std::env::var("LANGUAGE_IDENTIFIER_MODEL_PATH") {
        Ok(p) => p,
        Err(_) => {
            eprintln!("skip: LANGUAGE_IDENTIFIER_MODEL_PATH unset");
            return None;
        }
    };
    match FastTextClassifier::builder().model_path(&path).build() {
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
    assert_send_sync::<FastTextClassifier>();
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
        llm_resolver: None,
    };
    let r = identify_with("This is an English sentence about cats.", &opts);
    assert_eq!(r.primary_language.as_deref(), Some("en-US"), "{r:?}");
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
        llm_resolver: None,
    };
    let r = identify_with("Đây là một câu tiếng Việt về mèo.", &opts);
    assert_eq!(r.primary_language.as_deref(), Some("vi-VN"), "{r:?}");
}

#[test]
fn japanese_paragraph_keeps_japanese_primary() {
    let Some(c) = try_load() else { return };
    let opts = IdentifyOptions {
        ml_classifier: Some(&c),
        llm_resolver: None,
    };
    let r = identify_with("これは日本語の文です。", &opts);
    assert_eq!(r.primary_language.as_deref(), Some("ja"), "{r:?}");
}

#[test]
fn korean_paragraph_keeps_korean_primary() {
    let Some(c) = try_load() else { return };
    let opts = IdentifyOptions {
        ml_classifier: Some(&c),
        llm_resolver: None,
    };
    let r = identify_with("이것은 한국어 문장입니다.", &opts);
    assert_eq!(r.primary_language.as_deref(), Some("ko"), "{r:?}");
}

#[test]
fn chinese_input_drops_zh_label_from_layer9_reason() {
    let Some(c) = try_load() else { return };
    let opts = IdentifyOptions {
        ml_classifier: Some(&c),
        llm_resolver: None,
    };
    let r = identify_with("这是一个中文句子。", &opts);
    // Hans/Hant disambiguation is Layer 3's job; the deterministic
    // path still produces a primary language.
    let primary = r.primary_language.as_deref().unwrap_or("");
    assert!(
        matches!(primary, "zh-Hans" | "zh-Hant" | "zh"),
        "expected a Han-variant primary, got: {primary}"
    );
    // L9 must not surface `zh:...` because we drop the `zh` label
    // before aggregation. The Layer 9 reason line should mention only
    // the supported BCP 47 tags (or be absent if the model returned
    // nothing in our supported set).
    for r in &r.reasons {
        if let Some(rest) = r.strip_prefix("Layer 9 (ML classifier) — ") {
            assert!(
                !rest.contains("zh:"),
                "Layer 9 reason should not contain a bare `zh:` entry: {rest}"
            );
        }
    }
}

#[test]
fn french_input_downgrades_to_unsupported() {
    let Some(c) = try_load() else { return };
    let opts = IdentifyOptions {
        ml_classifier: Some(&c),
        llm_resolver: None,
    };
    let r = identify_with("Bonjour le monde", &opts);
    assert_eq!(r.status, Status::Unsupported, "{r:?}");
    assert!(r.primary_language.is_none());
    assert!(r.candidates.is_empty());
    assert!(
        r.reasons
            .iter()
            .any(|s| s.contains("unsupported language 'fr'")),
        "expected 'fr' downgrade reason, got: {:?}",
        r.reasons
    );
}

#[test]
fn chinese_input_is_not_downgraded_despite_zh_drop_in_classify() {
    // `zh` is dropped from classify() (variant disambiguation belongs
    // to Layer 3) but is still in the supported set — the unsupported
    // signal must therefore stay quiet on pure-Han inputs.
    let Some(c) = try_load() else { return };
    let opts = IdentifyOptions {
        ml_classifier: Some(&c),
        llm_resolver: None,
    };
    let r = identify_with("这是一个中文句子。", &opts);
    assert_ne!(r.status, Status::Unsupported, "{r:?}");
    assert!(r.primary_language.is_some());
}

#[test]
fn classifier_reused_across_calls() {
    let Some(c) = try_load() else { return };
    let opts = IdentifyOptions {
        ml_classifier: Some(&c),
        llm_resolver: None,
    };
    let a = identify_with("Hello there.", &opts);
    let b = identify_with("How are you today?", &opts);
    assert_eq!(a.primary_language.as_deref(), Some("en-US"));
    assert_eq!(b.primary_language.as_deref(), Some("en-US"));
    assert_eq!(a.status, Status::Resolved);
}
