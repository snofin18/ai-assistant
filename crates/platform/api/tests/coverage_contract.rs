//! Public API contract coverage for `assistant-platform-api`.
//!
//! The crate is pure types plus trait shapes. These tests exercise every public
//! constructor/accessor family and the pure validation branches without touching
//! a real desktop, window provider, or accessibility backend.
//!
//! This crate intentionally has no `#[allow]` exceptions, so tests use
//! `ok_or_fail` instead of `expect` / `unwrap` / `panic!`.

mod common;

use std::collections::BTreeMap;

use assistant_platform_api::{
    ApprovalRequirement, CapabilityEntry, CapabilityMatrix, CapabilityMatrixError, CaptureOptions,
    ChannelAvailability, ChannelState, CoordinateSpace, CoordinateSpaceKind, Degradation,
    DisplayInfo, ElementBounds, ElementQuery, ElementState, ErrorCode, Fingerprint, FocusPolicy,
    ImageRef, KeyChord, KeyModifier, KeyTarget, LocalHandleId, NormalizedPoint, OnAmbiguous,
    OnNotFound, PhysicalPoint, PlatformError, PointerAction, ProbeContext, ResolvedElement,
    ResolvedWindow, ResourceAccess, RiskLevel, ScrollTarget, Selection, SelectorCandidate,
    SelectorChain, SelectorKind, SelectorValue, SessionState, SideEffect, TextEditOp, Timeout,
    TreeOptions, TreeSnapshot, WindowFilter, WindowInfo, WindowState,
};

use crate::common::ok_or_fail;

fn fingerprint() -> Fingerprint {
    ok_or_fail(
        Fingerprint::parse(format!("sha256:{}", "0".repeat(64))),
        "valid fingerprint",
    )
}

fn resolved_window() -> ResolvedWindow {
    ResolvedWindow::new(LocalHandleId::new(41), "notepad-main".to_string())
}

fn resolved_element() -> ResolvedElement {
    ResolvedElement::new(
        LocalHandleId::new(42),
        LocalHandleId::new(41),
        "Document".to_string(),
    )
}

fn coordinate_space() -> CoordinateSpace {
    ok_or_fail(
        CoordinateSpace::new(CoordinateSpaceKind::LogicalPixels, 2.0, "\\\\.\\DISPLAY1"),
        "valid coordinate space",
    )
}

fn candidate(id: &str, score: f64, locale_dependent: bool) -> SelectorCandidate {
    ok_or_fail(
        SelectorCandidate::new(
            id,
            SelectorKind::AutomationId,
            SelectorValue::Text(format!("aid.{id}")),
            score,
            locale_dependent,
        ),
        "valid selector candidate",
    )
}

fn capability(
    id: &str,
    resource: ResourceAccess,
    side_effect: SideEffect,
    risk: RiskLevel,
    approval: ApprovalRequirement,
) -> CapabilityEntry {
    CapabilityEntry::new(id.to_string(), resource, side_effect, risk, approval)
}

#[test]
fn test_session_and_handle_accessors() {
    let primary = DisplayInfo::new("DISPLAY1".to_string(), 3, 2, true);
    assert_eq!(primary.id(), "DISPLAY1");
    assert!((primary.scale() - 1.5).abs() < f64::EPSILON);
    assert!(primary.is_primary());

    let fallback_scale = DisplayInfo::new("bad-scale".to_string(), 7, 0, false);
    assert!((fallback_scale.scale() - 1.0).abs() < f64::EPSILON);
    assert!(!fallback_scale.is_primary());

    let session = SessionState::new(true, false, true, vec![primary]);
    assert!(session.locked());
    assert!(!session.remote());
    assert!(session.headless());
    assert_eq!(session.displays().len(), 1);

    let id = LocalHandleId::new(41);
    assert_eq!(id.value(), 41);
    let window = resolved_window();
    assert_eq!(window.id(), id);
    assert_eq!(window.display_label(), "notepad-main");

    let element = resolved_element();
    assert_eq!(element.id(), LocalHandleId::new(42));
    assert_eq!(element.parent(), id);
    assert_eq!(element.role(), "Document");
}

#[test]
fn test_window_accessors_and_capture_options() {
    let any = WindowFilter::any();
    assert_eq!(any.app_id(), None);
    assert_eq!(any.title_regex(), None);

    let filtered = WindowFilter::for_app("com.microsoft.notepad");
    assert_eq!(filtered.app_id(), Some("com.microsoft.notepad"));
    assert_eq!(filtered.title_regex(), None);

    let info = WindowInfo::new(
        resolved_window(),
        "com.microsoft.notepad".to_string(),
        "Untitled - Notepad".to_string(),
    );
    assert_eq!(info.window().display_label(), "notepad-main");
    assert_eq!(info.app_id(), "com.microsoft.notepad");
    assert_eq!(info.title(), "Untitled - Notepad");

    let state = WindowState::new(true, false, true);
    assert!(state.minimized());
    assert!(!state.foreground());
    assert!(state.occluded());

    let options = CaptureOptions::new(true);
    assert!(options.redact());
    let image = ImageRef::new("blob.sha256.abc".to_string(), 800, 600);
    assert_eq!(image.blob_id(), "blob.sha256.abc");
    assert_eq!(image.width(), 800);
    assert_eq!(image.height(), 600);
}

#[test]
fn test_element_query_state_timeout_and_scroll_accessors() {
    let empty = ElementQuery::default();
    assert_eq!(empty.role(), None);
    assert_eq!(empty.automation_id(), None);

    let query = ElementQuery::by_automation_id("Editor");
    assert_eq!(query.role(), None);
    assert_eq!(query.automation_id(), Some("Editor"));

    let state = ElementState::new(true, false, true);
    assert!(state.enabled());
    assert!(!state.visible());
    assert!(state.focused());

    let timeout = Timeout::from_millis(750);
    assert_eq!(timeout.millis(), 750);
    let scroll = ScrollTarget::new(0.25, 0.75);
    assert!((scroll.horizontal_percent() - 0.25).abs() < f64::EPSILON);
    assert!((scroll.vertical_percent() - 0.75).abs() < f64::EPSILON);
}

#[test]
fn test_tree_options_snapshot_chain_and_key_chord_accessors() {
    let options = TreeOptions::new(Some(4), true);
    assert_eq!(options.max_depth(), Some(4));
    assert!(options.include_offscreen());
    let unbounded = TreeOptions::new(None, false);
    assert_eq!(unbounded.max_depth(), None);
    assert!(!unbounded.include_offscreen());

    let snapshot = TreeSnapshot::new(resolved_window(), fingerprint(), 17);
    assert_eq!(snapshot.window().display_label(), "notepad-main");
    assert_eq!(snapshot.fingerprint().digest(), "0".repeat(64));
    assert_eq!(snapshot.node_count(), 17);

    let chain = SelectorChain::new(vec![candidate("editor", 0.9, false)]);
    assert_eq!(chain.candidates().len(), 1);
    let chord = KeyChord::new(
        "V".to_string(),
        vec![KeyModifier::Control, KeyModifier::Shift],
    );
    assert_eq!(chord.key(), "V");
    assert_eq!(chord.modifiers().len(), 2);
}

#[test]
fn test_bounds_round_header_and_extreme_saturation() {
    let bounds = ok_or_fail(ElementBounds::new(10, 20, 42, 52), "valid bounds");
    assert_eq!(
        (
            bounds.left(),
            bounds.top(),
            bounds.right(),
            bounds.bottom(),
            bounds.width(),
            bounds.height(),
        ),
        (10, 20, 42, 52, 32, 32)
    );

    let extreme = ok_or_fail(
        ElementBounds::new(i32::MIN, i32::MIN, 0, 0),
        "extreme but valid bounds",
    );
    assert_eq!(extreme.width(), i32::MAX);
    assert_eq!(extreme.height(), i32::MAX);
}

#[test]
fn test_coordinate_and_geometry_accessors() {
    let space = coordinate_space();
    assert_eq!(space.kind(), CoordinateSpaceKind::LogicalPixels);
    assert!((space.scale() - 2.0).abs() < f64::EPSILON);
    assert_eq!(space.origin_display(), "\\\\.\\DISPLAY1");
    assert!(!space.is_identity());

    let identity = ok_or_fail(
        CoordinateSpace::new(CoordinateSpaceKind::LogicalPixels, 1.0, "\\\\.\\DISPLAY1"),
        "identity space",
    );
    assert!(identity.is_identity());

    let point = ok_or_fail(NormalizedPoint::new(3.0, 4.0), "valid point");
    assert!((point.x_logical() - 3.0).abs() < f64::EPSILON);
    assert!((point.y_logical() - 4.0).abs() < f64::EPSILON);
    let other = ok_or_fail(NormalizedPoint::new(0.0, 0.0), "valid point");
    assert!((point.distance_to(&other) - 5.0).abs() < f64::EPSILON);

    let physical: PhysicalPoint = ok_or_fail(point.to_physical(&space), "physical point");
    assert_eq!((physical.x_px(), physical.y_px()), (6, 8));
}

#[test]
fn test_coordinate_out_of_range_is_explicit() {
    let space = coordinate_space();
    let point = ok_or_fail(
        NormalizedPoint::new(f64::from(i32::MAX), 0.0),
        "large finite point",
    );
    let error = point.to_physical(&space).err();
    assert_eq!(
        error.as_ref().map(PlatformError::code),
        Some(ErrorCode::TargetNotFound)
    );
    assert!(
        error
            .as_ref()
            .is_some_and(|value| value.message().contains("physical point out of range"))
    );
}

#[test]
fn test_enum_public_shapes_are_constructible() {
    let insert = TextEditOp::Insert {
        at: 1,
        text: "x".to_string(),
    };
    let delete = TextEditOp::Delete { start: 0, end: 1 };
    let replace = TextEditOp::Replace {
        start: 0,
        end: 1,
        text: "y".to_string(),
    };
    assert!(format!("{insert:?}").contains("Insert"));
    assert!(format!("{delete:?}").contains("Delete"));
    assert!(format!("{replace:?}").contains("Replace"));

    assert_eq!(Selection::ByIndex(2), Selection::ByIndex(2));
    assert_eq!(
        Selection::ByStableValue("item-2".to_string()),
        Selection::ByStableValue("item-2".to_string())
    );

    let move_action = PointerAction::Move;
    let drag_action = PointerAction::DragTo {
        drop_at: ok_or_fail(NormalizedPoint::new(10.0, 20.0), "drop point"),
        drop_coordinate_space: coordinate_space(),
    };
    assert!(format!("{move_action:?}").contains("Move"));
    assert!(format!("{drag_action:?}").contains("DragTo"));

    let window_target = KeyTarget::Window(resolved_window());
    let element_target = KeyTarget::Element(resolved_element());
    let foreground_target = KeyTarget::Foreground;
    assert!(format!("{window_target:?}").contains("Window"));
    assert!(format!("{element_target:?}").contains("Element"));
    assert!(format!("{foreground_target:?}").contains("Foreground"));

    let policies = [
        FocusPolicy::AllowSteal,
        FocusPolicy::RequireUserConsent,
        FocusPolicy::NeverSteal,
    ];
    assert_eq!(policies.len(), 3);
}

#[test]
fn test_selector_candidate_and_target_accessors() {
    let primary = candidate("editor", 0.8, false).with_ttl_ms(0);
    assert_eq!(primary.id(), "editor");
    assert_eq!(primary.kind(), SelectorKind::AutomationId);
    assert_eq!(
        primary.value(),
        &SelectorValue::Text("aid.editor".to_string())
    );
    assert!((primary.score() - 0.8).abs() < f64::EPSILON);
    assert!(!primary.is_locale_dependent());
    assert_eq!(primary.ttl_ms(), Some(0));

    let localized = candidate("shape", 0.6, true);
    assert!((localized.effective_score("en", "zh") - 0.3).abs() < f64::EPSILON);
    assert!((localized.effective_score("zh", "zh") - 0.6).abs() < f64::EPSILON);

    let retry = OnNotFound::new(vec![10, 20], true);
    assert_eq!(retry.retry_after_ms(), [10, 20]);
    assert!(retry.then_escalate());
    let policy = ok_or_fail(
        assistant_platform_api::ResolutionPolicy::new(OnAmbiguous::ErrorAndAsk, retry, 1_500, 0.25),
        "valid resolution policy",
    );
    assert_eq!(policy.on_ambiguous(), OnAmbiguous::ErrorAndAsk);
    assert_eq!(policy.on_not_found().retry_after_ms(), [10, 20]);
    assert_eq!(policy.max_resolve_ms(), 1_500);
    assert!((policy.min_score_to_try() - 0.25).abs() < f64::EPSILON);

    let descriptor = assistant_platform_api::TargetDescriptor::new(
        "2.0".to_string(),
        "com.microsoft.notepad".to_string(),
        vec![primary],
        vec![localized],
        policy,
    );
    assert_eq!(descriptor.descriptor_version(), "2.0");
    assert_eq!(descriptor.app_id(), "com.microsoft.notepad");
    assert_eq!(descriptor.window_candidates().len(), 1);
    assert_eq!(descriptor.element_candidates().len(), 1);
    assert_eq!(descriptor.resolution_policy().max_resolve_ms(), 1_500);
    assert!(descriptor.validate().is_ok());
}

#[test]
fn test_resource_access_and_side_effect_round_trip() {
    let resources = [
        ResourceAccess::Read,
        ResourceAccess::Write,
        ResourceAccess::Send,
        ResourceAccess::Invoke,
        ResourceAccess::Destroy,
    ];
    assert_eq!(ResourceAccess::ALL, resources);
    for resource in resources {
        assert_eq!(
            ResourceAccess::parse(resource.as_str()).ok(),
            Some(resource)
        );
    }

    let effects = [
        SideEffect::None,
        SideEffect::UiFocus,
        SideEffect::Disk,
        SideEffect::ProcessSpawn,
        SideEffect::NetworkEgress,
        SideEffect::Clipboard,
        SideEffect::ExternalSystem,
    ];
    for effect in effects {
        assert_eq!(SideEffect::parse(effect.as_str()).ok(), Some(effect));
    }
}

#[test]
fn test_risk_and_approval_contract() {
    let risks = [
        (RiskLevel::L1, 1, ApprovalRequirement::Auto),
        (RiskLevel::L2, 2, ApprovalRequirement::Auto),
        (RiskLevel::L3, 3, ApprovalRequirement::Required),
        (RiskLevel::L4, 4, ApprovalRequirement::Required),
        (RiskLevel::L5, 5, ApprovalRequirement::Forbidden),
    ];
    for (risk, rank, approval) in risks {
        assert_eq!(risk.rank(), rank);
        assert_eq!(risk.required_approval(), approval);
    }

    assert_eq!(
        RiskLevel::escalate(RiskLevel::L2, RiskLevel::L4).ok(),
        Some(RiskLevel::L4)
    );
    assert_eq!(
        RiskLevel::escalate(RiskLevel::L4, RiskLevel::L3).err(),
        Some(CapabilityMatrixError::RiskDowngrade {
            from: RiskLevel::L4,
            to: RiskLevel::L3,
        })
    );
}

#[test]
fn test_capability_entry_and_probe_context_accessors() {
    let entry = capability(
        "uia.read_text",
        ResourceAccess::Read,
        SideEffect::None,
        RiskLevel::L1,
        ApprovalRequirement::Auto,
    );
    assert_eq!(entry.id(), "uia.read_text");
    assert_eq!(entry.resource(), ResourceAccess::Read);
    assert_eq!(entry.side_effect(), SideEffect::None);
    assert_eq!(entry.risk(), RiskLevel::L1);
    assert_eq!(entry.approval(), ApprovalRequirement::Auto);

    let probe = ProbeContext::new(
        "2026-10-12T00:00:00Z".to_string(),
        "windows".to_string(),
        true,
        false,
        true,
    );
    assert_eq!(probe.probed_at(), "2026-10-12T00:00:00Z");
    assert_eq!(probe.platform_os(), "windows");
    assert!(probe.session_locked());
    assert!(!probe.session_remote());
    assert!(probe.session_headless());
}

#[test]
fn test_capability_matrix_accessors_and_validation() {
    let probe = ProbeContext::new(
        "2026-10-12T00:00:00Z".to_string(),
        "windows".to_string(),
        false,
        false,
        false,
    );
    let mut channels = BTreeMap::new();
    channels.insert(
        "uia".to_string(),
        ChannelState::new(ChannelAvailability::Available, Some("ready".to_string())),
    );
    channels.insert(
        "capture".to_string(),
        ChannelState::new(ChannelAvailability::NeedsConsent, None),
    );
    let degradation = Degradation::new(
        "capture.needs-consent".to_string(),
        "capture requires consent".to_string(),
    );
    let entry = capability(
        "uia.set_value",
        ResourceAccess::Write,
        SideEffect::UiFocus,
        RiskLevel::L3,
        ApprovalRequirement::Required,
    );
    let matrix = CapabilityMatrix::new(probe, channels, vec![entry], vec![degradation]);

    assert_eq!(matrix.probed_at(), "2026-10-12T00:00:00Z");
    assert_eq!(matrix.platform_os(), "windows");
    assert!(!matrix.session_locked());
    assert!(!matrix.session_remote());
    assert!(!matrix.session_headless());
    assert_eq!(matrix.probe().platform_os(), "windows");
    assert_eq!(matrix.channels().len(), 2);
    assert_eq!(
        matrix.channels().get("uia").map(ChannelState::availability),
        Some(ChannelAvailability::Available)
    );
    assert_eq!(
        matrix.channels().get("uia").and_then(ChannelState::detail),
        Some("ready")
    );
    assert_eq!(matrix.capabilities().len(), 1);
    assert_eq!(matrix.degradations().len(), 1);
    assert_eq!(
        matrix.degradations().first().map(Degradation::id),
        Some("capture.needs-consent")
    );
    assert_eq!(
        matrix.degradations().first().map(Degradation::impact),
        Some("capture requires consent")
    );
    assert!(matrix.validate().is_ok());
}

#[test]
fn test_capability_matrix_errors_are_stable_and_displayable() {
    let errors = [
        CapabilityMatrixError::EmptyCapabilityList,
        CapabilityMatrixError::BlankCapabilityId,
        CapabilityMatrixError::DuplicateCapabilityId("same".to_string()),
        CapabilityMatrixError::RiskDowngrade {
            from: RiskLevel::L4,
            to: RiskLevel::L2,
        },
        CapabilityMatrixError::ApprovalMismatch {
            id: "write".to_string(),
            risk: RiskLevel::L3,
            declared: ApprovalRequirement::Auto,
            expected: ApprovalRequirement::Required,
        },
    ];
    for error in errors {
        assert_eq!(error.code(), ErrorCode::CapabilityMissing);
        assert!(!error.to_string().is_empty());
    }
    let platform_error = PlatformError::from(CapabilityMatrixError::BlankCapabilityId);
    assert_eq!(platform_error.code(), ErrorCode::CapabilityMissing);
}
