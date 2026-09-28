#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

//! Contract tests for recording validation and offline platform providers.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::task::{Context, Poll, Waker};

use assistant_platform_api::{
    ErrorCode, FocusPolicy, SelectorCandidate, SelectorChain, SelectorKind, SelectorValue,
    TreeOptions, UiAutomationProvider, WindowFilter, WindowProvider,
};
use assistant_protocol::serde_json::{Value, json};
use assistant_replay::{RECORDING_VERSION, Recording, ReplayError, ReplaySession};

fn fixture_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/recordings/core/notepad-like-basic.json")
}

fn fixture_json() -> String {
    std::fs::read_to_string(fixture_path()).expect("recording fixture must be readable")
}

fn mutated_fixture(mutate: impl FnOnce(&mut Value)) -> String {
    let mut value: Value =
        assistant_protocol::serde_json::from_str(&fixture_json()).expect("fixture must be JSON");
    mutate(&mut value);
    assistant_protocol::serde_json::to_string(&value).expect("mutated fixture must serialize")
}

fn poll_once<F: Future>(future: F) -> F::Output {
    let mut future = Box::pin(future);
    let mut context = Context::from_waker(Waker::noop());
    match future.as_mut().poll(&mut context) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("replay futures must complete without an executor"),
    }
}

fn candidate(id: &str, kind: SelectorKind, value: SelectorValue) -> SelectorCandidate {
    SelectorCandidate::new(id, kind, value, 1.0, false).expect("valid selector candidate")
}

fn automation_id(value: &str) -> SelectorCandidate {
    candidate(
        "automation-id",
        SelectorKind::AutomationId,
        SelectorValue::Text(value.to_string()),
    )
}

#[test]
fn loads_fixture_and_resolves_stable_automation_id() {
    let session = ReplaySession::from_json(&fixture_json()).expect("fixture must load");
    let chain = SelectorChain::new(vec![automation_id("EditorTextBox")]);
    let element = poll_once(
        session
            .ui_automation_provider()
            .resolve_element(&session.window(), &chain),
    )
    .expect("stable automation id must resolve");

    assert_eq!(element.id().value(), 102);
    assert_eq!(element.parent().value(), 101);
    assert_eq!(element.role(), "Edit");
}

#[test]
fn resolves_role_and_parent_candidate_in_chain_order() {
    let session = ReplaySession::from_json(&fixture_json()).expect("fixture must load");
    let host = candidate(
        "editor-host",
        SelectorKind::ClassAndRole,
        SelectorValue::ClassAndRole {
            class: "StackPanel".to_string(),
            role: "Pane".to_string(),
        },
    );
    let editor = candidate(
        "editor",
        SelectorKind::RoleAndParent,
        SelectorValue::RoleAndParent {
            role: "Edit".to_string(),
            parent_id: "editor-host".to_string(),
        },
    );
    let chain = SelectorChain::new(vec![editor, host]);
    let element = poll_once(
        session
            .ui_automation_provider()
            .resolve_element(&session.window(), &chain),
    )
    .expect("parent chain must resolve");

    assert_eq!(element.id().value(), 102);
}

#[test]
fn replays_recorded_text_and_tree_metadata() {
    let session = ReplaySession::from_json(&fixture_json()).expect("fixture must load");
    let provider = session.ui_automation_provider();
    let chain = SelectorChain::new(vec![automation_id("EditorTextBox")]);
    let element =
        poll_once(provider.resolve_element(&session.window(), &chain)).expect("element resolves");
    let text = poll_once(provider.read_text(&element)).expect("recorded text must replay");
    let snapshot = poll_once(provider.snapshot_tree(
        &session.window(),
        &assistant_platform_api::TreeOptions::new(None, true),
    ))
    .expect("recorded tree snapshot must replay");

    assert_eq!(text, "notepad-like fixture");
    assert_eq!(snapshot.node_count(), 4);
    assert_eq!(
        snapshot.fingerprint().as_str(),
        session.recording().window().fingerprint()
    );
}

#[test]
fn replays_window_metadata_without_faking_write_actions() {
    let session = ReplaySession::from_json(&fixture_json()).expect("fixture must load");
    let provider = session.window_provider();
    let windows =
        poll_once(provider.list_windows(&WindowFilter::for_app("notepad-like"))).expect("list");
    let state = poll_once(provider.window_state(&session.window())).expect("state");
    let bring = poll_once(provider.bring_to_front(&session.window(), FocusPolicy::NeverSteal));

    assert_eq!(windows.len(), 1);
    assert_eq!(windows[0].title(), "notepad-like");
    assert!(state.foreground());
    assert_eq!(bring.unwrap_err().code(), ErrorCode::CapabilityMissing);
}

#[test]
fn ambiguous_selector_is_rejected() {
    let raw = mutated_fixture(|value| {
        let duplicate = value["nodes"][2].clone();
        let nodes = value["nodes"].as_array_mut().expect("nodes array");
        let mut duplicate = duplicate;
        duplicate["local_handle_id"] = json!(200);
        nodes.push(duplicate);
    });
    let session =
        ReplaySession::from_json(&raw).expect("ambiguous recording is structurally valid");
    let chain = SelectorChain::new(vec![automation_id("EditorTextBox")]);
    let error = poll_once(
        session
            .ui_automation_provider()
            .resolve_element(&session.window(), &chain),
    )
    .unwrap_err();

    assert_eq!(error.code(), ErrorCode::TargetAmbiguous);
}

#[test]
fn missing_selector_is_rejected() {
    let session = ReplaySession::from_json(&fixture_json()).expect("fixture must load");
    let chain = SelectorChain::new(vec![automation_id("NoSuchNode")]);
    let error = poll_once(
        session
            .ui_automation_provider()
            .resolve_element(&session.window(), &chain),
    )
    .unwrap_err();

    assert_eq!(error.code(), ErrorCode::TargetNotFound);
}

#[test]
fn empty_selector_chain_is_invalid() {
    let session = ReplaySession::from_json(&fixture_json()).expect("fixture must load");
    let chain = SelectorChain::new(Vec::new());
    let error = poll_once(
        session
            .ui_automation_provider()
            .resolve_element(&session.window(), &chain),
    )
    .unwrap_err();

    assert_eq!(error.code(), ErrorCode::ToolInvalidArgs);
}

#[test]
fn unsupported_snapshot_cropping_is_capability_missing() {
    let session = ReplaySession::from_json(&fixture_json()).expect("fixture must load");
    let provider = session.ui_automation_provider();
    let depth_limited =
        poll_once(provider.snapshot_tree(&session.window(), &TreeOptions::new(Some(1), true)))
            .unwrap_err();

    assert_eq!(depth_limited.code(), ErrorCode::CapabilityMissing);
}

#[test]
fn unrecorded_write_actions_are_capability_missing() {
    let session = ReplaySession::from_json(&fixture_json()).expect("fixture must load");
    let provider = session.ui_automation_provider();
    let chain = SelectorChain::new(vec![automation_id("EditorTextBox")]);
    let element =
        poll_once(provider.resolve_element(&session.window(), &chain)).expect("element resolves");
    let error = poll_once(provider.set_value(&element, "changed")).unwrap_err();

    assert_eq!(error.code(), ErrorCode::CapabilityMissing);
}

#[test]
fn rejects_unknown_version() {
    let raw = mutated_fixture(|value| value["schema_version"] = json!(RECORDING_VERSION + 1));
    assert_eq!(
        Recording::from_json(&raw),
        Err(ReplayError::UnsupportedVersion(RECORDING_VERSION + 1))
    );
}

#[test]
fn rejects_duplicate_node_handle() {
    let raw = mutated_fixture(|value| {
        value["nodes"][1]["local_handle_id"] = json!(100);
    });
    assert_eq!(
        Recording::from_json(&raw),
        Err(ReplayError::DuplicateHandle(100))
    );
}

#[test]
fn rejects_orphan_parent() {
    let raw = mutated_fixture(|value| {
        value["nodes"][1]["parent_handle_id"] = json!(999);
    });
    assert_eq!(
        Recording::from_json(&raw),
        Err(ReplayError::OrphanParent {
            child: 101,
            parent: 999,
        })
    );
}

#[test]
fn rejects_parent_cycle() {
    let raw = mutated_fixture(|value| {
        value["nodes"][0]["parent_handle_id"] = json!(102);
    });
    assert_eq!(
        Recording::from_json(&raw),
        Err(ReplayError::ParentCycle(100))
    );
}

#[test]
fn rejects_dangling_text_reference() {
    let raw = mutated_fixture(|value| {
        value["read_text"][0]["element_handle_id"] = json!(999);
    });
    assert_eq!(
        Recording::from_json(&raw),
        Err(ReplayError::DanglingTextReference(999))
    );
}

#[test]
fn rejects_duplicate_text_outcome() {
    let raw = mutated_fixture(|value| {
        let duplicate = value["read_text"][0].clone();
        let outcomes = value["read_text"].as_array_mut().expect("read_text array");
        outcomes.push(duplicate);
    });
    assert_eq!(
        Recording::from_json(&raw),
        Err(ReplayError::DuplicateTextOutcome(102))
    );
}

#[test]
fn fingerprint_on_unknown_window_is_target_not_found() {
    let session = ReplaySession::from_json(&fixture_json()).expect("fixture must load");
    let wrong_window = assistant_platform_api::ResolvedWindow::new(
        assistant_platform_api::LocalHandleId::new(999),
        "wrong".to_string(),
    );
    let error = poll_once(session.ui_automation_provider().fingerprint(
        &wrong_window,
        &assistant_platform_api::FingerprintScope::WholeWindow,
    ))
    .unwrap_err();

    assert_eq!(error.code(), ErrorCode::TargetNotFound);
}
