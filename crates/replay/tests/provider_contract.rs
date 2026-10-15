#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

//! Public-contract coverage for the offline replay provider (`ReplaySession`).
//!
//! Why this file exists: `crates/replay/src/provider.rs` is the replay half of the selector
//! contract. ADR-0086 (exact `Name`) and ADR-0091 (`Name` + role) require replay to match nodes
//! with the same semantics as the Windows UIA search, and unrecorded capabilities must fail
//! closed. Those paths had no coverage, so a regression in the replay matcher would have been
//! invisible until a real-machine run.
//!
//! Boundary: these tests drive only the public provider surface plus the shared recording
//! fixture. They never touch production code and never read private state; every assertion is on
//! an observable public result (resolved handle, or the protocol `ErrorCode`).
//!
//! `expect` / `unwrap` / `panic` are allowed in `tests/` (AGENTS.md section 5.3).

use std::future::Future;
use std::path::Path;
use std::task::{Context, Poll, Waker};

use assistant_platform_api::{
    ElementQuery, ElementState, ErrorCode, FingerprintScope, LocalHandleId, OnAmbiguous,
    OnNotFound, PlatformError, ResolutionPolicy, ResolvedWindow, SelectorCandidate, SelectorChain,
    SelectorKind, SelectorValue, TargetDescriptor, Timeout, UiAutomationProvider, WindowProvider,
};
use assistant_protocol::serde_json::{Value, json};
use assistant_replay::ReplaySession;

/// The version-1 single-snapshot recording shared by every replay contract test.
fn fixture_json() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/recordings/core/notepad-like-basic.json");
    std::fs::read_to_string(path).expect("recording fixture must be readable")
}

/// Fixture with one structural edit applied, so the provider sees a different recorded tree.
fn mutated_fixture(mutate: impl FnOnce(&mut Value)) -> String {
    let mut value: Value = assistant_protocol::serde_json::from_str(&fixture_json())
        .expect("fixture must be valid JSON");
    mutate(&mut value);
    assistant_protocol::serde_json::to_string(&value).expect("mutated fixture must serialize")
}

/// Drive a provider future to completion; replay futures must never park (no runtime needed).
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

fn exact_name_and_role(name: &str, role: &str) -> SelectorCandidate {
    candidate(
        "exact-name-and-role",
        SelectorKind::ExactNameAndRole,
        SelectorValue::ExactNameAndRole {
            name: name.to_string(),
            role: role.to_string(),
        },
    )
}

fn assert_error_code<T: std::fmt::Debug>(result: Result<T, PlatformError>, expected: ErrorCode) {
    assert_eq!(result.unwrap_err().code(), expected);
}

/// ADR-0091 happy path: the recorded `Text` node named `Ready` is the only node whose name *and*
/// role both match, so replay resolves exactly that node.
#[test]
fn test_exact_name_and_role_matches_single_recorded_node() {
    let session = ReplaySession::from_json(&fixture_json()).expect("fixture must load");
    let chain = SelectorChain::new(vec![exact_name_and_role("Ready", "Text")]);
    let element = poll_once(
        session
            .ui_automation_provider()
            .resolve_element(&session.window(), &chain),
    )
    .expect("exact name + role must resolve");

    assert_eq!(element.id().value(), 103);
    assert_eq!(element.role(), "Text");
}

/// The role half of the pair must be honoured: the same name with a different role is not the
/// Windows `AndCondition(Name, ControlType)` result, so replay must report no match.
#[test]
fn test_exact_name_and_role_ignores_name_only_match_with_other_role() {
    let session = ReplaySession::from_json(&fixture_json()).expect("fixture must load");
    let chain = SelectorChain::new(vec![exact_name_and_role("Ready", "Button")]);
    assert_error_code(
        poll_once(
            session
                .ui_automation_provider()
                .resolve_element(&session.window(), &chain),
        ),
        ErrorCode::TargetNotFound,
    );
}

/// An empty name would match every node, so it is a configuration error, not a broad selector.
#[test]
fn test_exact_name_and_role_rejects_empty_name() {
    let session = ReplaySession::from_json(&fixture_json()).expect("fixture must load");
    let chain = SelectorChain::new(vec![exact_name_and_role("", "Text")]);
    assert_error_code(
        poll_once(
            session
                .ui_automation_provider()
                .resolve_element(&session.window(), &chain),
        ),
        ErrorCode::ToolInvalidArgs,
    );
}

/// `ExactNameAndRole` is only meaningful with its own value variant; other variants cannot be
/// interpreted, so they fail closed instead of silently degrading to a name-only match.
#[test]
fn test_exact_name_and_role_rejects_non_matching_value_variant() {
    let session = ReplaySession::from_json(&fixture_json()).expect("fixture must load");
    let chain = SelectorChain::new(vec![candidate(
        "exact-name-and-role-wrong-value",
        SelectorKind::ExactNameAndRole,
        SelectorValue::Text("Ready".to_string()),
    )]);
    assert_error_code(
        poll_once(
            session
                .ui_automation_provider()
                .resolve_element(&session.window(), &chain),
        ),
        ErrorCode::CapabilityMissing,
    );
}

/// Two recorded nodes with the same name *and* role must not be resolved by coin flip.
#[test]
fn test_exact_name_and_role_duplicate_nodes_are_ambiguous() {
    let raw = mutated_fixture(|value| {
        let mut duplicate = value["nodes"][3].clone();
        duplicate["local_handle_id"] = json!(104);
        value["nodes"]
            .as_array_mut()
            .expect("nodes array")
            .push(duplicate);
    });
    let session =
        ReplaySession::from_json(&raw).expect("ambiguous recording is structurally valid");
    let chain = SelectorChain::new(vec![exact_name_and_role("Ready", "Text")]);
    assert_error_code(
        poll_once(
            session
                .ui_automation_provider()
                .resolve_element(&session.window(), &chain),
        ),
        ErrorCode::TargetAmbiguous,
    );
}

/// `wait_for` is not recorded in replay v0; it must advertise that honestly rather than
/// reporting a timeout that would look like a live platform problem.
#[test]
fn test_wait_for_is_capability_missing() {
    let session = ReplaySession::from_json(&fixture_json()).expect("fixture must load");
    assert_error_code(
        poll_once(session.ui_automation_provider().wait_for(
            &session.window(),
            &ElementQuery::by_automation_id("EditorTextBox"),
            &ElementState::new(true, true, false),
            Timeout::from_millis(250),
        )),
        ErrorCode::CapabilityMissing,
    );
}

/// Element selectors are scoped to a resolved window; a handle from a different window must not
/// be silently re-pointed at the recorded one.
#[test]
fn test_resolve_element_rejects_mismatched_window_handle() {
    let session = ReplaySession::from_json(&fixture_json()).expect("fixture must load");
    let wrong_window = ResolvedWindow::new(LocalHandleId::new(999), "wrong".to_string());
    let chain = SelectorChain::new(vec![candidate(
        "automation-id",
        SelectorKind::AutomationId,
        SelectorValue::Text("EditorTextBox".to_string()),
    )]);
    assert_error_code(
        poll_once(
            session
                .ui_automation_provider()
                .resolve_element(&wrong_window, &chain),
        ),
        ErrorCode::TargetNotFound,
    );
}

/// A `RoleAndParent` candidate that references itself can never bottom out; the bounded depth
/// guard turns that into an explicit configuration error instead of unbounded recursion.
#[test]
fn test_role_and_parent_self_reference_exceeds_depth_limit() {
    let session = ReplaySession::from_json(&fixture_json()).expect("fixture must load");
    let chain = SelectorChain::new(vec![candidate(
        "self-referential",
        SelectorKind::RoleAndParent,
        SelectorValue::RoleAndParent {
            role: "Pane".to_string(),
            parent_id: "self-referential".to_string(),
        },
    )]);
    assert_error_code(
        poll_once(
            session
                .ui_automation_provider()
                .resolve_element(&session.window(), &chain),
        ),
        ErrorCode::ToolInvalidArgs,
    );
}

/// A candidate whose kind and value do not describe the same matcher matches nothing; replay
/// must report `TargetNotFound` rather than claiming a capability error.
#[test]
fn test_mismatched_selector_kind_and_value_matches_nothing() {
    let session = ReplaySession::from_json(&fixture_json()).expect("fixture must load");
    let chain = SelectorChain::new(vec![candidate(
        "kind-value-mismatch",
        SelectorKind::AutomationId,
        SelectorValue::ClassAndRole {
            class: "TextBox".to_string(),
            role: "Edit".to_string(),
        },
    )]);
    assert_error_code(
        poll_once(
            session
                .ui_automation_provider()
                .resolve_element(&session.window(), &chain),
        ),
        ErrorCode::TargetNotFound,
    );
}

/// A structurally malformed descriptor is rejected before replay looks at the recording.
#[test]
fn test_resolve_window_rejects_invalid_descriptor() {
    let session = ReplaySession::from_json(&fixture_json()).expect("fixture must load");
    let policy = ResolutionPolicy::new(
        OnAmbiguous::ErrorAndAsk,
        OnNotFound::new(Vec::new(), false),
        100,
        0.0,
    )
    .expect("valid resolution policy");
    let descriptor = TargetDescriptor::new(
        "  ".to_string(),
        "notepad-like".to_string(),
        vec![candidate(
            "window",
            SelectorKind::AutomationId,
            SelectorValue::Text("MainWindow".to_string()),
        )],
        Vec::new(),
        policy,
    );
    assert_error_code(
        poll_once(session.window_provider().resolve_window(&descriptor)),
        ErrorCode::ToolInvalidArgs,
    );
}

/// The recorded whole-window fingerprint is the only fingerprint replay can report; it must be
/// returned verbatim so verify / undo compare against the captured value, not a recomputed one.
#[test]
fn test_whole_window_fingerprint_replays_recorded_value() {
    let session = ReplaySession::from_json(&fixture_json()).expect("fixture must load");
    let fingerprint = poll_once(
        session
            .ui_automation_provider()
            .fingerprint(&session.window(), &FingerprintScope::WholeWindow),
    )
    .expect("recorded whole-window fingerprint must replay");

    assert_eq!(
        fingerprint.as_str(),
        session.recording().window().fingerprint()
    );
}
