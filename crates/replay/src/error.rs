//! Replay errors.

use std::fmt;

/// Errors produced while loading or validating a recording.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ReplayError {
    /// JSON is not parseable or has the wrong top-level shape.
    InvalidJson(String),
    /// A required field is missing or has the wrong type.
    InvalidField {
        /// Field path.
        field: String,
        /// Human-readable reason.
        message: String,
    },
    /// Schema version is unsupported.
    UnsupportedVersion(u32),
    /// Node handle is duplicated.
    DuplicateHandle(u64),
    /// A parent handle does not exist.
    OrphanParent {
        /// Child handle.
        child: u64,
        /// Missing parent handle.
        parent: u64,
    },
    /// Parent graph contains a cycle.
    ParentCycle(u64),
    /// Recorded text references an unknown node.
    DanglingTextReference(u64),
    /// Recording contains no nodes.
    EmptyTree,
}

impl fmt::Display for ReplayError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidJson(message) => write!(formatter, "invalid recording JSON: {message}"),
            Self::InvalidField { field, message } => {
                write!(formatter, "invalid field `{field}`: {message}")
            }
            Self::UnsupportedVersion(version) => {
                write!(formatter, "unsupported recording version: {version}")
            }
            Self::DuplicateHandle(handle) => write!(formatter, "duplicate node handle: {handle}"),
            Self::OrphanParent { child, parent } => {
                write!(formatter, "node {child} references missing parent {parent}")
            }
            Self::ParentCycle(handle) => write!(formatter, "parent cycle at node {handle}"),
            Self::DanglingTextReference(handle) => {
                write!(formatter, "recorded text references missing node {handle}")
            }
            Self::EmptyTree => write!(formatter, "recording contains no tree nodes"),
        }
    }
}

impl std::error::Error for ReplayError {}
