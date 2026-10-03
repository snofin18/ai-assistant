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
mod approval_grants;
mod assembly;
mod error;
pub mod notepad_files;
pub mod notepad_handlers;
pub mod notepad_registry;
mod notepad_rollback;
pub mod notepad_targets;
mod production;
mod production_policy;
mod production_run;
pub mod production_support;
mod reserved_invoker;
mod runtime;
mod runtime_binding;
mod runtime_dataflow;
mod runtime_host_ops;
mod runtime_tools;
mod target_lease;
mod task_package;
mod ui_control;
mod ui_events;
mod ui_ipc;
mod ui_server;

pub use adapters::{
    AuditSink, CharacterTokenEstimator, RootedAppMapReader, StorageMemoryRetriever,
    StorageSessionClock, StorageToolClock, TokenEstimator,
};
pub use approval_grants::{ApprovalGrants, GrantError, GrantRequest};
pub use assembly::{HostAssembly, HostAssemblyInput, HostComponents};
pub use error::HostAssemblyError;
pub use production::{ProductionConfig, ProductionError, ProductionHost, assemble_production_host};
pub use production_run::{PendingRuntimeApproval, ProductionRun};
pub use reserved_invoker::ReservedRuntimeInvoker;
pub use runtime::{
    EnvelopeObservationCollector, ObservationCollector, RuntimeExecutionError, RuntimeExecutor,
    StepExecutionOutcome, StepPolicy, ToolBusInvoker, ToolInvoker,
};
pub use runtime_binding::{BindingInvoker, RuntimeBindingState};
pub use runtime_dataflow::{
    ConditionExpr, ConditionOperand, ConditionOperator, DataflowError, RuntimeDataflowPlan,
    RuntimeStepBinding, collect_references, parse_condition, resolve_references,
};
pub use task_package::{TASK_PACKAGE_MODEL_ID, TaskPackageError, TaskPackageProvider};
pub use ui_control::{PendingApproval, PendingApprovals, TaskControlHandler};
pub use ui_events::SnapshotEventSource;
pub use ui_ipc::{
    UI_IPC_VERSION, UiAuthorizationScope, UiCommand, UiCommandError, UiCommandHandler,
    UiCommandOutcome, UiEvent, dispatch_ui_command, parse_ui_command, project_snapshot_events,
    task_status_name,
};
pub use ui_server::{
    NoEvents, UiEventSource, UiServerConfig, looks_like_image_path, process_ui_request,
    push_events, serve as serve_ui, serve_session, serve_session_with_events, serve_with_events,
};
