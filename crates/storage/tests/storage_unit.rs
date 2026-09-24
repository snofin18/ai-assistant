//! 纯逻辑单测（**无 IO**）：id 解析 / 取值往返 / 错误码映射 / 目录布局。
//!
//! 为什么单独一个文件：`crates/storage/src/**` 已接近单文件行数上限（ADR-0033），
//! 且这些用例不碰磁盘、不碰 SQLite，与 `storage_integration.rs` 的"真实 IO"用例分开更好定位失败。
//!
//! 测试内允许 `unwrap` / `expect` / `panic`：AGENTS.md §5.3 明确 `tests/` 内可 allow
//! （断言失败必须让测试炸掉，而不是被吞掉）。
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::collections::BTreeSet;
use std::path::PathBuf;

use assistant_protocol::ErrorCategory;
use assistant_storage::{BlobId, BlobKind, BlobOwner, StorageError, StoragePaths};

/// 空输入的 sha256 是公开已知值：用它把"内容寻址 = sha256(原始内容)"钉死。
#[test]
fn test_blob_id_of_content_matches_known_sha256_of_empty_input() {
    let expected = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    let id = BlobId::of_content(b"");
    assert_eq!(id.as_str(), expected);
    // 相对路径 = 前 2 位 / 完整 64 位（256 个子目录，避免单目录塞满文件）
    assert_eq!(id.relative_path(), format!("e3/{expected}"));
    assert_eq!(id.to_string(), expected);
}

/// `BlobId::parse` 只接受 64 位**小写** hex —— 大小写混用会让"同内容两个 id"。
#[test]
fn test_blob_id_parse_rejects_malformed_values() {
    let valid = "0".repeat(64);
    assert_eq!(BlobId::parse(&valid).expect("合法 id").as_str(), valid);

    let malformed = vec![
        String::new(),
        "0".to_owned(),
        "0".repeat(63),
        "0".repeat(65),
        "A".repeat(64), // 大写必须拒绝：否则同内容会有两个 id
        "g".repeat(64), // 非 hex 字符
        format!("{}-", "0".repeat(63)),
    ];
    for value in &malformed {
        let error = BlobId::parse(value).expect_err("非法 id 必须报错，不得静默截断/转小写");
        assert_eq!(error.reason_code(), "invalid_blob_id");
    }
}

/// `BlobKind` 的字符串形式是**进 DB 的稳定契约**：往返必须无损，未知取值不得静默降级。
#[test]
fn test_blob_kind_roundtrip_and_unknown_rejected() {
    for kind in [
        BlobKind::TreeSnapshot,
        BlobKind::Screenshot,
        BlobKind::Other,
    ] {
        assert_eq!(BlobKind::parse(kind.as_str()).expect("往返"), kind);
    }
    let error = BlobKind::parse("not_a_kind").expect_err("未知 kind 必须报错");
    assert_eq!(error.reason_code(), "invalid_argument");
}

/// owner 的两段都非空：空串会让"按 owner 清理"变成一个匹配所有人的通配符。
#[test]
fn test_blob_owner_rejects_empty_parts() {
    let owner = BlobOwner::new("step", "s_1").expect("合法 owner");
    assert_eq!(owner.kind(), "step");
    assert_eq!(owner.id(), "s_1");

    assert_eq!(
        BlobOwner::new("", "s_1")
            .expect_err("kind 空")
            .reason_code(),
        "invalid_argument"
    );
    assert_eq!(
        BlobOwner::new("step", "").expect_err("id 空").reason_code(),
        "invalid_argument"
    );
}

/// 不变量 1：`reason_code` 稳定且**互不重复**（它进日志与审计，重名会让排障失去分辨力）。
/// 不变量 2：除"忙/被锁"外一律 `Fatal`。
#[test]
fn test_error_reason_codes_are_unique_and_categories_are_stable() {
    // 唯一的 Transient：数据库忙 / 被锁（单写者设计下正常不应出现，但可退避重试）
    let busy = StorageError::Sqlite(rusqlite::Error::SqliteFailure(
        rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_BUSY),
        None,
    ));
    assert_eq!(busy.reason_code(), "sqlite");
    assert_eq!(busy.error_category(), ErrorCategory::Transient);
    assert!(!busy.requires_human(), "忙/被锁应可由调用方退避重试");

    // 同一个 reason_code 的另一个实例：非"忙/被锁"的 SQLite 错误必须升级为 Fatal
    let sqlite_fatal = StorageError::Sqlite(rusqlite::Error::InvalidQuery);
    assert_eq!(sqlite_fatal.reason_code(), "sqlite");
    assert_eq!(sqlite_fatal.error_category(), ErrorCategory::Fatal);
    assert!(sqlite_fatal.requires_human());

    let fatal_samples = vec![
        StorageError::SchemaVersionMismatch {
            expected: 1,
            found: None,
            detail: "外来库".to_owned(),
        },
        StorageError::MigrationChecksumMismatch {
            version: 1,
            recorded: "a".repeat(64),
            actual: "b".repeat(64),
        },
        StorageError::MigrationFailed {
            version: 1,
            detail: "syntax error".to_owned(),
        },
        StorageError::Io {
            path: PathBuf::from("x"),
            source: std::io::Error::other("permission denied"),
        },
        StorageError::EvidenceMissing {
            blob_id: "b".repeat(64),
            path: PathBuf::from("x"),
        },
        StorageError::EvidenceCorrupt {
            blob_id: "b".repeat(64),
            detail: "sha256 不符".to_owned(),
        },
        StorageError::BlobUnknown {
            blob_id: "b".repeat(64),
        },
        StorageError::InvalidArgument {
            field: "ttl_ms",
            detail: "必须 >= 0".to_owned(),
        },
        StorageError::InvalidBlobId {
            value: "zz".to_owned(),
        },
        StorageError::Compression {
            detail: "zstd".to_owned(),
        },
    ];

    let mut seen = BTreeSet::new();
    assert!(seen.insert(busy.reason_code()), "`sqlite` 只该出现一次");
    for error in &fatal_samples {
        assert!(
            seen.insert(error.reason_code()),
            "reason_code 重复：{}",
            error.reason_code()
        );
        assert_eq!(error.error_category(), ErrorCategory::Fatal);
        assert!(error.requires_human());
        // Display 必须非空：错误最终要能进日志给人看
        assert!(!error.to_string().is_empty());
    }
}

/// 目录布局是**对外承诺**（数据目录一旦发布就不能悄悄换名字）。
#[test]
fn test_storage_paths_layout_is_stable() {
    let paths = StoragePaths::new("D:/data/assistant");
    assert!(paths.database_file().ends_with("assistant.db"));
    assert!(paths.blob_root().ends_with("blobs"));
    assert!(paths.shadow_root().ends_with("shadow"));
    assert_eq!(paths.root(), std::path::Path::new("D:/data/assistant"));
}
