//! Unit tests for browser-runner step approval boundaries.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::BTreeMap;
use std::sync::Arc;

use assistant_agent_core::{ApprovalGrants, StepPolicy};
use assistant_core::InstructionAttribution;
use assistant_hitl::DiffPreview;
use assistant_policy::{Decision, Effect, PolicyRule, RuleConditions, RuleEffect, RuleId, RuleSet};
use assistant_protocol::{RiskLevel, ToolEffect, ToolEnvelope, ToolReversibility, ToolSchema};
use assistant_task_engine::{PlanStep, Reversibility, StepEffect, StepId, StepTimeouts, TaskId};
use assistant_tool_bus::{
    Clock, EnvelopeRequest, EvidenceDescriptor, SystemClock, assemble_envelope,
};
use serde_json::json;

use super::{
    BrowserTaskPolicy, FormApprovalPreview, FormApprovalPreviewError, FormReadSnapshot,
    MAX_VALUE_BYTES, grant_step_approvals, validate_step_approvals,
};

const EDGE_NAVIGATE: &str = "edge.cdp.navigate";
const EDGE_FORM_READ: &str = "edge.cdp.form_read";
const EDGE_FORM_FILL: &str = "edge.cdp.form_fill";
const APPROVAL_READBACK_EVIDENCE_KEY: &str = "assistant.browser_form_fill.approval_readback.v1";

#[test]
fn test_authoritative_navigate_schema_requires_step_approval()
-> Result<(), Box<dyn std::error::Error>> {
    let navigate = navigate_schema()?;
    assert_eq!(navigate.effect, ToolEffect::Write);
    assert_eq!(navigate.risk_level, RiskLevel::Medium);
    assert_eq!(navigate.reversibility, ToolReversibility::L2Compensation);
    let decision = approval_policy(navigate)?.evaluate_without_approval(&navigate_step())?;
    assert!(matches!(
        decision,
        Decision::AllowWithConfirmation { ref rule_id, .. }
            if rule_id == "confirm_medium_write"
    ));
    Ok(())
}

#[test]
fn test_step_approval_rejects_policy_allow_and_is_single_use()
-> Result<(), Box<dyn std::error::Error>> {
    let mut navigate = navigate_schema()?;
    navigate.risk_level = RiskLevel::Low;
    navigate.reversibility = ToolReversibility::L0UndoStack;
    let mut policy = approval_policy(navigate)?;
    policy.rules = RuleSet::new(vec![PolicyRule {
        id: RuleId::new("allow_low_write_for_test"),
        conditions: RuleConditions {
            effect: Some(vec![Effect::Write]),
            ..RuleConditions::default()
        },
        effect: RuleEffect::Allow,
    }]);
    let step = navigate_step();
    let error = validate_step_approvals(
        std::slice::from_ref(&step),
        &["navigate_page_1".to_owned()],
        &policy,
        &policy.catalog,
    )
    .expect_err("unnecessary approval must fail");
    assert!(error.to_string().contains("allows without confirmation"));

    let authoritative = navigate_schema()?;
    let policy = approval_policy(authoritative)?;
    grant_step_approvals(
        &policy.grants,
        &policy.task_id,
        std::slice::from_ref(&step.id),
        SystemClock.now_unix_ms(),
    )?;
    assert!(matches!(policy.decide(&step)?, Decision::Allow { .. }));
    assert!(matches!(
        policy.decide(&step)?,
        Decision::AllowWithConfirmation { .. }
    ));
    Ok(())
}

#[test]
fn test_form_approval_preview_uses_readback_and_redacts_values()
-> Result<(), Box<dyn std::error::Error>> {
    let before = FormReadSnapshot::from_envelope(&form_read_envelope())?;
    let fill = form_fill_envelope(approval_evidence(
        "http://127.0.0.1:8765",
        "http://127.0.0.1:8765/submitted",
        &json!([
            {"selector": "#name", "before": "", "after": "Ada"},
            {"selector": "#note", "before": "old", "after": "old"}
        ]),
    ))?;
    let preview = FormApprovalPreview::from_envelopes(
        &before,
        &fill,
        InstructionAttribution::plan_derived("Fill the local profile form")?,
    )?;
    assert_eq!(preview.form_origin, "http://127.0.0.1:8765");
    assert_eq!(preview.form_action, "http://127.0.0.1:8765/submitted");
    let DiffPreview::Fields(fields) = &preview.diff else {
        return Err("approval preview did not use field diff".into());
    };
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].field, "#name");
    assert_eq!(fields[0].before, "");
    assert_eq!(fields[0].after, "[redacted:3]");
    let rendered = format!("{preview:?}");
    assert!(!rendered.contains("Ada"));
    assert!(!rendered.contains("old"));
    Ok(())
}

#[test]
fn test_form_approval_preview_rejects_unchanged_values() -> Result<(), Box<dyn std::error::Error>> {
    let before = FormReadSnapshot::from_envelope(&form_read_envelope())?;
    let fill = form_fill_envelope(approval_evidence(
        "http://127.0.0.1:8765",
        "http://127.0.0.1:8765/submitted",
        &json!([
            {"selector": "#name", "before": "", "after": ""},
            {"selector": "#note", "before": "old", "after": "old"}
        ]),
    ))?;
    let error =
        FormApprovalPreview::from_envelopes(&before, &fill, InstructionAttribution::user_request())
            .expect_err("unchanged values must not produce an approval preview");
    assert_eq!(error, FormApprovalPreviewError::EmptyDiff);
    Ok(())
}

#[test]
fn test_form_approval_preview_rejects_url_and_count_drift() -> Result<(), Box<dyn std::error::Error>>
{
    let before = FormReadSnapshot::from_envelope(&form_read_envelope())?;
    let origin_drift = form_fill_envelope(approval_evidence(
        "http://127.0.0.1:9999",
        "http://127.0.0.1:8765/submitted",
        &json!([{"selector": "#name", "before": "", "after": "Ada"}]),
    ))?;
    assert_evidence_mismatch(&before, &origin_drift);

    let action_drift = form_fill_envelope(approval_evidence(
        "http://127.0.0.1:8765",
        "http://127.0.0.1:8765/other",
        &json!([
            {"selector": "#name", "before": "", "after": "Ada"},
            {"selector": "#note", "before": "old", "after": "old"}
        ]),
    ))?;
    assert_evidence_mismatch(&before, &action_drift);

    let count_drift = form_fill_envelope(approval_evidence(
        "http://127.0.0.1:8765",
        "http://127.0.0.1:8765/submitted",
        &json!([{"selector": "#name", "before": "", "after": "Ada"}]),
    ))?;
    assert_evidence_mismatch(&before, &count_drift);
    Ok(())
}

#[test]
fn test_form_approval_preview_rejects_field_state_drift() -> Result<(), Box<dyn std::error::Error>>
{
    let before = FormReadSnapshot::from_envelope(&form_read_envelope())?;
    let selector_drift = form_fill_envelope(approval_evidence(
        "http://127.0.0.1:8765",
        "http://127.0.0.1:8765/submitted",
        &json!([
            {"selector": "#note", "before": "old", "after": "old"},
            {"selector": "#name", "before": "", "after": "Ada"}
        ]),
    ))?;
    assert_evidence_mismatch(&before, &selector_drift);

    let before_drift = form_fill_envelope(approval_evidence(
        "http://127.0.0.1:8765",
        "http://127.0.0.1:8765/submitted",
        &json!([
            {"selector": "#name", "before": "different", "after": "Ada"},
            {"selector": "#note", "before": "old", "after": "old"}
        ]),
    ))?;
    assert_evidence_mismatch(&before, &before_drift);
    Ok(())
}

fn assert_evidence_mismatch(before: &FormReadSnapshot, fill: &ToolEnvelope) {
    let error =
        FormApprovalPreview::from_envelopes(before, fill, InstructionAttribution::user_request())
            .expect_err("form state drift must fail closed");
    assert!(matches!(
        error,
        FormApprovalPreviewError::EvidenceMismatch { .. }
    ));
}

#[test]
fn test_form_approval_preview_rejects_evidence_over_fixed_bound()
-> Result<(), Box<dyn std::error::Error>> {
    let before = FormReadSnapshot::from_envelope(&form_read_envelope())?;
    let fill = form_fill_envelope(approval_evidence(
        "http://127.0.0.1:8765",
        "http://127.0.0.1:8765/submitted",
        &json!([
            {"selector": "#name", "before": "", "after": "x".repeat(MAX_VALUE_BYTES + 1)},
            {"selector": "#note", "before": "old", "after": "old"}
        ]),
    ))?;
    let error =
        FormApprovalPreview::from_envelopes(&before, &fill, InstructionAttribution::user_request())
            .expect_err("oversized readback evidence must fail");
    assert!(matches!(
        error,
        FormApprovalPreviewError::InvalidEvidence { .. }
    ));
    Ok(())
}

#[test]
fn test_form_approval_preview_rejects_missing_or_unknown_evidence()
-> Result<(), Box<dyn std::error::Error>> {
    let before = FormReadSnapshot::from_envelope(&form_read_envelope())?;
    let missing = ToolEnvelope::ok(
        EDGE_FORM_FILL.to_owned(),
        "task-1".to_owned(),
        "fill-form".to_owned(),
        json!({"filled_fields": 1, "submitted": false}),
    );
    assert_eq!(
        FormApprovalPreview::from_envelopes(
            &before,
            &missing,
            InstructionAttribution::user_request(),
        )
        .expect_err("missing evidence must fail"),
        FormApprovalPreviewError::MissingEvidence
    );

    let mut readback = approval_evidence(
        "http://127.0.0.1:8765",
        "http://127.0.0.1:8765/submitted",
        &json!([
            {"selector": "#name", "before": "", "after": "Ada"},
            {"selector": "#note", "before": "old", "after": "old"}
        ]),
    )
    .as_object()
    .and_then(|extra| extra.get(APPROVAL_READBACK_EVIDENCE_KEY))
    .and_then(serde_json::Value::as_object)
    .cloned()
    .ok_or("approval evidence fixture is missing")?;
    readback.insert("unexpected".to_owned(), json!(true));
    let unknown = form_fill_envelope(json!({
        "assistant.browser_form_fill.approval_readback.v1": readback
    }))?;
    let error = FormApprovalPreview::from_envelopes(
        &before,
        &unknown,
        InstructionAttribution::user_request(),
    )
    .expect_err("unknown evidence fields must fail");
    assert!(matches!(
        error,
        FormApprovalPreviewError::InvalidEvidence { .. }
    ));
    Ok(())
}

#[test]
fn test_form_approval_preview_preserves_app_content_high_risk()
-> Result<(), Box<dyn std::error::Error>> {
    let before = FormReadSnapshot::from_envelope(&form_read_envelope())?;
    let fill = form_fill_envelope(approval_evidence(
        "http://127.0.0.1:8765",
        "http://127.0.0.1:8765/submitted",
        &json!([
            {"selector": "#name", "before": "", "after": "Ada"},
            {"selector": "#note", "before": "old", "after": "old"}
        ]),
    ))?;
    let preview = FormApprovalPreview::from_envelopes(
        &before,
        &fill,
        InstructionAttribution::app_content("evidence://form/page-1")?,
    )?;
    assert!(preview.attribution.is_high_risk());
    Ok(())
}

fn form_read_envelope() -> ToolEnvelope {
    ToolEnvelope::ok(
        EDGE_FORM_READ.to_owned(),
        "task-1".to_owned(),
        "read-form".to_owned(),
        json!({
            "origin": "http://127.0.0.1:8765",
            "action": "http://127.0.0.1:8765/submitted",
            "fields": [
                {"selector": "#name", "value": ""},
                {"selector": "#note", "value": "old"}
            ],
            "fingerprint": "sha256:before",
            "previous_fingerprint": "sha256:before"
        }),
    )
}

fn form_fill_envelope(
    extra: serde_json::Value,
) -> Result<ToolEnvelope, Box<dyn std::error::Error>> {
    Ok(assemble_envelope(
        &EnvelopeRequest::new(
            EDGE_FORM_FILL,
            "task-1",
            "fill-form",
            json!({
                "filled_fields": 2,
                "submitted": false,
                "fingerprint": "sha256:after"
            }),
        )
        .with_evidence(EvidenceDescriptor {
            extra: Some(extra),
            ..EvidenceDescriptor::default()
        }),
    )?)
}

fn approval_evidence(origin: &str, action: &str, fields: &serde_json::Value) -> serde_json::Value {
    serde_json::Value::Object(serde_json::Map::from_iter([(
        APPROVAL_READBACK_EVIDENCE_KEY.to_owned(),
        json!({
            "form_origin": origin,
            "form_action": action,
            "fields": fields
        }),
    )]))
}

fn approval_policy(schema: ToolSchema) -> Result<BrowserTaskPolicy, Box<dyn std::error::Error>> {
    Ok(BrowserTaskPolicy::new(
        RuleSet::example_v0(),
        BTreeMap::from([(schema.name.clone(), schema)]),
        "com.microsoft.edge".to_owned(),
        Arc::new(ApprovalGrants::new()),
        TaskId::new("edge_t5_1_books_list_to_csv")?,
    ))
}

fn navigate_schema() -> Result<ToolSchema, Box<dyn std::error::Error>> {
    let root: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(tools_path())?)?;
    let schema = root
        .get("tools")
        .and_then(serde_json::Value::as_array)
        .and_then(|tools| {
            tools.iter().find_map(|tool| {
                let schema = tool.get("schema")?;
                (schema.get("name").and_then(serde_json::Value::as_str) == Some(EDGE_NAVIGATE))
                    .then(|| schema.clone())
            })
        })
        .ok_or("authoritative navigate schema is missing")?;
    Ok(serde_json::from_value(schema)?)
}

fn navigate_step() -> PlanStep {
    PlanStep {
        id: StepId::new("navigate_page_1").expect("step id"),
        sequence: 1,
        tool: EDGE_NAVIGATE.to_owned(),
        args: json!({
            "url": "http://127.0.0.1:8765/page-1.html",
            "load_condition": "network_idle",
        }),
        depends_on: Vec::new(),
        postconditions: vec![json!({
            "kind": "state_changed",
            "fingerprint_scope": "cdp_target",
            "within_ms": 3000,
            "error_code": "VerifyFailed",
        })],
        effect: StepEffect::Write,
        reversibility: Reversibility::L2Compensation,
        point_of_no_return: false,
        timeouts: StepTimeouts::new(1_000, 30_000, 3_000).expect("timeouts"),
    }
}

fn tools_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../adapters/browser.edge/tools/tools.json")
}
