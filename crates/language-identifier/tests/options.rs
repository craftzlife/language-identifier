use std::sync::atomic::{AtomicUsize, Ordering};

use language_identifier::{
    identify, identify_with, Candidate, IdentifyOptions, MlClassifier, LlmResolver, Status,
};

/// Trivial `MlClassifier` that ignores the input and returns a fixed
/// score list. Useful for verifying the L9 hook routes through
/// aggregate.
struct FixedMl(Vec<(String, f32)>);
impl MlClassifier for FixedMl {
    fn classify(&self, _: &str) -> Vec<(String, f32)> {
        self.0.clone()
    }
}

/// `LlmResolver` stub that counts invocations and returns a
/// configurable answer.
struct StubLlm {
    calls: AtomicUsize,
    answer: Option<String>,
}
impl StubLlm {
    fn new(answer: Option<&str>) -> Self {
        Self {
            calls: AtomicUsize::new(0),
            answer: answer.map(str::to_string),
        }
    }
    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}
impl LlmResolver for StubLlm {
    fn resolve(&self, _: &str, _: &[Candidate]) -> Option<String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.answer.clone()
    }
}

#[test]
fn default_options_match_identify() {
    for input in [
        "",
        "Hello",
        "giáo viên đại học",
        "田中先生は大学で日本語を教えています。",
        "学",
        "Hello こんにちは world",
    ] {
        let a = identify(input);
        let b = identify_with(input, &IdentifyOptions::default());
        assert_eq!(a.status, b.status, "status diverged for {input:?}");
        assert_eq!(
            a.primary_language, b.primary_language,
            "primary diverged for {input:?}"
        );
        let aa: Vec<_> = a.candidates.iter().map(|c| &c.language).collect();
        let bb: Vec<_> = b.candidates.iter().map(|c| &c.language).collect();
        assert_eq!(aa, bb, "candidate order diverged for {input:?}");
        assert_eq!(a.reasons, b.reasons, "reasons diverged for {input:?}");
    }
}

#[test]
fn ml_classifier_influences_candidate_ordering() {
    // W1-style ambiguous Han input: with no markers, script aggregation
    // splits Han ~evenly across ja / zh-Hans / zh-Hant. A strong ML
    // signal toward zh-Hans should reorder the candidates.
    let input = "学";
    let baseline = identify(input);
    assert!(
        matches!(
            baseline.primary_language.as_deref(),
            Some("ja") | Some("zh-Hans") | Some("zh-Hant")
        ),
        "unexpected baseline primary: {:?}",
        baseline.primary_language
    );

    let ml = FixedMl(vec![("zh-Hans".into(), 1.0)]);
    let opts = IdentifyOptions {
        ml_classifier: Some(&ml),
        llm_resolver: None,
    };
    let with_ml = identify_with(input, &opts);
    assert_eq!(
        with_ml.primary_language.as_deref(),
        Some("zh-Hans"),
        "ML bonus failed to make zh-Hans primary: {:?}",
        with_ml.candidates
    );
    assert!(
        with_ml
            .reasons
            .iter()
            .any(|r| r.contains("Layer 9 (ML classifier)")),
        "expected Layer 9 reason, got: {:?}",
        with_ml.reasons
    );
}

#[test]
fn llm_resolver_only_fires_on_ambiguous() {
    let resolver = StubLlm::new(None);
    let opts = IdentifyOptions {
        ml_classifier: None,
        llm_resolver: Some(&resolver),
    };

    let resolved = identify_with("giáo viên đại học", &opts);
    assert_eq!(resolved.status, Status::Resolved);
    assert_eq!(resolver.calls(), 0, "resolver fired on Resolved input");

    let ambiguous = identify_with("学", &opts);
    assert_eq!(ambiguous.status, Status::Ambiguous);
    assert_eq!(
        resolver.calls(),
        1,
        "resolver should fire once on Ambiguous input"
    );
}

#[test]
fn llm_resolver_changes_primary_but_not_status() {
    let resolver = StubLlm::new(Some("ja"));
    let opts = IdentifyOptions {
        ml_classifier: None,
        llm_resolver: Some(&resolver),
    };
    let r = identify_with("学", &opts);
    assert_eq!(r.status, Status::Ambiguous, "L10 must not change status");
    assert_eq!(r.primary_language.as_deref(), Some("ja"));
    assert!(
        r.reasons
            .iter()
            .any(|s| s.contains("Layer 10 (LLM) refined primary")),
        "expected L10 refinement note, got: {:?}",
        r.reasons
    );
}

#[test]
fn llm_resolver_skipped_for_resolved_input() {
    let resolver = StubLlm::new(Some("zz-Made-Up"));
    let opts = IdentifyOptions {
        ml_classifier: None,
        llm_resolver: Some(&resolver),
    };
    let r = identify_with("giáo viên đại học", &opts);
    assert_eq!(r.status, Status::Resolved);
    assert_eq!(
        r.primary_language.as_deref(),
        Some("vi-VN"),
        "resolver must not touch a Resolved primary"
    );
    assert_eq!(resolver.calls(), 0);
    assert!(
        !r.reasons.iter().any(|s| s.contains("Layer 10")),
        "no L10 note should appear for Resolved input"
    );
}

#[test]
fn llm_resolver_none_answer_keeps_primary_with_note() {
    let resolver = StubLlm::new(None);
    let opts = IdentifyOptions {
        ml_classifier: None,
        llm_resolver: Some(&resolver),
    };
    let baseline = identify("学");
    let r = identify_with("学", &opts);
    assert_eq!(r.status, Status::Ambiguous);
    assert_eq!(r.primary_language, baseline.primary_language);
    assert!(
        r.reasons
            .iter()
            .any(|s| s.contains("Layer 10 (LLM) returned no decision")),
        "expected 'no decision' note, got: {:?}",
        r.reasons
    );
}
