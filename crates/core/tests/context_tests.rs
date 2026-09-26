//! Integration tests for Core context budgeting and compression.

#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unwrap_used
)]

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use assistant_core::{
    CompressionError, ContextBudget, ContextEntry, ContextManager, ContextRetention,
    ContextSummary, CoreError, HistoryCompressor, MemorySessionStore, MessageContent,
    OmissionReason, SessionStoreError, TokenCount,
};
use assistant_protocol::ErrorCode;
use common::{FixedClock, create_linear_session, manager, message_id, new_message, session_id};

#[derive(Default)]
struct CountingCompressor {
    calls: AtomicUsize,
    summary_tokens: u64,
    fail: bool,
    mismatch_sources: bool,
}

impl HistoryCompressor for CountingCompressor {
    fn summarize(
        &self,
        fragments: &[assistant_core::ContextFragment],
    ) -> Result<ContextSummary, CompressionError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.fail {
            return Err(CompressionError::new(
                "compressor_unavailable",
                ErrorCode::ModelNetworkFailure,
                "test compressor unavailable",
            ));
        }
        let sources = if self.mismatch_sources {
            vec![message_id("wrong")]
        } else {
            fragments
                .iter()
                .map(|fragment| fragment.message_id().clone())
                .collect()
        };
        Ok(ContextSummary::new("summary", TokenCount::new(self.summary_tokens), sources).unwrap())
    }
}

#[test]
fn context_summary_validates_inputs() {
    assert!(ContextSummary::new("", TokenCount::new(1), vec![]).is_err());
    assert!(ContextSummary::new("bad\0summary", TokenCount::new(1), vec![]).is_err());
    assert!(ContextSummary::new("x", TokenCount::new(0), vec![]).is_err());
    let duplicate = message_id("m_1");
    assert!(
        ContextSummary::new("x", TokenCount::new(1), vec![duplicate.clone(), duplicate]).is_err()
    );
    assert!(MessageContent::new("").is_err());
}

#[test]
fn context_budget_validates_bounds() {
    assert!(matches!(
        ContextBudget::new(TokenCount::new(0), TokenCount::new(0)),
        Err(CoreError::InvalidBudget { .. })
    ));
    assert!(matches!(
        ContextBudget::new(TokenCount::new(10), TokenCount::new(10)),
        Err(CoreError::InvalidBudget { .. })
    ));
    let budget = ContextBudget::new(TokenCount::new(10), TokenCount::new(3)).unwrap();
    assert_eq!(budget.max_total_tokens().get(), 10);
    assert_eq!(budget.reserved_output_tokens().get(), 3);
    assert_eq!(budget.available_input_tokens().get(), 7);
}

#[test]
fn required_context_that_cannot_fit_fails_closed() {
    let store = Arc::new(MemorySessionStore::new());
    let clock = Arc::new(FixedClock::new(100));
    clock.set(101);
    let mut sessions = manager(store, clock);
    let session = session_id("s_1");
    sessions.create_session(session.clone(), "goal").unwrap();
    let required = sessions
        .append_message(
            &session,
            new_message("required", None, 10, ContextRetention::Required),
        )
        .unwrap();
    let snapshot = sessions.snapshot(&session).unwrap();
    let budget = ContextBudget::new(TokenCount::new(10), TokenCount::new(1)).unwrap();
    let compressor = CountingCompressor::default();
    assert!(matches!(
        ContextManager::new().build_context(snapshot, &required, budget, &compressor),
        Err(CoreError::RequiredContextExceedsBudget { .. })
    ));
    assert_eq!(compressor.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn context_drops_policy_marked_history_with_an_explicit_reason() {
    let store = Arc::new(MemorySessionStore::new());
    let clock = Arc::new(FixedClock::new(100));
    let mut sessions = manager(store, clock);
    let session = session_id("s_1");
    sessions.create_session(session.clone(), "goal").unwrap();
    let required = sessions
        .append_message(
            &session,
            new_message("required", None, 3, ContextRetention::Required),
        )
        .unwrap();
    let droppable = sessions
        .append_message(
            &session,
            new_message(
                "droppable",
                Some(required.as_str()),
                5,
                ContextRetention::Droppable,
            ),
        )
        .unwrap();
    let snapshot = sessions.snapshot(&session).unwrap();
    let budget = ContextBudget::new(TokenCount::new(5), TokenCount::new(1)).unwrap();
    let compressor = CountingCompressor::default();
    let projection = ContextManager::new()
        .build_context(snapshot, &droppable, budget, &compressor)
        .unwrap();
    assert_eq!(projection.entries().len(), 1);
    assert_eq!(projection.omissions().len(), 1);
    assert_eq!(
        projection.omissions()[0].reason(),
        OmissionReason::DroppedByPolicy
    );
    assert!(projection.omissions()[0].recoverable());
    assert_eq!(projection.used_tokens().get(), 3);
    assert_eq!(projection.available_tokens().get(), 4);
    assert_eq!(compressor.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn context_summarizes_older_history_and_labels_every_source() {
    let store = Arc::new(MemorySessionStore::new());
    let clock = Arc::new(FixedClock::new(100));
    let mut sessions = manager(store, clock);
    let (session, messages) = create_linear_session(&mut sessions);
    let snapshot = sessions.snapshot(&session).unwrap();
    let budget = ContextBudget::new(TokenCount::new(15), TokenCount::new(1)).unwrap();
    let compressor = CountingCompressor {
        calls: AtomicUsize::new(0),
        summary_tokens: 2,
        fail: false,
        mismatch_sources: false,
    };
    let projection = ContextManager::new()
        .build_context(snapshot, &messages[2], budget, &compressor)
        .unwrap();
    assert_eq!(compressor.calls.load(Ordering::SeqCst), 1);
    assert!(matches!(projection.entries()[0], ContextEntry::Message(_)));
    assert!(matches!(projection.entries()[1], ContextEntry::Summary(_)));
    assert!(matches!(projection.entries()[2], ContextEntry::Message(_)));
    assert_eq!(projection.omissions().len(), 1);
    assert_eq!(
        projection.omissions()[0].reason(),
        OmissionReason::ReplacedBySummary
    );
    assert_eq!(projection.used_tokens().get(), 13);
    let ContextEntry::Summary(summary) = &projection.entries()[1] else {
        panic!("expected summary");
    };
    assert_eq!(summary.source_message_ids(), &[messages[1].clone()]);
}

#[test]
fn context_does_not_compress_when_everything_fits() {
    let store = Arc::new(MemorySessionStore::new());
    let clock = Arc::new(FixedClock::new(100));
    let mut sessions = manager(store, clock);
    let (session, messages) = create_linear_session(&mut sessions);
    let snapshot = sessions.snapshot(&session).unwrap();
    let budget = ContextBudget::new(TokenCount::new(20), TokenCount::new(1)).unwrap();
    let compressor = CountingCompressor {
        calls: AtomicUsize::new(0),
        summary_tokens: 2,
        fail: false,
        mismatch_sources: false,
    };
    let projection = ContextManager::new()
        .build_context(snapshot, &messages[2], budget, &compressor)
        .unwrap();
    assert_eq!(compressor.calls.load(Ordering::SeqCst), 0);
    assert_eq!(projection.entries().len(), 3);
    assert!(projection.omissions().is_empty());
    assert_eq!(projection.used_tokens().get(), 15);
}

#[test]
fn context_compression_failure_is_not_silently_dropped() {
    let store = Arc::new(MemorySessionStore::new());
    let clock = Arc::new(FixedClock::new(100));
    let mut sessions = manager(store, clock);
    let (session, messages) = create_linear_session(&mut sessions);
    let snapshot = sessions.snapshot(&session).unwrap();
    let budget = ContextBudget::new(TokenCount::new(15), TokenCount::new(1)).unwrap();
    let compressor = CountingCompressor {
        calls: AtomicUsize::new(0),
        summary_tokens: 2,
        fail: true,
        mismatch_sources: false,
    };
    let error = ContextManager::new()
        .build_context(snapshot, &messages[2], budget, &compressor)
        .unwrap_err();
    assert_eq!(error.error_code(), ErrorCode::ModelNetworkFailure);
    assert_eq!(error.reason_code(), "compressor_unavailable");
}

#[test]
fn context_rejects_summary_source_mismatch_and_oversized_summary() {
    let store = Arc::new(MemorySessionStore::new());
    let clock = Arc::new(FixedClock::new(100));
    let mut sessions = manager(store, clock);
    let (session, messages) = create_linear_session(&mut sessions);
    let snapshot = sessions.snapshot(&session).unwrap();
    let budget = ContextBudget::new(TokenCount::new(15), TokenCount::new(1)).unwrap();

    let mismatch = CountingCompressor {
        calls: AtomicUsize::new(0),
        summary_tokens: 2,
        fail: false,
        mismatch_sources: true,
    };
    let error = ContextManager::new()
        .build_context(snapshot, &messages[2], budget, &mismatch)
        .unwrap_err();
    assert_eq!(error.reason_code(), "summary_source_mismatch");

    let oversized = CountingCompressor {
        calls: AtomicUsize::new(0),
        summary_tokens: 10,
        fail: false,
        mismatch_sources: false,
    };
    assert!(matches!(
        ContextManager::new().build_context(snapshot, &messages[2], budget, &oversized),
        Err(CoreError::RequiredContextExceedsBudget { .. })
    ));
}

#[test]
fn context_without_messages_and_missing_leaf_are_deterministic() {
    let store = Arc::new(MemorySessionStore::new());
    let clock = Arc::new(FixedClock::new(100));
    let mut sessions = manager(store, clock);
    let session = session_id("s_1");
    sessions.create_session(session.clone(), "goal").unwrap();
    let snapshot = sessions.snapshot(&session).unwrap();
    assert!(snapshot.latest_leaf().is_none());
    let budget = ContextBudget::new(TokenCount::new(10), TokenCount::new(1)).unwrap();
    let compressor = CountingCompressor::default();
    assert!(matches!(
        ContextManager::new().build_context(snapshot, &message_id("missing"), budget, &compressor),
        Err(CoreError::MessageNotFound { .. })
    ));
}

#[test]
fn core_errors_expose_stable_codes_and_messages() {
    let errors = vec![
        CoreError::InvalidIdentifier {
            kind: "session",
            value: "bad".to_string(),
        },
        CoreError::InvalidContent {
            field: "field",
            reason: "reason".to_string(),
        },
        CoreError::SessionAlreadyExists {
            session_id: "s".to_string(),
        },
        CoreError::SessionNotFound {
            session_id: "s".to_string(),
        },
        CoreError::SessionEnded {
            session_id: "s".to_string(),
        },
        CoreError::MessageNotFound {
            session_id: "s".to_string(),
            message_id: "m".to_string(),
        },
        CoreError::ParentMessageNotFound {
            session_id: "s".to_string(),
            parent_message_id: "p".to_string(),
        },
        CoreError::MessageHasChildren {
            message_id: "m".to_string(),
        },
        CoreError::InvalidSessionSnapshot {
            reason: "reason".to_string(),
        },
        CoreError::InvalidBudget {
            field: "field",
            reason: "reason".to_string(),
        },
        CoreError::RequiredContextExceedsBudget {
            required_tokens: 1,
            available_tokens: 2,
        },
        CoreError::CompressionFailed(CompressionError::new(
            "compression",
            ErrorCode::ModelInvalidOutput,
            "detail",
        )),
        CoreError::SessionStore(SessionStoreError::new("store", "detail")),
        CoreError::NumericOverflow { field: "field" },
    ];
    assert_eq!(errors[0].error_code(), ErrorCode::ToolInvalidArgs);
    assert_eq!(errors[2].error_code(), ErrorCode::ToolInvalidArgs);
    assert_eq!(errors[4].error_code(), ErrorCode::Fatal);
    assert_eq!(errors[5].error_code(), ErrorCode::TargetNotFound);
    assert_eq!(errors[10].error_code(), ErrorCode::PolicyDenied);
    assert_eq!(errors[11].error_code(), ErrorCode::ModelInvalidOutput);
    assert_eq!(errors[12].error_code(), ErrorCode::Fatal);
    assert_eq!(errors[13].error_code(), ErrorCode::Fatal);
    for error in errors {
        assert!(!error.reason_code().is_empty());
        assert!(!error.to_string().is_empty());
        let _ = std::error::Error::source(&error);
    }

    let compression = CompressionError::new("compression", ErrorCode::Fatal, "detail");
    assert_eq!(compression.reason_code(), "compression");
    assert_eq!(compression.error_code(), ErrorCode::Fatal);
    assert_eq!(compression.detail(), "detail");
    assert_eq!(
        compression.to_string(),
        "context compression failed (compression): detail"
    );
    let store = SessionStoreError::new("store", "detail");
    assert_eq!(store.reason_code(), "store");
    assert_eq!(store.detail(), "detail");
    assert_eq!(store.to_string(), "session store failed (store): detail");
}
