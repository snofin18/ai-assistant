//! 纯逻辑单测（**无 IO**）：规范化 / hash / ring buffer / durability / 错误映射。
//!
//! 为什么单独一个文件：这些用例不碰磁盘、不碰 SQLite，与 `audit_integration.rs` 的"真实 IO"
//! 用例分开更好定位失败。
//!
//! 测试内允许 `expect` / `unwrap` / `panic`（AGENTS.md §5.3：`tests/` 内可 allow；
//! 断言失败必须让测试炸掉，而不是被吞掉）。
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

mod common;

use assistant_audit::{
    AuditError, DEFAULT_MAX_EVENTS, DEFAULT_MAX_INTERVAL_MS, Durability, PushOutcome, RingBuffer,
    TamperKind, canonical_payload, compute_self_hash,
};
use assistant_protocol::ErrorCategory;

/// 规范化必须**抹掉 `self_hash`**（否则 hash 会自指，永远算不出来）且可重复。
#[test]
fn test_canonical_payload_clears_self_hash_and_is_deterministic() {
    let mut event = common::sample_event_with_action("notepad.read_text");
    event.self_hash = "f".repeat(64);

    let first = canonical_payload(&event).expect("规范化");
    let second = canonical_payload(&event).expect("规范化");
    assert_eq!(first, second, "同一事件必须得到同一规范化串");
    assert!(
        first.contains("\"self_hash\":\"\""),
        "self_hash 必须被置空：{first}"
    );
    assert!(
        !first.contains(&"f".repeat(64)),
        "规范化串里不得残留原 self_hash"
    );
}

/// `self_hash`：稳定、64 位小写 hex、对"内容"与"链指针"都敏感。
#[test]
fn test_compute_self_hash_is_stable_and_input_sensitive() {
    let read = common::sample_event_with_action("notepad.read_text");
    let write = common::sample_event_with_action("notepad.write_text");
    let canonical_read = canonical_payload(&read).expect("规范化");
    let canonical_write = canonical_payload(&write).expect("规范化");

    let hash_read = compute_self_hash("", &canonical_read);
    assert_eq!(
        hash_read,
        compute_self_hash("", &canonical_read),
        "必须稳定"
    );
    assert_eq!(hash_read.len(), 64, "sha256 小写 hex = 64 字符");
    assert!(
        hash_read
            .chars()
            .all(|ch| ch.is_ascii_hexdigit() && !ch.is_ascii_uppercase()),
        "必须是小写 hex：{hash_read}"
    );

    assert_ne!(
        hash_read,
        compute_self_hash("", &canonical_write),
        "改内容必须改 hash"
    );
    assert_ne!(
        hash_read,
        compute_self_hash("abc", &canonical_read),
        "改链指针必须改 hash"
    );
}

/// ring buffer 到容量即报 `Full`（调用方据此 flush，绝不静默丢）。
#[test]
fn test_ring_buffer_reports_full_at_capacity() {
    let mut buffer = RingBuffer::with_capacity(3);
    assert_eq!(buffer.capacity(), 3);
    assert!(buffer.is_empty());

    assert_eq!(buffer.push(1_u8), PushOutcome::Buffered);
    assert_eq!(buffer.push(2_u8), PushOutcome::Buffered);
    assert_eq!(buffer.push(3_u8), PushOutcome::Full);
    assert_eq!(buffer.len(), 3);
    assert_eq!(buffer.last(), Some(&3_u8));
    assert_eq!(buffer.iter().copied().collect::<Vec<u8>>(), vec![1, 2, 3]);

    buffer.clear();
    assert!(buffer.is_empty());
}

/// 容量 0 抬到 1：容量 0 等于"每条都立刻 flush"，那是 `Immediate` 的语义。
#[test]
fn test_ring_buffer_capacity_zero_is_clamped_to_one() {
    let mut buffer: RingBuffer<u8> = RingBuffer::with_capacity(0);
    assert_eq!(buffer.capacity(), 1);
    assert_eq!(buffer.push(7_u8), PushOutcome::Full);
    assert_eq!(buffer.len(), 1);
}

/// 默认档位必须与 `docs/storage-design.md` §3.3 的「200 ms 或 100 条」逐字一致。
#[test]
fn test_durability_default_is_batched_100_events_200_ms() {
    assert_eq!(DEFAULT_MAX_EVENTS, 100);
    assert_eq!(DEFAULT_MAX_INTERVAL_MS, 200);
    assert_eq!(
        Durability::default(),
        Durability::Batched {
            max_events: 100,
            max_interval_ms: 200
        }
    );
    assert_eq!(Durability::default(), Durability::batched_default());
    assert!(Durability::default().buffers_events());
    assert!(!Durability::Immediate.buffers_events());
}

/// 档位名进日志与审计，**不得随版本改名**。
#[test]
fn test_durability_mode_names_are_stable() {
    assert_eq!(Durability::batched_default().mode_name(), "batched");
    assert_eq!(Durability::Immediate.mode_name(), "immediate");
    assert_eq!(Durability::SeparateDbFull.mode_name(), "separate_db_full");
}

/// 断链种类名进日志与审计，**不得随版本改名**。
#[test]
fn test_tamper_kind_names_are_stable() {
    assert_eq!(TamperKind::HashMismatch.name(), "hash_mismatch");
    assert_eq!(TamperKind::OrphanedRecord.name(), "orphaned_record");
    assert_eq!(TamperKind::UnreadablePayload.name(), "unreadable_payload");
}

/// 错误码与分类：除"库忙 / 被锁"外一律 `Fatal`（审计问题要人工介入，不许自动重试掩盖）。
#[test]
fn test_audit_error_reason_codes_and_categories() {
    let invalid = AuditError::InvalidArgument {
        field: "max_events",
        detail: "0".to_owned(),
    };
    assert_eq!(invalid.reason_code(), "invalid_argument");
    assert_eq!(invalid.error_category(), ErrorCategory::Fatal);
    assert!(invalid.requires_human());

    let unsupported = AuditError::UnsupportedDurability {
        mode: "separate_db_full",
        detail: "未落地".to_owned(),
    };
    assert_eq!(unsupported.reason_code(), "unsupported_durability");
    assert_eq!(unsupported.error_category(), ErrorCategory::Fatal);

    let broken = AuditError::ChainBroken {
        detail: "断裂".to_owned(),
    };
    assert_eq!(broken.reason_code(), "chain_broken");
    assert_eq!(broken.error_category(), ErrorCategory::Fatal);

    // SQLITE_BUSY = 5 → 唯一可自动重试的情形
    let busy = AuditError::Sqlite(rusqlite::Error::SqliteFailure(
        rusqlite::ffi::Error::new(5),
        Some("database is locked".to_owned()),
    ));
    assert_eq!(busy.reason_code(), "sqlite");
    assert_eq!(busy.error_category(), ErrorCategory::Transient);
    assert!(!busy.requires_human());
}
