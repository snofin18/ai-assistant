//! Private CLI runner for one browser task package.
//!
//! Responsibilities:
//! - parse explicit dedicated-profile browser inputs and one task package;
//! - render the package to a Plan and execute it through the real `ToolBus`;
//! - optionally normalize three committed list-page DOM outputs into the T5.1 CSV;
//! - export only committed results to new, read-back-verified JSON/CSV files.
//!
//! Boundaries:
//! - does not log in, submit forms, or use the user's normal browser profile;
//! - does not turn a scripted-runtime test into real Edge acceptance evidence.
//!
//! Invariants:
//! 1. every step must be `Committed` before any output file is created;
//! 2. an existing output path is never overwritten;
//! 3. `ToolBus` shutdown and browser process-tree close are attempted on every path.

use std::collections::BTreeMap;
use std::future::Future;
use std::io::{self, Write};
use std::process::ExitCode;
use std::sync::{Arc, Mutex};

use assistant_agent_core::{
    ApprovalGrants, BindingInvoker, EnvelopeObservationCollector, RuntimeBindingState,
    RuntimeExecutionError, RuntimeExecutor, StepExecutionOutcome, TaskPackageProvider,
    ToolBusInvoker, ToolInvoker,
};
use assistant_core::{Planner, PlannerRequest};
use assistant_model_gateway::{CancellationToken, ModelProvider};
use assistant_policy::RuleSet;
use assistant_protocol::{ToolEnvelope, ToolSchema};
use assistant_task_engine::{
    Budget, Plan, PlanId, PlanStep, TaskEngine, TaskEvent, TaskId, TaskSnapshot, TaskStatus,
};
use assistant_tool_bus::{Clock, MountSelection, SystemClock, ToolBus, ToolBusConfig};
use serde_json::{Map, Value, json};

use crate::browser_runtime::{BrowserRuntime, BrowserRuntimeError};

#[path = "browser_task_approval.rs"]
mod browser_task_approval;
#[path = "browser_task_csv.rs"]
mod browser_task_csv;
#[path = "browser_task_support.rs"]
mod browser_task_support;

use browser_task_approval::{BrowserTaskPolicy, grant_step_approvals, validate_step_approvals};
use browser_task_csv::normalize_t5_1_outputs;
use browser_task_support::{BrowserTaskError, BrowserTaskOptions, BrowserTaskResult, write_output};

const PLAN_BUDGET_STEPS: u64 = 32;
const PLAN_BUDGET_MS: u64 = 120_000;
const PLAN_BUDGET_TOKENS: u64 = 100_000;
const PLAN_BUDGET_COST_USD: f64 = 1.0;

struct BrowserExecution<'arguments> {
    app_id: &'arguments str,
    now_ms: i64,
    approved_step_ids: &'arguments [String],
}

type BrowserExecutor<'bus> = RuntimeExecutor<
    assistant_task_engine::MemoryCheckpointStore,
    BrowserTaskPolicy,
    BindingInvoker<RecordingInvoker<ToolBusInvoker<'bus>>>,
    EnvelopeObservationCollector,
>;

/// Runs the private CLI mode.
pub async fn run_cli(arguments: &[String]) -> ExitCode {
    let rest = arguments.get(1..).unwrap_or(&[]);
    match run(rest).await {
        Ok(result) => {
            if writeln!(
                io::stdout().lock(),
                "browser task run: task={} steps={} outputs={}",
                result.task_id,
                result.steps.len(),
                result.outputs.len()
            )
            .is_ok()
            {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(error) => {
            let _ = writeln!(
                io::stderr().lock(),
                "browser task [{:?}]: {error}",
                error.error_code()
            );
            ExitCode::from(2)
        }
    }
}

async fn run(arguments: &[String]) -> Result<BrowserTaskResult, BrowserTaskError> {
    let options = BrowserTaskOptions::parse(arguments)?;
    let task_inputs = options.read_task_inputs()?;
    let provider = TaskPackageProvider::from_package_file_with_inputs(
        &options.task_package_path,
        &task_inputs,
    )?;
    let runtime = BrowserRuntime::launch(options.runtime).await?;
    let (mut browser, registry, tool_schemas, selection) = runtime.into_parts();
    let tool_schemas = browser_tool_schemas(tool_schemas, selection)?;
    let mounted_names = tool_schemas
        .iter()
        .map(|schema| schema.name.clone())
        .collect::<Vec<_>>();
    let (bus, _report) = ToolBus::start(
        registry,
        MountSelection::names(mounted_names),
        ToolBusConfig::new(format!("browser-task-{}", provider.task_id().as_str())),
        Arc::new(SystemClock),
    )
    .await
    .map_err(BrowserTaskError::from)?;

    let execution = execute_with_bus(
        &bus,
        provider,
        tool_schemas,
        selection.app_id(),
        SystemClock.now_unix_ms(),
        &options.approved_step_ids,
    )
    .await;
    let shutdown = bus.shutdown().await.map_err(BrowserTaskError::from);
    let close = browser.close().map_err(BrowserRuntimeError::from);

    let result = execution?;
    shutdown?;
    close?;
    let csv = options
        .csv
        .as_ref()
        .map(|csv| normalize_t5_1_outputs(&result.outputs, &csv.origin))
        .transpose()?;
    let csv_path = options.csv.as_ref().map(|csv| csv.output_path.as_path());
    write_output(&options.output_path, &result, csv_path, csv.as_ref())?;
    Ok(result)
}

fn browser_tool_schemas(
    schemas: Vec<ToolSchema>,
    selection: crate::browser_runtime::BrowserSelection,
) -> Result<Vec<ToolSchema>, BrowserTaskError> {
    let prefix = format!("{}.cdp.", selection.as_str());
    let schemas = schemas
        .into_iter()
        .filter(|schema| schema.name.starts_with(&prefix))
        .collect::<Vec<_>>();
    if schemas.len() == 3 {
        Ok(schemas)
    } else {
        Err(BrowserTaskError::Arguments {
            reason: format!(
                "browser task runner requires exactly three `{prefix}` schemas, got {}",
                schemas.len()
            ),
        })
    }
}

async fn execute_with_bus(
    bus: &ToolBus,
    provider: TaskPackageProvider,
    tool_schemas: Vec<ToolSchema>,
    app_id: &str,
    now_ms: i64,
    approved_step_ids: &[String],
) -> Result<BrowserTaskResult, BrowserTaskError> {
    let task_id = provider.task_id().clone();
    let provider_arc: Arc<dyn ModelProvider> = Arc::new(provider.clone());
    let request = PlannerRequest::new(
        PlanId::new(format!("p_{}", task_id.as_str())).map_err(|error| {
            BrowserTaskError::Planner {
                reason: error.to_string(),
            }
        })?,
        task_id.clone(),
        provider.goal().to_owned(),
        tool_schemas.clone(),
        Budget::new(
            PLAN_BUDGET_STEPS,
            PLAN_BUDGET_MS,
            PLAN_BUDGET_TOKENS,
            PLAN_BUDGET_COST_USD,
        )
        .map_err(|error| BrowserTaskError::Planner {
            reason: error.to_string(),
        })?,
    )
    .map_err(|error| BrowserTaskError::Planner {
        reason: error.to_string(),
    })?;
    let plan = Planner::new(provider_arc)
        .generate_plan(&request, CancellationToken::new())
        .map_err(|error| BrowserTaskError::Planner {
            reason: error.to_string(),
        })?;
    execute_plan(
        bus,
        provider,
        plan,
        tool_schemas,
        BrowserExecution {
            app_id,
            now_ms,
            approved_step_ids,
        },
    )
    .await
}

async fn execute_plan(
    bus: &ToolBus,
    provider: TaskPackageProvider,
    plan: Plan,
    tool_schemas: Vec<ToolSchema>,
    execution: BrowserExecution<'_>,
) -> Result<BrowserTaskResult, BrowserTaskError> {
    let task_id = plan.task_id.clone();
    let ordered = plan
        .ordered_steps()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let catalog = tool_schemas
        .into_iter()
        .map(|schema| (schema.name.clone(), schema))
        .collect::<BTreeMap<_, _>>();
    let policy = prepare_browser_policy(&task_id, &catalog, &ordered, &execution)?;
    let mut engine = TaskEngine::new(assistant_task_engine::MemoryCheckpointStore::new());
    engine.create_task(plan, execution.now_ms)?;
    engine.apply_task_event(
        &task_id,
        TaskEvent::SubmitForApproval,
        execution.now_ms.saturating_add(1),
    )?;
    engine.apply_task_event(
        &task_id,
        TaskEvent::ApprovePlan,
        execution.now_ms.saturating_add(2),
    )?;

    let envelopes = Arc::new(Mutex::new(BTreeMap::<String, ToolEnvelope>::new()));
    let binding_state = Arc::new(Mutex::new(RuntimeBindingState::new(Map::new())));
    let latest_snapshot = Arc::new(Mutex::new(None::<TaskSnapshot>));
    let invoker = RecordingInvoker::new(ToolBusInvoker::new(bus), Arc::clone(&envelopes));
    let binding = BindingInvoker::new(
        invoker,
        Arc::new(provider.dataflow_plan().clone()),
        Arc::clone(&binding_state),
        Arc::clone(&latest_snapshot),
    );
    let executor = RuntimeExecutor::new(engine, policy, binding, EnvelopeObservationCollector);
    let (executor, steps) = drive_steps(
        executor,
        &task_id,
        &ordered,
        &binding_state,
        &latest_snapshot,
        execution.now_ms,
    )
    .await?;
    let engine = executor.into_engine();
    let snapshot = engine.load_snapshot(&task_id)?;
    if snapshot.status != TaskStatus::Completed {
        return Err(BrowserTaskError::StepNotCommitted {
            step_id: "<task>".to_owned(),
            outcome: format!("{:?}", snapshot.status),
        });
    }
    let outputs = collect_outputs(provider.dataflow_plan(), &envelopes)?;
    Ok(BrowserTaskResult {
        task_id: provider.declared_task_id().to_owned(),
        outputs,
        steps,
    })
}

fn prepare_browser_policy(
    task_id: &TaskId,
    catalog: &BTreeMap<String, ToolSchema>,
    ordered: &[PlanStep],
    execution: &BrowserExecution<'_>,
) -> Result<BrowserTaskPolicy, BrowserTaskError> {
    let approval_grants = Arc::new(ApprovalGrants::new());
    let policy = BrowserTaskPolicy::new(
        RuleSet::example_v0(),
        catalog.clone(),
        execution.app_id.to_owned(),
        Arc::clone(&approval_grants),
        task_id.clone(),
    );
    let approved_steps =
        validate_step_approvals(ordered, execution.approved_step_ids, &policy, catalog)?;
    grant_step_approvals(&approval_grants, task_id, &approved_steps, execution.now_ms)?;
    Ok(policy)
}

async fn drive_steps<'bus>(
    mut executor: BrowserExecutor<'bus>,
    task_id: &TaskId,
    ordered: &[PlanStep],
    binding_state: &Arc<Mutex<RuntimeBindingState>>,
    latest_snapshot: &Arc<Mutex<Option<TaskSnapshot>>>,
    now_ms: i64,
) -> Result<(BrowserExecutor<'bus>, Vec<Value>), BrowserTaskError> {
    let mut steps = Vec::new();
    for step in ordered {
        let outcome = executor
            .advance(task_id, &step.id, now_ms.saturating_add(3))
            .await?;
        let snapshot = match outcome {
            StepExecutionOutcome::Committed(snapshot) => snapshot,
            other => {
                binding_state
                    .lock()
                    .map_err(|_| {
                        BrowserTaskError::Runtime(RuntimeExecutionError::Tool {
                            reason: "runtime binding state is unavailable".to_owned(),
                        })
                    })?
                    .discard(step.id.as_str());
                return Err(BrowserTaskError::StepNotCommitted {
                    step_id: step.id.to_string(),
                    outcome: format!("{other:?}"),
                });
            }
        };
        binding_state
            .lock()
            .map_err(|_| {
                BrowserTaskError::Runtime(RuntimeExecutionError::Tool {
                    reason: "runtime binding state is unavailable".to_owned(),
                })
            })?
            .commit(step.id.as_str());
        *latest_snapshot.lock().map_err(|_| {
            BrowserTaskError::Runtime(RuntimeExecutionError::Tool {
                reason: "latest task snapshot state is unavailable".to_owned(),
            })
        })? = Some(snapshot.clone());
        let step_snapshot =
            snapshot
                .step(&step.id)
                .ok_or_else(|| BrowserTaskError::StepNotCommitted {
                    step_id: step.id.to_string(),
                    outcome: "committed snapshot omitted the step".to_owned(),
                })?;
        steps.push(json!({
            "id": step.id.as_str(),
            "tool": step.tool,
            "status": "committed",
            "post_fingerprint": step_snapshot.post_fingerprint,
        }));
    }
    Ok((executor, steps))
}

struct RecordingInvoker<Inner> {
    inner: Inner,
    envelopes: Arc<Mutex<BTreeMap<String, ToolEnvelope>>>,
}

impl<Inner> RecordingInvoker<Inner> {
    const fn new(inner: Inner, envelopes: Arc<Mutex<BTreeMap<String, ToolEnvelope>>>) -> Self {
        Self { inner, envelopes }
    }
}

impl<Inner> ToolInvoker for RecordingInvoker<Inner>
where
    Inner: ToolInvoker + Send + Sync,
{
    fn invoke(
        &self,
        task_id: &TaskId,
        step: &PlanStep,
    ) -> impl Future<Output = Result<ToolEnvelope, RuntimeExecutionError>> + Send {
        let step_id = step.id.to_string();
        let invocation = self.inner.invoke(task_id, step);
        async move {
            let envelope = invocation.await?;
            self.envelopes
                .lock()
                .map_err(|_| RuntimeExecutionError::Tool {
                    reason: "browser task envelope recorder is unavailable".to_owned(),
                })?
                .insert(step_id, envelope.clone());
            Ok(envelope)
        }
    }
}

fn collect_outputs(
    dataflow: &assistant_agent_core::RuntimeDataflowPlan,
    envelopes: &Arc<Mutex<BTreeMap<String, ToolEnvelope>>>,
) -> Result<Map<String, Value>, BrowserTaskError> {
    let recorded = envelopes
        .lock()
        .map_err(|_| BrowserTaskError::ToolBus {
            reason: "browser task envelope recorder is unavailable".to_owned(),
        })?
        .clone();
    let mut outputs = Map::new();
    for step_id in dataflow.step_ids() {
        let Some(binding) = dataflow.get(&step_id) else {
            continue;
        };
        let Some(envelope) = recorded.get(&step_id) else {
            continue;
        };
        let Some(data) = envelope.data.as_ref().and_then(|data| data.0.as_object()) else {
            continue;
        };
        for output in &binding.outputs {
            let value = extract_browser_output(&binding.tool, output, data).ok_or_else(|| {
                BrowserTaskError::MissingOutput {
                    step_id: step_id.clone(),
                    output: output.clone(),
                }
            })?;
            outputs.insert(output.clone(), value);
        }
    }
    Ok(outputs)
}

fn extract_browser_output(tool: &str, output: &str, data: &Map<String, Value>) -> Option<Value> {
    if let Some(value) = data.get(output) {
        return Some(value.clone());
    }
    match (tool, output) {
        ("edge.cdp.navigate" | "chrome.cdp.navigate", output) if output.ends_with("_ready") => {
            data.get("ready").cloned()
        }
        ("edge.cdp.dom_read" | "chrome.cdp.dom_read", output) if output.ends_with("_dom") => {
            data.get("dom").cloned()
        }
        _ => None,
    }
}

#[cfg(test)]
#[path = "browser_task_runtime_tests.rs"]
mod tests;
