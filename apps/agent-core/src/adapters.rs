//! Binary-layer adapters from Core traits to concrete infrastructure.
//!
//! These adapters are deliberately outside `crates/core`: Core defines the
//! persistence and retrieval boundaries, while the binary assembly owns the
//! SQLite handle, file access, clocks, and error translation.

use std::sync::{Arc, Mutex};

use assistant_audit::{AppendOutcome, AuditLog, AuditSubject, ChainVerification, Durability};
use assistant_core::{
    AppMapFileReader, MemoryRetrievalError, MemoryRetrievalHit, MemoryRetriever, SessionClock,
    TokenCount,
};
use assistant_protocol::{AuditEvent, ErrorCode};
use assistant_storage::{Clock, Database, MemoryQuery, search_memory};
use assistant_tool_bus::Clock as ToolClock;

use crate::HostAssemblyError;

/// Shared assembly-owned database handle.
///
/// `rusqlite::Connection` is `Send` but not `Sync`; the mutex makes the single
/// writer safe to share with the adapters while preserving the storage
/// design's one-writer rule.
pub type DatabaseHandle = Arc<Mutex<Database>>;

/// Estimates token cost for a storage snippet.
pub trait TokenEstimator: Send + Sync {
    /// Returns a positive conservative estimate for the supplied text.
    fn estimate(&self, text: &str) -> TokenCount;
}

/// Conservative character-count estimator used until a provider tokenizer is
/// injected by a later provider card.
#[derive(Debug, Default, Clone, Copy)]
pub struct CharacterTokenEstimator;

impl TokenEstimator for CharacterTokenEstimator {
    fn estimate(&self, text: &str) -> TokenCount {
        TokenCount::new(
            u64::try_from(text.chars().count())
                .unwrap_or(u64::MAX)
                .max(1),
        )
    }
}

/// Storage-backed memory retrieval adapter.
pub struct StorageMemoryRetriever {
    database: DatabaseHandle,
    token_estimator: Arc<dyn TokenEstimator>,
}

impl StorageMemoryRetriever {
    /// Creates an adapter over the assembly-owned database.
    #[must_use]
    pub fn new(database: DatabaseHandle, token_estimator: Arc<dyn TokenEstimator>) -> Self {
        Self {
            database,
            token_estimator,
        }
    }
}

impl MemoryRetriever for StorageMemoryRetriever {
    fn retrieve(
        &self,
        query: &MemoryQuery,
    ) -> Result<Vec<MemoryRetrievalHit>, MemoryRetrievalError> {
        let results = {
            let database = self.database.lock().map_err(|_| {
                MemoryRetrievalError::new(
                    "memory_database_poisoned",
                    ErrorCode::Fatal,
                    "the assembly-owned database mutex is poisoned",
                )
            })?;
            let results = search_memory(database.connection(), query).map_err(|error| {
                MemoryRetrievalError::new(
                    error.reason_code(),
                    error.error_category(),
                    error.to_string(),
                )
            })?;
            drop(database);
            results
        };
        results
            .into_iter()
            .map(|result| {
                let estimate = self.token_estimator.estimate(&result.snippet);
                MemoryRetrievalHit::new(result, estimate).map_err(|error| {
                    MemoryRetrievalError::new(
                        error.reason_code(),
                        error.error_code(),
                        error.to_string(),
                    )
                })
            })
            .collect()
    }
}

/// Adapter from the storage clock contract to Core's session clock contract.
#[derive(Clone)]
pub struct StorageSessionClock {
    clock: Arc<dyn Clock>,
}

impl StorageSessionClock {
    /// Wraps one storage clock for Core session construction.
    #[must_use]
    pub fn new(clock: Arc<dyn Clock>) -> Self {
        Self { clock }
    }
}

impl SessionClock for StorageSessionClock {
    fn now_unix_ms(&self) -> i64 {
        self.clock.now_unix_ms()
    }
}

/// Adapter from the storage clock contract to the tool-bus clock contract.
#[derive(Clone)]
pub struct StorageToolClock {
    clock: Arc<dyn Clock>,
}

impl StorageToolClock {
    /// Wraps one storage clock for tool-bus construction.
    #[must_use]
    pub fn new(clock: Arc<dyn Clock>) -> Self {
        Self { clock }
    }
}

impl ToolClock for StorageToolClock {
    fn now_unix_ms(&self) -> i64 {
        self.clock.now_unix_ms()
    }
}

/// Assembly-owned audit sink.
///
/// `AuditLog` borrows the database connection, so the sink stores the owning
/// handle and opens a short-lived writer on demand. This avoids self-referential
/// state and keeps the single write connection owned by the assembly point.
#[derive(Clone)]
pub struct AuditSink {
    database: DatabaseHandle,
    durability: Durability,
}

impl AuditSink {
    /// Creates an audit sink over the assembly-owned database.
    #[must_use]
    pub const fn new(database: DatabaseHandle, durability: Durability) -> Self {
        Self {
            database,
            durability,
        }
    }

    /// Appends one event and flushes it before releasing the database lock.
    ///
    /// # Errors
    ///
    /// Returns [`HostAssemblyError::Audit`] when the audit schema is missing or
    /// the durability mode is unsupported.
    pub fn append(
        &self,
        event: &AuditEvent,
        subject: &AuditSubject,
    ) -> Result<AppendOutcome, HostAssemblyError> {
        let outcome = {
            let database = self.database.lock().map_err(|_| HostAssemblyError::Audit {
                reason: "the assembly-owned database mutex is poisoned".to_owned(),
            })?;
            let mut log = AuditLog::new(database.connection(), database.clock(), self.durability)
                .map_err(|error| audit_error(&error))?;
            let outcome = log
                .append(event, subject)
                .map_err(|error| audit_error(&error))?;
            let outcome = if let AppendOutcome::Buffered { .. } = outcome {
                let written = log.flush().map_err(|error| audit_error(&error))?;
                AppendOutcome::Flushed { written }
            } else {
                outcome
            };
            drop(log);
            drop(database);
            outcome
        };
        Ok(outcome)
    }

    /// Verifies the audit hash chain while holding the database lock.
    ///
    /// # Errors
    ///
    /// Returns [`HostAssemblyError::Audit`] when the audit schema or chain
    /// cannot be read.
    pub fn verify_chain(&self) -> Result<ChainVerification, HostAssemblyError> {
        let verification = {
            let database = self.database.lock().map_err(|_| HostAssemblyError::Audit {
                reason: "the assembly-owned database mutex is poisoned".to_owned(),
            })?;
            let log = AuditLog::new(database.connection(), database.clock(), self.durability)
                .map_err(|error| audit_error(&error))?;
            let verification = log.verify_chain().map_err(|error| audit_error(&error))?;
            drop(log);
            drop(database);
            verification
        };
        Ok(verification)
    }
}

fn audit_error(error: &assistant_audit::AuditError) -> HostAssemblyError {
    HostAssemblyError::Audit {
        reason: error.to_string(),
    }
}

/// File reader rooted at the assembly's data directory.
///
/// The relative path is still validated by Core before this adapter is called;
/// the adapter additionally resolves it under the configured root.
pub struct RootedAppMapReader {
    root: std::path::PathBuf,
}

impl RootedAppMapReader {
    /// Creates a reader rooted at one directory.
    #[must_use]
    pub fn new(root: impl Into<std::path::PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

impl AppMapFileReader for RootedAppMapReader {
    fn read_to_string(
        &self,
        relative_path: &std::path::Path,
    ) -> Result<String, assistant_core::AppMapReadError> {
        let path = self.root.join(relative_path);
        std::fs::read_to_string(path)
            .map_err(|error| assistant_core::AppMapReadError::Unreadable(error.to_string()))
    }
}
