//! Approval-scope validation for reserved runtime steps.
//!
//! This module only parses the closed-context `scope_options` subset; it does
//! not grant or consume approvals.

use assistant_protocol::serde_json::Value;

pub(super) fn approval_scope_refusal(scopes: &[Value]) -> Option<String> {
    for scope in scopes {
        match scope.as_str() {
            Some("once" | "task" | "session") => {}
            Some(other) => return Some(format!("unknown approval scope `{other}`")),
            None => return Some("scope_options entries must be strings".to_owned()),
        }
    }
    None
}
