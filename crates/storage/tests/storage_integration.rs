//! 存储层集成测试（TASK-012）：**临时目录下的真实 SQLite + 真实 blob 文件**。
//!
//! 为什么必须真跑 IO：本 crate 的契约就是"库 + 文件"的一致性（铁律 1）。用 mock 去测
//! `evidence_missing` / `evidence_corrupt` 只会测出 mock 自己的行为。
//!
//! 任务卡 `DoD` 强制要求的三条负向用例，在本文件里各有专属测试：
//!   1. 坏 `schema_version` → `open()` 拒绝启动（无版本表 / 版本表被改坏）
//!   2. blob 被篡改 1 字节 → `evidence_corrupt`
//!   3. DB 行在而 blob 文件不在 → `evidence_missing`
//!
//! 核心表 CRUD 在 `storage_records.rs`，纯逻辑单测在 `storage_unit.rs`，共享夹具在 `common/`。
//!
//! 测试内允许 `unwrap` / `expect` / `panic`（AGENTS.md §5.3：`tests/` 内可 allow）。
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

mod common;

use std::sync::Arc;

use assistant_storage::{
    BlobId, BlobKind, BlobOwner, Database, IntegrityIssueKind, SCHEMA_VERSION, StorageError,
};
use common::{FixedClock, TestDir, count_files, count_rows, file_size, open_database};
use rusqlite::Connection;
// ───────────────────────── 迁移 / 启动校验 ─────────────────────────

/// 全新库：建目录 → 建表 → 版本 = 二进制期望值。
#[test]
fn test_open_fresh_database_reaches_latest_schema_version() {
    let dir = TestDir::new("fresh");
    let clock = Arc::new(FixedClock::new(1_700_000_000_000));
    let database = open_database(&dir, &clock);

    assert_eq!(database.schema_version().expect("读版本"), SCHEMA_VERSION);
    assert!(
        database.paths().database_file().is_file(),
        "主库文件必须存在"
    );
    assert!(database.paths().blob_root().is_dir(), "blob 池目录必须建好");
    assert!(
        database.paths().shadow_root().is_dir(),
        "影子副本目录必须建好"
    );
    assert_eq!(count_rows(database.connection(), "schema_migrations"), 1);
}

/// 幂等：对已初始化的库重复 `open()` 只做校验，不重复建表、不重复记账。
#[test]
fn test_open_is_idempotent() {
    let dir = TestDir::new("idempotent");
    let clock = Arc::new(FixedClock::new(1_700_000_000_000));

    let first = open_database(&dir, &clock);
    let version = first.schema_version().expect("读版本");
    first.close().expect("关闭");

    let second = open_database(&dir, &clock);
    assert_eq!(second.schema_version().expect("读版本"), version);
    assert_eq!(count_rows(second.connection(), "schema_migrations"), 1);
}

/// 负向用例 ①-a：有业务表却没有版本表（外来库 / 被篡改的库）→ 拒绝启动。
#[test]
fn test_open_rejects_database_without_version_table() {
    let dir = TestDir::new("no-version-table");
    let paths = dir.paths();
    paths.ensure_layout().expect("建目录");

    {
        let connection = Connection::open(paths.database_file()).expect("直接开库");
        connection
            .execute_batch("CREATE TABLE tasks (id TEXT PRIMARY KEY);")
            .expect("造一张业务表");
    }

    let clock = Arc::new(FixedClock::new(1_700_000_000_000));
    let error = Database::open(&paths, clock).expect_err("必须拒绝启动");
    assert_eq!(error.reason_code(), "schema_version_mismatch");
    match error {
        StorageError::SchemaVersionMismatch { found, .. } => assert_eq!(found, None),
        other => panic!("期望 SchemaVersionMismatch，实际 {other:?}"),
    }
}

/// 负向用例 ①-b：版本表里出现本二进制不认识的版本（降级启动 / 被改坏）→ 拒绝启动。
#[test]
fn test_open_rejects_unknown_schema_version() {
    let dir = TestDir::new("future-version");
    let paths = dir.paths();
    let clock = Arc::new(FixedClock::new(1_700_000_000_000));

    let database = open_database(&dir, &clock);
    database
        .connection()
        .execute(
            "UPDATE schema_migrations SET version = ?1",
            rusqlite::params![SCHEMA_VERSION + 1],
        )
        .expect("改坏版本号");
    database.close().expect("关闭");

    let error = Database::open(&paths, clock).expect_err("必须拒绝启动");
    assert_eq!(error.reason_code(), "schema_version_mismatch");
    match error {
        StorageError::SchemaVersionMismatch { found, .. } => {
            assert_eq!(found, Some(SCHEMA_VERSION + 1));
        }
        other => panic!("期望 SchemaVersionMismatch，实际 {other:?}"),
    }
}

/// 负向用例 ①-c：已应用的迁移被事后改动（记账 sha256 不符）→ 拒绝启动。
#[test]
fn test_open_rejects_tampered_migration_checksum() {
    let dir = TestDir::new("checksum");
    let paths = dir.paths();
    let clock = Arc::new(FixedClock::new(1_700_000_000_000));

    let database = open_database(&dir, &clock);
    database
        .connection()
        .execute(
            "UPDATE schema_migrations SET checksum = ?1",
            rusqlite::params!["0".repeat(64)],
        )
        .expect("改坏记账");
    database.close().expect("关闭");

    let error = Database::open(&paths, clock).expect_err("必须拒绝启动");
    assert_eq!(error.reason_code(), "migration_checksum_mismatch");
    match error {
        StorageError::MigrationChecksumMismatch { version, .. } => assert_eq!(version, 1),
        other => panic!("期望 MigrationChecksumMismatch，实际 {other:?}"),
    }
}

// ───────────────────────── blob 池 ─────────────────────────

/// 往返 + 去重：同内容两次写入 → 同一个 id、磁盘上仍只有一份、表里也只有一行。
#[test]
fn test_blob_put_get_roundtrip_and_dedupe() {
    let dir = TestDir::new("roundtrip");
    let clock = Arc::new(FixedClock::new(1_700_000_000_000));
    let database = open_database(&dir, &clock);
    let blobs = database.blob_store();
    let content = "行首 CJK 与 emoji 🚀\n第二行\n".as_bytes();

    let first = blobs
        .put(database.connection(), BlobKind::TreeSnapshot, content)
        .expect("写入");
    assert_eq!(first, BlobId::of_content(content));
    assert_eq!(
        blobs.get(database.connection(), &first).expect("读取"),
        content
    );
    assert!(blobs.path_for(&first).is_file());

    let second = blobs
        .put(database.connection(), BlobKind::TreeSnapshot, content)
        .expect("重复写入");
    assert_eq!(second, first, "同内容必须得到同一个 blob_id");
    assert_eq!(count_files(&dir.paths().blob_root()), 1, "去重后只应有一份");
    assert_eq!(count_rows(database.connection(), "blobs"), 1);
}

/// 负向用例 ②：blob 被篡改 1 字节 → `evidence_corrupt`（绝不返回"大概正确"的数据）。
#[test]
fn test_blob_get_detects_single_byte_tamper() {
    let dir = TestDir::new("tamper");
    let clock = Arc::new(FixedClock::new(1_700_000_000_000));
    let database = open_database(&dir, &clock);
    let blobs = database.blob_store();
    let content = vec![b'a'; 4096];

    let blob_id = blobs
        .put(database.connection(), BlobKind::Screenshot, &content)
        .expect("写入");
    let path = blobs.path_for(&blob_id);

    let mut raw = std::fs::read(&path).expect("读文件");
    let last = raw.last_mut().expect("文件非空");
    *last ^= 0x01;
    std::fs::write(&path, &raw).expect("写回");

    let error = blobs
        .get(database.connection(), &blob_id)
        .expect_err("篡改后必须报错");
    assert_eq!(error.reason_code(), "evidence_corrupt");
}

/// 篡改的第二种形态：文件仍是**合法 zstd 帧**（解压会成功），但内容 sha256 与 id 不符。
#[test]
fn test_blob_get_detects_forged_valid_frame() {
    let dir = TestDir::new("forged");
    let clock = Arc::new(FixedClock::new(1_700_000_000_000));
    let database = open_database(&dir, &clock);
    let blobs = database.blob_store();

    let blob_id = blobs
        .put(database.connection(), BlobKind::Other, b"original payload")
        .expect("写入");
    let forged = zstd::bulk::compress(b"forged payload", 3).expect("构造合法帧");
    std::fs::write(blobs.path_for(&blob_id), &forged).expect("覆盖");

    let error = blobs
        .get(database.connection(), &blob_id)
        .expect_err("sha256 不符必须报错");
    assert_eq!(error.reason_code(), "evidence_corrupt");
    assert!(
        error.to_string().contains("sha256"),
        "错误文本要能指认是校验失败：{error}"
    );
}

/// 负向用例 ③：DB 行在而文件不在 → `evidence_missing`（不得当成"没有这个 blob"）。
#[test]
fn test_blob_missing_file_reports_evidence_missing() {
    let dir = TestDir::new("missing");
    let clock = Arc::new(FixedClock::new(1_700_000_000_000));
    let database = open_database(&dir, &clock);
    let blobs = database.blob_store();
    let connection = database.connection();
    let content = b"will be deleted".to_vec();

    let blob_id = blobs
        .put(connection, BlobKind::Other, &content)
        .expect("写入");
    std::fs::remove_file(blobs.path_for(&blob_id)).expect("删除文件");

    let error = blobs.get(connection, &blob_id).expect_err("读取必须报错");
    assert_eq!(error.reason_code(), "evidence_missing");

    let owner = BlobOwner::new("step", "s_1").expect("owner");
    let error = blobs
        .add_reference(connection, &blob_id, &owner)
        .expect_err("引用一个丢了文件的 blob 必须报错");
    assert_eq!(error.reason_code(), "evidence_missing");

    // 也不许"静默重写"：先让人看到数据丢了
    let error = blobs
        .put(connection, BlobKind::Other, &content)
        .expect_err("行在文件不在时不得静默重写");
    assert_eq!(error.reason_code(), "evidence_missing");

    let issues = blobs.verify_integrity(connection, false).expect("扫描");
    assert_eq!(issues.len(), 1);
    assert_eq!(
        issues.first().expect("issue").kind,
        IntegrityIssueKind::FileMissing
    );
}

/// 孤儿文件（写盘成功但记账失败）：内容对得上就"收养"，对不上就报 `evidence_corrupt`。
#[test]
fn test_blob_put_adopts_orphan_file_or_reports_corruption() {
    let dir = TestDir::new("orphan");
    let clock = Arc::new(FixedClock::new(1_700_000_000_000));
    let database = open_database(&dir, &clock);
    let blobs = database.blob_store();
    let connection = database.connection();
    let content = b"orphan content".to_vec();

    // 手工造出"文件在、行不在"的孤儿，并且内容与 id 匹配
    let blob_id = BlobId::of_content(&content);
    let path = blobs.path_for(&blob_id);
    std::fs::create_dir_all(path.parent().expect("父目录")).expect("建子目录");
    let compressed = zstd::bulk::compress(&content, 3).expect("压缩");
    std::fs::write(&path, &compressed).expect("写孤儿文件");

    let adopted = blobs
        .put(connection, BlobKind::Other, &content)
        .expect("收养孤儿文件");
    assert_eq!(adopted, blob_id);
    assert_eq!(blobs.get(connection, &blob_id).expect("读取"), content);
    assert_eq!(count_files(&dir.paths().blob_root()), 1, "收养不得多写一份");

    // 同一个 id 但文件内容对不上：必须报 evidence_corrupt，不能覆盖掉现场
    let other_id = BlobId::of_content(b"another content");
    let other_path = blobs.path_for(&other_id);
    std::fs::create_dir_all(other_path.parent().expect("父目录")).expect("建子目录");
    let wrong = zstd::bulk::compress(b"totally different", 3).expect("压缩");
    std::fs::write(&other_path, &wrong).expect("写错内容的文件");

    let error = blobs
        .put(connection, BlobKind::Other, b"another content")
        .expect_err("孤儿内容不符必须报错");
    assert_eq!(error.reason_code(), "evidence_corrupt");
}

/// 引用计数：一行一个引用 ⇒ 重复引用/重复解引用天然幂等，不会"多减一次"。
#[test]
fn test_blob_reference_counting_is_idempotent() {
    let dir = TestDir::new("refcount");
    let clock = Arc::new(FixedClock::new(1_000_000));
    let database = open_database(&dir, &clock);
    let blobs = database.blob_store();
    let connection = database.connection();

    let blob_id = blobs
        .put(connection, BlobKind::Other, b"referenced")
        .expect("写入");
    let owner_a = BlobOwner::new("step", "s_a").expect("owner");
    let owner_b = BlobOwner::new("evidence", "e_b").expect("owner");

    assert!(
        blobs
            .add_reference(connection, &blob_id, &owner_a)
            .expect("引用")
    );
    assert!(
        !blobs
            .add_reference(connection, &blob_id, &owner_a)
            .expect("重复引用"),
        "同一 owner 重复引用必须是幂等的 no-op"
    );
    assert_eq!(
        blobs.reference_count(connection, &blob_id).expect("计数"),
        1
    );

    assert!(
        blobs
            .add_reference(connection, &blob_id, &owner_b)
            .expect("引用")
    );
    assert_eq!(
        blobs.reference_count(connection, &blob_id).expect("计数"),
        2
    );

    assert!(
        blobs
            .remove_reference(connection, &blob_id, &owner_a)
            .expect("解引用")
    );
    assert!(
        !blobs
            .remove_reference(connection, &blob_id, &owner_a)
            .expect("重复解引用"),
        "重复解引用不得把计数减成负数"
    );
    assert_eq!(
        blobs.reference_count(connection, &blob_id).expect("计数"),
        1
    );
}

/// GC：只回收"引用数 0 **且** 创建时间早于 TTL"的 blob；行在文件不在时如实报告。
#[test]
fn test_garbage_collection_respects_references_and_ttl() {
    let dir = TestDir::new("gc");
    let clock = Arc::new(FixedClock::new(1_000_000));
    let database = open_database(&dir, &clock);
    let blobs = database.blob_store();
    let connection = database.connection();

    let blob_id = blobs
        .put(connection, BlobKind::Other, b"gc me")
        .expect("写入");
    let path = blobs.path_for(&blob_id);
    let stored_bytes = file_size(&path);
    let owner = BlobOwner::new("step", "s_gc").expect("owner");
    assert!(
        blobs
            .add_reference(connection, &blob_id, &owner)
            .expect("引用")
    );

    // 还有引用：即使 TTL = 0 也不许回收
    let report = blobs.collect_garbage(connection, 0).expect("GC");
    assert!(report.deleted.is_empty(), "有引用的 blob 不得被回收");
    assert!(path.is_file());

    assert!(
        blobs
            .remove_reference(connection, &blob_id, &owner)
            .expect("解引用")
    );
    assert_eq!(
        blobs.reference_count(connection, &blob_id).expect("计数"),
        0
    );

    // 引用数 0，但 TTL 未到：仍不许回收（cutoff = now - ttl < created_at）
    let report = blobs.collect_garbage(connection, 1_000).expect("GC");
    assert!(report.deleted.is_empty(), "TTL 未到不得回收");
    assert!(path.is_file());

    // TTL 到了：文件 + 元数据行一起删，并如实回报回收的压缩后字节数
    clock.advance_to(2_000_000);
    let report = blobs.collect_garbage(connection, 1_000).expect("GC");
    assert_eq!(report.deleted.len(), 1);
    assert_eq!(report.deleted.first(), Some(&blob_id));
    assert_eq!(report.reclaimed_bytes, stored_bytes);
    assert!(report.rows_without_file.is_empty());
    assert!(!path.is_file(), "文件必须删掉");
    assert_eq!(count_rows(connection, "blobs"), 0, "元数据行必须删掉");
    assert_eq!(count_rows(connection, "blob_refs"), 0);

    // 已回收的 blob 再读：是"从未登记"（BlobUnknown），不是"文件丢了"
    let error = blobs.get(connection, &blob_id).expect_err("已回收");
    assert_eq!(error.reason_code(), "blob_unknown");
}

/// GC 的第二种形态：行在、文件早就没了 —— 如实报告并清掉行（不谎报回收字节数）。
#[test]
fn test_garbage_collection_reports_rows_without_file() {
    let dir = TestDir::new("gc-rowless");
    let clock = Arc::new(FixedClock::new(1_000_000));
    let database = open_database(&dir, &clock);
    let blobs = database.blob_store();
    let connection = database.connection();

    let blob_id = blobs
        .put(connection, BlobKind::Other, b"gone")
        .expect("写入");
    std::fs::remove_file(blobs.path_for(&blob_id)).expect("先删文件");

    let report = blobs.collect_garbage(connection, 0).expect("GC");
    assert_eq!(report.rows_without_file.len(), 1);
    assert_eq!(report.rows_without_file.first(), Some(&blob_id));
    assert_eq!(report.deleted.len(), 1);
    assert_eq!(report.deleted.first(), Some(&blob_id));
    assert_eq!(
        report.reclaimed_bytes, 0,
        "文件本就不在，不能谎报回收字节数"
    );
    assert_eq!(count_rows(connection, "blobs"), 0);
}

/// GC 参数校验：负 TTL 是调用方 bug，必须报错而不是当成 0。
#[test]
fn test_garbage_collection_rejects_negative_ttl() {
    let dir = TestDir::new("gc-negative");
    let clock = Arc::new(FixedClock::new(1_000_000));
    let database = open_database(&dir, &clock);

    let error = database
        .blob_store()
        .collect_garbage(database.connection(), -1)
        .expect_err("负 TTL 必须报错");
    assert_eq!(error.reason_code(), "invalid_argument");
}

/// 一致性扫描：深扫能抓"内容与 id 不符"，浅扫能抓"文件被截断"，且都不误报好数据。
#[test]
fn test_verify_integrity_deep_reports_content_mismatch() {
    let dir = TestDir::new("integrity-deep");
    let clock = Arc::new(FixedClock::new(1_000));
    let database = open_database(&dir, &clock);
    let blobs = database.blob_store();
    let connection = database.connection();

    let good = blobs
        .put(connection, BlobKind::Other, b"good payload")
        .expect("写入");
    let tampered = blobs
        .put(connection, BlobKind::Other, &[b'a'; 64])
        .expect("写入");
    let forged = zstd::bulk::compress(&[b'b'; 8192], 3).expect("构造合法帧");
    std::fs::write(blobs.path_for(&tampered), &forged).expect("覆盖");

    let issues = blobs.verify_integrity(connection, true).expect("深扫");
    assert_eq!(issues.len(), 1, "只有被篡改的那一个该报");
    let issue = issues.first().expect("issue");
    assert_eq!(issue.blob_id, tampered);
    assert!(matches!(
        &issue.kind,
        IntegrityIssueKind::ContentMismatch(_)
    ));
    assert!(!issues.iter().any(|item| item.blob_id == good));
}

/// 浅扫的廉价检查：文件长度与记账的压缩后长度不符（截断 / 半写）→ 立刻暴露。
#[test]
fn test_verify_integrity_shallow_detects_truncated_file() {
    let dir = TestDir::new("integrity-shallow");
    let clock = Arc::new(FixedClock::new(1_000));
    let database = open_database(&dir, &clock);
    let blobs = database.blob_store();
    let connection = database.connection();

    let blob_id = blobs
        .put(connection, BlobKind::Other, &[b'c'; 4096])
        .expect("写入");
    let path = blobs.path_for(&blob_id);
    let mut raw = std::fs::read(&path).expect("读文件");
    raw.truncate(raw.len() / 2);
    std::fs::write(&path, &raw).expect("截断");

    let issues = blobs.verify_integrity(connection, false).expect("浅扫");
    assert_eq!(issues.len(), 1);
    assert!(matches!(
        &issues.first().expect("issue").kind,
        IntegrityIssueKind::ContentMismatch(_)
    ));

    let issues = blobs.verify_integrity(connection, true).expect("深扫");
    assert_eq!(issues.len(), 1, "截断的文件在深扫里也必须报");
}

/// 没有问题的库：深扫与浅扫都必须返回空报告（否则"报警疲劳"会让真问题被忽略）。
#[test]
fn test_verify_integrity_is_clean_for_healthy_store() {
    let dir = TestDir::new("integrity-clean");
    let clock = Arc::new(FixedClock::new(1_000));
    let database = open_database(&dir, &clock);
    let blobs = database.blob_store();
    let connection = database.connection();

    blobs
        .put(connection, BlobKind::Other, b"one")
        .expect("写入");
    blobs
        .put(connection, BlobKind::Other, b"two")
        .expect("写入");

    assert!(
        blobs
            .verify_integrity(connection, false)
            .expect("浅扫")
            .is_empty()
    );
    assert!(
        blobs
            .verify_integrity(connection, true)
            .expect("深扫")
            .is_empty()
    );
}

/// 读一个从未登记过的 id：是 `blob_unknown`（调用方 bug），不是"文件丢了"。
#[test]
fn test_blob_get_unknown_id_reports_blob_unknown() {
    let dir = TestDir::new("unknown");
    let clock = Arc::new(FixedClock::new(1_000));
    let database = open_database(&dir, &clock);

    let unknown = BlobId::of_content(b"never stored");
    let error = database
        .blob_store()
        .get(database.connection(), &unknown)
        .expect_err("未登记的 id 必须报错");
    assert_eq!(error.reason_code(), "blob_unknown");
}
