//! Provider-level contract for the deterministic task-package Plan source (ADR-0058 D2).
//!
//! Why an integration test: every call below is a **public** entry point re-exported by
//! `assistant-agent-core`. The existing `task_package.rs` suite proves the rendered plan; this
//! file pins the provider metadata, the declared-identity accessors, the generated default goal,
//! the file-read failure, and the cancellation boundary.
//!
//! Test lint exceptions are opened by the single line below (AGENTS.md §5.3).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::path::Path;

use assistant_agent_core::{TASK_PACKAGE_MODEL_ID, TaskPackageError, TaskPackageProvider};
use assistant_model_gateway::{
    CacheHints, CancellationToken, CompletionRequest, Message, MessageRole, ModelProvider,
    ToolChoice,
};
use assistant_protocol::serde_json::json;

/// A minimal valid package: one `tool` step the 1a assertion table covers.
fn package(goal: Option<&str>) -> String {
    let mut value = json!({
        "task_id": "notepad.contract.probe",
        "steps": [{
            "id": "replace",
            "kind": "tool",
            "tool": "notepad.file.replace_text",
            "args": {"old_text": "a", "new_text": "b", "expected_replacements": 1}
        }]
    });
    if let Some(goal) = goal {
        value
            .as_object_mut()
            .expect("package must be an object")
            .insert("goal".to_owned(), json!(goal));
    }
    value.to_string()
}

fn request() -> CompletionRequest {
    CompletionRequest::new(
        vec![Message::new(MessageRole::User, "go")],
        Vec::new(),
        ToolChoice::None,
        CacheHints::disabled(),
    )
}

/// The provider advertises the deterministic model id and streaming capability.
#[test]
fn test_provider_metadata_is_declared_and_streaming() {
    let provider =
        TaskPackageProvider::from_package_json(&package(Some("do work"))).expect("render");
    assert_eq!(provider.model_id().as_str(), TASK_PACKAGE_MODEL_ID);
    let capabilities = provider.capabilities();
    assert!(capabilities.supports_streaming());
    assert_eq!(capabilities.max_context_tokens().get(), 8_192);
    let _pricing = provider.pricing();
}

/// `count_tokens` counts message characters with a floor of one.
#[test]
fn test_count_tokens_counts_characters_with_a_floor_of_one() {
    let provider =
        TaskPackageProvider::from_package_json(&package(Some("do work"))).expect("render");
    let counted = provider
        .count_tokens(&[
            Message::new(MessageRole::User, "abc"),
            Message::new(MessageRole::User, ""),
        ])
        .expect("count");
    assert_eq!(counted.get(), 3);
    let empty = provider.count_tokens(&[]).expect("count");
    assert_eq!(empty.get(), 1);
}

/// The declared identity accessors report the package values.
#[test]
fn test_accessors_expose_declared_identity_and_goal() {
    let provider =
        TaskPackageProvider::from_package_json(&package(Some("write a report"))).expect("render");
    assert_eq!(provider.declared_task_id(), "notepad.contract.probe");
    assert_eq!(provider.goal(), "write a report");
    assert!(!provider.task_id().as_str().is_empty());
    assert!(!provider.plan_json().is_empty());
}

/// A missing or blank `goal` becomes a deterministic generated goal.
#[test]
fn test_default_goal_is_generated_when_absent_or_blank() {
    for goal in [None, Some("   ")] {
        let provider = TaskPackageProvider::from_package_json(&package(goal)).expect("render");
        assert!(
            provider.goal().starts_with("Execute task package "),
            "unexpected default goal: {}",
            provider.goal()
        );
    }
}

/// Reading a package that does not exist fails with the `Read` variant, not a panic.
#[test]
fn test_from_package_file_missing_read_is_explicit() {
    let missing = Path::new("does-not-exist/task-package.json");
    let error =
        TaskPackageProvider::from_package_file(missing).expect_err("missing file must fail");
    assert!(matches!(error, TaskPackageError::Read { .. }));
}

/// A cancelled request never starts a stream.
#[test]
fn test_complete_honours_cancellation() {
    let provider =
        TaskPackageProvider::from_package_json(&package(Some("do work"))).expect("render");
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    assert!(
        provider.complete(request(), cancellation).is_err(),
        "a cancelled request must not start a stream"
    );
}
