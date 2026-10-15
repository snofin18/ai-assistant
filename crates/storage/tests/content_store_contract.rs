//! Public contract coverage for blob-store failure and boundary paths.

#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unwrap_used
)]

mod common;

use std::sync::Arc;

use assistant_storage::{
    BlobId, BlobKind, BlobOwner, BlobStore, Clock, Database, IntegrityIssueKind, StorageError,
    StoragePaths,
};
use common::{FixedClock, TestDir, count_rows, open_database};
use rusqlite::Connection;

fn store(label: &str) -> (TestDir, Arc<FixedClock>, Database, BlobStore) {
    let directory = TestDir::new(label);
    let clock = Arc::new(FixedClock::new(1_700_000_000_000));
    let database = open_database(&directory, &clock);
    let blobs = database.blob_store();
    (directory, clock, database, blobs)
}

fn put(blobs: &BlobStore, connection: &Connection, bytes: &[u8]) -> BlobId {
    blobs
        .put(connection, BlobKind::Other, bytes)
        .expect("blob write must succeed")
}

#[test]
fn test_blob_store_surfaces_directory_collision_and_missing_query_table() {
    let (directory, clock, database, blobs) = store("content-directory");
    let layout = BlobId::of_content(b"layout payload");
    assert_eq!(blobs.root(), directory.paths().blob_root());
    assert_eq!(
        blobs.path_for(&layout),
        directory.paths().blob_root().join(layout.relative_path())
    );
    let rendered = format!("{blobs:?}");
    assert!(rendered.contains("BlobStore"));
    assert!(rendered.contains("root"));
    assert!(rendered.contains("blobs"));
    assert_eq!(clock.now_unix_ms(), 1_700_000_000_000);

    let connection = database.connection();
    let collision = put(&blobs, connection, b"directory collision");
    let collision_path = blobs.path_for(&collision);
    std::fs::remove_file(&collision_path).expect("remove blob file before replacing it");
    std::fs::create_dir_all(&collision_path).expect("replace blob file with a directory");

    let error = blobs
        .collect_garbage(connection, 0)
        .expect_err("remove_file on a directory must fail explicitly");
    assert!(matches!(error, StorageError::Io { .. }));
    assert_eq!(count_rows(connection, "blobs"), 1);

    let (_directory, _clock, database, blobs) = store("content-count-query");
    let connection = database.connection();
    let id = put(&blobs, connection, b"reference count failure");
    connection
        .execute_batch("DROP TABLE blob_refs")
        .expect("drop reference table for the failure contract");
    let error = blobs
        .reference_count(connection, &id)
        .expect_err("missing reference table must surface SQLite failure");
    assert!(matches!(error, StorageError::Sqlite { .. }));
}

#[test]
fn test_verify_integrity_deep_same_length_forgery_reports_content_mismatch() {
    let (_directory, _clock, database, blobs) = store("content-forgery");
    let connection = database.connection();
    let id = put(&blobs, connection, b"same length target payload");
    let path = blobs.path_for(&id);
    let original_length = std::fs::read(&path).expect("read original blob").len();
    let mut forged = zstd::bulk::compress(&[b'x'; 8192], 3).expect("build a valid zstd frame");
    forged.resize(original_length, 0);
    std::fs::write(&path, forged).expect("write same-length forged payload");

    let issues = blobs
        .verify_integrity(connection, true)
        .expect("deep integrity scan must return an issue, not fail");
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].blob_id, id);
    assert!(matches!(
        issues[0].kind,
        IntegrityIssueKind::ContentMismatch(_)
    ));
}

#[cfg(windows)]
#[test]
fn test_verify_integrity_deep_surfaces_read_lock_io_failure() {
    use std::os::windows::fs::OpenOptionsExt;

    let (_directory, _clock, database, blobs) = store("content-read-lock");
    let connection = database.connection();
    let id = put(&blobs, connection, b"exclusive read lock");
    let _lock = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(blobs.path_for(&id))
        .expect("lock the blob against concurrent reads");

    let error = blobs
        .verify_integrity(connection, true)
        .expect_err("deep scan must surface a read-lock IO failure");
    assert!(matches!(error, StorageError::Io { .. }));
}

#[test]
fn test_write_atomically_reports_parent_temporary_and_rename_failures() {
    let (directory, clock, database, blobs) = store("content-write-errors");
    let connection = database.connection();

    let blocked_root = directory.paths().root().join("blocked-root");
    std::fs::write(&blocked_root, b"not a directory")
        .expect("create a file where a root is needed");
    let blocked = BlobStore::new(&StoragePaths::new(&blocked_root), clock);
    let error = blocked
        .put(connection, BlobKind::Other, b"parent failure")
        .expect_err("creating a blob root below a file must fail");
    assert!(matches!(error, StorageError::Io { .. }));

    let temporary = BlobId::of_content(b"temporary collision");
    let temporary_path = blobs.path_for(&temporary);
    std::fs::create_dir_all(temporary_path.parent().expect("blob path has a parent"))
        .expect("create blob parent directory");
    std::fs::create_dir_all(temporary_path.with_extension("tmp")).expect("reserve temporary path");
    let error = blobs
        .put(connection, BlobKind::Other, b"temporary collision")
        .expect_err("writing the temporary file over a directory must fail");
    assert!(matches!(error, StorageError::Io { .. }));

    let rename = BlobId::of_content(b"rename collision");
    let rename_path = blobs.path_for(&rename);
    std::fs::create_dir_all(&rename_path).expect("reserve final blob path as directory");
    let error = blobs
        .put(connection, BlobKind::Other, b"rename collision")
        .expect_err("renaming a file over a directory must fail");
    assert!(matches!(error, StorageError::Io { .. }));
}

#[test]
fn test_blob_store_surfaces_sqlite_insert_failures_for_blobs_and_references() {
    let (_directory, _clock, database, blobs) = store("content-sqlite-insert");
    let connection = database.connection();
    connection
        .execute_batch(
            "CREATE TRIGGER reject_blob_insert
             BEFORE INSERT ON blobs BEGIN
                 SELECT RAISE(ABORT, 'reject blob insert');
             END;",
        )
        .expect("install blob insert rejection");
    let error = blobs
        .put(connection, BlobKind::Other, b"insert failure")
        .expect_err("blob insert must surface SQLite failure");
    assert!(matches!(error, StorageError::Sqlite { .. }));

    connection
        .execute_batch("DROP TRIGGER reject_blob_insert")
        .expect("remove blob insert rejection");
    let target = put(&blobs, connection, b"reference target");
    connection
        .execute_batch(
            "CREATE TRIGGER reject_blob_reference_insert
             BEFORE INSERT ON blob_refs BEGIN
                 SELECT RAISE(ABORT, 'reject reference insert');
             END;",
        )
        .expect("install reference insert rejection");
    let owner = BlobOwner::new("step", "s_insert").expect("owner");
    let error = blobs
        .add_reference(connection, &target, &owner)
        .expect_err("reference insert must surface SQLite failure");
    assert!(matches!(error, StorageError::Sqlite { .. }));
    assert_eq!(
        blobs.reference_count(connection, &target).expect("count"),
        0
    );
}

#[test]
fn test_blob_store_surfaces_sqlite_delete_failures_for_references_and_gc() {
    let (_directory, _clock, database, blobs) = store("content-sqlite-delete");
    let connection = database.connection();
    let target = put(&blobs, connection, b"reference delete");
    let owner = BlobOwner::new("step", "s_delete").expect("owner");
    assert!(
        blobs
            .add_reference(connection, &target, &owner)
            .expect("reference")
    );
    connection
        .execute_batch(
            "CREATE TRIGGER reject_blob_reference_delete
             BEFORE DELETE ON blob_refs BEGIN
                 SELECT RAISE(ABORT, 'reject reference delete');
             END;",
        )
        .expect("install reference delete rejection");
    let error = blobs
        .remove_reference(connection, &target, &owner)
        .expect_err("reference delete must surface SQLite failure");
    assert!(matches!(error, StorageError::Sqlite { .. }));

    connection
        .execute_batch("DROP TRIGGER reject_blob_reference_delete")
        .expect("remove reference delete rejection");
    assert!(
        blobs
            .remove_reference(connection, &target, &owner)
            .expect("remove reference")
    );
    connection
        .execute_batch(
            "CREATE TRIGGER reject_blob_delete
             BEFORE DELETE ON blobs BEGIN
                 SELECT RAISE(ABORT, 'reject blob delete');
             END;",
        )
        .expect("install blob delete rejection");
    let error = blobs
        .collect_garbage(connection, 0)
        .expect_err("GC delete failure must not be reported as success");
    assert!(matches!(error, StorageError::Sqlite { .. }));
    assert_eq!(count_rows(connection, "blobs"), 1);
}
