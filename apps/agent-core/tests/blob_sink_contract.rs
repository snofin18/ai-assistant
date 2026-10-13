//! Cross-platform public contract tests for `StorageBlobSink`.
//!
//! Responsibilities: prove one-time attachment, fail-closed errors, content-address validation,
//! and successful `BlobKind::Screenshot` persistence without touching a real GUI.
//!
//! Boundaries: no production API changes, no private-field access, no network or credentials.
//! Related documents: ADR-0076 D6/D8 and `apps/agent-core/src/blob_sink.rs`.

#![allow(clippy::expect_used, clippy::panic)]

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use assistant_agent_core::StorageBlobSink;
use assistant_agent_core::adapters::DatabaseHandle;
use assistant_platform_api::{ErrorCode, ImageBlobSink, PlatformError};
use assistant_storage::{
    BlobId, BlobKind, Clock, Database, MIGRATIONS, MigrationSet, StoragePaths,
};

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
    fn new(label: &str) -> Result<Self, std::io::Error> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(std::io::Error::other)?
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "assistant-blob-sink-{label}-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path)?;
        Ok(Self { path })
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn open_database_handle(
    directory: &TestDirectory,
    with_blob_schema: bool,
) -> Result<DatabaseHandle, Box<dyn std::error::Error>> {
    let mut migrations = MigrationSet::new();
    if with_blob_schema {
        migrations.register_all(MIGRATIONS)?;
        migrations.register_all(assistant_audit::MIGRATIONS)?;
    }
    let paths = StoragePaths::new(&directory.path);
    let database = Database::open(&paths, Arc::new(FixedClock), &migrations)?;
    Ok(Arc::new(Mutex::new(database)))
}

fn setup(
    label: &str,
) -> Result<(TestDirectory, DatabaseHandle, StorageBlobSink), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new(label)?;
    let handle = open_database_handle(&directory, true)?;
    let sink = StorageBlobSink::new();
    assert!(sink.attach(&handle), "database attachment must succeed");
    Ok((directory, handle, sink))
}

const fn test_bgra() -> [u8; 8] {
    [1, 2, 3, 4, 5, 6, 7, 8]
}

fn assert_fatal(error: &PlatformError, expected_fragment: &str) {
    assert_eq!(error.code(), ErrorCode::Fatal);
    assert!(
        error.message().contains(expected_fragment),
        "expected `{expected_fragment}` in `{}`",
        error.message()
    );
}

fn store_error(sink: &dyn ImageBlobSink, width: u32, height: u32, bgra: &[u8]) -> PlatformError {
    sink.store_bgra(width, height, "sha256:unused", bgra)
        .expect_err("invalid input must fail closed")
}

#[test]
fn test_store_before_attach_or_on_leaked_sink_fails_closed() {
    let bgra = test_bgra();
    let error = store_error(&StorageBlobSink::new(), 2, 1, &bgra);
    assert_fatal(&error, "no database handle has been attached");
    let error = store_error(StorageBlobSink::new().leak(), 2, 1, &bgra);
    assert_fatal(&error, "no database handle has been attached");
}

#[test]
fn test_store_rejects_bgra_length_mismatch_before_database_lookup() {
    let error = store_error(&StorageBlobSink::new(), 2, 1, &[0, 1, 2]);
    assert_fatal(&error, "needs 8");
}

#[test]
fn test_attach_is_one_shot_and_stores_screenshot_bytes() -> Result<(), Box<dyn std::error::Error>> {
    let (directory, handle, sink) = setup("success")?;
    let second_directory = TestDirectory::new("second")?;
    let second = open_database_handle(&second_directory, true)?;
    assert!(
        !sink.attach(&second),
        "a second attachment must be rejected without replacement"
    );

    let bgra = test_bgra();
    let address = BlobId::of_content(&bgra).as_str().to_owned();
    sink.store_bgra(2, 1, &address, &bgra)?;

    let database = handle.lock().map_err(|_| "database mutex poisoned")?;
    let stored = database
        .blob_store()
        .get(database.connection(), &BlobId::parse(&address)?)?;
    assert_eq!(stored, bgra);
    let kind: String = database.connection().query_row(
        "SELECT kind FROM blobs WHERE blob_id = ?1",
        [address.as_str()],
        |row| row.get(0),
    )?;
    assert_eq!(kind, BlobKind::Screenshot.as_str());
    drop(database);
    drop(directory);
    Ok(())
}

#[test]
fn test_store_rejects_mismatched_content_address() -> Result<(), Box<dyn std::error::Error>> {
    let (_directory, _handle, sink) = setup("mismatch")?;
    let error = sink
        .store_bgra(2, 1, &"f".repeat(71), &test_bgra())
        .expect_err("a mismatched content address must fail closed");
    assert_fatal(&error, "does not match the platform-computed");
    Ok(())
}

#[test]
fn test_store_rejects_poisoned_database_mutex() -> Result<(), Box<dyn std::error::Error>> {
    let (_directory, handle, sink) = setup("poisoned")?;
    let poison_handle = Arc::clone(&handle);
    let _ = std::thread::spawn(move || {
        let _guard = poison_handle.lock().expect("lock before poisoning");
        panic!("poison the database mutex");
    })
    .join();
    let error = store_error(&sink, 2, 1, &test_bgra());
    assert_fatal(&error, "mutex is poisoned");
    Ok(())
}

#[test]
fn test_store_maps_storage_failure_to_platform_error() -> Result<(), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new("storage-failure")?;
    let handle = open_database_handle(&directory, false)?;
    let sink = StorageBlobSink::new();
    assert!(sink.attach(&handle), "database attachment must succeed");
    let bgra = test_bgra();
    let address = BlobId::of_content(&bgra).as_str().to_owned();
    let error = sink
        .store_bgra(2, 1, &address, &bgra)
        .expect_err("a missing blob schema must fail closed");
    assert!(error.message().contains("blob sink:"));
    Ok(())
}

#[test]
fn test_store_fails_after_attached_database_handle_is_dropped()
-> Result<(), Box<dyn std::error::Error>> {
    let (_directory, handle, sink) = setup("expired")?;
    drop(handle);
    let error = store_error(&sink, 2, 1, &test_bgra());
    assert_fatal(&error, "already gone");
    Ok(())
}
