//! 平台层错误类型（铁律 1：无静默失败）。
//!
//! 职责：把平台实现的失败统一成**带 `ErrorCode` 的结构化错误**，让上层能按码分派重试/升级。
//! 边界：**不新增错误码** —— 复用 `assistant_protocol::ErrorCode`（新增 = ADR，见 `docs/spec/error-codes.md`）。
//!
//! ## 不变量
//! 1. `message` 与 `evidence_ref` **永不为空**（空 message 会用错误码的 model 文案兜底）
//! 2. `evidence_ref` **只由** `ErrorDefinition` 派生 —— 调用方不得手写（否则证据链会指向不存在的位置）

use std::fmt;

use assistant_protocol::{ErrorCode, ErrorDefinition};

/// 平台层统一返回值。
pub type PlatformResult<T> = Result<T, PlatformError>;

/// 带 `ErrorCode` 的平台层错误。
///
/// 为什么不用 `String` 当错误：上层要按**类别**决定重试 / 升级 / 报错（架构 v2 §8.7），
/// 字符串拼接的错误无法被机器分派（AGENTS.md §5.3）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformError {
    /// 错误类别（复用协议枚举；本 crate 不新增码）。
    code: ErrorCode,
    /// 人类可读细节；为空时用错误码的 model 文案兜底。
    message: String,
    /// 证据引用（由 `ErrorDefinition` 派生，指向该错误的处置文档）。
    evidence_ref: String,
}

impl PlatformError {
    /// 构造一个平台层错误。
    ///
    /// - `message` 为空白时，用 `code` 的 `message_for_model` 兜底（**不**静默留空）
    /// - `evidence_ref` 一律从 `ErrorDefinition::for_category` 取，调用方无法覆盖
    #[must_use]
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        let definition = ErrorDefinition::for_category(code);
        let message = message.into();
        let message = if message.trim().is_empty() {
            definition.message_for_model
        } else {
            message
        };
        Self {
            code,
            message,
            evidence_ref: definition.evidence_ref,
        }
    }

    /// 错误类别。
    #[must_use]
    pub const fn code(&self) -> ErrorCode {
        self.code
    }

    /// 人类可读细节（保证非空）。
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// 证据引用（保证非空；指向该错误的处置文档）。
    #[must_use]
    pub fn evidence_ref(&self) -> &str {
        &self.evidence_ref
    }
}

impl fmt::Display for PlatformError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:?}: {} ({})",
            self.code, self.message, self.evidence_ref
        )
    }
}

impl std::error::Error for PlatformError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blank_message_falls_back_to_model_text() {
        let error = PlatformError::new(ErrorCode::TargetNotFound, "   ");
        assert!(!error.message().trim().is_empty(), "message 不得为空");
        assert!(!error.evidence_ref().is_empty(), "evidence_ref 不得为空");
    }

    #[test]
    fn test_code_and_message_are_preserved() {
        let error = PlatformError::new(ErrorCode::PolicyDenied, "blocked by policy");
        assert_eq!(error.code(), ErrorCode::PolicyDenied);
        assert_eq!(error.message(), "blocked by policy");
        assert!(error.to_string().contains("blocked by policy"));
    }
}
