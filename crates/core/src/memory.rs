//! Bounded memory projection from App Map entries and storage retrieval hits.
//!
//! App Map parsing and validation live in [`crate::app_map`]. This module owns
//! the caller-selected injection policy, source traceability, deduplication,
//! and explicit budget omissions. It never performs filesystem, SQL, network,
//! or platform access.
//!
//! Related: `docs/spec/core-orchestration.md`, ADR-0053, TASK-208.

use std::fmt;
use std::sync::Arc;

use assistant_storage::{MemoryQuery, MemoryRecordKind, MemorySearchResult};

use crate::app_map::AppMapLoader;
use crate::error::{CoreError, CoreResult, MemoryRetrievalError};
use crate::identifiers::TokenCount;

/// One retrieved storage hit plus its caller-supplied token estimate.
#[derive(Debug, Clone)]
pub struct MemoryRetrievalHit {
    result: MemorySearchResult,
    token_estimate: TokenCount,
}

impl MemoryRetrievalHit {
    /// Creates a retrieval hit.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidContent`] when the token estimate is zero or
    /// the source reference is empty.
    pub fn new(result: MemorySearchResult, token_estimate: TokenCount) -> CoreResult<Self> {
        if result.source_reference.trim().is_empty() {
            return Err(CoreError::InvalidContent {
                field: "memory_result.source_reference",
                reason: "must not be empty".to_owned(),
            });
        }
        if token_estimate.get() == 0 {
            return Err(CoreError::InvalidContent {
                field: "memory_result.token_estimate",
                reason: "must be positive".to_owned(),
            });
        }
        Ok(Self {
            result,
            token_estimate,
        })
    }

    /// Storage-owned retrieval result.
    #[must_use]
    pub const fn result(&self) -> &MemorySearchResult {
        &self.result
    }

    /// Token estimate supplied by the injected adapter.
    #[must_use]
    pub const fn token_estimate(&self) -> TokenCount {
        self.token_estimate
    }
}

/// Injected retrieval adapter.
pub trait MemoryRetriever: Send + Sync {
    /// Retrieves storage records for one query.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryRetrievalError`] preserving the backend's stable
    /// `reason_code` and protocol error category.
    fn retrieve(
        &self,
        query: &MemoryQuery,
    ) -> Result<Vec<MemoryRetrievalHit>, MemoryRetrievalError>;
}

/// A request for bounded memory injection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryRequest {
    app_map_path: String,
    app_map_entry_ids: Vec<String>,
    query: MemoryQuery,
    budget_tokens: TokenCount,
}

impl MemoryRequest {
    /// Creates a request.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidContent`] for an empty path, duplicate entry
    /// ids, or an empty entry id.
    pub fn new(
        app_map_path: impl Into<String>,
        app_map_entry_ids: Vec<String>,
        query: MemoryQuery,
        budget_tokens: TokenCount,
    ) -> CoreResult<Self> {
        let app_map_path = app_map_path.into();
        if app_map_path.trim().is_empty() {
            return Err(CoreError::InvalidContent {
                field: "memory_request.app_map_path",
                reason: "must not be empty".to_owned(),
            });
        }
        let mut seen = std::collections::BTreeSet::new();
        for entry_id in &app_map_entry_ids {
            if entry_id.trim().is_empty() {
                return Err(CoreError::InvalidContent {
                    field: "memory_request.app_map_entry_ids",
                    reason: "entry id must not be empty".to_owned(),
                });
            }
            if !seen.insert(entry_id) {
                return Err(CoreError::InvalidContent {
                    field: "memory_request.app_map_entry_ids",
                    reason: format!("duplicate entry id {entry_id}"),
                });
            }
        }
        Ok(Self {
            app_map_path,
            app_map_entry_ids,
            query,
            budget_tokens,
        })
    }

    /// Relative App Map path.
    #[must_use]
    pub fn app_map_path(&self) -> &str {
        &self.app_map_path
    }

    /// App Map entry ids requested by the caller.
    #[must_use]
    pub fn app_map_entry_ids(&self) -> &[String] {
        &self.app_map_entry_ids
    }

    /// Storage retrieval query.
    #[must_use]
    pub const fn query(&self) -> &MemoryQuery {
        &self.query
    }

    /// Injection budget.
    #[must_use]
    pub const fn budget_tokens(&self) -> TokenCount {
        self.budget_tokens
    }
}

/// Source of one injected memory segment.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum MemorySegmentOrigin {
    /// A selected App Map entry.
    AppMap {
        /// App Map entry id.
        entry_id: String,
    },
    /// A storage record returned by the injected retriever.
    StoredRecord {
        /// Storage record kind.
        record_kind: MemoryRecordKind,
        /// Storage record id.
        record_id: String,
    },
}

/// One bounded memory segment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemorySegment {
    content: String,
    source_reference: String,
    origin: MemorySegmentOrigin,
    token_estimate: TokenCount,
}

impl MemorySegment {
    /// Returns injectable text.
    #[must_use]
    pub fn content(&self) -> &str {
        &self.content
    }

    /// Returns the source file/record and original position.
    #[must_use]
    pub fn source_reference(&self) -> &str {
        &self.source_reference
    }

    /// Returns the segment origin.
    #[must_use]
    pub const fn origin(&self) -> &MemorySegmentOrigin {
        &self.origin
    }

    /// Returns the token estimate.
    #[must_use]
    pub const fn token_estimate(&self) -> TokenCount {
        self.token_estimate
    }
}

/// Why a candidate memory segment was omitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MemoryOmissionReason {
    /// Another selected segment had the same source reference.
    DuplicateSource,
    /// The segment did not fit the remaining budget.
    OverBudget,
}

/// Explicit record of an omitted memory segment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryOmission {
    source_reference: String,
    reason: MemoryOmissionReason,
}

impl MemoryOmission {
    /// Returns the omitted source reference.
    #[must_use]
    pub fn source_reference(&self) -> &str {
        &self.source_reference
    }

    /// Returns the omission reason.
    #[must_use]
    pub const fn reason(&self) -> MemoryOmissionReason {
        self.reason
    }
}

/// Bounded memory projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryProjection {
    segments: Vec<MemorySegment>,
    omissions: Vec<MemoryOmission>,
    used_tokens: TokenCount,
    budget_tokens: TokenCount,
}

impl MemoryProjection {
    /// Returns selected segments in injection order.
    #[must_use]
    pub fn segments(&self) -> &[MemorySegment] {
        &self.segments
    }

    /// Returns every omitted candidate with an explicit reason.
    #[must_use]
    pub fn omissions(&self) -> &[MemoryOmission] {
        &self.omissions
    }

    /// Returns consumed tokens.
    #[must_use]
    pub const fn used_tokens(&self) -> TokenCount {
        self.used_tokens
    }

    /// Returns the requested budget.
    #[must_use]
    pub const fn budget_tokens(&self) -> TokenCount {
        self.budget_tokens
    }
}

/// Memory component assembled from injected App Map and retrieval adapters.
pub struct Memory {
    app_map_loader: AppMapLoader,
    retriever: Arc<dyn MemoryRetriever>,
}

impl fmt::Debug for Memory {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("Memory").finish_non_exhaustive()
    }
}

impl Memory {
    /// Creates a Memory component.
    #[must_use]
    pub fn new(
        reader: Arc<dyn crate::app_map::AppMapFileReader>,
        retriever: Arc<dyn MemoryRetriever>,
    ) -> Self {
        Self {
            app_map_loader: AppMapLoader::new(reader),
            retriever,
        }
    }

    /// Loads and validates one App Map.
    ///
    /// # Errors
    ///
    /// Propagates [`AppMapLoader::load`].
    pub fn load_app_map(&self, relative_path: &str) -> CoreResult<crate::app_map::AppMap> {
        self.app_map_loader.load(relative_path)
    }

    /// Retrieves storage memory through the injected adapter.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::MemoryRetrieval`] preserving the backend reason code.
    pub fn retrieve(&self, query: &MemoryQuery) -> CoreResult<Vec<MemoryRetrievalHit>> {
        self.retriever.retrieve(query).map_err(CoreError::from)
    }

    /// Builds a bounded, traceable memory projection.
    ///
    /// App Map entries are selected only by the caller-supplied id list. Stored
    /// retrieval hits are appended in retriever order. Duplicate source
    /// references and over-budget candidates are recorded as omissions rather
    /// than silently discarded.
    ///
    /// # Errors
    ///
    /// - App Map loading/validation failures;
    /// - [`CoreError::AppMapEntryNotFound`] for a requested missing entry;
    /// - [`CoreError::MemoryRetrieval`] for backend retrieval failures.
    pub fn build_projection(&self, request: &MemoryRequest) -> CoreResult<MemoryProjection> {
        let app_map = self.load_app_map(request.app_map_path())?;
        let mut candidates = Vec::new();
        for entry_id in request.app_map_entry_ids() {
            let entry = app_map
                .entry(entry_id)
                .ok_or_else(|| CoreError::AppMapEntryNotFound {
                    entry_id: entry_id.clone(),
                })?;
            candidates.push(MemorySegment {
                content: entry.content().to_owned(),
                source_reference: entry.source_reference().to_owned(),
                origin: MemorySegmentOrigin::AppMap {
                    entry_id: entry.id().to_owned(),
                },
                token_estimate: entry.token_estimate(),
            });
        }
        for hit in self.retrieve(request.query())? {
            let result = hit.result;
            candidates.push(MemorySegment {
                content: result.snippet,
                source_reference: result.source_reference,
                origin: MemorySegmentOrigin::StoredRecord {
                    record_kind: result.record_kind,
                    record_id: result.record_id,
                },
                token_estimate: hit.token_estimate,
            });
        }
        apply_budget(candidates, request.budget_tokens())
    }
}

fn apply_budget(
    candidates: Vec<MemorySegment>,
    budget_tokens: TokenCount,
) -> CoreResult<MemoryProjection> {
    let mut segments = Vec::new();
    let mut omissions = Vec::new();
    let mut used_tokens = TokenCount::default();
    let mut seen_sources = std::collections::BTreeSet::new();
    for candidate in candidates {
        if !seen_sources.insert(candidate.source_reference.clone()) {
            omissions.push(MemoryOmission {
                source_reference: candidate.source_reference,
                reason: MemoryOmissionReason::DuplicateSource,
            });
            continue;
        }
        let next = used_tokens.checked_add(candidate.token_estimate)?;
        if next > budget_tokens {
            omissions.push(MemoryOmission {
                source_reference: candidate.source_reference,
                reason: MemoryOmissionReason::OverBudget,
            });
            continue;
        }
        used_tokens = next;
        segments.push(candidate);
    }
    Ok(MemoryProjection {
        segments,
        omissions,
        used_tokens,
        budget_tokens,
    })
}
