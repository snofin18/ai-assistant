//! Production composition root for the 1a Host (ADR-0058 D1).
//!
//! Responsibilities:
//! - load the adapter's target and tool declarations;
//! - register the five Notepad handlers into the real in-process `ToolBus`;
//! - assemble the deterministic task-package Provider, `RuntimeExecutor`, and
//!   `SnapshotEventSource`;
//! - fail closed before startup when a required component is missing.
//!
//! Boundaries:
//! - does not define policy semantics, platform actions, or tool schemas;
//! - does not implement a second Plan representation;
//! - does not start a real LLM or make network calls.
//!
//! Invariants:
//! 1. the production mode is separate from `--self-check`;
//! 2. the tool registry contains exactly the five declared Notepad tools;
//! 3. a task can commit only through the real `RuntimeExecutor` and a real
//!    `VerificationReceipt`;
//! 4. a missing platform fingerprint escalates instead of fabricating one.
//!
//! Related documents: ADR-0058, ADR-0056, ADR-0057, ADR-0053,
//! `docs/spec/runtime-execution.md`.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use assistant_audit::Durability;
use assistant_core::{
    AppMapFileReader, HistoryCompressor, MemoryRetriever, MemorySessionStore, SessionStore,
};
use assistant_model_gateway::{
    CancellationToken, ModelProvider, ModelRouter, NoJitter, SystemMonotonicClock, ThreadSleeper,
};
use assistant_platform_api::{UiAutomationProvider, WindowProvider};
use assistant_policy::{Decision, Effect, EgressDestination, Reversibility, RuleSet};
use assistant_protocol::{ErrorCode, ToolSchema};
use assistant_storage::Clock;
use assistant_task_engine::{
    Budget, MemoryCheckpointStore, Plan, PlanId, PlanStep, StepId, TaskEngine, TaskEvent, TaskId,
    TaskSnapshot,
};
use assistant_tool_bus::ToolRegistry;
use thiserror::Error;

use crate::HostAssembly;
use crate::assembly::{HostAssemblyInput, HostComponents};
use crate::notepad_registry::{NotepadRegistryBuild, build_notepad_registry};
use crate::notepad_targets::NotepadTargetCatalog;
use crate::production_run::ProductionRun;
use crate::production_support::{EmptyRetriever, NoopCompressor};
use crate::runtime::{
    EnvelopeObservationCollector, RuntimeExecutionError, RuntimeExecutor, StepExecutionOutcome,
    StepPolicy,
};
use crate::task_package::{TaskPackageError, TaskPackageProvider};
use crate::ui_events::SnapshotEventSource;
use crate::ui_server::UiServerConfig;

const PRODUCTION_PLAN_BUDGET_STEPS: u64 = 32;
const PRODUCTION_PLAN_BUDGET_MS: u64 = 120_000;
const PRODUCTION_PLAN_BUDGET_TOKENS: u64 = 100_000;
const PRODUCTION_PLAN_BUDGET_COST_USD: f64 = 1.0;

/// Paths and UI configuration required by the production mode.
#[derive(Debug, Clone)]
pub struct ProductionConfig {
    /// Root for the Host database and audit files.
    pub data_root: PathBuf,
    /// Root of the declarative Notepad adapter package.
    pub adapter_root: PathBuf,
    /// The task package that supplies the deterministic 1a Plan.
    pub task_package_path: PathBuf,
    /// UI listener settings. The caller must provide a token and peer allow-list.
    pub ui_config: UiServerConfig,
}

impl ProductionConfig {
    /// Creates a production configuration.
    #[must_use]
    pub fn new(
        data_root: impl Into<PathBuf>,
        adapter_root: impl Into<PathBuf>,
        task_package_path: impl Into<PathBuf>,
        ui_config: UiServerConfig,
    ) -> Self {
        Self {
            data_root: data_root.into(),
            adapter_root: adapter_root.into(),
            task_package_path: task_package_path.into(),
            ui_config,
        }
    }
}

/// Structured production-root failures.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ProductionError {
    /// A required field is empty or inconsistent.
    #[error("invalid production configuration `{field}`: {reason}")]
    InvalidConfiguration {
        /// Invalid field.
        field: &'static str,
        /// Failure detail.
        reason: String,
    },

    /// The task package cannot become a deterministic Plan.
    #[error(transparent)]
    TaskPackage(#[from] TaskPackageError),

    /// Host assembly failed.
    #[error(transparent)]
    Host(#[from] crate::HostAssemblyError),

    /// The model router could not be assembled.
    #[error("model router assembly failed: {reason}")]
    ModelRouter {
        /// Failure detail.
        reason: String,
    },

    /// The Planner rejected the deterministic package or its tool catalog.
    #[error("planner failed: {reason}")]
    Planner {
        /// Failure detail.
        reason: String,
    },

    /// The task engine rejected a lifecycle operation.
    #[error(transparent)]
    TaskEngine(#[from] assistant_task_engine::TaskEngineError),

    /// Runtime execution failed or cannot safely continue.
    #[error(transparent)]
    Runtime(#[from] RuntimeExecutionError),

    /// `ToolBus` shutdown failed.
    #[error("tool bus shutdown failed: {reason}")]
    ToolBus {
        /// Failure detail.
        reason: String,
    },

    /// The production UI listener failed.
    #[error("UI server failed: {reason}")]
    UiServer {
        /// Failure detail.
        reason: String,
    },
}

impl ProductionError {
    /// Returns the stable protocol category for this startup or run failure.
    #[must_use]
    pub const fn error_code(&self) -> ErrorCode {
        match self {
            Self::InvalidConfiguration { .. } => ErrorCode::ToolInvalidArgs,
            Self::TaskPackage(_) | Self::ModelRouter { .. } => ErrorCode::CapabilityMissing,
            Self::Host(error) => error.error_code(),
            Self::Planner { .. } => ErrorCode::ModelInvalidOutput,
            _ => ErrorCode::Fatal,
        }
    }
}

/// Production Host plus its deterministic Plan source and UI event state.
pub struct ProductionHost<P> {
    host: HostComponents<P>,
    provider: TaskPackageProvider,
    tool_catalog: Vec<ToolSchema>,
    policy_catalog: BTreeMap<String, ToolSchema>,
    app_id: String,
    latest_snapshot: Arc<Mutex<Option<TaskSnapshot>>>,
    ui_config: UiServerConfig,
}

/// Assembles the production Host from explicit dependencies.
///
/// # Errors
///
/// Returns [`ProductionError`] when configuration is invalid, an adapter
/// declaration cannot be loaded, the task package cannot produce a Plan source,
/// or the Host/ToolBus cannot be assembled.
pub async fn assemble_production_host<P>(
    config: ProductionConfig,
    platform: P,
    clock: Arc<dyn Clock>,
) -> Result<ProductionHost<P>, ProductionError>
where
    P: WindowProvider + UiAutomationProvider + Clone + Send + Sync + 'static,
{
    validate_config(&config)?;
    let (targets, registry_build, provider) = load_production_assets(&config, &platform)?;
    let provider_arc: Arc<dyn ModelProvider> = Arc::new(provider.clone());
    let model_id = provider.model_id().clone();
    let router =
        ModelRouter::new(model_id.clone(), Vec::new(), vec![model_id]).map_err(|error| {
            ProductionError::ModelRouter {
                reason: error.to_string(),
            }
        })?;
    let tool_catalog = registry_build.tool_schemas.clone();
    let policy_catalog = tool_catalog
        .iter()
        .map(|schema| (schema.name.clone(), schema.clone()))
        .collect::<BTreeMap<_, _>>();

    let input = HostAssemblyInput::new(&config.data_root, clock, platform)
        .with_session_store(Arc::new(MemorySessionStore::new()) as Arc<dyn SessionStore>)
        .with_memory_retriever(Arc::new(EmptyRetriever) as Arc<dyn MemoryRetriever>)
        .with_app_map_reader(
            Arc::new(crate::RootedAppMapReader::new(&config.adapter_root))
                as Arc<dyn AppMapFileReader>,
        )
        .with_planner_provider(Arc::clone(&provider_arc))
        .with_model_router(router)
        .with_model_providers(vec![provider_arc])
        .with_model_runtime(
            Arc::new(SystemMonotonicClock::default()),
            Arc::new(ThreadSleeper),
            Arc::new(NoJitter),
        )
        .with_policy_rules(RuleSet::example_v0())
        .with_tool_registry(registry_build.registry)
        .with_history_compressor(Arc::new(NoopCompressor) as Arc<dyn HistoryCompressor>)
        .with_durability(Durability::Immediate);

    let host = HostAssembly::new(input).assemble().await?;
    let mounted_notepad = host
        .toolset_report()
        .mounted
        .iter()
        .filter(|name| name.starts_with("notepad."))
        .count();
    if mounted_notepad != 5 {
        return Err(ProductionError::InvalidConfiguration {
            field: "toolset_report.mounted",
            reason: format!("expected 5 mounted Notepad tools, found {mounted_notepad}"),
        });
    }
    Ok(ProductionHost {
        host,
        provider,
        tool_catalog,
        policy_catalog,
        app_id: targets.app_id().to_owned(),
        latest_snapshot: Arc::new(Mutex::new(None)),
        ui_config: config.ui_config,
    })
}

fn load_production_assets<P>(
    config: &ProductionConfig,
    platform: &P,
) -> Result<
    (
        Arc<NotepadTargetCatalog>,
        NotepadRegistryBuild,
        TaskPackageProvider,
    ),
    ProductionError,
>
where
    P: WindowProvider + UiAutomationProvider + Clone + Send + Sync + 'static,
{
    let tools_path = config.adapter_root.join("tools").join("tools.json");
    let targets_path = config.adapter_root.join("selectors").join("targets.json");
    let targets = Arc::new(NotepadTargetCatalog::load(&targets_path).map_err(|error| {
        ProductionError::InvalidConfiguration {
            field: "adapter_root.selectors.targets",
            reason: error.to_string(),
        }
    })?);
    let registry_build = build_notepad_registry(
        Arc::new(platform.clone()),
        Arc::clone(&targets),
        &tools_path,
    )
    .map_err(|error| ProductionError::InvalidConfiguration {
        field: "adapter_root.tools.tools",
        reason: error.to_string(),
    })?;
    validate_registry_not_empty(&registry_build.registry)?;
    let provider = TaskPackageProvider::from_package_file(&config.task_package_path)?;
    Ok((targets, registry_build, provider))
}

fn validate_registry_not_empty(registry: &ToolRegistry) -> Result<(), ProductionError> {
    if !registry.is_empty() {
        return Ok(());
    }
    Err(ProductionError::InvalidConfiguration {
        field: "tool_registry",
        reason: "production tool registry must not be empty".to_owned(),
    })
}

impl<P> ProductionHost<P>
where
    P: WindowProvider + UiAutomationProvider + Send + Sync + 'static,
{
    /// Builds one validated Plan from the deterministic task package.
    ///
    /// # Errors
    ///
    /// Returns [`ProductionError`] when the Plan identifiers, budget, tool
    /// catalog, deterministic provider output, or task-engine validation fails.
    pub fn plan_task(&self) -> Result<Plan, ProductionError> {
        let request = assistant_core::PlannerRequest::new(
            PlanId::new(format!("p_{}", self.provider.task_id().as_str())).map_err(|error| {
                ProductionError::InvalidConfiguration {
                    field: "plan_id",
                    reason: error.to_string(),
                }
            })?,
            self.provider.task_id().clone(),
            self.provider.goal().to_owned(),
            self.tool_catalog.clone(),
            Budget::new(
                PRODUCTION_PLAN_BUDGET_STEPS,
                PRODUCTION_PLAN_BUDGET_MS,
                PRODUCTION_PLAN_BUDGET_TOKENS,
                PRODUCTION_PLAN_BUDGET_COST_USD,
            )
            .map_err(|error| ProductionError::Planner {
                reason: error.to_string(),
            })?,
        )
        .map_err(|error| ProductionError::Planner {
            reason: error.to_string(),
        })?;
        self.host
            .planner()
            .generate_plan(&request, CancellationToken::new())
            .map_err(|error| ProductionError::Planner {
                reason: error.to_string(),
            })
    }

    /// Executes a Plan through the real `RuntimeExecutor` and `ToolBus`.
    ///
    /// # Errors
    ///
    /// Returns [`ProductionError`] when task-engine state changes, policy,
    /// approval, tool invocation, observation collection, verification, or
    /// final snapshot loading cannot complete safely.
    pub async fn execute_plan(
        &self,
        plan: Plan,
        now_ms: i64,
    ) -> Result<ProductionRun, ProductionError> {
        let task_id = plan.task_id.clone();
        let steps = plan
            .ordered_steps()
            .into_iter()
            .map(|step| step.id.clone())
            .collect::<Vec<_>>();
        let mut engine = TaskEngine::new(MemoryCheckpointStore::new());
        engine.create_task(plan, now_ms)?;
        engine.apply_task_event(
            &task_id,
            TaskEvent::SubmitForApproval,
            now_ms.saturating_add(1),
        )?;
        engine.apply_task_event(&task_id, TaskEvent::ApprovePlan, now_ms.saturating_add(2))?;

        let policy = CatalogStepPolicy {
            rules: self.host.policy().clone(),
            catalog: self.policy_catalog.clone(),
            target_app: self.app_id.clone(),
        };
        let mut executor = RuntimeExecutor::new(
            engine,
            policy,
            crate::reserved_invoker::ReservedRuntimeInvoker::new(self.host.tool_bus()),
            EnvelopeObservationCollector,
        );
        let mut snapshots = Vec::with_capacity(steps.len());
        for step_id in steps {
            let outcome = executor
                .advance(&task_id, &step_id, now_ms.saturating_add(3))
                .await?;
            self.apply_step_outcome(outcome, &mut snapshots, &step_id, &task_id)?;
        }
        let engine = executor.into_engine();
        let final_snapshot = engine.load_snapshot(&task_id)?;
        self.set_latest_snapshot(&final_snapshot);
        Ok(ProductionRun::new(snapshots, final_snapshot, engine))
    }

    fn apply_step_outcome(
        &self,
        outcome: StepExecutionOutcome,
        snapshots: &mut Vec<TaskSnapshot>,
        step_id: &StepId,
        task_id: &TaskId,
    ) -> Result<(), ProductionError> {
        match outcome {
            StepExecutionOutcome::Committed(snapshot) => {
                self.set_latest_snapshot(&snapshot);
                snapshots.push(snapshot);
                Ok(())
            }
            StepExecutionOutcome::PolicyDenied(snapshot) => {
                self.set_latest_snapshot(&snapshot);
                Err(ProductionError::Runtime(RuntimeExecutionError::Policy {
                    reason: format!(
                        "policy denied step {} in task {}",
                        step_id.as_str(),
                        task_id.as_str()
                    ),
                }))
            }
            StepExecutionOutcome::AwaitingApproval => {
                Err(ProductionError::Runtime(RuntimeExecutionError::Policy {
                    reason: format!(
                        "step {} requires human approval before execution",
                        step_id.as_str()
                    ),
                }))
            }
            StepExecutionOutcome::ToolFailed {
                snapshot,
                code,
                message,
            } => {
                self.set_latest_snapshot(&snapshot);
                Err(ProductionError::Runtime(RuntimeExecutionError::Tool {
                    reason: format!("step {} failed with {code:?}: {message}", step_id.as_str()),
                }))
            }
            StepExecutionOutcome::NeedsHuman { snapshot, reason } => {
                self.set_latest_snapshot(&snapshot);
                Err(ProductionError::Runtime(
                    RuntimeExecutionError::Observation { reason },
                ))
            }
            StepExecutionOutcome::VerificationFailed { snapshot, outcome } => {
                self.set_latest_snapshot(&snapshot);
                Err(ProductionError::Runtime(
                    RuntimeExecutionError::Postconditions {
                        reason: format!("verification failed: {outcome:?}"),
                    },
                ))
            }
        }
    }

    /// Creates a production event source reading the latest executed snapshot.
    #[must_use]
    pub fn snapshot_event_source(
        &self,
    ) -> SnapshotEventSource<impl FnMut() -> Option<TaskSnapshot> + 'static> {
        let latest = Arc::clone(&self.latest_snapshot);
        SnapshotEventSource::new(move || latest.lock().ok().and_then(|value| value.clone()))
    }

    /// Returns the latest snapshot, when one has been produced.
    #[must_use]
    pub fn latest_snapshot(&self) -> Option<TaskSnapshot> {
        self.latest_snapshot
            .lock()
            .ok()
            .and_then(|value| value.clone())
    }

    /// Returns the validated UI listener configuration.
    #[must_use]
    pub const fn ui_config(&self) -> &UiServerConfig {
        &self.ui_config
    }

    /// Returns the deterministic Plan source's task id.
    #[must_use]
    pub const fn task_id(&self) -> &TaskId {
        self.provider.task_id()
    }

    /// Shuts down the owned `ToolBus`.
    ///
    /// # Errors
    ///
    /// Returns [`ProductionError::ToolBus`] when either MCP side fails to stop
    /// within its bounded shutdown window.
    pub async fn shutdown(self) -> Result<(), ProductionError> {
        self.host
            .shutdown()
            .await
            .map_err(|error| ProductionError::ToolBus {
                reason: error.to_string(),
            })
    }

    fn set_latest_snapshot(&self, snapshot: &TaskSnapshot) {
        if let Ok(mut latest) = self.latest_snapshot.lock() {
            *latest = Some(snapshot.clone());
        }
    }
}

struct CatalogStepPolicy {
    rules: RuleSet,
    catalog: BTreeMap<String, ToolSchema>,
    target_app: String,
}

impl StepPolicy for CatalogStepPolicy {
    fn decide(&self, step: &PlanStep) -> Result<Decision, RuntimeExecutionError> {
        let metadata =
            self.catalog
                .get(&step.tool)
                .ok_or_else(|| RuntimeExecutionError::Policy {
                    reason: format!("tool `{}` is absent from the policy catalog", step.tool),
                })?;
        let context = assistant_policy::EvaluationContext {
            effect: policy_effect(step.effect),
            risk_level: metadata.risk_level,
            reversibility: policy_reversibility(step.reversibility),
            unattended: false,
            tainted: false,
            target_app: self.target_app.clone(),
            egress: EgressDestination::None,
        };
        self.rules
            .evaluate(&context)
            .map_err(|error| RuntimeExecutionError::Policy {
                reason: error.to_string(),
            })
    }
}

const fn policy_effect(effect: assistant_task_engine::StepEffect) -> Effect {
    match effect {
        assistant_task_engine::StepEffect::Read => Effect::Read,
        _ => Effect::Write,
    }
}

const fn policy_reversibility(
    reversibility: assistant_task_engine::Reversibility,
) -> Reversibility {
    match reversibility {
        assistant_task_engine::Reversibility::L0UndoStack => Reversibility::L0UndoStack,
        assistant_task_engine::Reversibility::L1Snapshot => Reversibility::L1Snapshot,
        assistant_task_engine::Reversibility::L2Compensation => Reversibility::L2Compensating,
        _ => Reversibility::L3Irreversible,
    }
}

fn validate_config(config: &ProductionConfig) -> Result<(), ProductionError> {
    if config.data_root.as_os_str().is_empty() {
        return Err(ProductionError::InvalidConfiguration {
            field: "data_root",
            reason: "must not be empty".to_owned(),
        });
    }
    if config.adapter_root.as_os_str().is_empty() {
        return Err(ProductionError::InvalidConfiguration {
            field: "adapter_root",
            reason: "must not be empty".to_owned(),
        });
    }
    if config.task_package_path.as_os_str().is_empty() {
        return Err(ProductionError::InvalidConfiguration {
            field: "task_package_path",
            reason: "must not be empty".to_owned(),
        });
    }
    if config.ui_config.pipe_name.trim().is_empty() {
        return Err(ProductionError::InvalidConfiguration {
            field: "ui_config.pipe_name",
            reason: "must not be empty".to_owned(),
        });
    }
    if config
        .ui_config
        .token_environment_variable
        .trim()
        .is_empty()
    {
        return Err(ProductionError::InvalidConfiguration {
            field: "ui_config.token_environment_variable",
            reason: "must not be empty".to_owned(),
        });
    }
    if config.ui_config.allowed_peer_images.is_empty() {
        return Err(ProductionError::InvalidConfiguration {
            field: "ui_config.allowed_peer_images",
            reason: "must contain at least one allowed peer image".to_owned(),
        });
    }
    Ok(())
}

#[cfg(test)]
#[path = "production_tests.rs"]
mod tests;
