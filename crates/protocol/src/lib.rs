//! # protocol crate (TASK-103 stage 1A1 基础设施)
//!
//! 职责：作为**协议单一事实源**——所有跨 crate / 跨语言边界的类型都由
//! `xtask codegen` 从 `protocol/**/*.json` 生成。
//!
//! ## 不变量
//! 1. 生成文件首行 = `// GENERATED — DO NOT EDIT`（手编辑会被 `codegen --check` 检测为 drift = CI 红灯）
//! 2. 所有公开类型 `#[non_exhaustive]`（便于向后兼容扩展）
//! 3. 所有公开类型 `Serialize + Deserialize<'de>`
//! 4. `ErrorCode` 枚举 = 13 类（与 v2 §8.7 对齐）；新增 = ADR
//! 5. Hash chain (audit-event prev_hash/self_hash) 使用 SHA-256 hex (64 chars lowercase)
//!
//! ## Lint policy (per ADR-0035)
//! - workspace 继承父约束
//! - 本 crate 顶部按需 per-line `#[allow(...)]` + 注释（**禁止** sledgehammer module-level `clippy::all`）
//! - 见依赖登记 `docs/DEPENDENCIES.md`

#![deny(unsafe_code)]
#![allow(clippy::doc_markdown)]
// Chinese docs use unbackticked identifiers (intentional per ADR-0021)
// Generated files (under generated/) need additional allows because codegen emits simple code.
// Per ADR-0035 we can allow them at crate level since they only apply to the generated submodule.
#![allow(
    clippy::manual_string_new,
    clippy::match_same_arms,
    clippy::missing_const_for_fn,
    clippy::too_many_lines,
    clippy::module_name_repetitions,
    clippy::self_named_constructors,
    clippy::enum_variant_names,
    clippy::use_self
)]
//! - 17 audit_event_type variants + 4 audit_actor + 8 source_kind + 5 truncation_reason
//! + 4 risk_level + 13 error_category: all self-documenting, no per-field docs needed.
#![allow(missing_docs)]

mod generated;

pub use generated::audit_event::{AuditActor, AuditEvent, AuditEventType, Cost, PolicyDecision};
pub use generated::capability::{Capability, CapabilityStability};
pub use generated::envelope::{
    EnvelopeData, EnvelopeError, Evidence, Metrics, Source, SourceKind, ToolEnvelope, Truncation,
    TruncationReason,
};
pub use generated::error_code::{ErrorCategory, ErrorCode, ErrorDefinition};
pub use generated::tool_schema::{RiskLevel, ToolSchema};

/// Re-export `serde_json` for downstream consumers (audit_event's open fields).
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

    /// 不变量 4：每类都有 message_for_model/user + hint + evidence_ref
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

    /// ErrorCategory serde name = PascalCase (per naming.md §5 + ADR-0021)
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

    /// envelope error round-trip + evidence_ref auto-injected
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

    /// 不变量 5：prev_hash 接受空串（首事件）+ self_hash 匹配 SHA-256 模式
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
            prev_hash: Some("".to_string()),
            self_hash: "0".repeat(64),
        };
        let json = serde_json::to_string(&ev).expect("serialize");
        let back: AuditEvent = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.self_hash.len(), 64);
        assert_eq!(back.prev_hash, Some("".to_string()));
    }
}
