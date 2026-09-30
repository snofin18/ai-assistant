//! Binary-layer Host assembly for the local AI assistant.
//!
//! Responsibilities:
//! - construct the single writable database and apply the complete migration set;
//! - inject session, memory, model, policy, tool-bus, audit, and platform
//!   dependencies into Core components;
//! - keep all assembly logic outside `crates/core` (ADR-0053 D1 / D5).
//!
//! Boundaries:
//! - no orchestration algorithm is implemented here;
//! - no concrete model vendor or network provider is defined here;
//! - no UI or target-application adapter is defined here;
//! - no second assembly point is provided.
//!
//! Invariants:
//! - every required component is explicitly injected;
//! - a missing or invalid component fails with a stable reason code;
//! - the database handle is owned by the assembled Host and shared with
//!   short-lived audit writers.
//!
//! Related documents: `docs/spec/core-orchestration.md`, ADR-0038, ADR-0053.

#![deny(unsafe_code)]

pub mod adapters;
mod assembly;
mod error;
mod runtime;
mod ui_control;
mod ui_ipc;
mod ui_server;

pub use adapters::{
    AuditSink, CharacterTokenEstimator, RootedAppMapReader, StorageMemoryRetriever,
    StorageSessionClock, StorageToolClock, TokenEstimator,
};
pub use assembly::{HostAssembly, HostAssemblyInput, HostComponents};
pub use error::HostAssemblyError;
pub use runtime::{
    EnvelopeObservationCollector, ObservationCollector, RuntimeExecutionError, RuntimeExecutor,
    StepExecutionOutcome, StepPolicy, ToolBusInvoker, ToolInvoker,
};
pub use ui_control::TaskControlHandler;
pub use ui_ipc::{
    UI_IPC_VERSION, UiAuthorizationScope, UiCommand, UiCommandError, UiCommandHandler,
    UiCommandOutcome, UiEvent, dispatch_ui_command, parse_ui_command, project_snapshot_events,
    task_status_name,
};
pub use ui_server::{
    UiServerConfig, looks_like_image_path, process_ui_request, serve as serve_ui, serve_session,
};
