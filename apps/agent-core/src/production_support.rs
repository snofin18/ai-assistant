//! Minimal injected dependencies for the 1a production Host.

use assistant_core::{
    CompressionError, ContextFragment, ContextSummary, HistoryCompressor, MemoryRetrievalError,
    MemoryRetrievalHit, MemoryRetriever,
};
use assistant_protocol::ErrorCode;
use assistant_storage::MemoryQuery;

/// Empty memory retriever used until a storage-backed memory adapter is wired.
pub(crate) struct EmptyRetriever;

impl MemoryRetriever for EmptyRetriever {
    fn retrieve(
        &self,
        _query: &MemoryQuery,
    ) -> Result<Vec<MemoryRetrievalHit>, MemoryRetrievalError> {
        Ok(Vec::new())
    }
}

/// Compressor that fails explicitly; the 1a path never invokes compression.
pub(crate) struct NoopCompressor;

impl HistoryCompressor for NoopCompressor {
    fn summarize(
        &self,
        _fragments: &[ContextFragment],
    ) -> Result<ContextSummary, CompressionError> {
        Err(CompressionError::new(
            "compressor_unavailable",
            ErrorCode::CapabilityMissing,
            "the 1a production path does not invoke compression",
        ))
    }
}
