//! 访问审计契约：每次访问都产生记录 / 记录不含明文 / 审计失败即拒绝（fail-closed）。
//!
//! 全部用 `InMemorySecretStore` —— **不碰**真实 OS 密钥库（CI 上没有）。
//!
//! 测试内允许 `expect` / `unwrap` / `panic`（AGENTS.md §5.3：`tests/` 内可 allow）。
//! **不**允许 `indexing_slicing`：取第 n 条记录一律走 `record_at`（TASK-052 的既定做法）。
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

mod common;

use std::sync::Arc;

use assistant_secrets::{
    AuditedSecretStore, InMemorySecretStore, SecretAccessOperation, SecretAccessOutcome,
    SecretError, SecretName, SecretStore, SecretValue,
};

use crate::common::{FailingAuditor, FixedClock, RecordingAuditor};

const CANARY: &str = "sk-CANARY-do-not-leak-9f3a1c";
const FIXED_TS: i64 = 1_756_000_000_000;

/// 测试夹具：内存后端 + 记录型审计出口（句柄可共享）+ 停在固定时刻的时钟。
fn fixture() -> (
    AuditedSecretStore<InMemorySecretStore, RecordingAuditor>,
    RecordingAuditor,
) {
    let auditor = RecordingAuditor::new();
    let store = AuditedSecretStore::new(
        InMemorySecretStore::new(),
        auditor.clone(),
        Arc::new(FixedClock::new(FIXED_TS)),
    );
    (store, auditor)
}

/// 取第 `index` 条记录。
///
/// 为什么不用 `records[index]`：workspace `[lints.clippy] indexing_slicing = "deny"` 对所有
/// 目标生效；越界时这里报的是"该索引的记录应存在"，比 panic 的越界信息更贴题。
fn record_at(auditor: &RecordingAuditor, index: usize) -> assistant_secrets::SecretAccessRecord {
    auditor
        .records()
        .into_iter()
        .nth(index)
        .expect("该索引的记录应存在")
}

#[test]
fn test_audited_get_existing_records_read_allowed() {
    let (store, auditor) = fixture();
    let name = SecretName::new("model.openai.api_key").expect("名字");
    store
        .set(&name, &SecretValue::new(CANARY).expect("值"))
        .expect("写");

    let value = store.get(&name).expect("读").expect("应存在");
    assert_eq!(value.expose(), CANARY);

    assert_eq!(auditor.count(), 2, "写 + 读 = 两条记录");
    let write = record_at(&auditor, 0);
    assert_eq!(write.operation(), SecretAccessOperation::Write);
    assert_eq!(write.outcome(), SecretAccessOutcome::Allowed);
    let read = record_at(&auditor, 1);
    assert_eq!(read.operation(), SecretAccessOperation::Read);
    assert_eq!(read.outcome(), SecretAccessOutcome::Allowed);
    assert_eq!(read.name().as_str(), "model.openai.api_key");
}

#[test]
fn test_audited_get_missing_records_not_found() {
    let (store, auditor) = fixture();
    let name = SecretName::new("absent").expect("名字");

    assert!(store.get(&name).expect("读").is_none());

    assert_eq!(auditor.count(), 1);
    let only = record_at(&auditor, 0);
    assert_eq!(only.operation(), SecretAccessOperation::Read);
    assert_eq!(only.outcome(), SecretAccessOutcome::NotFound);
}

#[test]
fn test_audited_delete_records_delete_operation() {
    let (store, auditor) = fixture();
    let name = SecretName::new("token").expect("名字");
    store
        .set(&name, &SecretValue::new(CANARY).expect("值"))
        .expect("写");

    assert!(store.delete(&name).expect("删"));
    assert!(!store.delete(&name).expect("再删"));

    assert_eq!(auditor.count(), 3);
    let first_delete = record_at(&auditor, 1);
    assert_eq!(first_delete.operation(), SecretAccessOperation::Delete);
    assert_eq!(first_delete.outcome(), SecretAccessOutcome::Allowed);
    let second_delete = record_at(&auditor, 2);
    assert_eq!(second_delete.operation(), SecretAccessOperation::Delete);
    assert_eq!(second_delete.outcome(), SecretAccessOutcome::NotFound);
}

#[test]
fn test_audited_contains_records_read_operation() {
    let (store, auditor) = fixture();
    let name = SecretName::new("token").expect("名字");

    assert!(!store.contains(&name).expect("探测"));

    assert_eq!(auditor.count(), 1);
    // 存在性探测同样泄露"某个 key 有没有配"，故按 read 记
    let only = record_at(&auditor, 0);
    assert_eq!(only.operation(), SecretAccessOperation::Read);
    assert_eq!(only.outcome(), SecretAccessOutcome::NotFound);
}

#[test]
fn test_audited_record_uses_injected_clock_timestamp() {
    let auditor = RecordingAuditor::new();
    let store = AuditedSecretStore::new(
        InMemorySecretStore::new(),
        auditor.clone(),
        Arc::new(FixedClock::new(1_756_123_456_789)),
    );
    let name = SecretName::new("token").expect("名字");
    store.get(&name).expect("读");

    assert_eq!(record_at(&auditor, 0).ts_unix_ms(), 1_756_123_456_789);
}

#[test]
fn test_audited_records_never_contain_plaintext() {
    let (store, auditor) = fixture();
    let name = SecretName::new("model.openai.api_key").expect("名字");
    store
        .set(&name, &SecretValue::new(CANARY).expect("值"))
        .expect("写");
    store.get(&name).expect("读");
    store.delete(&name).expect("删");

    assert_eq!(auditor.count(), 3);
    for record in auditor.records() {
        let rendered = format!("{record:?}");
        assert!(!rendered.contains(CANARY), "记录泄露了明文：{rendered}");
        let displayed = record.to_string();
        assert!(!displayed.contains(CANARY), "记录泄露了明文：{displayed}");
    }
}

#[test]
fn test_audited_get_with_failing_auditor_is_rejected_and_leaks_nothing() {
    // 先在**未装饰**的后端里放一条值（避免审计出口参与写入）
    let backend = InMemorySecretStore::new();
    let name = SecretName::new("model.openai.api_key").expect("名字");
    backend
        .set(&name, &SecretValue::new(CANARY).expect("值"))
        .expect("写");

    let store =
        AuditedSecretStore::new(backend, FailingAuditor, Arc::new(FixedClock::new(FIXED_TS)));

    let error = store.get(&name).expect_err("审计写不进去 ⇒ 必须拒绝该次读");
    assert_eq!(error.reason_code(), "audit_rejected");
    assert!(matches!(error, SecretError::AuditRejected { .. }));
    // 报错文本里也不能出现明文
    assert!(!error.to_string().contains(CANARY), "错误文本泄露了明文");
}

#[test]
fn test_audited_write_with_failing_auditor_is_rejected() {
    let store = AuditedSecretStore::new(
        InMemorySecretStore::new(),
        FailingAuditor,
        Arc::new(FixedClock::new(FIXED_TS)),
    );
    let name = SecretName::new("token").expect("名字");

    let error = store
        .set(&name, &SecretValue::new(CANARY).expect("值"))
        .expect_err("审计写不进去 ⇒ 写也不视为成功");
    assert_eq!(error.reason_code(), "audit_rejected");
    // 写入本身可能已生效（本 crate 不假装回滚）；重试是安全的（set 幂等），
    // 调用方看到的语义是"这次操作未被完整记录，故不视为成功"。
    assert!(store.get(&name).is_err(), "审计仍失败 ⇒ 读同样被拒");
}

#[test]
fn test_audited_not_found_is_distinct_from_failed_outcome() {
    // 反证：正常后端的"没有这条"记 `not_found`，**不**记 `failed` —— 两者不可混用
    let (store, auditor) = fixture();
    let name = SecretName::new("token").expect("名字");
    store.delete(&name).expect("删不存在的键");

    let only = record_at(&auditor, 0);
    assert_eq!(only.outcome(), SecretAccessOutcome::NotFound);
    assert_ne!(only.outcome(), SecretAccessOutcome::Failed);
}
