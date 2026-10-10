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

fn expect_recording_error(mutate: impl FnOnce(&mut Value)) -> ReplayError {
    let raw = mutated_fixture(mutate);
    Recording::from_json(&raw).expect_err("mutated recording must be rejected")
}

fn poll_once<F: Future>(future: F) -> F::Output {
    let mut future = Box::pin(future);
    let mut context = Context::from_waker(Waker::noop());
    match future.as_mut().poll(&mut context) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("replay futures must complete without an executor"),
    }
}

fn assert_error_code<T: std::fmt::Debug>(
    result: Result<T, assistant_platform_api::PlatformError>,
    expected: ErrorCode,
) {
    assert_eq!(result.unwrap_err().code(), expected);
}

type RecordingMutation = fn(&mut Value);
type RecordingCase<'a> = (&'a str, RecordingMutation);

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

fn exact_name(value: &str) -> SelectorCandidate {
    candidate(
        "exact-name",
        SelectorKind::ExactName,
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
fn exact_name_matches_only_the_complete_display_name() {
    let raw = mutated_fixture(|value| {
        let mut near_match = value["nodes"][3].clone();
        near_match["local_handle_id"] = json!(104);
        near_match["name"] = json!("Ready!");
        value["nodes"]
            .as_array_mut()
            .expect("nodes array")
            .push(near_match);
    });
    let session = ReplaySession::from_json(&raw).expect("fixture must load");
    let chain = SelectorChain::new(vec![exact_name("Ready")]);
    let element = poll_once(
        session
            .ui_automation_provider()
            .resolve_element(&session.window(), &chain),
    )
    .expect("exact name must resolve");

    assert_eq!(element.id().value(), 103);
}

#[test]
fn exact_name_rejects_empty_values_before_matching() {
    let session = ReplaySession::from_json(&fixture_json()).expect("fixture must load");
    let chain = SelectorChain::new(vec![exact_name("")]);
    let error = poll_once(
        session
            .ui_automation_provider()
            .resolve_element(&session.window(), &chain),
    )
    .unwrap_err();

    assert_eq!(error.code(), ErrorCode::ToolInvalidArgs);
}

#[test]
fn exact_name_rejects_non_text_values() {
    let session = ReplaySession::from_json(&fixture_json()).expect("fixture must load");
    let chain = SelectorChain::new(vec![candidate(
        "exact-name-invalid",
        SelectorKind::ExactName,
        SelectorValue::ClassAndRole {
            class: "Group".to_string(),
            role: "Group".to_string(),
        },
    )]);
    let error = poll_once(
        session
            .ui_automation_provider()
            .resolve_element(&session.window(), &chain),
    )
    .unwrap_err();

    assert_eq!(error.code(), ErrorCode::CapabilityMissing);
}

#[test]
fn exact_name_rejects_duplicate_complete_names() {
    let raw = mutated_fixture(|value| {
        let mut duplicate = value["nodes"][3].clone();
        duplicate["local_handle_id"] = json!(104);
        value["nodes"]
            .as_array_mut()
            .expect("nodes array")
            .push(duplicate);
    });
    let session = ReplaySession::from_json(&raw).expect("fixture must load");
    let chain = SelectorChain::new(vec![exact_name("Ready")]);
    let error = poll_once(
        session
            .ui_automation_provider()
            .resolve_element(&session.window(), &chain),
    )
    .unwrap_err();

    assert_eq!(error.code(), ErrorCode::TargetAmbiguous);
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
    let bounds = poll_once(provider.element_bounds(&element)).expect("recorded bounds must replay");
    let snapshot = poll_once(provider.snapshot_tree(
        &session.window(),
        &assistant_platform_api::TreeOptions::new(None, true),
    ))
    .expect("recorded tree snapshot must replay");

    assert_eq!(text, "notepad-like fixture");
    assert_eq!(bounds.left(), 8);
    assert_eq!(bounds.top(), 48);
    assert_eq!(bounds.right(), 884);
    assert_eq!(bounds.bottom(), 420);
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

#[test]
fn replay_error_display_is_stable_for_every_variant() {
    let cases = [
        (
            ReplayError::InvalidJson("bad".to_string()),
            "invalid recording JSON: bad",
        ),
        (
            ReplayError::InvalidField {
                field: "nodes[0].role".to_string(),
                message: "must not be blank".to_string(),
            },
            "invalid field `nodes[0].role`: must not be blank",
        ),
        (
            ReplayError::UnsupportedVersion(9),
            "unsupported recording version: 9",
        ),
        (ReplayError::DuplicateHandle(7), "duplicate node handle: 7"),
        (
            ReplayError::OrphanParent {
                child: 8,
                parent: 9,
            },
            "node 8 references missing parent 9",
        ),
        (ReplayError::ParentCycle(10), "parent cycle at node 10"),
        (
            ReplayError::DanglingTextReference(11),
            "recorded text references missing node 11",
        ),
        (
            ReplayError::DuplicateTextOutcome(12),
            "node 12 has multiple recorded text outcomes",
        ),
        (ReplayError::EmptyTree, "recording contains no tree nodes"),
    ];

    for (error, expected) in cases {
        assert_eq!(error.to_string(), expected);
    }
}

#[test]
fn recording_rejects_malformed_top_level_and_window_fields() {
    assert!(matches!(
        Recording::from_json("{"),
        Err(ReplayError::InvalidJson(_))
    ));
    assert!(matches!(
        Recording::from_json("[]"),
        Err(ReplayError::InvalidField { .. })
    ));

    let cases: &[RecordingCase<'_>] = &[
        ("blank recording id", |value| {
            value["recording_id"] = json!("  ");
        }),
        ("missing window", |value| {
            value.as_object_mut().expect("object").remove("window");
        }),
        ("zero window handle", |value| {
            value["window"]["local_handle_id"] = json!(0);
        }),
        ("blank window display label", |value| {
            value["window"]["display_label"] = json!("");
        }),
        ("invalid window fingerprint", |value| {
            value["window"]["fingerprint"] = json!("not-a-fingerprint");
        }),
        ("missing nodes", |value| {
            value.as_object_mut().expect("object").remove("nodes");
        }),
        ("empty nodes", |value| value["nodes"] = json!([])),
    ];

    assert_invalid_recording_cases(cases);
}

#[test]
fn recording_rejects_malformed_tree_and_text_fields() {
    let cases: &[RecordingCase<'_>] = &[
        ("node is not an object", |value| {
            value["nodes"][0] = json!(1);
        }),
        ("blank node role", |value| {
            value["nodes"][0]["role"] = json!(" ");
        }),
        ("node handle collides with window", |value| {
            value["nodes"][0]["local_handle_id"] = json!(1);
        }),
        ("bounds has the wrong arity", |value| {
            value["nodes"][0]["bounds"] = json!([0, 0, 900]);
        }),
        ("bounds entry is not an integer", |value| {
            value["nodes"][0]["bounds"][0] = json!("0");
        }),
        ("pattern entry is not a string", |value| {
            value["nodes"][0]["patterns"] = json!([1]);
        }),
        ("automation id is neither string nor null", |value| {
            value["nodes"][0]["automation_id"] = json!(1);
        }),
        ("read text is not an array", |value| {
            value["read_text"] = json!({});
        }),
        ("read text entry is not an object", |value| {
            value["read_text"][0] = json!("text");
        }),
        ("read text is missing", |value| {
            value["read_text"][0]
                .as_object_mut()
                .expect("object")
                .remove("text");
        }),
    ];

    assert_invalid_recording_cases(cases);
}

fn assert_invalid_recording_cases(cases: &[RecordingCase<'_>]) {
    for (name, mutate) in cases {
        let error = expect_recording_error(*mutate);
        assert!(
            matches!(
                error,
                ReplayError::InvalidField { .. } | ReplayError::EmptyTree
            ),
            "{name}: {error:?}"
        );
    }
}

#[test]
fn provider_unrecorded_actions_fail_closed() {
    let session = ReplaySession::from_json(&fixture_json()).expect("fixture must load");
    let provider = session.ui_automation_provider();
    let window = session.window();
    let chain = SelectorChain::new(vec![automation_id("EditorTextBox")]);
    let element = poll_once(provider.resolve_element(&window, &chain)).expect("element resolves");

    assert_error_code(
        poll_once(provider.edit_text(
            &element,
            &assistant_platform_api::TextEditOp::Insert {
                at: 0,
                text: "x".to_string(),
            },
        )),
        ErrorCode::CapabilityMissing,
    );
    assert_error_code(
        poll_once(provider.invoke_action(&element, "click")),
        ErrorCode::CapabilityMissing,
    );
    assert_error_code(
        poll_once(provider.select(&element, &assistant_platform_api::Selection::ByIndex(0))),
        ErrorCode::CapabilityMissing,
    );
    assert_error_code(
        poll_once(provider.scroll(
            &element,
            &assistant_platform_api::ScrollTarget::new(0.0, 0.0),
        )),
        ErrorCode::CapabilityMissing,
    );
    assert_error_code(
        poll_once(
            provider.pointer_action(
                &assistant_platform_api::CoordinateSpace::new(
                    assistant_platform_api::CoordinateSpaceKind::LogicalPixels,
                    1.0,
                    r"\\.\DISPLAY1",
                )
                .expect("valid coordinate space"),
                assistant_platform_api::NormalizedPoint::new(0.0, 0.0)
                    .expect("finite normalized point"),
                &assistant_platform_api::PointerAction::Click,
            ),
        ),
        ErrorCode::CapabilityMissing,
    );
    assert_error_code(
        poll_once(provider.key_action(
            &assistant_platform_api::KeyChord::new("A".to_string(), Vec::new()),
            &assistant_platform_api::KeyTarget::Foreground,
        )),
        ErrorCode::CapabilityMissing,
    );
    assert_error_code(
        poll_once(
            session
                .window_provider()
                .capture(&window, &assistant_platform_api::CaptureOptions::new(true)),
        ),
        ErrorCode::CapabilityMissing,
    );
}

#[test]
fn provider_rejects_wrong_handles_and_invalid_bounds() {
    let session = ReplaySession::from_json(&fixture_json()).expect("fixture must load");
    let provider = session.ui_automation_provider();
    let wrong_window = assistant_platform_api::ResolvedWindow::new(
        assistant_platform_api::LocalHandleId::new(999),
        "wrong".to_string(),
    );
    assert_error_code(
        poll_once(provider.snapshot_tree(&wrong_window, &TreeOptions::new(None, true))),
        ErrorCode::TargetNotFound,
    );
    assert_error_code(
        poll_once(session.window_provider().window_state(&wrong_window)),
        ErrorCode::TargetNotFound,
    );

    let missing = assistant_platform_api::ResolvedElement::new(
        assistant_platform_api::LocalHandleId::new(999),
        assistant_platform_api::LocalHandleId::new(0),
        "Edit".to_string(),
    );
    assert_error_code(
        poll_once(provider.read_text(&missing)),
        ErrorCode::TargetNotFound,
    );
    assert_error_code(
        poll_once(provider.element_bounds(&missing)),
        ErrorCode::TargetNotFound,
    );

    let no_text = assistant_platform_api::ResolvedElement::new(
        assistant_platform_api::LocalHandleId::new(101),
        assistant_platform_api::LocalHandleId::new(100),
        "Pane".to_string(),
    );
    assert_error_code(
        poll_once(provider.read_text(&no_text)),
        ErrorCode::CapabilityMissing,
    );

    let overflow = ReplaySession::from_json(&mutated_fixture(|value| {
        value["nodes"][0]["bounds"] = json!([0, 0, i64::MAX, 600]);
    }))
    .expect("structurally valid recording");
    let overflow_element = assistant_platform_api::ResolvedElement::new(
        assistant_platform_api::LocalHandleId::new(100),
        assistant_platform_api::LocalHandleId::new(0),
        "Window".to_string(),
    );
    assert_error_code(
        poll_once(
            overflow
                .ui_automation_provider()
                .element_bounds(&overflow_element),
        ),
        ErrorCode::Fatal,
    );

    let empty = ReplaySession::from_json(&mutated_fixture(|value| {
        value["nodes"][0]["bounds"] = json!([10, 10, 10, 20]);
    }))
    .expect("structurally valid recording");
    assert_error_code(
        poll_once(
            empty
                .ui_automation_provider()
                .element_bounds(&overflow_element),
        ),
        ErrorCode::TargetUnresponsive,
    );
}

#[test]
fn provider_covers_window_filtering_and_fingerprint_scopes() {
    let session = ReplaySession::from_json(&fixture_json()).expect("fixture must load");
    let windows = session.window_provider();
    assert_eq!(
        poll_once(windows.list_windows(&WindowFilter::any()))
            .expect("any filter")
            .len(),
        1
    );
    assert!(
        poll_once(windows.list_windows(&WindowFilter::for_app("other")))
            .expect("other app")
            .is_empty()
    );

    let wrong_app = assistant_platform_api::TargetDescriptor::new(
        "2.0".to_string(),
        "other".to_string(),
        vec![automation_id("MainWindow")],
        Vec::new(),
        assistant_platform_api::ResolutionPolicy::new(
            assistant_platform_api::OnAmbiguous::ErrorAndAsk,
            assistant_platform_api::OnNotFound::new(Vec::new(), false),
            100,
            0.0,
        )
        .expect("valid policy"),
    );
    assert_error_code(
        poll_once(windows.resolve_window(&wrong_app)),
        ErrorCode::TargetNotFound,
    );

    let element = assistant_platform_api::ResolvedElement::new(
        assistant_platform_api::LocalHandleId::new(102),
        assistant_platform_api::LocalHandleId::new(101),
        "Edit".to_string(),
    );
    assert_error_code(
        poll_once(session.ui_automation_provider().fingerprint(
            &session.window(),
            &assistant_platform_api::FingerprintScope::Element(element),
        )),
        ErrorCode::CapabilityMissing,
    );
}

#[test]
fn provider_covers_parent_resolution_failure_paths_and_offscreen_filtering() {
    let session = ReplaySession::from_json(&fixture_json()).expect("fixture must load");
    let provider = session.ui_automation_provider();
    let child = candidate(
        "missing-parent",
        SelectorKind::RoleAndParent,
        SelectorValue::RoleAndParent {
            role: "Edit".to_string(),
            parent_id: "not-a-candidate".to_string(),
        },
    );
    assert_error_code(
        poll_once(provider.resolve_element(&session.window(), &SelectorChain::new(vec![child]))),
        ErrorCode::ToolInvalidArgs,
    );

    let raw = mutated_fixture(|value| {
        let mut duplicate = value["nodes"][1].clone();
        duplicate["local_handle_id"] = json!(200);
        value["nodes"]
            .as_array_mut()
            .expect("nodes")
            .push(duplicate);
        value["nodes"][0]["is_offscreen"] = json!(true);
    });
    let ambiguous = ReplaySession::from_json(&raw).expect("structurally valid recording");
    let host = candidate(
        "host",
        SelectorKind::AutomationId,
        SelectorValue::Text("EditorHost".to_string()),
    );
    let nested_child = candidate(
        "nested-child",
        SelectorKind::RoleAndParent,
        SelectorValue::RoleAndParent {
            role: "Edit".to_string(),
            parent_id: "host".to_string(),
        },
    );
    assert_error_code(
        poll_once(ambiguous.ui_automation_provider().resolve_element(
            &ambiguous.window(),
            &SelectorChain::new(vec![nested_child, host]),
        )),
        ErrorCode::TargetAmbiguous,
    );
    let snapshot = poll_once(
        ambiguous
            .ui_automation_provider()
            .snapshot_tree(&ambiguous.window(), &TreeOptions::new(None, false)),
    )
    .expect("snapshot without offscreen nodes");
    assert_eq!(snapshot.node_count(), 4);
}
