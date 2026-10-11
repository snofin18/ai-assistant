//! Public contract coverage for Core session persistence, errors, and review.
//!
//! These tests stay on the public crate boundary and use deterministic in-memory
//! dependencies. They do not read the filesystem, network, or desktop.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::sync::Arc;

use assistant_core::{
    CleanContextReviewDecision, CleanContextReviewError, CleanContextReviewRequest,
    CleanContextReviewer, CompressionError, ContextRetention, CoreError, HighRiskStepSummary,
    MemoryRetrievalError, MemorySessionStore, MessageContent, MessageId, MessageNode,
    MessageNodeParts, MessageRole, NewMessage, SessionClock, SessionId, SessionSnapshot,
    SessionSnapshotParts, SessionStatus, SessionStore, SessionStoreError, TokenCount,
};
use assistant_model_gateway::{
    CancellationToken, CompletionEvent, CompletionRequest, CompletionStream, DurationMs,
    FinishReason, Message, ModelGatewayError, ModelId, ModelProvider, ModelResult, Pricing,
    ProviderCapabilities, ProviderFeatures, TokenCount as ModelTokenCount,
};
use assistant_protocol::ErrorCode;

const REVIEW_MODEL: &str = "review-small-model";

struct FixedClock(i64);

impl SessionClock for FixedClock {
    fn now_unix_ms(&self) -> i64 {
        self.0
    }
}

struct ScriptedProvider {
    model_id: ModelId,
    events: Vec<ModelResult<Option<CompletionEvent>>>,
}

impl ModelProvider for ScriptedProvider {
    fn model_id(&self) -> &ModelId {
        &self.model_id
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities::new(ProviderFeatures::STREAMING, ModelTokenCount::new(8_192))
    }

    fn pricing(&self) -> Pricing {
        Pricing::new(1, 1, 1)
    }

    fn count_tokens(&self, _messages: &[Message]) -> ModelResult<ModelTokenCount> {
        Ok(ModelTokenCount::new(1))
    }

    fn complete(
        &self,
        _request: CompletionRequest,
        _cancellation: CancellationToken,
    ) -> ModelResult<Box<dyn CompletionStream>> {
        Ok(Box::new(ScriptedStream {
            events: self.events.clone(),
            index: 0,
        }))
    }
}

struct ScriptedStream {
    events: Vec<ModelResult<Option<CompletionEvent>>>,
    index: usize,
}

impl CompletionStream for ScriptedStream {
    fn next_event(
        &mut self,
        _cancellation: &CancellationToken,
        _timeout: DurationMs,
    ) -> ModelResult<Option<CompletionEvent>> {
        let Some(event) = self.events.get(self.index).cloned() else {
            return Ok(None);
        };
        self.index += 1;
        event
    }
}

fn reviewer_with_events(events: Vec<ModelResult<Option<CompletionEvent>>>) -> CleanContextReviewer {
    let provider: Arc<dyn ModelProvider> = Arc::new(ScriptedProvider {
        model_id: ModelId::new(REVIEW_MODEL).expect("review model id"),
        events,
    });
    CleanContextReviewer::new(provider)
}

fn text_events(text: &str) -> Vec<ModelResult<Option<CompletionEvent>>> {
    vec![
        Ok(Some(CompletionEvent::TextDelta(text.to_string()))),
        Ok(Some(CompletionEvent::Finished(FinishReason::Stop))),
    ]
}

fn review_request(goal: &str, summary: &str) -> CleanContextReviewRequest {
    let snapshot = SessionSnapshot::restore(SessionSnapshotParts {
        id: SessionId::new("s_review_contract").expect("session id"),
        goal: goal.to_string(),
        status: SessionStatus::Active,
        created_at_unix_ms: 0,
        ended_at_unix_ms: None,
        revision: 0,
        messages: Vec::new(),
    })
    .expect("review snapshot");
    CleanContextReviewRequest::from_session(
        &snapshot,
        HighRiskStepSummary::new(summary).expect("step summary"),
    )
}

fn snapshot(id: &str, revision: u64) -> SessionSnapshot {
    SessionSnapshot::restore(SessionSnapshotParts {
        id: SessionId::new(id).expect("session id"),
        goal: "read the selected text".to_string(),
        status: SessionStatus::Active,
        created_at_unix_ms: 1,
        ended_at_unix_ms: None,
        revision,
        messages: Vec::new(),
    })
    .expect("session snapshot")
}

fn message_node(
    id: &str,
    parent_id: Option<&str>,
    sequence: u32,
    token_estimate: u64,
) -> assistant_core::CoreResult<MessageNode> {
    MessageNode::restore(MessageNodeParts {
        id: MessageId::new(id).expect("message id"),
        parent_id: parent_id.map(|value| MessageId::new(value).expect("parent id")),
        sequence,
        role: MessageRole::User,
        content: MessageContent::new("read").expect("message content"),
        token_estimate: TokenCount::new(token_estimate),
        retention: ContextRetention::Required,
    })
}

fn review_text(text: &str) -> Result<CleanContextReviewDecision, CleanContextReviewError> {
    reviewer_with_events(text_events(text)).review(
        &review_request("Read the selected text.", "Read the selected text."),
        CancellationToken::new(),
    )
}

#[test]
fn test_memory_session_store_contract_rejects_duplicate_missing_and_stale_revisions() {
    let store = MemorySessionStore::new();
    let session_id = SessionId::new("s_store_contract").expect("session id");
    assert!(
        store
            .load_session(&session_id)
            .expect("empty load")
            .is_none()
    );

    let initial = snapshot("s_store_contract", 0);
    store.insert_session(&initial).expect("initial insert");
    assert_eq!(
        store.load_session(&session_id).expect("initial load"),
        Some(initial.clone())
    );

    let duplicate = store
        .insert_session(&initial)
        .expect_err("duplicate insert must fail");
    assert_eq!(duplicate.reason_code(), "session_already_exists");

    let updated = snapshot("s_store_contract", 1);
    store.update_session(&updated).expect("revision update");
    assert_eq!(
        store.load_session(&session_id).expect("updated load"),
        Some(updated)
    );

    let stale = snapshot("s_store_contract", 1);
    let stale_error = store
        .update_session(&stale)
        .expect_err("stale revision must fail");
    assert_eq!(stale_error.reason_code(), "revision_conflict");

    let missing = snapshot("s_store_missing", 0);
    let missing_error = store
        .update_session(&missing)
        .expect_err("missing session must fail");
    assert_eq!(missing_error.reason_code(), "session_not_found");

    let overflow = snapshot("s_store_overflow", u64::MAX);
    store
        .insert_session(&overflow)
        .expect("overflow revision insert");
    let overflow_error = store
        .update_session(&overflow)
        .expect_err("revision overflow must fail");
    assert_eq!(overflow_error.reason_code(), "revision_overflow");
}

fn core_error_cases_a() -> Vec<(CoreError, ErrorCode, &'static str)> {
    let cases = [
        (
            CoreError::InvalidIdentifier {
                kind: "session",
                value: "bad".to_string(),
            },
            ErrorCode::ToolInvalidArgs,
            "invalid_identifier",
        ),
        (
            CoreError::InvalidContent {
                field: "message.content",
                reason: "empty".to_string(),
            },
            ErrorCode::ToolInvalidArgs,
            "invalid_content",
        ),
        (
            CoreError::SessionAlreadyExists {
                session_id: "s".to_string(),
            },
            ErrorCode::ToolInvalidArgs,
            "session_already_exists",
        ),
        (
            CoreError::SessionNotFound {
                session_id: "s".to_string(),
            },
            ErrorCode::TargetNotFound,
            "session_not_found",
        ),
        (
            CoreError::SessionEnded {
                session_id: "s".to_string(),
            },
            ErrorCode::Fatal,
            "session_ended",
        ),
        (
            CoreError::MessageNotFound {
                session_id: "s".to_string(),
                message_id: "m".to_string(),
            },
            ErrorCode::TargetNotFound,
            "message_not_found",
        ),
        (
            CoreError::ParentMessageNotFound {
                session_id: "s".to_string(),
                parent_message_id: "m".to_string(),
            },
            ErrorCode::TargetNotFound,
            "parent_message_not_found",
        ),
        (
            CoreError::MessageHasChildren {
                message_id: "m".to_string(),
            },
            ErrorCode::ToolInvalidArgs,
            "message_has_children",
        ),
        (
            CoreError::InvalidSessionSnapshot {
                reason: "bad tree".to_string(),
            },
            ErrorCode::ToolInvalidArgs,
            "invalid_session_snapshot",
        ),
        (
            CoreError::InvalidBudget {
                field: "context",
                reason: "bad".to_string(),
            },
            ErrorCode::ToolInvalidArgs,
            "invalid_budget",
        ),
    ];
    cases.to_vec()
}

fn core_error_cases_b() -> Vec<(CoreError, ErrorCode, &'static str)> {
    let cases = [
        (
            CoreError::RequiredContextExceedsBudget {
                required_tokens: 2,
                available_tokens: 1,
            },
            ErrorCode::PolicyDenied,
            "required_context_exceeds_budget",
        ),
        (
            CoreError::CompressionFailed(CompressionError::new(
                "compression_failed",
                ErrorCode::Transient,
                "detail",
            )),
            ErrorCode::Transient,
            "compression_failed",
        ),
        (
            CoreError::SessionStore(SessionStoreError::new("store_failed", "detail")),
            ErrorCode::Fatal,
            "store_failed",
        ),
        (
            CoreError::NumericOverflow { field: "revision" },
            ErrorCode::Fatal,
            "numeric_overflow",
        ),
        (
            CoreError::PlannerModel(ModelGatewayError::Timeout {
                model_id: ModelId::new(REVIEW_MODEL).expect("model id"),
                timeout: DurationMs::new(1),
            }),
            ErrorCode::ModelNetworkFailure,
            "planner_model_failure",
        ),
        (
            CoreError::InvalidPlan {
                reason: "bad plan".to_string(),
            },
            ErrorCode::ModelInvalidOutput,
            "invalid_plan",
        ),
        (
            CoreError::InvalidPlannerOutput {
                reason: "bad output".to_string(),
            },
            ErrorCode::ModelInvalidOutput,
            "invalid_planner_output",
        ),
        (
            CoreError::UnknownPlannerTool {
                step_id: "step".to_string(),
                tool: "unknown".to_string(),
            },
            ErrorCode::ModelInvalidOutput,
            "unknown_planner_tool",
        ),
    ];
    cases.to_vec()
}

fn core_error_cases_c() -> Vec<(CoreError, ErrorCode, &'static str)> {
    let cases = [
        (
            CoreError::AppMapMissing {
                path: "map.json".to_string(),
            },
            ErrorCode::TargetNotFound,
            "app_map_missing",
        ),
        (
            CoreError::AppMapUnreadable {
                path: "map.json".to_string(),
                reason: "read failed".to_string(),
            },
            ErrorCode::ToolInvalidArgs,
            "app_map_unreadable",
        ),
        (
            CoreError::AppMapCorrupt {
                path: "map.json".to_string(),
                reason: "bad json".to_string(),
            },
            ErrorCode::ToolInvalidArgs,
            "app_map_corrupt",
        ),
        (
            CoreError::AppMapVersionMismatch {
                expected: 1,
                found: 2,
            },
            ErrorCode::ToolInvalidArgs,
            "app_map_version_mismatch",
        ),
        (
            CoreError::AppMapPathTraversal {
                path: "../map.json".to_string(),
            },
            ErrorCode::PolicyDenied,
            "app_map_path_traversal",
        ),
        (
            CoreError::AppMapEntryNotFound {
                entry_id: "missing".to_string(),
            },
            ErrorCode::TargetNotFound,
            "app_map_entry_not_found",
        ),
        (
            CoreError::MemoryRetrieval(MemoryRetrievalError::new(
                "memory_failed",
                ErrorCode::Transient,
                "detail",
            )),
            ErrorCode::Transient,
            "memory_failed",
        ),
    ];
    cases.to_vec()
}

#[test]
fn test_core_error_codes_reason_codes_and_display_are_stable() {
    for (error, code, reason_code) in core_error_cases_a()
        .into_iter()
        .chain(core_error_cases_b())
        .chain(core_error_cases_c())
    {
        assert_eq!(error.error_code(), code);
        assert_eq!(error.reason_code(), reason_code);
        assert!(!error.to_string().is_empty());
    }

    for error in [
        CoreError::CompressionFailed(CompressionError::new(
            "compression_failed",
            ErrorCode::Transient,
            "detail",
        )),
        CoreError::SessionStore(SessionStoreError::new("store_failed", "detail")),
        CoreError::MemoryRetrieval(MemoryRetrievalError::new(
            "memory_failed",
            ErrorCode::Transient,
            "detail",
        )),
    ] {
        assert!(std::error::Error::source(&error).is_some());
    }

    assert!(
        !CompressionError::new("compression", ErrorCode::Transient, "detail")
            .to_string()
            .is_empty()
    );
    assert!(
        !SessionStoreError::new("store", "detail")
            .to_string()
            .is_empty()
    );
    assert!(
        !MemoryRetrievalError::new("memory", ErrorCode::Transient, "detail")
            .to_string()
            .is_empty()
    );
}

#[test]
fn test_clean_context_review_error_contract_is_stable_and_fail_closed() {
    let model_error = CleanContextReviewError::Model(ModelGatewayError::Timeout {
        model_id: ModelId::new(REVIEW_MODEL).expect("model id"),
        timeout: DurationMs::new(1),
    });
    assert_eq!(model_error.error_code(), ErrorCode::ModelNetworkFailure);
    assert_eq!(
        model_error.reason_code(),
        "clean_context_review_model_failure"
    );
    assert!(std::error::Error::source(&model_error).is_some());
    assert!(!model_error.to_string().is_empty());

    let invalid_input = CleanContextReviewError::InvalidInput {
        field: "review.summary",
        reason: "bad".to_string(),
    };
    assert_eq!(invalid_input.error_code(), ErrorCode::ToolInvalidArgs);
    assert_eq!(
        invalid_input.reason_code(),
        "clean_context_review_invalid_input"
    );
    assert!(std::error::Error::source(&invalid_input).is_none());

    let invalid_output = CleanContextReviewError::InvalidModelOutput {
        reason: "bad".to_string(),
    };
    assert_eq!(invalid_output.error_code(), ErrorCode::ModelInvalidOutput);
    assert_eq!(
        invalid_output.reason_code(),
        "clean_context_review_invalid_output"
    );
    assert!(!invalid_input.to_string().is_empty());
    assert!(!invalid_output.to_string().is_empty());

    let inconsistent = CleanContextReviewError::Inconsistent {
        reason: "mismatch".to_string(),
    };
    assert_eq!(inconsistent.error_code(), ErrorCode::PolicyDenied);
    assert_eq!(
        inconsistent.reason_code(),
        "clean_context_review_inconsistent"
    );
    assert!(!inconsistent.to_string().is_empty());

    let request = review_request("Read.", "Read.");
    assert_eq!(request.original_user_request(), "Read.");
    assert_eq!(request.high_risk_step_summary().as_str(), "Read.");

    let converted: CleanContextReviewError = ModelGatewayError::Timeout {
        model_id: ModelId::new(REVIEW_MODEL).expect("model id"),
        timeout: DurationMs::new(1),
    }
    .into();
    assert!(matches!(converted, CleanContextReviewError::Model(_)));
}

#[test]
fn test_clean_context_review_input_and_timeout_bounds_fail_closed() {
    assert!(HighRiskStepSummary::new("").is_err());
    assert!(HighRiskStepSummary::new("x".repeat(8_193)).is_err());
    assert!(HighRiskStepSummary::new("bad\0summary").is_err());
    assert!(HighRiskStepSummary::new("safe summary").is_ok());

    assert!(
        reviewer_with_events(Vec::new())
            .with_event_timeout(DurationMs::new(0))
            .is_err()
    );
    assert!(
        reviewer_with_events(Vec::new())
            .with_event_timeout(DurationMs::new(501))
            .is_err()
    );
    assert!(
        reviewer_with_events(Vec::new())
            .with_event_timeout(DurationMs::new(500))
            .is_ok()
    );
}

#[test]
fn test_clean_context_review_parser_contracts_fail_closed() {
    let cases = [
        ("[]", "clean_context_review_invalid_output"),
        ("{}", "clean_context_review_invalid_output"),
        (
            r#"{"verdict":"consistent","reason":"ok","extra":1}"#,
            "clean_context_review_invalid_output",
        ),
        (
            r#"{"verdict":1,"reason":"ok"}"#,
            "clean_context_review_invalid_output",
        ),
        (
            r#"{"verdict":"maybe","reason":"ok"}"#,
            "clean_context_review_invalid_output",
        ),
        (
            r#"{"verdict":"consistent","reason":""}"#,
            "clean_context_review_invalid_output",
        ),
        (
            r#"{"verdict":"consistent","reason":"bad\u0000"}"#,
            "clean_context_review_invalid_output",
        ),
        (
            &format!(
                r#"{{"verdict":"consistent","reason":"{}"}}"#,
                "x".repeat(2_049)
            ),
            "clean_context_review_invalid_output",
        ),
    ];

    for (text, reason_code) in cases {
        let error = review_text(text).expect_err("invalid review output");
        assert_eq!(error.reason_code(), reason_code);
    }

    let allowed =
        review_text(r#"{"verdict":"consistent","reason":"matches"}"#).expect("consistent review");
    assert!(allowed.is_allowed());
    assert_eq!(allowed.reason(), "matches");

    let rejected = review_text(r#"{"verdict":"inconsistent","reason":"mismatch"}"#)
        .expect("inconsistent review");
    assert!(!rejected.is_allowed());
    assert_eq!(rejected.reason(), "mismatch");
}

#[test]
fn test_clean_context_review_provider_event_contracts_fail_closed() {
    let stop_only = reviewer_with_events(vec![Ok(Some(CompletionEvent::Finished(
        FinishReason::Stop,
    )))]);
    let error = stop_only
        .review(&review_request("Read.", "Read."), CancellationToken::new())
        .expect_err("stop without output");
    assert!(matches!(
        error,
        CleanContextReviewError::InvalidModelOutput { .. }
    ));

    let empty_stream = reviewer_with_events(vec![Ok(None)]);
    let error = empty_stream
        .review(&review_request("Read.", "Read."), CancellationToken::new())
        .expect_err("empty stream");
    assert!(matches!(
        error,
        CleanContextReviewError::InvalidModelOutput { .. }
    ));

    let length_finish = reviewer_with_events(vec![Ok(Some(CompletionEvent::Finished(
        FinishReason::Length,
    )))]);
    let error = length_finish
        .review(&review_request("Read.", "Read."), CancellationToken::new())
        .expect_err("non-stop finish");
    assert!(matches!(
        error,
        CleanContextReviewError::InvalidModelOutput { .. }
    ));

    let invalid_event =
        reviewer_with_events(vec![Ok(Some(CompletionEvent::TextDelta(String::new())))]);
    let error = invalid_event
        .review(&review_request("Read.", "Read."), CancellationToken::new())
        .expect_err("invalid provider event");
    assert!(matches!(error, CleanContextReviewError::Model(_)));
}

#[test]
fn test_memory_session_manager_contract_keeps_store_failures_explicit() {
    let store: Arc<dyn SessionStore> = Arc::new(MemorySessionStore::new());
    let mut manager = assistant_core::SessionManager::new(store, Arc::new(FixedClock(100)));
    let session_id = SessionId::new("s_manager_contract").expect("session id");
    manager
        .create_session(session_id.clone(), "read")
        .expect("create session");
    manager
        .append_message(
            &session_id,
            NewMessage {
                id: MessageId::new("m_contract").expect("message id"),
                parent_id: None,
                role: MessageRole::User,
                content: MessageContent::new("read").expect("content"),
                token_estimate: TokenCount::new(1),
                retention: assistant_core::ContextRetention::Required,
            },
        )
        .expect("append message");
    assert_eq!(
        manager
            .snapshot(&session_id)
            .expect("snapshot")
            .messages()
            .len(),
        1
    );
}

#[test]
fn test_session_snapshot_message_tree_contracts_fail_closed() {
    assert!(message_node("m_zero", None, 0, 0).is_err());
    let valid_node = message_node("m_valid", None, 0, 1).expect("valid message");
    let child_node = message_node("m_child", Some("m_valid"), 1, 1).expect("child message");
    let valid_snapshot = SessionSnapshot::restore(SessionSnapshotParts {
        id: SessionId::new("s_message_path").expect("session id"),
        goal: "read".to_string(),
        status: SessionStatus::Active,
        created_at_unix_ms: 1,
        ended_at_unix_ms: None,
        revision: 0,
        messages: vec![valid_node, child_node],
    })
    .expect("valid snapshot");
    let child_id = MessageId::new("m_child").expect("message id");
    assert_eq!(
        valid_snapshot
            .message_path(&child_id)
            .expect("valid message path")
            .len(),
        2
    );
    assert_eq!(valid_snapshot.latest_leaf(), Some(&child_id));
    assert!(
        valid_snapshot
            .message_path(&MessageId::new("m_missing").expect("message id"))
            .is_err()
    );

    let duplicate_a = message_node("m_duplicate", None, 0, 1).expect("duplicate a");
    let duplicate_b = message_node("m_duplicate", None, 1, 1).expect("duplicate b");
    assert!(
        SessionSnapshot::restore(SessionSnapshotParts {
            id: SessionId::new("s_duplicate").expect("session id"),
            goal: "read".to_string(),
            status: SessionStatus::Active,
            created_at_unix_ms: 1,
            ended_at_unix_ms: None,
            revision: 0,
            messages: vec![duplicate_a, duplicate_b],
        })
        .is_err()
    );

    let orphan = message_node("m_orphan", Some("m_missing"), 0, 1).expect("orphan message");
    assert!(
        SessionSnapshot::restore(SessionSnapshotParts {
            id: SessionId::new("s_orphan").expect("session id"),
            goal: "read".to_string(),
            status: SessionStatus::Active,
            created_at_unix_ms: 1,
            ended_at_unix_ms: None,
            revision: 0,
            messages: vec![orphan],
        })
        .is_err()
    );
}
