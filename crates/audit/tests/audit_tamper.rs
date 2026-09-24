//! 真库集成测试（二）：append-only 护栏 + hash chain 篡改检出。
//!
//! 为什么单独一份：这些用例都在"**摘掉护栏、直接改库**"的前提下做反证 —— 它们是本卡最核心
//! 的安全断言（"篡改可被检出"），单独一份便于定位失败。
//!
//! 注意：用例里 `DROP TRIGGER` 是**刻意的**——它模拟"攻击者拿到了数据库文件"。
//! 真实部署里触发器是第一道锁；这里要证明的是**第二道锁（hash chain）独立有效**。
//!
//! 测试内允许 `expect` / `unwrap` / `panic`（AGENTS.md §5.3）。
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

mod common;

use std::sync::Arc;

use assistant_audit::{AuditError, AuditSubject, TamperKind};
use common::{
    FixedClock, TestDir, all_records, audit_row_count, drop_trigger, immediate_log, open_database,
    sample_event,
};

/// 数据库侧护栏：UPDATE / DELETE 必须被触发器拒绝。
#[test]
fn test_append_only_triggers_reject_update_and_delete() {
    let clock = Arc::new(FixedClock::new(1_700_000_000_000));
    let dir = TestDir::new("append-only");
    let database = open_database(&dir, &clock);
    let mut log = immediate_log(database.connection(), &clock);

    log.append(
        &sample_event("s_ro", "notepad.read_text", "2026-09-24T00:00:00Z"),
        &AuditSubject::unattached(),
    )
    .expect("append");

    let connection = database.connection();
    let id = all_records(&log).first().expect("至少一行").id.clone();

    let update = connection.execute(
        "UPDATE audit_logs SET detail_json = '{}' WHERE id = ?1",
        rusqlite::params![id],
    );
    let update_error = update.expect_err("UPDATE 必须被触发器拒绝");
    assert!(
        update_error.to_string().contains("append-only"),
        "错误信息应说明 append-only：{update_error}"
    );

    let delete = connection.execute("DELETE FROM audit_logs", []);
    let delete_error = delete.expect_err("DELETE 必须被触发器拒绝");
    assert!(
        delete_error.to_string().contains("append-only"),
        "错误信息应说明 append-only：{delete_error}"
    );

    assert_eq!(audit_row_count(connection), 1, "那一行还在");
}

/// 改内容（且改成非法 JSON）必须被检出 —— 这一行连解析都过不去，仍要报 `UnreadablePayload`
/// 而不是让整次校验以"反序列化失败"告终。
#[test]
fn test_verify_chain_detects_content_tamper_even_when_payload_is_unreadable() {
    let clock = Arc::new(FixedClock::new(1_700_000_000_000));
    let dir = TestDir::new("tamper-content");
    let database = open_database(&dir, &clock);
    let mut log = immediate_log(database.connection(), &clock);

    log.append(
        &sample_event("s_t1", "notepad.read_text", "2026-09-24T00:00:00Z"),
        &AuditSubject::unattached(),
    )
    .expect("append");
    log.append(
        &sample_event("s_t2", "notepad.write_text", "2026-09-24T00:00:01Z"),
        &AuditSubject::unattached(),
    )
    .expect("append");
    assert!(log.verify_chain().expect("verify").is_intact());

    let connection = database.connection();
    let id = all_records(&log).first().expect("至少一行").id.clone();

    // 模拟"绕过 crate 直接改库"的攻击者：先摘掉护栏，再改内容
    drop_trigger(connection, "audit_logs_no_update");
    connection
        .execute(
            "UPDATE audit_logs SET detail_json = ?1 WHERE id = ?2",
            rusqlite::params!["{\"version\":\"1.0\"}", id],
        )
        .expect("改内容");

    let verification = log.verify_chain().expect("verify 仍应能给出结论");
    assert!(!verification.is_intact(), "篡改必须被检出");
    assert!(
        verification
            .findings
            .iter()
            .any(|finding| finding.kind == TamperKind::UnreadablePayload),
        "应报 unreadable_payload：{:?}",
        verification.findings
    );
}

/// 改合法 JSON 里的字段（内容被改但结构仍可解析）必须被检出为 `HashMismatch`。
#[test]
fn test_verify_chain_detects_wellformed_content_tamper() {
    let clock = Arc::new(FixedClock::new(1_700_000_000_000));
    let dir = TestDir::new("tamper-wellformed");
    let database = open_database(&dir, &clock);
    let mut log = immediate_log(database.connection(), &clock);

    log.append(
        &sample_event("s_w1", "notepad.read_text", "2026-09-24T00:00:00Z"),
        &AuditSubject::unattached(),
    )
    .expect("append");

    let connection = database.connection();
    let record = all_records(&log).first().expect("至少一行").clone();
    let tampered = serde_json::to_string(&record.event)
        .expect("序列化")
        .replace("notepad.read_text", "notepad.write_text");

    drop_trigger(connection, "audit_logs_no_update");
    connection
        .execute(
            "UPDATE audit_logs SET detail_json = ?1 WHERE id = ?2",
            rusqlite::params![tampered, record.id],
        )
        .expect("改内容");

    let verification = log.verify_chain().expect("verify");
    assert!(!verification.is_intact());
    assert!(
        verification
            .findings
            .iter()
            .any(|finding| finding.kind == TamperKind::HashMismatch),
        "应报 hash_mismatch：{:?}",
        verification.findings
    );
}

/// 改 `prev_hash` 必须被检出（`OrphanedRecord`：从创世走不到这一行）。
#[test]
fn test_verify_chain_detects_prev_hash_tamper() {
    let clock = Arc::new(FixedClock::new(1_700_000_000_000));
    let dir = TestDir::new("tamper-prev");
    let database = open_database(&dir, &clock);
    let mut log = immediate_log(database.connection(), &clock);

    for index in 0..3_u8 {
        log.append(
            &sample_event(
                &format!("s_p{index}"),
                "notepad.read_text",
                "2026-09-24T00:00:00Z",
            ),
            &AuditSubject::unattached(),
        )
        .expect("append");
    }

    let connection = database.connection();
    let target = all_records(&log).last().expect("至少一行").id.clone();
    drop_trigger(connection, "audit_logs_no_update");
    connection
        .execute(
            "UPDATE audit_logs SET prev_hash = ?1 WHERE id = ?2",
            rusqlite::params!["0".repeat(64), target],
        )
        .expect("改链指针");

    let verification = log.verify_chain().expect("verify");
    assert!(!verification.is_intact());
    assert!(
        verification
            .findings
            .iter()
            .any(|finding| finding.kind == TamperKind::OrphanedRecord),
        "应报 orphaned_record：{:?}",
        verification.findings
    );
}

/// 删中间行必须被检出（后续行不可达）。
#[test]
fn test_verify_chain_detects_deleted_middle_row() {
    let clock = Arc::new(FixedClock::new(1_700_000_000_000));
    let dir = TestDir::new("tamper-delete");
    let database = open_database(&dir, &clock);
    let mut log = immediate_log(database.connection(), &clock);

    for index in 0..3_u8 {
        log.append(
            &sample_event(
                &format!("s_d{index}"),
                "notepad.read_text",
                "2026-09-24T00:00:00Z",
            ),
            &AuditSubject::unattached(),
        )
        .expect("append");
    }

    let connection = database.connection();
    let middle = all_records(&log).first().expect("至少一行").id.clone();
    drop_trigger(connection, "audit_logs_no_delete");
    connection
        .execute(
            "DELETE FROM audit_logs WHERE id = ?1",
            rusqlite::params![middle],
        )
        .expect("删中间行");

    let verification = log.verify_chain().expect("verify");
    assert!(!verification.is_intact());
    assert_eq!(verification.checked, 2, "删完只剩 2 行");
    assert_eq!(verification.reachable, 0, "创世那一行被删 ⇒ 之后全部不可达");
    assert!(
        verification
            .findings
            .iter()
            .all(|finding| finding.kind == TamperKind::OrphanedRecord),
        "应全部是 orphaned_record：{:?}",
        verification.findings
    );
}

/// 严格模式：链不自洽时必须**返回错误**，不给调用方"忘了看 findings"的机会。
#[test]
fn test_verify_chain_strict_returns_error_when_broken() {
    let clock = Arc::new(FixedClock::new(1_700_000_000_000));
    let dir = TestDir::new("strict");
    let database = open_database(&dir, &clock);
    let mut log = immediate_log(database.connection(), &clock);

    for index in 0..2_u8 {
        log.append(
            &sample_event(
                &format!("s_strict{index}"),
                "notepad.read_text",
                "2026-09-24T00:00:00Z",
            ),
            &AuditSubject::unattached(),
        )
        .expect("append");
    }
    assert!(log.verify_chain_strict().expect("自洽时应 Ok").is_intact());

    // 只删首行（留下不可达的后继行）—— 注意：**整表清空**无法从库内检出，
    // 那需要外部锚点，属已知限制（见 PL-044），不在本用例的判据内。
    let first = all_records(&log).first().expect("至少一行").id.clone();
    drop_trigger(database.connection(), "audit_logs_no_delete");
    database
        .connection()
        .execute(
            "DELETE FROM audit_logs WHERE id = ?1",
            rusqlite::params![first],
        )
        .expect("删首行");

    let failure = log.verify_chain_strict();
    assert!(
        matches!(failure, Err(AuditError::ChainBroken { .. })),
        "断链时必须返回 ChainBroken"
    );
}

/// PL-043 的原始触发场景：`VACUUM` 之后链尾必须**仍然正确**。
///
/// 为什么这条能证伪旧实现：旧实现的链尾查询是 `ORDER BY rowid DESC LIMIT 1`，而本表**没有**
/// `INTEGER PRIMARY KEY` —— SQLite 文档明说 `VACUUM` 可以重排这种表的 rowid。新实现用显式
/// `sequence`（迁移 0003 / ADR-0040 D2），与物理存储顺序解耦。断言「VACUUM 前后链尾一致 +
/// VACUUM 之后新行仍接得上旧链尾 + 整链自洽」。
#[test]
fn test_vacuum_does_not_change_chain_tail() {
    let clock = Arc::new(FixedClock::new(1_700_000_000_000));
    let dir = TestDir::new("vacuum");
    let database = open_database(&dir, &clock);
    let mut log = immediate_log(database.connection(), &clock);
    for index in 0..3 {
        log.append(
            &sample_event("s_1", &format!("act_{index}"), "2026-09-24T00:00:00Z"),
            &AuditSubject::unattached(),
        )
        .expect("append");
    }
    let tail_before = log.persisted_hash().to_owned();
    assert_eq!(tail_before.len(), 64);

    // VACUUM 不能在事务里跑；此处没有未提交事务。
    database
        .connection()
        .execute_batch("VACUUM;")
        .expect("VACUUM 审计库");

    // 重开写入器：它读回的链尾必须与 VACUUM 前一致
    let mut reopened = immediate_log(database.connection(), &clock);
    assert_eq!(
        reopened.persisted_hash(),
        tail_before,
        "VACUUM 后链尾不得改变（链序由 sequence 决定，与 rowid 无关）"
    );

    reopened
        .append(
            &sample_event("s_1", "act_after_vacuum", "2026-09-24T00:00:01Z"),
            &AuditSubject::unattached(),
        )
        .expect("append");
    let records = all_records(&reopened);
    assert_eq!(
        records.last().expect("最后一行").prev_hash,
        tail_before,
        "VACUUM 后新行必须接上旧链尾"
    );
    assert!(reopened.verify_chain().expect("verify").is_intact());
    database.close().expect("关闭");
}
