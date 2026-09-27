//! Stable errors returned by the binary-layer Host assembly.
//!
//! The assembly point owns startup and therefore must fail closed when any
//! required component is absent or cannot be constructed. The variant retains
//! the backend reason code so the caller can distinguish, for example, a
//! missing migration from a missing provider without parsing display text.

use assistant_protocol::ErrorCode;
use thiserror::Error;

/// Failure while validating or constructing the Host.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum HostAssemblyError {
    /// A required dependency was not supplied by the caller.
    #[error("required Host component `{component}` is missing")]
    MissingComponent {
        /// Stable component name.
        component: &'static str,
    },
    /// A supplied value is structurally invalid.
    #[error("invalid Host configuration `{field}`: {reason}")]
    InvalidConfiguration {
        /// Configuration field.
        field: &'static str,
        /// Human-readable failure.
        reason: String,
    },
    /// The storage layer could not be opened or migrated.
    #[error("storage assembly failed ({reason_code}): {message}")]
    Storage {
        /// Stable storage reason code.
        reason_code: String,
        /// Conservative protocol category.
        code: ErrorCode,
        /// Human-readable failure.
        message: String,
    },
    /// The audit sink could not be initialized.
    #[error("audit assembly failed: {reason}")]
    Audit {
        /// Human-readable failure.
        reason: String,
    },
    /// The in-process tool bus could not be started.
    #[error("tool-bus assembly failed: {reason}")]
    ToolBus {
        /// Human-readable failure.
        reason: String,
    },
    /// A migration set or database version could not be validated.
    #[error("migration assembly failed: {reason}")]
    Migration {
        /// Human-readable failure.
        reason: String,
    },
}

impl HostAssemblyError {
    /// Returns the stable protocol category for this failure.
    #[must_use]
    pub const fn error_code(&self) -> ErrorCode {
        match self {
            Self::MissingComponent { .. } => ErrorCode::CapabilityMissing,
            Self::InvalidConfiguration { .. } => ErrorCode::ToolInvalidArgs,
            Self::Storage { code, .. } => *code,
            Self::Audit { .. } | Self::ToolBus { .. } | Self::Migration { .. } => ErrorCode::Fatal,
        }
    }

    /// Returns a stable machine-readable reason code.
    #[must_use]
    pub fn reason_code(&self) -> &str {
        match self {
            Self::MissingComponent { .. } => "host_component_missing",
            Self::InvalidConfiguration { .. } => "host_configuration_invalid",
            Self::Storage { reason_code, .. } => reason_code,
            Self::Audit { .. } => "host_audit_assembly_failed",
            Self::ToolBus { .. } => "host_tool_bus_assembly_failed",
            Self::Migration { .. } => "host_migration_assembly_failed",
        }
    }
}
