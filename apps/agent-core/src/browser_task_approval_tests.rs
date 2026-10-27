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
use assistant_policy::{Decision, Effect, PolicyRule, RuleConditions, RuleEffect, RuleId, RuleSet};
use assistant_protocol::{RiskLevel, ToolEffect, ToolReversibility, ToolSchema};
use assistant_task_engine::{PlanStep, Reversibility, StepEffect, StepId, StepTimeouts, TaskId};
use assistant_tool_bus::{Clock, SystemClock};
use serde_json::json;

use super::{BrowserTaskPolicy, grant_step_approvals, validate_step_approvals};

const EDGE_NAVIGATE: &str = "edge.cdp.navigate";

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
