//! Planner tool-catalog validation tests.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use assistant_core::{CoreResult, PlannerRequest};
use assistant_protocol::ToolSchema;
use assistant_protocol::serde_json::{self, Value, json};
use assistant_task_engine::{Budget, PlanId, TaskId};

fn tool(name: &str) -> ToolSchema {
    serde_json::from_value(json!({
        "version": "1.0",
        "name": name,
        "description": format!("test tool {name}"),
        "input": {"type": "object"},
        "output": {"type": "object"},
        "effect": "read",
        "reversibility": "l0_undo_stack",
        "risk_level": "low"
    }))
    .unwrap()
}

fn request_with_raw_schema(schema: Value) -> CoreResult<PlannerRequest> {
    PlannerRequest::new(
        PlanId::new("p_1").unwrap(),
        TaskId::new("t_1").unwrap(),
        "goal",
        vec![serde_json::from_value(schema).unwrap()],
        Budget::new(10, 60_000, 1_000, 0.5).unwrap(),
    )
}

#[test]
fn test_planner_request_rejects_duplicate_catalog_or_empty_goal() {
    let duplicate = vec![tool("notepad.text.read"), tool("notepad.text.read")];
    assert!(
        PlannerRequest::new(
            PlanId::new("p_1").unwrap(),
            TaskId::new("t_1").unwrap(),
            "goal",
            duplicate,
            Budget::new(10, 60_000, 1_000, 0.5).unwrap(),
        )
        .is_err()
    );

    assert!(
        PlannerRequest::new(
            PlanId::new("p_1").unwrap(),
            TaskId::new("t_1").unwrap(),
            "",
            vec![tool("notepad.text.read")],
            Budget::new(10, 60_000, 1_000, 0.5).unwrap(),
        )
        .is_err()
    );
}

#[test]
fn test_planner_request_rejects_bad_tool_catalog_fields() {
    for schema in [
        json!({
            "version": "9.9",
            "name": "notepad.text.read",
            "description": "bad version",
            "input": {"type": "object"},
            "output": {"type": "object"},
            "effect": "read",
            "reversibility": "l0_undo_stack",
            "risk_level": "low"
        }),
        json!({
            "version": "1.0",
            "name": "_bad.tool.read",
            "description": "bad name",
            "input": {"type": "object"},
            "output": {"type": "object"},
            "effect": "read",
            "reversibility": "l0_undo_stack",
            "risk_level": "low"
        }),
        json!({
            "version": "1.0",
            "name": "notepad.text.read",
            "description": "bad input",
            "input": "not an object",
            "output": {"type": "object"},
            "effect": "read",
            "reversibility": "l0_undo_stack",
            "risk_level": "low"
        }),
        json!({
            "version": "1.0",
            "name": "notepad.text.write",
            "description": "critical write",
            "input": {"type": "object"},
            "output": {"type": "object"},
            "effect": "write",
            "reversibility": "l3_irreversible",
            "risk_level": "critical",
            "requires_approval": false
        }),
        json!({
            "version": "1.0",
            "name": "notepad.text.read",
            "description": "bad tag",
            "input": {"type": "object"},
            "output": {"type": "object"},
            "effect": "read",
            "reversibility": "l0_undo_stack",
            "risk_level": "low",
            "tags": ["Bad-Tag"]
        }),
    ] {
        assert!(request_with_raw_schema(schema).is_err());
    }
}
