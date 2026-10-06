//! Contract tests for the host-side `visual_assert` evidence source (ADR-0079).

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use assistant_agent_core::{
    ObservationCollector, RuntimeExecutionError, StorageVisualObservationCollector,
};
use assistant_protocol::{ToolEnvelope, serde_json::json};
use assistant_storage::{
    BlobId, BlobKind, Clock, Database, MIGRATIONS, MigrationSet, StoragePaths,
};
use assistant_task_engine::{PlanStep, Reversibility, StepEffect, StepId, StepTimeouts};

struct FixedClock;

impl Clock for FixedClock {
    fn now_unix_ms(&self) -> i64 {
        1_700_000_000_000
    }
}

struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    fn new(label: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos());
        let path = std::env::temp_dir().join(format!(
            "assistant-visual-source-{label}-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("create temp data root");
        Self { path }
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

type Handle = Arc<Mutex<Database>>;

fn open_handle(directory: &TestDirectory) -> Handle {
    let mut migrations = MigrationSet::new();
    migrations
        .register_all(MIGRATIONS)
        .expect("storage migrations register");
    migrations
        .register_all(assistant_audit::MIGRATIONS)
        .expect("audit migrations register");
    let paths = StoragePaths::new(&directory.path);
    let database =
        Database::open(&paths, Arc::new(FixedClock), &migrations).expect("open database");
    Arc::new(Mutex::new(database))
}

fn put_screenshot(handle: &Handle, bytes: &[u8]) -> BlobId {
    let database = handle.lock().expect("database lock");
    database
        .blob_store()
        .put(database.connection(), BlobKind::Screenshot, bytes)
        .expect("store screenshot")
}

fn descriptor(blob_id: &BlobId, width: u32, height: u32) -> serde_json::Value {
    json!({
        "blob_id": blob_id.as_str(),
        "width": width,
        "height": height,
    })
}

fn envelope(reference: &serde_json::Value, observed: &serde_json::Value) -> ToolEnvelope {
    ToolEnvelope::ok(
        "paint.canvas.capture".to_owned(),
        "t_visual_source".to_owned(),
        "s_visual_source".to_owned(),
        json!({
            "visual_observation": {
                "reference": reference,
                "observed": observed,
                "confidence": 0.95,
            }
        }),
    )
}

fn step() -> PlanStep {
    PlanStep {
        id: StepId::new("s_visual_source").expect("step id"),
        sequence: 1,
        tool: "paint.canvas.capture".to_owned(),
        args: json!({}),
        depends_on: Vec::new(),
        postconditions: vec![json!({
            "kind": "visual_assert",
            "field": "pixels",
            "op": "mean_abs_diff_within",
            "max_mean_abs_diff": 0,
            "confidence_min": 0.8
        })],
        effect: StepEffect::Write,
        reversibility: Reversibility::L0UndoStack,
        point_of_no_return: false,
        timeouts: StepTimeouts::new(500, 2_000, 1_000).expect("timeouts"),
    }
}

fn collector(handle: &Handle) -> StorageVisualObservationCollector {
    StorageVisualObservationCollector::new(Arc::clone(handle))
}

#[test]
fn test_visual_source_loads_two_matching_screenshots() {
    let directory = TestDirectory::new("matching");
    let handle = open_handle(&directory);
    let reference = put_screenshot(&handle, &[0, 0, 255, 255]);
    let observed = put_screenshot(&handle, &[0, 0, 255, 255]);
    let envelope = envelope(&descriptor(&reference, 1, 1), &descriptor(&observed, 1, 1));

    let visual = collector(&handle)
        .observe_visual(&step(), &envelope)
        .expect("visual source must load")
        .expect("visual observation must be present");

    assert_eq!(visual.expected.pixels(), &[77]);
    assert_eq!(visual.actual.pixels(), &[77]);
    assert!((visual.confidence - 0.95).abs() < f64::EPSILON);
}

#[test]
fn test_visual_source_rejects_missing_reference_blob() {
    let directory = TestDirectory::new("missing");
    let handle = open_handle(&directory);
    let observed = put_screenshot(&handle, &[0, 0, 255, 255]);
    let missing = BlobId::of_content(b"never stored");
    let envelope = envelope(&descriptor(&missing, 1, 1), &descriptor(&observed, 1, 1));

    let error = collector(&handle)
        .observe_visual(&step(), &envelope)
        .expect_err("missing blob must fail closed");
    assert!(error.to_string().contains("could not be read"), "{error}");
}

#[test]
fn test_visual_source_rejects_dimension_and_byte_count_mismatch() {
    let directory = TestDirectory::new("size");
    let handle = open_handle(&directory);
    let reference = put_screenshot(&handle, &[0, 0, 255, 255]);
    let observed = put_screenshot(&handle, &[0, 0, 255, 255]);
    let envelope = envelope(&descriptor(&reference, 2, 1), &descriptor(&observed, 1, 1));

    let error = collector(&handle)
        .observe_visual(&step(), &envelope)
        .expect_err("dimension mismatch must fail closed");
    assert!(error.to_string().contains("needs 8 bytes"), "{error}");
}

#[test]
fn test_visual_source_rejects_partial_observation_objects() {
    let directory = TestDirectory::new("partial");
    let handle = open_handle(&directory);
    let observed = put_screenshot(&handle, &[0, 0, 255, 255]);
    let envelope = envelope(
        &json!({"blob_id": observed.as_str(), "width": 1}),
        &descriptor(&observed, 1, 1),
    );

    let error = collector(&handle)
        .observe_visual(&step(), &envelope)
        .expect_err("partial image descriptor must fail closed");
    assert!(error.to_string().contains("height"), "{error}");
}

#[test]
fn test_visual_source_rejects_unknown_descriptor_fields() {
    let directory = TestDirectory::new("unknown-field");
    let handle = open_handle(&directory);
    let observed = put_screenshot(&handle, &[0, 0, 255, 255]);
    let envelope = envelope(
        &json!({
            "blob_id": observed.as_str(),
            "width": 1,
            "height": 1,
            "unexpected": "value",
        }),
        &descriptor(&observed, 1, 1),
    );

    let error = collector(&handle)
        .observe_visual(&step(), &envelope)
        .expect_err("unknown fields must fail closed");
    assert!(error.to_string().contains("unknown field"), "{error}");
}

#[test]
fn test_visual_source_returns_none_when_envelope_has_no_visual_field() {
    let directory = TestDirectory::new("none");
    let handle = open_handle(&directory);
    let plain = ToolEnvelope::ok(
        "paint.canvas.capture".to_owned(),
        "t_visual_source".to_owned(),
        "s_visual_source".to_owned(),
        json!({"text": "not visual"}),
    );

    let visual = collector(&handle)
        .observe_visual(&step(), &plain)
        .expect("a non-visual envelope is not an error");
    assert!(visual.is_none());
}

#[test]
fn test_visual_source_maps_bad_metadata_to_runtime_error() {
    let directory = TestDirectory::new("error-type");
    let handle = open_handle(&directory);
    let observed = put_screenshot(&handle, &[0, 0, 255, 255]);
    let envelope = envelope(
        &json!({"width": 1, "height": 1}),
        &descriptor(&observed, 1, 1),
    );

    let error = collector(&handle)
        .observe_visual(&step(), &envelope)
        .expect_err("malformed metadata must fail");
    assert!(matches!(error, RuntimeExecutionError::Observation { .. }));
}
