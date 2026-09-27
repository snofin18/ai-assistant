//! Contract tests for the single binary-layer Host assembly point.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(windows)]
use assistant_agent_core::{
    CharacterTokenEstimator, RootedAppMapReader, StorageMemoryRetriever, StorageSessionClock,
    StorageToolClock,
};
use assistant_agent_core::{HostAssembly, HostAssemblyError, HostAssemblyInput};
#[cfg(windows)]
use assistant_audit::{AppendOutcome, AuditSubject};
#[cfg(windows)]
use assistant_core::SessionClock;
use assistant_core::{
    AppMapFileReader, AppMapReadError, ContextFragment, ContextSummary, HistoryCompressor,
    MemoryRetrievalHit, MemoryRetriever, MemorySessionStore, SessionStore,
};
use assistant_model_gateway::{
    CompletionEvent, CompletionRequest, CompletionStream, DurationMs, Message, ModelGatewayError,
    ModelId, ModelProvider, ModelResult, ModelRouter, NoJitter, Pricing, ProviderCapabilities,
    ProviderFeatures, RetryPolicy, SystemMonotonicClock, ThreadSleeper, TokenCount, Usage,
};
use assistant_platform_windows::WindowsPlatform;
use assistant_policy::RuleSet;
use assistant_protocol::{ErrorCode, RiskLevel, ToolEffect, ToolReversibility};
use assistant_storage::{Clock, MemoryQuery};
#[cfg(windows)]
use assistant_storage::{
    Database, MIGRATIONS as STORAGE_MIGRATIONS, MemoryRecord, MemoryRecordKind, MigrationSet,
    StoragePaths, insert_memory_record,
};
#[cfg(windows)]
use assistant_tool_bus::Clock as ToolClock;
use assistant_tool_bus::{
    CallContext, ToolBusError, ToolDefinition, ToolHandler, ToolOutput, ToolRegistry,
};
use serde_json::{Map, Value};

const FIXED_NOW_MS: i64 = 1_700_000_000_000;

#[derive(Debug)]
struct FixedClock;

impl Clock for FixedClock {
    fn now_unix_ms(&self) -> i64 {
        FIXED_NOW_MS
    }
}

struct EmptyRetriever;

impl MemoryRetriever for EmptyRetriever {
    fn retrieve(
        &self,
        _query: &MemoryQuery,
    ) -> Result<Vec<MemoryRetrievalHit>, assistant_core::MemoryRetrievalError> {
        Ok(Vec::new())
    }
}

struct EmptyReader;

impl AppMapFileReader for EmptyReader {
    fn read_to_string(&self, _relative_path: &Path) -> Result<String, AppMapReadError> {
        Err(AppMapReadError::NotFound)
    }
}

struct NoopCompressor;

impl HistoryCompressor for NoopCompressor {
    fn summarize(
        &self,
        _fragments: &[ContextFragment],
    ) -> Result<ContextSummary, assistant_core::CompressionError> {
        Err(assistant_core::CompressionError::new(
            "compressor_unavailable",
            ErrorCode::CapabilityMissing,
            "assembly contract test does not invoke context compression",
        ))
    }
}

struct NoopStream;

impl CompletionStream for NoopStream {
    fn next_event(
        &mut self,
        _cancellation: &assistant_model_gateway::CancellationToken,
        _timeout: DurationMs,
    ) -> ModelResult<Option<CompletionEvent>> {
        Ok(Some(CompletionEvent::Usage(Usage::new(
            TokenCount::new(1),
            TokenCount::new(1),
            TokenCount::new(1),
        ))))
    }
}

struct NoopProvider {
    model_id: ModelId,
}

impl NoopProvider {
    fn new() -> Result<Self, ModelGatewayError> {
        Ok(Self {
            model_id: ModelId::new("assembly_noop")?,
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

    fn count_tokens(&self, _messages: &[Message]) -> ModelResult<TokenCount> {
        Ok(TokenCount::new(1))
    }

    fn complete(
        &self,
        _request: CompletionRequest,
        _cancellation: assistant_model_gateway::CancellationToken,
    ) -> ModelResult<Box<dyn CompletionStream>> {
        Ok(Box::new(NoopStream))
    }
}

struct EchoTool;

impl ToolHandler for EchoTool {
    fn call(
        &self,
        _call: &CallContext,
        arguments: &Map<String, Value>,
    ) -> Result<ToolOutput, ToolBusError> {
        Ok(ToolOutput::json(Value::Object(arguments.clone())))
    }
}

struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    fn new(label: &str) -> std::io::Result<Self> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(std::io::Error::other)?
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "assistant-agent-core-{label}-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path)?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn test_registry() -> Result<ToolRegistry, ToolBusError> {
    let mut registry = ToolRegistry::new();
    let definition = ToolDefinition::new(
        "assembly.echo.echo",
        "Echo arguments for the assembly contract test.",
        RiskLevel::Low,
        ToolEffect::Read,
        ToolReversibility::L0UndoStack,
        serde_json::json!({"type": "object"}),
    )?;
    registry.register(definition, Arc::new(EchoTool))?;
    Ok(registry)
}

fn base_input(
    directory: &TestDirectory,
    provider: Arc<dyn ModelProvider>,
) -> Result<HostAssemblyInput<WindowsPlatform>, ModelGatewayError> {
    let model_id = provider.model_id().clone();
    let router = ModelRouter::new(model_id.clone(), Vec::new(), vec![model_id])?;
    Ok(HostAssemblyInput::new(
        directory.path(),
        Arc::new(FixedClock),
        WindowsPlatform::new(),
    )
    .with_planner_provider(Arc::clone(&provider))
    .with_model_router(router)
    .with_model_providers(vec![provider])
    .with_model_runtime(
        Arc::new(SystemMonotonicClock::default()),
        Arc::new(ThreadSleeper),
        Arc::new(NoJitter),
    )
    .with_retry_policy(RetryPolicy::default())
    .with_policy_rules(RuleSet::example_v0())
    .with_history_compressor(Arc::new(NoopCompressor)))
}

#[cfg(windows)]
#[tokio::test]
async fn test_host_assembly_constructs_all_components() -> Result<(), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new("all-components")?;
    let provider: Arc<dyn ModelProvider> = Arc::new(NoopProvider::new()?);
    let input = base_input(&directory, Arc::clone(&provider))?
        .with_session_store(Arc::new(MemorySessionStore::new()) as Arc<dyn SessionStore>)
        .with_memory_retriever(Arc::new(EmptyRetriever))
        .with_app_map_reader(Arc::new(EmptyReader))
        .with_tool_registry(test_registry()?)
        .with_durability(assistant_audit::Durability::Immediate);

    let host = HostAssembly::new(input).assemble().await?;
    assert_eq!(host.toolset_report().mounted.len(), 3);
    assert!(host.audit().verify_chain()?.is_intact());
    assert!(
        host.sessions()
            .lock()
            .expect("session lock")
            .snapshot(&assistant_core::SessionId::new("missing")?)
            .is_err()
    );
    host.shutdown().await?;
    Ok(())
}

#[cfg(windows)]
#[tokio::test]
async fn test_host_assembly_rejects_missing_component() -> Result<(), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new("missing-component")?;
    let provider: Arc<dyn ModelProvider> = Arc::new(NoopProvider::new()?);
    let input = base_input(&directory, provider)?.with_tool_registry(test_registry()?);

    let Err(error) = HostAssembly::new(input).assemble().await else {
        panic!("missing session store must fail");
    };
    assert_eq!(error.reason_code(), "host_component_missing");
    assert_eq!(error.error_code(), ErrorCode::CapabilityMissing);
    assert!(matches!(
        error,
        HostAssemblyError::MissingComponent {
            component: "session_store"
        }
    ));
    Ok(())
}

#[cfg(windows)]
#[tokio::test]
async fn test_assembly_adapters_cover_storage_and_audit_paths()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new("adapters")?;
    let provider: Arc<dyn ModelProvider> = Arc::new(NoopProvider::new()?);
    let input = base_input(&directory, provider)?
        .with_session_store(Arc::new(MemorySessionStore::new()) as Arc<dyn SessionStore>)
        .with_memory_retriever(Arc::new(EmptyRetriever))
        .with_app_map_reader(Arc::new(EmptyReader))
        .with_tool_registry(test_registry()?)
        .with_durability(assistant_audit::Durability::Immediate);
    let host = HostAssembly::new(input).assemble().await?;

    let database = host.database_handle();
    {
        let database = database.lock().expect("database lock");
        insert_memory_record(
            database.connection(),
            &MemoryRecord {
                record_kind: MemoryRecordKind::Note,
                record_id: "note-1".to_owned(),
                source_reference: "test#L1".to_owned(),
                content: "alpha beta".to_owned(),
                updated_at: FIXED_NOW_MS,
            },
        )?;
    }
    let retriever =
        StorageMemoryRetriever::new(host.database_handle(), Arc::new(CharacterTokenEstimator));
    let hits = retriever.retrieve(&MemoryQuery::new("alpha"))?;
    assert_eq!(hits.len(), 1);
    assert_eq!(
        hits.first().expect("hit").token_estimate(),
        assistant_core::TokenCount::new(10)
    );

    assert_eq!(
        StorageSessionClock::new(Arc::new(FixedClock)).now_unix_ms(),
        FIXED_NOW_MS
    );
    assert_eq!(
        StorageToolClock::new(Arc::new(FixedClock)).now_unix_ms(),
        FIXED_NOW_MS
    );

    let reader = RootedAppMapReader::new(directory.path());
    assert!(reader.read_to_string(Path::new("missing.json")).is_err());
    std::fs::write(directory.path().join("app.json"), "{}")?;
    assert_eq!(reader.read_to_string(Path::new("app.json"))?, "{}");

    let event = serde_json::from_value(serde_json::json!({
        "version": "1.0",
        "event_type": "incident.reported",
        "ts": "2026-09-27T00:00:00.000Z",
        "session_id": "assembly-test",
        "actor": "system",
        "action": "assembly.test",
        "args": {"reason": "adapter coverage"},
        "prev_hash": "",
        "self_hash": ""
    }))?;
    assert!(matches!(
        host.audit().append(&event, &AuditSubject::unattached())?,
        AppendOutcome::Flushed { written: 1 }
    ));
    assert!(host.audit().verify_chain()?.is_intact());

    let _ = host.database();
    let _ = host.sessions();
    let _ = host.context();
    let _ = host.planner();
    let _ = host.memory();
    let _ = host.model_gateway();
    let _ = host.policy();
    let _ = host.tool_bus();
    let _ = host.toolset_report();
    let _ = host.compressor();
    let _ = host.platform();
    host.shutdown().await?;
    Ok(())
}

#[test]
fn test_assembly_point_is_confined_to_agent_core() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir.join("../..");
    let core_manifest = std::fs::read_to_string(workspace_root.join("crates/core/Cargo.toml"))
        .expect("read core manifest");
    for forbidden in [
        "assistant-audit",
        "assistant-policy",
        "assistant-tool-bus",
        "assistant-verify",
        "assistant-undo",
        "assistant-lease",
        "assistant-secrets",
        "assistant-ipc",
    ] {
        assert!(
            !core_manifest.contains(forbidden),
            "core manifest must not contain {forbidden}"
        );
    }

    let assembly_source =
        std::fs::read_to_string(manifest_dir.join("src/assembly.rs")).expect("read assembly");
    for marker in [
        "assistant_audit::MIGRATIONS",
        "ToolBus::start",
        "ModelGateway::with_runtime",
        "AuditSink::new",
    ] {
        assert!(assembly_source.contains(marker), "missing marker {marker}");
    }
}

#[cfg(windows)]
#[tokio::test]
async fn test_host_assembly_rejects_non_intact_audit_chain()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new("broken-audit-chain")?;
    let mut migrations = MigrationSet::new();
    migrations.register_all(STORAGE_MIGRATIONS)?;
    migrations.register_all(assistant_audit::MIGRATIONS)?;
    let database = Database::open(
        &StoragePaths::new(directory.path()),
        Arc::new(FixedClock),
        &migrations,
    )?;
    database.connection().execute_batch(
        "INSERT INTO audit_logs
         (id, prev_hash, ts, actor, task_id, step_id, event_type, detail_json)
         VALUES ('broken', 'broken', 0, 'system', NULL, NULL, 'incident.reported', '{}')",
    )?;
    drop(database);

    let provider: Arc<dyn ModelProvider> = Arc::new(NoopProvider::new()?);
    let input = base_input(&directory, provider)?
        .with_session_store(Arc::new(MemorySessionStore::new()) as Arc<dyn SessionStore>)
        .with_memory_retriever(Arc::new(EmptyRetriever))
        .with_app_map_reader(Arc::new(EmptyReader))
        .with_tool_registry(test_registry()?)
        .with_durability(assistant_audit::Durability::Immediate);

    let Err(error) = HostAssembly::new(input).assemble().await else {
        panic!("non-intact audit chain must fail assembly");
    };
    assert_eq!(error.reason_code(), "host_audit_assembly_failed");
    assert_eq!(error.error_code(), ErrorCode::Fatal);
    Ok(())
}

#[cfg(not(windows))]
#[tokio::test]
async fn test_host_assembly_rejects_unsupported_platform() -> Result<(), Box<dyn std::error::Error>>
{
    let directory = TestDirectory::new("unsupported-platform")?;
    let provider: Arc<dyn ModelProvider> = Arc::new(NoopProvider::new()?);
    let input = base_input(&directory, provider)?
        .with_session_store(Arc::new(MemorySessionStore::new()) as Arc<dyn SessionStore>)
        .with_memory_retriever(Arc::new(EmptyRetriever))
        .with_app_map_reader(Arc::new(EmptyReader))
        .with_tool_registry(test_registry()?);

    let Err(error) = HostAssembly::new(input).assemble().await else {
        return Err("non-Windows Host assembly unexpectedly succeeded".into());
    };
    assert!(matches!(
        error,
        HostAssemblyError::InvalidConfiguration {
            field: "platform",
            ..
        }
    ));
    Ok(())
}
