//! Executable self-check composition root.
//!
//! The production composition root will supply a real Provider, persistent
//! `SessionStore`, and adapter package. This binary proves that the assembly
//! point is executable with explicit deterministic dependencies and fails
//! closed when any required component is missing.

use std::io::{self, Write};
use std::process::ExitCode;
use std::sync::Arc;

use assistant_agent_core::{HostAssembly, HostAssemblyError, HostAssemblyInput};
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
use assistant_storage::{MemoryQuery, SystemClock};
use assistant_tool_bus::ToolRegistry;

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
    let mut arguments = std::env::args().skip(1);
    if let Some(argument) = arguments.next()
        && argument != "--self-check"
    {
        return ExitCode::from(2);
    }
    if arguments.next().is_some() {
        return ExitCode::from(2);
    }

    match run_self_check().await {
        Ok(()) => {
            if report_ok() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(error) => {
            let _ = report_error(&error);
            ExitCode::from(2)
        }
    }
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

fn report_error(error: &HostAssemblyError) -> bool {
    let mut stderr = io::stderr().lock();
    writeln!(stderr, "assistant-agent-core self-check failed: {error}").is_ok()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use assistant_core::{AppMapFileReader, HistoryCompressor, MemoryRetriever};
    use assistant_model_gateway::{CancellationToken, CompletionStream, DurationMs, ModelProvider};

    use super::{
        EmptyReader, EmptyRetriever, NoopCompressor, NoopProvider, NoopStream, run_self_check,
    };

    #[tokio::test]
    async fn test_self_check_assembles_and_shuts_down() -> Result<(), super::HostAssemblyError> {
        run_self_check().await
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
