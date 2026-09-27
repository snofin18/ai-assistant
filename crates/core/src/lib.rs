//! # assistant-core
//!
//! Core orchestration components for session lifecycle, message trees, context
//! selection, trimming, compression, token budgeting, model-output planning,
//! App Map loading, and bounded memory projection.
//!
//! The crate does not assemble other components or hold a database connection.
//! A binary creates [`SessionManager`], [`ContextManager`], and [`Planner`]
//! from injected dependencies. The storage adapter for [`SessionStore`] belongs
//! to the binary assembly card (TASK-029).
//!
//! ## Boundaries
//!
//! - No platform API calls or platform implementation dependencies.
//! - No policy decisions, tool execution, audit writes, or Host assembly.
//! - No SQL and no hidden global state.
//! - No silent context loss: trimming and compression produce explicit
//!   [`ContextOmission`] records.
//! - Planner never accepts a step whose tool is absent from the caller-supplied
//!   catalog and never repairs malformed model output.
//! - Memory never reads files directly; App Map content is parsed and validated
//!   behind an injected reader.
//!
//! ## Invariants
//!
//! 1. Dependencies stay within ADR-0053 D2.
//! 2. All IDs, content, restored snapshots, and budgets are validated before use.
//! 3. Session mutations persist before the in-memory snapshot is replaced.
//! 4. A required context that exceeds budget fails closed.
//! 5. Compression failure never degrades into silent history loss.
//! 6. Planner model output is parsed and validated before producing a Plan.
//!
//! ## Related Documents
//!
//! `docs/spec/core-orchestration.md`, ADR-0053, and architecture v2 sections
//! 7.1, 7.2, and 11.3.

#![deny(unsafe_code)]

mod app_map;
mod context;
mod error;
mod identifiers;
mod memory;
mod message;
mod planner;
mod session;
mod store;

pub use app_map::{
    APP_MAP_VERSION, AppMap, AppMapEntry, AppMapFileReader, AppMapLoader, AppMapReadError,
};
pub use context::{
    ContextBudget, ContextEntry, ContextFragment, ContextManager, ContextOmission,
    ContextProjection, ContextSummary, HistoryCompressor, OmissionReason,
};
pub use error::{CompressionError, CoreError, CoreResult, MemoryRetrievalError, SessionStoreError};
pub use identifiers::{MessageId, SessionId, TokenCount};
pub use memory::{
    Memory, MemoryOmission, MemoryOmissionReason, MemoryProjection, MemoryRequest,
    MemoryRetrievalHit, MemoryRetriever, MemorySegment, MemorySegmentOrigin,
};
pub use message::{
    ContextRetention, MessageContent, MessageNode, MessageNodeParts, MessageRole, SessionSnapshot,
    SessionSnapshotParts, SessionStatus,
};
pub use planner::{Planner, PlannerRequest};
pub use session::{NewMessage, SessionClock, SessionManager};
pub use store::{MemorySessionStore, SessionStore};

pub use assistant_storage::{MemoryQuery, MemoryRecordKind, MemorySearchResult};
