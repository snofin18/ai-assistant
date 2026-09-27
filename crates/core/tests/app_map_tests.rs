//! Deterministic App Map loading and validation contract tests.
#![allow(clippy::expect_used)]

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use assistant_core::{
    APP_MAP_VERSION, AppMapFileReader, AppMapReadError, Memory, MemoryQuery, MemoryRequest,
    MemoryRetrievalError, MemoryRetrievalHit, MemoryRetriever, MemorySegmentOrigin, TokenCount,
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

fn memory_component(reader: Arc<FakeReader>, retriever: FakeRetriever) -> Memory {
    Memory::new(reader, Arc::new(retriever))
}

const fn empty_retriever() -> FakeRetriever {
    FakeRetriever {
        hits: Vec::new(),
        error: None,
    }
}

#[test]
fn test_app_map_loader_validates_and_loads_entries() {
    let reader = reader_with(
        APP_MAP_PATH,
        app_map_json(
            APP_MAP_VERSION,
            &entry_json("save", "app-map.json#/entries/0", "save rules", 7),
        ),
    );
    let memory = memory_component(reader, empty_retriever());

    let app_map = memory.load_app_map(APP_MAP_PATH).expect("load App Map");
    assert_eq!(app_map.version(), APP_MAP_VERSION);
    assert_eq!(app_map.app_id(), "com.microsoft.notepad");
    assert_eq!(app_map.entries().len(), 1);
    let entry = app_map.entry("save").expect("entry");
    assert_eq!(
        entry.source_reference(),
        "adapters/notepad/app-map.json#/entries/0"
    );
    assert_eq!(entry.entry_index(), 0);
    assert_eq!(entry.token_estimate(), TokenCount::new(10));
}

#[test]
fn test_app_map_loader_missing_file_fails_closed() {
    let memory = memory_component(Arc::new(FakeReader::default()), empty_retriever());

    let error = memory.load_app_map(APP_MAP_PATH).expect_err("missing file");
    assert_eq!(error.reason_code(), "app_map_missing");
    assert_eq!(error.error_code(), ErrorCode::TargetNotFound);
}

#[test]
fn test_app_map_loader_corrupt_json_fails_closed() {
    let memory = memory_component(
        reader_with(APP_MAP_PATH, "{not-json".to_owned()),
        empty_retriever(),
    );

    let error = memory.load_app_map(APP_MAP_PATH).expect_err("corrupt map");
    assert_eq!(error.reason_code(), "app_map_corrupt");
    assert_eq!(error.error_code(), ErrorCode::ToolInvalidArgs);
}

#[test]
fn test_app_map_loader_version_mismatch_fails_closed() {
    let memory = memory_component(
        reader_with(
            APP_MAP_PATH,
            app_map_json(
                APP_MAP_VERSION + 1,
                &entry_json("save", "app-map.json#/entries/0", "save rules", 7),
            ),
        ),
        empty_retriever(),
    );

    let error = memory
        .load_app_map(APP_MAP_PATH)
        .expect_err("version mismatch");
    assert_eq!(error.reason_code(), "app_map_version_mismatch");
    assert_eq!(error.error_code(), ErrorCode::ToolInvalidArgs);
}

#[test]
fn test_app_map_loader_rejects_path_traversal_before_reader_call() {
    let memory = memory_component(Arc::new(FakeReader::default()), empty_retriever());

    let error = memory
        .load_app_map("../secret.json")
        .expect_err("traversal must fail");
    assert_eq!(error.reason_code(), "app_map_path_traversal");
    assert_eq!(error.error_code(), ErrorCode::PolicyDenied);
}

#[test]
fn test_app_map_loader_reports_unreadable_and_invalid_shapes() {
    let mut reader = FakeReader::default();
    reader.errors.insert(
        "unreadable.json".to_owned(),
        AppMapReadError::Unreadable("permission denied".to_owned()),
    );
    let memory = memory_component(Arc::new(reader), empty_retriever());
    let error = memory
        .load_app_map("unreadable.json")
        .expect_err("unreadable file");
    assert_eq!(error.reason_code(), "app_map_unreadable");
    assert_eq!(error.error_code(), ErrorCode::ToolInvalidArgs);

    let invalid_documents = [
        "[]",
        r#"{"version":1,"app_id":"app"}"#,
        r#"{"version":1,"app_id":"app","entries":[]}"#,
        r#"{"version":1,"app_id":"app","entries":[1]}"#,
        r#"{"version":1,"app_id":"app","entries":[{"id":"a","title":"a","content":"a","source_reference":"r","token_estimate":1,"extra":true}]}"#,
        r#"{"version":1,"app_id":"app","entries":[{"id":"a","title":"a","content":"a","source_reference":"r"}]}"#,
        r#"{"version":1,"app_id":"app","entries":[{"id":"a","title":"a","content":"a","source_reference":"r","token_estimate":0}]}"#,
        r#"{"version":1,"app_id":"app","entries":[{"id":"a","title":"a","content":"a","source_reference":"r","token_estimate":1},{"id":"a","title":"b","content":"b","source_reference":"r2","token_estimate":1}]}"#,
    ];
    for document in invalid_documents {
        let candidate_memory = memory_component(
            reader_with(APP_MAP_PATH, document.to_owned()),
            empty_retriever(),
        );
        let error = candidate_memory
            .load_app_map(APP_MAP_PATH)
            .expect_err("invalid document");
        assert_eq!(error.reason_code(), "app_map_corrupt");
        assert_eq!(error.error_code(), ErrorCode::ToolInvalidArgs);
    }
}

#[test]
fn test_app_map_path_validation_rejects_non_normal_components() {
    let memory = memory_component(Arc::new(FakeReader::default()), empty_retriever());
    for path in ["", ".", "./app.json", "dir/../app.json", "dir\\app.json"] {
        let error = memory.load_app_map(path).expect_err("unsafe path");
        assert_eq!(error.reason_code(), "app_map_path_traversal");
        assert_eq!(error.error_code(), ErrorCode::PolicyDenied);
    }
}

#[test]
fn test_app_map_token_estimate_is_conservative_and_provenance_is_canonical() {
    let document = r#"{
  "version": 1,
  "app_id": "app",
  "entries": [{
    "id": "entry",
    "title": "entry",
    "content": "0123456789",
    "source_reference": "forged#/somewhere",
    "token_estimate": 1
  }]
}"#
    .to_string();
    let memory = memory_component(reader_with(APP_MAP_PATH, document), empty_retriever());
    let entry = memory
        .load_app_map(APP_MAP_PATH)
        .expect("load")
        .entry("entry")
        .expect("entry")
        .clone();
    assert_eq!(
        entry.source_reference(),
        "adapters/notepad/app-map.json#/entries/0"
    );
    assert_eq!(entry.token_estimate(), TokenCount::new(10));

    let request = MemoryRequest::new(
        APP_MAP_PATH,
        vec!["entry".to_owned()],
        MemoryQuery::new("query"),
        TokenCount::new(9),
    )
    .expect("request");
    let projection = memory.build_projection(&request).expect("projection");
    assert!(projection.segments().is_empty());
    let omission = projection.omissions().first().expect("omission");
    assert_eq!(
        omission.reason(),
        assistant_core::MemoryOmissionReason::OverBudget
    );
    assert_eq!(omission.token_estimate(), TokenCount::new(10));
    assert_eq!(omission.origin(), &entry_origin());
}

fn entry_origin() -> MemorySegmentOrigin {
    MemorySegmentOrigin::AppMap {
        entry_id: "entry".to_owned(),
        app_map_path: APP_MAP_PATH.to_owned(),
        entry_index: 0,
    }
}

#[test]
fn test_app_map_content_size_limit_is_enforced() {
    let content = "x".repeat(16_385);
    let document = app_map_json(
        APP_MAP_VERSION,
        &entry_json("large", "ignored", &content, 1),
    );
    let memory = memory_component(reader_with(APP_MAP_PATH, document), empty_retriever());
    let error = memory
        .load_app_map(APP_MAP_PATH)
        .expect_err("oversized content");
    assert_eq!(error.reason_code(), "app_map_corrupt");
}
