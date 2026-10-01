//! Executable self-check and production composition roots.
//!
//! `--self-check` proves the assembly point is executable with deterministic
//! dependencies. `--production` assembles the adapter tools, task-package Plan
//! source, `RuntimeExecutor`, and UI event source for the 1a dry run.

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use assistant_agent_core::{
    HostAssembly, HostAssemblyError, HostAssemblyInput, ProductionConfig, ProductionError,
    TaskControlHandler, UiServerConfig, assemble_production_host, serve_with_events,
};
use assistant_audit::Durability;
use assistant_core::{
    AppMapFileReader, AppMapReadError, CompressionError, ContextFragment, ContextSummary,
    HistoryCompressor, MemoryRetrievalError, MemoryRetrievalHit, MemoryRetriever,
    MemorySessionStore, SessionStore,
};
use assistant_model_gateway::{
    CompletionEvent, CompletionRequest, CompletionStream, DurationMs, FinishReason, Message,
    ModelGatewayError, ModelId, ModelProvider, ModelResult, ModelRouter, NoJitter, Pricing,
    ProviderCapabilities, ProviderFeatures, SystemMonotonicClock, ThreadSleeper, TokenCount, Usage,
};
use assistant_platform_windows::WindowsPlatform;
use assistant_policy::RuleSet;
use assistant_storage::{Clock, MemoryQuery, SystemClock};
use assistant_tool_bus::ToolRegistry;
use serde_json::{Map, Value};

struct EmptyRetriever;

impl MemoryRetriever for EmptyRetriever {
    fn retrieve(
        &self,
        _query: &MemoryQuery,
    ) -> Result<Vec<MemoryRetrievalHit>, MemoryRetrievalError> {
        Ok(Vec::new())
    }
}

struct EmptyReader;

impl AppMapFileReader for EmptyReader {
    fn read_to_string(&self, _relative_path: &std::path::Path) -> Result<String, AppMapReadError> {
        Err(AppMapReadError::NotFound)
    }
}

struct NoopCompressor;

impl HistoryCompressor for NoopCompressor {
    fn summarize(
        &self,
        _fragments: &[ContextFragment],
    ) -> Result<ContextSummary, CompressionError> {
        Err(CompressionError::new(
            "compressor_unavailable",
            assistant_protocol::ErrorCode::CapabilityMissing,
            "self-check does not invoke context compression",
        ))
    }
}

struct NoopStream {
    step: u8,
}

impl CompletionStream for NoopStream {
    fn next_event(
        &mut self,
        _cancellation: &assistant_model_gateway::CancellationToken,
        _timeout: DurationMs,
    ) -> ModelResult<Option<CompletionEvent>> {
        let event = match self.step {
            0 => CompletionEvent::Usage(Usage::new(
                TokenCount::new(1),
                TokenCount::new(1),
                TokenCount::new(1),
            )),
            1 => CompletionEvent::Finished(FinishReason::Stop),
            _ => return Ok(None),
        };
        self.step = self.step.saturating_add(1);
        Ok(Some(event))
    }
}

struct NoopProvider {
    model_id: ModelId,
}

impl NoopProvider {
    fn new() -> Result<Self, ModelGatewayError> {
        Ok(Self {
            model_id: ModelId::new("self_check")?,
        })
    }
}

impl ModelProvider for NoopProvider {
    fn model_id(&self) -> &ModelId {
        &self.model_id
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities::new(ProviderFeatures::STREAMING, TokenCount::new(8_192))
    }

    fn pricing(&self) -> Pricing {
        Pricing::new(1, 1, 1)
    }

    fn count_tokens(&self, messages: &[Message]) -> ModelResult<TokenCount> {
        Ok(TokenCount::new(
            u64::try_from(messages.len()).unwrap_or(u64::MAX).max(1),
        ))
    }

    fn complete(
        &self,
        _request: CompletionRequest,
        _cancellation: assistant_model_gateway::CancellationToken,
    ) -> ModelResult<Box<dyn CompletionStream>> {
        Ok(Box::new(NoopStream { step: 0 }))
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    if arguments.is_empty()
        || (arguments.len() == 1
            && arguments
                .first()
                .is_some_and(|argument| argument == "--self-check"))
    {
        return match run_self_check().await {
            Ok(()) => {
                if report_ok() {
                    ExitCode::SUCCESS
                } else {
                    ExitCode::FAILURE
                }
            }
            Err(error) => {
                let _ = report_error(&error.to_string());
                ExitCode::from(2)
            }
        };
    }
    if arguments
        .first()
        .is_some_and(|argument| argument == "--production")
    {
        let rest = arguments.get(1..).unwrap_or(&[]);
        return match parse_production_options(rest) {
            Ok(options) => match run_production(options.config, options.serve_ui).await {
                Ok(report) => {
                    if report_production_ok(&report) {
                        ExitCode::SUCCESS
                    } else {
                        ExitCode::FAILURE
                    }
                }
                Err(error) => {
                    let _ = report_error(&error.to_string());
                    ExitCode::from(2)
                }
            },
            Err(reason) => {
                let _ = report_error(&reason);
                ExitCode::from(2)
            }
        };
    }
    let _ = report_error(
        "usage: assistant-agent-core [--self-check | --production \
         --task-package <path> --ui-peer <path> [--task-inputs <path>] [--adapter-root <path>] \
         [--data-root <path>] [--ui-pipe <name>] [--ui-token-env <name>] [--serve-ui]]",
    );
    ExitCode::from(2)
}

struct ProductionOptions {
    config: ProductionConfig,
    serve_ui: bool,
}

fn parse_production_options(arguments: &[String]) -> Result<ProductionOptions, String> {
    let mut adapter_root = PathBuf::from("adapters/com.microsoft.notepad");
    let mut data_root = std::env::temp_dir().join(format!(
        "assistant-agent-core-production-{}",
        std::process::id()
    ));
    let mut task_package_path = None;
    let mut task_inputs_path = None;
    let mut ui_pipe = "assistant-agent-core-ui".to_owned();
    let mut ui_token_environment_variable = "ASSISTANT_AGENT_CORE_UI_TOKEN".to_owned();
    let mut ui_peer = None;
    let mut serve_ui = false;
    let mut iterator = arguments.iter();
    while let Some(argument) = iterator.next() {
        match argument.as_str() {
            "--adapter-root" => {
                adapter_root = PathBuf::from(next_value(&mut iterator, argument)?);
            }
            "--data-root" => {
                data_root = PathBuf::from(next_value(&mut iterator, argument)?);
            }
            "--task-package" => {
                task_package_path = Some(PathBuf::from(next_value(&mut iterator, argument)?));
            }
            "--task-inputs" => {
                task_inputs_path = Some(PathBuf::from(next_value(&mut iterator, argument)?));
            }
            "--ui-pipe" => {
                ui_pipe = next_value(&mut iterator, argument)?;
            }
            "--ui-token-env" => {
                ui_token_environment_variable = next_value(&mut iterator, argument)?;
            }
            "--ui-peer" => {
                ui_peer = Some(PathBuf::from(next_value(&mut iterator, argument)?));
            }
            "--serve-ui" => {
                serve_ui = true;
            }
            _ => return Err(format!("unknown production argument `{argument}`")),
        }
    }
    let task_package_path =
        task_package_path.ok_or_else(|| "missing --task-package <path>".to_owned())?;
    let task_inputs = match task_inputs_path {
        Some(path) => read_task_inputs(&path)?,
        None => Map::new(),
    };
    let ui_peer = ui_peer.ok_or_else(|| "missing --ui-peer <path>".to_owned())?;
    let ui_config = UiServerConfig::new(
        ui_pipe,
        ui_token_environment_variable,
        Duration::from_secs(10),
    )
    .with_allowed_peer(ui_peer);
    Ok(ProductionOptions {
        config: ProductionConfig::new(data_root, adapter_root, task_package_path, ui_config)
            .with_task_inputs(task_inputs),
        serve_ui,
    })
}

fn read_task_inputs(path: &Path) -> Result<Map<String, Value>, String> {
    let body = std::fs::read_to_string(path)
        .map_err(|error| format!("failed to read task inputs `{}`: {error}", path.display()))?;
    let value: Value = serde_json::from_str(&body).map_err(|error| {
        format!(
            "task inputs `{}` are not valid JSON: {error}",
            path.display()
        )
    })?;
    value
        .as_object()
        .cloned()
        .ok_or_else(|| "task inputs must be a JSON object".to_owned())
}

fn next_value(iterator: &mut std::slice::Iter<'_, String>, option: &str) -> Result<String, String> {
    iterator
        .next()
        .filter(|value| !value.starts_with("--"))
        .cloned()
        .ok_or_else(|| format!("{option} requires a value"))
}

struct ProductionReport {
    task_id: String,
    status: &'static str,
    committed_steps: usize,
}

async fn run_production(
    config: ProductionConfig,
    serve_ui: bool,
) -> Result<ProductionReport, ProductionError> {
    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    let now_ms = clock.now_unix_ms();
    let host = assemble_production_host(config, WindowsPlatform::new(), Arc::clone(&clock)).await?;
    let plan = host.plan_task()?;
    let task_id = plan.task_id.to_string();
    let run = host.execute_plan(plan, now_ms).await?;
    let report = ProductionReport {
        task_id,
        status: run.final_snapshot.status.as_str(),
        committed_steps: run.snapshots.len(),
    };
    if serve_ui {
        let mut handler = TaskControlHandler::new(run.into_engine(), Arc::clone(&clock))
            .with_approvals(host.approvals());
        let mut events = host.snapshot_event_source();
        serve_with_events(host.ui_config(), &mut handler, &mut events).map_err(|error| {
            ProductionError::UiServer {
                reason: error.to_string(),
            }
        })?;
    } else {
        let _engine = run.into_engine();
    }
    host.shutdown().await?;
    Ok(report)
}

fn report_production_ok(report: &ProductionReport) -> bool {
    let mut stdout = io::stdout().lock();
    writeln!(
        stdout,
        "assistant-agent-core production: task={} status={} committed_steps={}",
        report.task_id, report.status, report.committed_steps
    )
    .is_ok()
}

async fn run_self_check() -> Result<(), HostAssemblyError> {
    let data_root = std::env::temp_dir().join(format!(
        "assistant-agent-core-self-check-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&data_root).map_err(|error| {
        HostAssemblyError::InvalidConfiguration {
            field: "self_check_data_root",
            reason: error.to_string(),
        }
    })?;

    let provider: Arc<dyn ModelProvider> =
        Arc::new(
            NoopProvider::new().map_err(|error| HostAssemblyError::InvalidConfiguration {
                field: "self_check_provider",
                reason: error.to_string(),
            })?,
        );
    let model_id = provider.model_id().clone();
    let router =
        ModelRouter::new(model_id.clone(), Vec::new(), vec![model_id]).map_err(|error| {
            HostAssemblyError::InvalidConfiguration {
                field: "self_check_router",
                reason: error.to_string(),
            }
        })?;
    let input = HostAssemblyInput::new(&data_root, Arc::new(SystemClock), WindowsPlatform::new())
        .with_session_store(Arc::new(MemorySessionStore::new()) as Arc<dyn SessionStore>)
        .with_memory_retriever(Arc::new(EmptyRetriever))
        .with_app_map_reader(Arc::new(EmptyReader))
        .with_planner_provider(Arc::clone(&provider))
        .with_model_router(router)
        .with_model_providers(vec![provider])
        .with_model_runtime(
            Arc::new(SystemMonotonicClock::default()),
            Arc::new(ThreadSleeper),
            Arc::new(NoJitter),
        )
        .with_policy_rules(RuleSet::example_v0())
        .with_tool_registry(ToolRegistry::new())
        .with_history_compressor(Arc::new(NoopCompressor))
        .with_durability(Durability::Immediate);
    let host = HostAssembly::new(input).assemble().await?;
    host.shutdown().await?;

    if let Err(error) = std::fs::remove_dir_all(&data_root)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        return Err(HostAssemblyError::InvalidConfiguration {
            field: "self_check_cleanup",
            reason: error.to_string(),
        });
    }
    Ok(())
}

fn report_ok() -> bool {
    let mut stdout = io::stdout().lock();
    writeln!(stdout, "assistant-agent-core self-check: ok").is_ok()
}

fn report_error(error: &str) -> bool {
    let mut stderr = io::stderr().lock();
    writeln!(stderr, "assistant-agent-core failed: {error}").is_ok()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use assistant_core::{AppMapFileReader, HistoryCompressor, MemoryRetriever};
    use assistant_model_gateway::{CancellationToken, CompletionStream, DurationMs, ModelProvider};

    use super::{
        EmptyReader, EmptyRetriever, NoopCompressor, NoopProvider, NoopStream, run_self_check,
    };

    #[cfg(windows)]
    #[tokio::test]
    async fn test_self_check_assembles_and_shuts_down() -> Result<(), super::HostAssemblyError> {
        run_self_check().await
    }

    #[cfg(not(windows))]
    #[tokio::test]
    async fn test_self_check_fails_closed_on_unsupported_platform()
    -> Result<(), Box<dyn std::error::Error>> {
        let Some(error) = run_self_check().await.err() else {
            return Err("non-Windows self-check unexpectedly succeeded".into());
        };
        assert!(matches!(
            error,
            super::HostAssemblyError::InvalidConfiguration {
                field: "platform",
                ..
            }
        ));
        Ok(())
    }

    #[test]
    fn test_self_check_helpers_are_explicit() -> Result<(), Box<dyn std::error::Error>> {
        assert!(
            EmptyRetriever
                .retrieve(&assistant_storage::MemoryQuery::new("missing"))?
                .is_empty()
        );
        assert!(matches!(
            EmptyReader.read_to_string(Path::new("missing.json")),
            Err(assistant_core::AppMapReadError::NotFound)
        ));
        assert!(NoopCompressor.summarize(&[]).is_err());

        let mut stream = NoopStream { step: 0 };
        let cancellation = CancellationToken::new();
        assert!(
            stream
                .next_event(&cancellation, DurationMs::new(1))?
                .is_some()
        );
        assert!(
            stream
                .next_event(&cancellation, DurationMs::new(1))?
                .is_some()
        );
        assert!(
            stream
                .next_event(&cancellation, DurationMs::new(1))?
                .is_none()
        );

        let provider = NoopProvider::new()?;
        assert_eq!(provider.model_id().as_str(), "self_check");
        assert_eq!(provider.count_tokens(&[])?.get(), 1);
        Ok(())
    }
}
