//! # protocol crate (TASK-103 stage 1A1 基础设施)
//!
//! 职责：作为**协议单一事实源**——所有跨 crate / 跨语言边界的类型都由
//! `xtask codegen` 从 `protocol/**/*.json` 生成。
//!
//! ## 不变量
//! 1. 生成文件首行 = `// GENERATED — DO NOT EDIT`（手编辑会被 `codegen --check` 检测为 drift = CI 红灯）
//! 2. 所有公开类型 `#[non_exhaustive]`（便于向后兼容扩展）
//! 3. 所有公开类型 `Serialize + Deserialize<'de>`
//! 4. `ErrorCategory` 恰好 13 类（与 v2 §8.7 对齐）；`ErrorCode` 是它的类型别名；新增 = ADR
//! 5. 审计事件哈希链（`prev_hash` / `self_hash`）使用 SHA-256 hex（64 位小写）
//!
//! ## Lint 政策（ADR-0035）
//! - 继承 workspace [lints]，本 crate 不额外放宽任何 workspace 级 lint
//! - 手写代码**零** `#[allow]`；唯一的豁免是 `mod generated;` 上的一行，且带原因注释
//! - 依赖登记见 `docs/DEPENDENCIES.md`

#![deny(unsafe_code)]

// 生成物专用豁免（ADR-0035 §决策 2：per-line allow + 原因注释 + 任务卡 §5 登记）。
// 为什么整块豁免而不是逐条修：`generated/**` 由 xtask codegen 从 `protocol/*.json` 产出，
// 字段/变体的文档与标识符写法都在 schema 里，在 Rust 侧重复写一遍只会制造第二份事实源。
// 豁免范围仅限这个模块，手写代码不受影响。
#[allow(
    missing_docs,
    clippy::doc_markdown,
    clippy::use_self,
    clippy::too_many_lines,
    clippy::match_same_arms,
    clippy::missing_const_for_fn,
    clippy::manual_string_new
)]
mod generated;

pub use generated::audit_event::{
    AuditActor, AuditEvent, AuditEventType, Cost, PolicyDecision, PolicyScopeOption,
};
pub use generated::capability::{Capability, CapabilityStability};
pub use generated::envelope::{
    EnvelopeData, EnvelopeError, Evidence, Metrics, Source, SourceKind, ToolEnvelope, Truncation,
    TruncationReason,
};
pub use generated::error_code::{ErrorCategory, ErrorCode, ErrorDefinition};
pub use generated::tool_schema::{RiskLevel, ToolSchema};

/// Re-export `serde_json` for downstream consumers (`audit_event` 的开放字段用得到).
pub use serde_json;

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    /// 不变量 4：13 类
    #[test]
    fn test_error_code_count_is_13() {
        assert_eq!(ErrorCategory::ALL.len(), 13);
    }

    /// 不变量 4：每类都有 `message_for_model` / `message_for_user` / `hint` / `evidence_ref`
    #[test]
    fn test_all_categories_have_definition() {
        for category in ErrorCategory::ALL {
            let def = ErrorDefinition::for_category(category);
            assert!(
                !def.message_for_model.is_empty(),
                "missing message_for_model for {category:?}"
            );
            assert!(
                !def.message_for_user.is_empty(),
                "missing message_for_user for {category:?}"
            );
            assert!(!def.hint.is_empty(), "missing hint for {category:?}");
            assert!(
                !def.evidence_ref.is_empty(),
                "missing evidence_ref for {category:?}"
            );
        }
    }

    /// 不变量 4：retryable 与 v2 §8.7 默认策略列对齐
    #[test]
    fn test_retryable_alignment_with_v2() {
        // 重试/自愈类
        for c in [
            ErrorCategory::ModelInvalidOutput,
            ErrorCategory::ModelNetworkFailure,
            ErrorCategory::TargetNotFound,
            ErrorCategory::TargetUnresponsive,
            ErrorCategory::VerifyFailed,
            ErrorCategory::Transient,
        ] {
            assert!(c.retryable(), "{c:?} should be retryable per v2 §8.7");
        }
        // 拒绝/终止类
        for c in [
            ErrorCategory::ToolInvalidArgs,
            ErrorCategory::PolicyDenied,
            ErrorCategory::TargetAmbiguous,
            ErrorCategory::CapabilityMissing,
            ErrorCategory::PlatformPermission,
            ErrorCategory::UserInteraction,
            ErrorCategory::Fatal,
        ] {
            assert!(!c.retryable(), "{c:?} should NOT be retryable per v2 §8.7");
        }
    }

    /// `ErrorCategory` 的 serde 名字 = PascalCase（naming.md §5 + ADR-0021）
    #[test]
    fn test_error_category_serde_names() {
        let cases = [
            (ErrorCategory::ModelInvalidOutput, "\"ModelInvalidOutput\""),
            (ErrorCategory::PlatformPermission, "\"PlatformPermission\""),
            (ErrorCategory::UserInteraction, "\"UserInteraction\""),
        ];
        for (cat, expected) in cases {
            let json = serde_json::to_string(&cat).expect("serialize");
            assert_eq!(json, expected);
        }
    }

    /// envelope ok=true round-trip + 必填字段保留
    #[test]
    fn test_envelope_ok_roundtrip() {
        let env = ToolEnvelope::ok(
            "notepad.read_text".to_string(),
            "t_9f2".to_string(),
            "s_3".to_string(),
            serde_json::json!({"result": 42}),
        );
        assert!(env.ok);
        assert!(env.error.is_none());
        assert_eq!(env.tool, "notepad.read_text");
        assert_eq!(env.task_id, "t_9f2");
        assert_eq!(env.step_id, "s_3");
        assert!(!env.untrusted);
        let json = serde_json::to_string(&env).expect("serialize");
        let back: ToolEnvelope = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.tool, env.tool);
        assert_eq!(back.task_id, env.task_id);
        assert_eq!(back.step_id, env.step_id);
        assert!(back.ok);
        assert!(back.data.is_some());
    }

    /// envelope error round-trip + `evidence_ref` 自动注入
    #[test]
    fn test_envelope_error_roundtrip() {
        let env = ToolEnvelope::error(
            "notepad.read_text".to_string(),
            "t_9f2".to_string(),
            "s_3".to_string(),
            ErrorCode::TargetNotFound,
            "window not found",
        );
        assert!(!env.ok);
        let err = env.error.expect("error must be present when ok=false");
        assert_eq!(err.code, ErrorCode::TargetNotFound);
        assert_eq!(err.message, "window not found");
        let def = ErrorDefinition::for_category(ErrorCode::TargetNotFound);
        assert_eq!(err.evidence_ref.as_deref(), Some(def.evidence_ref.as_str()));
    }

    /// 不变量 5：`prev_hash` 接受空串（首事件）+ `self_hash` 匹配 SHA-256 模式
    #[test]
    fn test_audit_event_hash_chain_pattern() {
        let ev = AuditEvent {
            version: "1.0".to_string(),
            event_type: "tool.called".to_string(),
            ts: "2026-09-23T00:00:00Z".to_string(),
            session_id: "t_1".to_string(),
            actor: AuditActor::Agent,
            action: "notepad.read_text".to_string(),
            target: None,
            args: None,
            result: None,
            policy_decision: None,
            duration_ms: Some(100),
            cost: None,
            prev_hash: Some(String::new()),
            self_hash: "0".repeat(64),
        };
        let json = serde_json::to_string(&ev).expect("serialize");
        let back: AuditEvent = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.self_hash.len(), 64);
        assert_eq!(back.prev_hash, Some(String::new()));
    }

    /// ADR-0048: confirmation fields survive the protocol boundary, while the
    /// original minimal shape remains deserializable.
    #[test]
    fn test_policy_decision_confirmation_projection() {
        let decision: PolicyDecision = serde_json::from_value(serde_json::json!({
            "allow": true,
            "rule_id": "confirm_medium_write",
            "reason": "human confirmation required before execution",
            "scope_options": ["once", "this_task"],
            "show_diff": true
        }))
        .expect("confirmation decision");
        assert_eq!(
            decision.scope_options,
            Some(vec![PolicyScopeOption::Once, PolicyScopeOption::ThisTask])
        );
        assert_eq!(decision.show_diff, Some(true));

        let minimal: PolicyDecision = serde_json::from_value(serde_json::json!({
            "allow": true,
            "rule_id": "allow_read_low_risk"
        }))
        .expect("legacy minimal decision");
        assert!(minimal.scope_options.is_none());
        assert!(minimal.show_diff.is_none());
    }
}
