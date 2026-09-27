//! Deterministic Memory contract tests.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use assistant_core::{
    AppMapFileReader, AppMapReadError, CoreError, Memory, MemoryOmissionReason, MemoryQuery,
    MemoryRecordKind, MemoryRequest, MemoryRetrievalError, MemoryRetrievalHit, MemoryRetriever,
    MemorySearchResult, MemorySegmentOrigin, TokenCount,
};
use assistant_protocol::ErrorCode;

const APP_MAP_PATH: &str = "adapters/notepad/app-map.json";

#[derive(Default)]
struct FakeReader {
    files: BTreeMap<String, String>,
    errors: BTreeMap<String, AppMapReadError>,
}

impl AppMapFileReader for FakeReader {
    fn read_to_string(&self, relative_path: &Path) -> Result<String, AppMapReadError> {
        let key = relative_path.to_string_lossy().replace('\\', "/");
        if let Some(error) = self.errors.get(&key) {
            return Err(error.clone());
        }
        self.files
            .get(&key)
            .cloned()
            .ok_or(AppMapReadError::NotFound)
    }
}

struct FakeRetriever {
    hits: Vec<MemoryRetrievalHit>,
    error: Option<MemoryRetrievalError>,
}

impl MemoryRetriever for FakeRetriever {
    fn retrieve(
        &self,
        _query: &MemoryQuery,
    ) -> Result<Vec<MemoryRetrievalHit>, MemoryRetrievalError> {
        if let Some(error) = &self.error {
            return Err(error.clone());
        }
        Ok(self.hits.clone())
    }
}

fn app_map_json(version: u32, entries: &str) -> String {
    format!(
        r#"{{
  "version": {version},
  "app_id": "com.microsoft.notepad",
  "entries": [{entries}]
}}"#
    )
}

fn entry_json(id: &str, source_reference: &str, content: &str, tokens: u64) -> String {
    format!(
        r#"{{
  "id": "{id}",
  "title": "{id}",
  "content": "{content}",
  "source_reference": "{source_reference}",
  "token_estimate": {tokens}
}}"#
    )
}

fn reader_with(path: &str, contents: String) -> Arc<FakeReader> {
    let mut reader = FakeReader::default();
    reader.files.insert(path.to_owned(), contents);
    Arc::new(reader)
}

fn hit(
    kind: MemoryRecordKind,
    record_id: &str,
    source_reference: &str,
    snippet: &str,
    tokens: u64,
) -> MemoryRetrievalHit {
    MemoryRetrievalHit::new(
        MemorySearchResult {
            record_kind: kind,
            record_id: record_id.to_owned(),
            source_reference: source_reference.to_owned(),
            snippet: snippet.to_owned(),
            score: 0.5,
        },
        TokenCount::new(tokens),
    )
    .expect("valid hit")
}

fn memory_component(reader: Arc<FakeReader>, retriever: FakeRetriever) -> Memory {
    Memory::new(reader, Arc::new(retriever))
}

#[test]
fn test_memory_projection_selects_only_requested_entries_and_hits() {
    let entries = format!(
        "{},{}",
        entry_json("save", "app-map.json#/entries/0", "save rules", 3),
        entry_json("close", "app-map.json#/entries/1", "close rules", 4)
    );
    let retriever = FakeRetriever {
        hits: vec![hit(
            MemoryRecordKind::Preference,
            "pref-1",
            "settings#L2",
            "prefers concise output",
            5,
        )],
        error: None,
    };
    let memory = memory_component(
        reader_with(APP_MAP_PATH, app_map_json(1, &entries)),
        retriever,
    );
    let request = MemoryRequest::new(
        APP_MAP_PATH,
        vec!["save".to_owned()],
        MemoryQuery::new("output"),
        TokenCount::new(100),
    )
    .expect("request");

    let projection = memory.build_projection(&request).expect("projection");
    assert_eq!(projection.segments().len(), 2);
    assert_eq!(
        projection.segments().first().expect("map segment").origin(),
        &MemorySegmentOrigin::AppMap {
            entry_id: "save".to_owned(),
            app_map_path: APP_MAP_PATH.to_owned(),
            entry_index: 0,
        }
    );
    assert_eq!(
        projection
            .segments()
            .get(1)
            .expect("hit segment")
            .source_reference(),
        "settings#L2"
    );
}

#[test]
fn test_memory_projection_records_duplicate_and_over_budget_omissions() {
    let entries = format!(
        "{},{}",
        entry_json("save", "shared#L1", "save rules", 3),
        entry_json("close", "close#L1", "close rules", 4)
    );
    let retriever = FakeRetriever {
        hits: vec![
            hit(MemoryRecordKind::Note, "note-1", "shared#L1", "x", 2),
            hit(MemoryRecordKind::Note, "note-2", "shared#L1", "y", 2),
        ],
        error: None,
    };
    let memory = memory_component(
        reader_with(APP_MAP_PATH, app_map_json(1, &entries)),
        retriever,
    );
    let request = MemoryRequest::new(
        APP_MAP_PATH,
        vec!["save".to_owned(), "close".to_owned()],
        MemoryQuery::new("rules"),
        TokenCount::new(14),
    )
    .expect("request");

    let projection = memory.build_projection(&request).expect("projection");
    assert_eq!(projection.segments().len(), 2);
    assert_eq!(projection.used_tokens(), TokenCount::new(12));
    assert!(projection.omissions().iter().any(|omission| {
        omission.source_reference() == "adapters/notepad/app-map.json#/entries/1"
            && omission.reason() == MemoryOmissionReason::OverBudget
            && omission.token_estimate() == TokenCount::new(11)
    }));
    assert!(projection.omissions().iter().any(|omission| {
        omission.source_reference() == "shared#L1"
            && omission.reason() == MemoryOmissionReason::DuplicateSource
    }));
}

#[test]
fn test_memory_retriever_error_preserves_storage_reason_code() {
    let retriever = FakeRetriever {
        hits: Vec::new(),
        error: Some(MemoryRetrievalError::new(
            "memory_query_invalid",
            ErrorCode::ToolInvalidArgs,
            "query rejected by storage",
        )),
    };
    let memory = memory_component(
        reader_with(
            APP_MAP_PATH,
            app_map_json(
                1,
                &entry_json("save", "app-map.json#/entries/0", "save rules", 3),
            ),
        ),
        retriever,
    );
    let request = MemoryRequest::new(
        APP_MAP_PATH,
        Vec::new(),
        MemoryQuery::new("bad"),
        TokenCount::new(10),
    )
    .expect("request");

    let error = memory
        .build_projection(&request)
        .expect_err("backend error");
    assert_eq!(error.reason_code(), "memory_query_invalid");
    assert_eq!(error.error_code(), ErrorCode::ToolInvalidArgs);
}

#[test]
fn test_memory_request_rejects_duplicate_entry_ids() {
    let error = MemoryRequest::new(
        APP_MAP_PATH,
        vec!["save".to_owned(), "save".to_owned()],
        MemoryQuery::new("save"),
        TokenCount::new(10),
    )
    .expect_err("duplicate ids must fail");
    assert_eq!(error.reason_code(), "invalid_content");
}

#[test]
fn test_memory_request_and_hit_validation_fail_closed() {
    let empty_path = MemoryRequest::new(
        "",
        Vec::new(),
        MemoryQuery::new("query"),
        TokenCount::new(1),
    )
    .expect_err("empty path");
    assert_eq!(empty_path.reason_code(), "invalid_content");

    let empty_entry = MemoryRequest::new(
        APP_MAP_PATH,
        vec![String::new()],
        MemoryQuery::new("query"),
        TokenCount::new(1),
    )
    .expect_err("empty entry id");
    assert_eq!(empty_entry.reason_code(), "invalid_content");

    let empty_reference = MemoryRetrievalHit::new(
        MemorySearchResult {
            record_kind: MemoryRecordKind::Note,
            record_id: "note".to_owned(),
            source_reference: String::new(),
            snippet: "text".to_owned(),
            score: 0.0,
        },
        TokenCount::new(1),
    )
    .expect_err("empty source");
    assert_eq!(empty_reference.reason_code(), "invalid_content");

    let zero_tokens = MemoryRetrievalHit::new(
        MemorySearchResult {
            record_kind: MemoryRecordKind::Note,
            record_id: "note".to_owned(),
            source_reference: "source#L1".to_owned(),
            snippet: "text".to_owned(),
            score: 0.0,
        },
        TokenCount::new(0),
    )
    .expect_err("zero tokens");
    assert_eq!(zero_tokens.reason_code(), "invalid_content");
}

#[test]
fn test_memory_projection_rejects_missing_entry_id() {
    let memory = memory_component(
        reader_with(
            APP_MAP_PATH,
            app_map_json(
                1,
                &entry_json("save", "app-map.json#/entries/0", "save rules", 3),
            ),
        ),
        FakeRetriever {
            hits: Vec::new(),
            error: None,
        },
    );
    let request = MemoryRequest::new(
        APP_MAP_PATH,
        vec!["missing".to_owned()],
        MemoryQuery::new("query"),
        TokenCount::new(10),
    )
    .expect("request");
    let error = memory
        .build_projection(&request)
        .expect_err("missing entry");
    assert_eq!(error.reason_code(), "app_map_entry_not_found");
    assert_eq!(error.error_code(), ErrorCode::TargetNotFound);
}

#[test]
fn test_memory_error_mappings_and_display_are_stable() {
    let errors = [
        CoreError::AppMapMissing {
            path: "app.json".to_owned(),
        },
        CoreError::AppMapUnreadable {
            path: "app.json".to_owned(),
            reason: "denied".to_owned(),
        },
        CoreError::AppMapCorrupt {
            path: "app.json".to_owned(),
            reason: "bad".to_owned(),
        },
        CoreError::AppMapVersionMismatch {
            expected: 1,
            found: 2,
        },
        CoreError::AppMapPathTraversal {
            path: "../app.json".to_owned(),
        },
        CoreError::AppMapEntryNotFound {
            entry_id: "missing".to_owned(),
        },
        CoreError::MemoryRetrieval(MemoryRetrievalError::new(
            "storage_down",
            ErrorCode::Transient,
            "database busy",
        )),
    ];
    let expected = [
        ("app_map_missing", ErrorCode::TargetNotFound),
        ("app_map_unreadable", ErrorCode::ToolInvalidArgs),
        ("app_map_corrupt", ErrorCode::ToolInvalidArgs),
        ("app_map_version_mismatch", ErrorCode::ToolInvalidArgs),
        ("app_map_path_traversal", ErrorCode::PolicyDenied),
        ("app_map_entry_not_found", ErrorCode::TargetNotFound),
        ("storage_down", ErrorCode::Transient),
    ];
    for (error, (reason_code, error_code)) in errors.iter().zip(expected) {
        assert_eq!(error.reason_code(), reason_code);
        assert_eq!(error.error_code(), error_code);
        assert!(!error.to_string().is_empty());
    }
}

#[test]
fn test_memory_projection_checks_budget_before_duplicate_source() {
    let retriever = FakeRetriever {
        hits: vec![
            hit(MemoryRecordKind::Note, "large", "same#L1", "x", 10),
            hit(MemoryRecordKind::Note, "small", "same#L1", "y", 1),
        ],
        error: None,
    };
    let memory = memory_component(
        reader_with(
            APP_MAP_PATH,
            app_map_json(
                1,
                &entry_json("save", "app-map.json#/entries/0", "save rules", 1),
            ),
        ),
        retriever,
    );
    let request = MemoryRequest::new(
        APP_MAP_PATH,
        Vec::new(),
        MemoryQuery::new("query"),
        TokenCount::new(5),
    )
    .expect("request");
    let projection = memory.build_projection(&request).expect("projection");
    assert_eq!(projection.segments().len(), 1);
    assert_eq!(
        projection.segments().first().expect("segment").content(),
        "y"
    );
    assert_eq!(projection.omissions().len(), 1);
    assert_eq!(
        projection.omissions().first().expect("omission").reason(),
        MemoryOmissionReason::OverBudget
    );
}
