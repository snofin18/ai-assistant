//! The single Host assembly point.
//!
//! This module is intentionally in the binary layer. `crates/core` exposes
//! orchestration components but does not know how to construct storage, audit,
//! policy, tool-bus, model-gateway, or platform implementations.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use assistant_audit::Durability;
use assistant_core::{
    AppMapFileReader, ContextManager, HistoryCompressor, Memory, MemoryRetriever, Planner,
    SessionManager, SessionStore,
};
use assistant_model_gateway::{ModelGateway, ModelProvider, ModelRouter, RetryPolicy};
use assistant_policy::RuleSet;
use assistant_storage::{
    Clock, Database, MIGRATIONS as STORAGE_MIGRATIONS, MigrationSet, StoragePaths,
};
use assistant_tool_bus::{MountReport, MountSelection, ToolBus, ToolBusConfig, ToolRegistry};

use crate::HostAssemblyError;
use crate::adapters::{AuditSink, DatabaseHandle, StorageSessionClock, StorageToolClock};

/// Inputs required to assemble one Host.
///
/// Every component is explicit. The assembly point never creates a hidden
/// default for a missing provider, store, platform, or policy.
pub struct HostAssemblyInput<P> {
    data_root: PathBuf,
    clock: Arc<dyn Clock>,
    session_store: Option<Arc<dyn SessionStore>>,
    memory_retriever: Option<Arc<dyn MemoryRetriever>>,
    app_map_reader: Option<Arc<dyn AppMapFileReader>>,
    planner_provider: Option<Arc<dyn ModelProvider>>,
    model_router: Option<ModelRouter>,
    model_providers: Vec<Arc<dyn ModelProvider>>,
    retry_policy: RetryPolicy,
    policy_rules: Option<RuleSet>,
    tool_registry: Option<ToolRegistry>,
    compressor: Option<Arc<dyn HistoryCompressor>>,
    durability: Durability,
    tool_session_id: String,
    platform: P,
}

impl<P> HostAssemblyInput<P> {
    /// Creates an input with no optional component installed.
    #[must_use]
    pub fn new(data_root: impl Into<PathBuf>, clock: Arc<dyn Clock>, platform: P) -> Self {
        Self {
            data_root: data_root.into(),
            clock,
            session_store: None,
            memory_retriever: None,
            app_map_reader: None,
            planner_provider: None,
            model_router: None,
            model_providers: Vec::new(),
            retry_policy: RetryPolicy::default(),
            policy_rules: None,
            tool_registry: None,
            compressor: None,
            durability: Durability::default(),
            tool_session_id: "assistant-host".to_owned(),
            platform,
        }
    }

    /// Injects the session persistence boundary.
    #[must_use]
    pub fn with_session_store(mut self, store: Arc<dyn SessionStore>) -> Self {
        self.session_store = Some(store);
        self
    }

    /// Injects the storage-backed memory retriever.
    #[must_use]
    pub fn with_memory_retriever(mut self, retriever: Arc<dyn MemoryRetriever>) -> Self {
        self.memory_retriever = Some(retriever);
        self
    }

    /// Injects the App Map file reader.
    #[must_use]
    pub fn with_app_map_reader(mut self, reader: Arc<dyn AppMapFileReader>) -> Self {
        self.app_map_reader = Some(reader);
        self
    }

    /// Injects the provider used directly by Planner.
    #[must_use]
    pub fn with_planner_provider(mut self, provider: Arc<dyn ModelProvider>) -> Self {
        self.planner_provider = Some(provider);
        self
    }

    /// Injects the router used by the model gateway.
    #[must_use]
    pub fn with_model_router(mut self, router: ModelRouter) -> Self {
        self.model_router = Some(router);
        self
    }

    /// Injects the provider pool used by the model gateway.
    #[must_use]
    pub fn with_model_providers(mut self, providers: Vec<Arc<dyn ModelProvider>>) -> Self {
        self.model_providers = providers;
        self
    }

    /// Overrides the retry policy used by the model gateway.
    #[must_use]
    pub const fn with_retry_policy(mut self, retry_policy: RetryPolicy) -> Self {
        self.retry_policy = retry_policy;
        self
    }

    /// Injects the policy rule set.
    #[must_use]
    pub fn with_policy_rules(mut self, rules: RuleSet) -> Self {
        self.policy_rules = Some(rules);
        self
    }

    /// Injects the tool registry mounted into the MCP tool bus.
    #[must_use]
    pub fn with_tool_registry(mut self, registry: ToolRegistry) -> Self {
        self.tool_registry = Some(registry);
        self
    }

    /// Injects the context history compressor.
    #[must_use]
    pub fn with_history_compressor(mut self, compressor: Arc<dyn HistoryCompressor>) -> Self {
        self.compressor = Some(compressor);
        self
    }

    /// Overrides audit durability.
    #[must_use]
    pub const fn with_durability(mut self, durability: Durability) -> Self {
        self.durability = durability;
        self
    }

    /// Overrides the tool-bus session id.
    #[must_use]
    pub fn with_tool_session_id(mut self, session_id: impl Into<String>) -> Self {
        self.tool_session_id = session_id.into();
        self
    }
}

/// Host assembled by the binary layer.
pub struct HostComponents<P> {
    database: DatabaseHandle,
    sessions: Mutex<SessionManager>,
    context: ContextManager,
    planner: Planner,
    memory: Memory,
    model_gateway: ModelGateway,
    policy: RuleSet,
    tool_bus: ToolBus,
    toolset_report: MountReport,
    audit: AuditSink,
    compressor: Arc<dyn HistoryCompressor>,
    platform: P,
}

impl<P> HostComponents<P> {
    /// Returns the sole database handle owned by the Host.
    #[must_use]
    pub fn database(&self) -> &Mutex<Database> {
        &self.database
    }

    /// Returns a cloneable handle to the assembly-owned database.
    #[must_use]
    pub fn database_handle(&self) -> DatabaseHandle {
        Arc::clone(&self.database)
    }

    /// Returns the session manager lock.
    #[must_use]
    pub const fn sessions(&self) -> &Mutex<SessionManager> {
        &self.sessions
    }

    /// Returns the context manager.
    #[must_use]
    pub const fn context(&self) -> &ContextManager {
        &self.context
    }

    /// Returns the Planner.
    #[must_use]
    pub const fn planner(&self) -> &Planner {
        &self.planner
    }

    /// Returns the Memory component.
    #[must_use]
    pub const fn memory(&self) -> &Memory {
        &self.memory
    }

    /// Returns the model gateway.
    #[must_use]
    pub const fn model_gateway(&self) -> &ModelGateway {
        &self.model_gateway
    }

    /// Returns the policy rule set.
    #[must_use]
    pub const fn policy(&self) -> &RuleSet {
        &self.policy
    }

    /// Returns the running in-process tool bus.
    #[must_use]
    pub const fn tool_bus(&self) -> &ToolBus {
        &self.tool_bus
    }

    /// Returns the tool mount report captured at startup.
    #[must_use]
    pub const fn toolset_report(&self) -> &MountReport {
        &self.toolset_report
    }

    /// Returns the audit sink.
    #[must_use]
    pub const fn audit(&self) -> &AuditSink {
        &self.audit
    }

    /// Returns the injected context compressor.
    #[must_use]
    pub const fn compressor(&self) -> &Arc<dyn HistoryCompressor> {
        &self.compressor
    }

    /// Returns the platform implementation.
    #[must_use]
    pub const fn platform(&self) -> &P {
        &self.platform
    }

    /// Shuts down the in-process tool bus.
    ///
    /// # Errors
    ///
    /// Returns [`HostAssemblyError::ToolBus`] when either MCP side fails to
    /// stop within its bounded shutdown window.
    pub async fn shutdown(self) -> Result<(), HostAssemblyError> {
        self.tool_bus
            .shutdown()
            .await
            .map_err(|error| HostAssemblyError::ToolBus {
                reason: error.to_string(),
            })
    }
}

/// One-shot assembly operation.
pub struct HostAssembly<P> {
    input: HostAssemblyInput<P>,
}

impl<P> HostAssembly<P> {
    /// Creates an assembly operation.
    #[must_use]
    pub const fn new(input: HostAssemblyInput<P>) -> Self {
        Self { input }
    }
}

impl<P> HostAssembly<P>
where
    P: 'static,
{
    /// Validates inputs and constructs the complete Host.
    ///
    /// # Errors
    ///
    /// Returns a typed [`HostAssemblyError`] for missing components, invalid
    /// configuration, migration/storage failure, audit initialization failure,
    /// model-gateway construction failure, or tool-bus startup failure.
    pub async fn assemble(self) -> Result<HostComponents<P>, HostAssemblyError> {
        let Self { input } = self;
        validate_input(&input)?;
        let database = open_database(&input)?;
        let audit = AuditSink::new(Arc::clone(&database), input.durability);
        // Validate the audit schema before returning a partially assembled Host.
        let _audit_probe = audit.verify_chain()?;

        let session_store = require(input.session_store, "session_store")?;
        let session_clock = Arc::new(StorageSessionClock::new(Arc::clone(&input.clock)));
        let sessions = SessionManager::new(session_store, session_clock);

        let compressor = require(input.compressor, "history_compressor")?;
        let context = ContextManager::new();

        let planner_provider = require(input.planner_provider, "planner_provider")?;
        let planner = Planner::new(planner_provider);

        let app_map_reader = require(input.app_map_reader, "app_map_reader")?;
        let memory_retriever = require(input.memory_retriever, "memory_retriever")?;
        let memory = Memory::new(app_map_reader, memory_retriever);

        let model_router = require(input.model_router, "model_router")?;
        if input.model_providers.is_empty() {
            return Err(HostAssemblyError::MissingComponent {
                component: "model_providers",
            });
        }
        let model_gateway =
            ModelGateway::new(model_router, input.retry_policy, input.model_providers).map_err(
                |error| HostAssemblyError::InvalidConfiguration {
                    field: "model_gateway",
                    reason: error.to_string(),
                },
            )?;

        let policy = require(input.policy_rules, "policy_rules")?;
        let tool_registry = require(input.tool_registry, "tool_registry")?;
        let tool_clock = Arc::new(StorageToolClock::new(Arc::clone(&input.clock)));
        let (tool_bus, toolset_report) = ToolBus::start(
            tool_registry,
            MountSelection::all(),
            ToolBusConfig::new(input.tool_session_id),
            tool_clock,
        )
        .await
        .map_err(|error| HostAssemblyError::ToolBus {
            reason: error.to_string(),
        })?;

        Ok(HostComponents {
            database,
            sessions: Mutex::new(sessions),
            context,
            planner,
            memory,
            model_gateway,
            policy,
            tool_bus,
            toolset_report,
            audit,
            compressor,
            platform: input.platform,
        })
    }
}

fn validate_input<P>(input: &HostAssemblyInput<P>) -> Result<(), HostAssemblyError> {
    if input.data_root.as_os_str().is_empty() {
        return Err(HostAssemblyError::InvalidConfiguration {
            field: "data_root",
            reason: "must not be empty".to_owned(),
        });
    }
    if input.tool_session_id.trim().is_empty() {
        return Err(HostAssemblyError::InvalidConfiguration {
            field: "tool_session_id",
            reason: "must not be empty".to_owned(),
        });
    }
    Ok(())
}

fn open_database<P>(input: &HostAssemblyInput<P>) -> Result<DatabaseHandle, HostAssemblyError> {
    let mut migrations = MigrationSet::new();
    migrations
        .register_all(STORAGE_MIGRATIONS)
        .map_err(|error| HostAssemblyError::Migration {
            reason: error.to_string(),
        })?;
    migrations
        .register_all(assistant_audit::MIGRATIONS)
        .map_err(|error| HostAssemblyError::Migration {
            reason: error.to_string(),
        })?;
    let paths = StoragePaths::new(&input.data_root);
    let database = Database::open(&paths, Arc::clone(&input.clock), &migrations)
        .map_err(|error| storage_error(&error))?;
    Ok(Arc::new(Mutex::new(database)))
}

fn require<T>(value: Option<T>, component: &'static str) -> Result<T, HostAssemblyError> {
    value.ok_or(HostAssemblyError::MissingComponent { component })
}

fn storage_error(error: &assistant_storage::StorageError) -> HostAssemblyError {
    HostAssemblyError::Storage {
        reason_code: error.reason_code().to_owned(),
        code: error.error_category(),
        message: error.to_string(),
    }
}
