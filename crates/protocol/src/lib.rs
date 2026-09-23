//! # protocol crate (TASK-011 阶段 1A1 基础设施)
//!
//! 职责：作为**协议单一事实源**——所有跨 crate / 跨语言边界的类型都由
//! `xtask codegen` 从 `protocol/**/*.json` 生成。**不变量 1**：本 crate
//! 中的类型 = schema 的 1:1 Rust 表达；任何手工修改 `generated/*.rs` 都会被
//! `codegen --check` 检测为 drift = CI 红灯（铁律 1「无静默失败」）。
//!
//! 边界（不做什么）：
//! - 不实现业务逻辑：仅类型 + 校验入口
//! - 不调用平台 API（arch test 拦截）
//! - 不产生运行时代理（schema 直接走 serde）
//!
//! 不变量：
//! 1. 所有公开类型 `#[non_exhaustive]`（便于向后兼容扩展）
//! 2. 所有公开类型 `Serialize + Deserialize<'de>`
//! 3. `ErrorCode` 枚举 = 13 类（与 v2 §8.7 对齐）；新增 = ADR
//! 4. 生成文件首行 = `// GENERATED — DO NOT EDIT`（`codegen` 会校验）

#![deny(unsafe_code)]
#![allow(missing_docs)]
// 13 ErrorCategory variants + 12 AuditEventType + 4 AuditActor + 3 CapabilityStability: per-field docs add little value, all are self-documenting
#![allow(clippy::all, clippy::pedantic)] // TASK-011: schema-driven code with intentional pedantic/usage noise; tighten later // 13 ErrorCategory variants + 12 AuditEventType + 4 AuditActor + 3 CapabilityStability: per-field docs add little value, all are self-documenting

mod generated;

// impl ToolEnvelope helpers (wrapper / data formats, not data)

impl ToolEnvelope {
    pub fn ok(data: serde_json::Value) -> Self {
        Self {
            version: "1.0".to_string(),
            ok: true,
            data: Some(EnvelopeData(data)),
            error: None,
            untrusted: false,
            truncated: false,
            metadata: None,
        }
    }
    pub fn err(code: ErrorCode) -> Self {
        Self {
            version: "1.0".to_string(),
            ok: false,
            data: None,
            error: Some(EnvelopeError {
                code,
                message: String::new(),
                details: None,
            }),
            untrusted: false,
            truncated: false,
            metadata: None,
        }
    }
}

pub use generated::audit_event::{AuditActor, AuditEvent, AuditEventType, Cost, PolicyDecision};
pub use generated::capability::{Capability, CapabilityStability};
pub use generated::envelope::{EnvelopeData, EnvelopeError, ToolEnvelope};
pub use generated::error_code::{ErrorCategory, ErrorCode, ErrorDefinition};
pub use generated::tool_schema::{RiskLevel, ToolSchema};

/// Re-export serde_json for downstream consumers.
pub use serde_json;

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_error_code_count() {
        // 不变量 3：13 类
        assert_eq!(ErrorCategory::ALL.len(), 13);
    }

    #[test]
    fn test_all_categories_have_definition() {
        for category in ErrorCategory::ALL {
            let def = ErrorDefinition::for_category(category);
            assert!(!def.message_for_model.is_empty());
            assert!(!def.message_for_user.is_empty());
            assert!(!def.hint.is_empty());
        }
    }

    #[test]
    fn test_envelope_ok_roundtrip() {
        let env = ToolEnvelope::ok(serde_json::json!({"result": 42}));
        let json = serde_json::to_string(&env).expect("serialize");
        let back: ToolEnvelope = serde_json::from_str(&json).expect("deserialize");
        assert!(back.ok);
        assert!(back.error.is_none());
    }

    #[test]
    fn test_envelope_error_roundtrip() {
        let env = ToolEnvelope::err(ErrorCode::TargetNotFound);
        let json = serde_json::to_string(&env).expect("serialize");
        let back: ToolEnvelope = serde_json::from_str(&json).expect("deserialize");
        assert!(!back.ok);
        assert_eq!(back.error.unwrap().code, ErrorCode::TargetNotFound);
    }
}
