//! End-to-end integration test for the Apple FoundationModels resolver.
//!
//! The test actually calls the on-device system model, which only works
//! on a machine where Apple Intelligence is enabled and the model
//! assets are downloaded. Set `LID_RUN_LLM_TESTS=1` to opt in;
//! otherwise the test prints why it skipped and passes vacuously. This
//! mirrors how `tests/fasttext.rs` gates on the `LID_TEST_FASTTEXT_MODEL`
//! env var.

#![cfg(all(target_os = "macos", feature = "llm-apple-foundation"))]

use language_identifier::{
    identify_lines_with, AppleFoundationResolver, IdentifyOptions, LlmResolver, Status,
};

fn skip_unless_opted_in() -> bool {
    if std::env::var("LID_RUN_LLM_TESTS").ok().as_deref() != Some("1") {
        eprintln!(
            "skipping: set LID_RUN_LLM_TESTS=1 to exercise the on-device \
             Apple FoundationModels resolver"
        );
        return true;
    }
    false
}

#[test]
fn resolver_opens_session_or_reports_unavailable() {
    if skip_unless_opted_in() {
        return;
    }
    // We can't assert success unconditionally — Apple Intelligence may
    // be off — but the constructor must at least return cleanly with
    // either Ok or the documented unavailable error.
    match AppleFoundationResolver::new() {
        Ok(_) => {}
        Err(e) => eprintln!("model unavailable on this machine: {e}"),
    }
}

#[test]
fn resolver_picks_ja_for_japanese_carrier_with_chinese_examples() {
    if skip_unless_opted_in() {
        return;
    }
    let Ok(resolver) = AppleFoundationResolver::new() else {
        eprintln!("model unavailable — skipping resolve test");
        return;
    };

    // SDD §8.4 Case M3 — Japanese carrier sentences embedding Chinese
    // example phrases. Layers 0–9 surface this as `Status::Ambiguous`
    // with `ja` and `zh-*` competing closely; the LLM should pick `ja`.
    let lines = [
        "日本語の文の中に 中文老师在大学教中文 という中国語の例文が含まれている場合、システムは日本語を主言語として扱い、中国語部分を別のセグメントとして検出する必要があります。",
        "この入力では 先生 という言葉が日本語にも中国語にも存在するため、王先生今天不在 という中国語の文脈を使って判断することが重要です。",
        "日本語では 先生は大学で日本語を教えています と言えますが、中国語では 王老师在大学教中文 のように表現するため、両方の言語が混在していることを検出する必要があります。",
    ];
    let opts = IdentifyOptions {
        ml_classifier: None,
        llm_resolver: Some(&resolver as &dyn LlmResolver),
    };
    let result = identify_lines_with(&lines, &opts);

    // The LLM must not promote the result out of `Ambiguous` — per SDD
    // §7.2 the status stays ambiguous; only `primaryLanguage` changes.
    assert_eq!(result.status, Status::Ambiguous, "{result:?}");
    assert_eq!(result.primary_language.as_deref(), Some("ja"), "{result:?}");
}
