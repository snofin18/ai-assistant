//! `guard.rs` 的单元测试：锁记录编解码 / slug 派生 / `decide_acquire` 纯决策（ADR-0028）的白盒测试。
//!
//! **为什么单独一个文件**：`guard.rs` 连同测试会超过单文件行数阈值
//! （AGENTS.md §5.3 的 600 行硬上限 / gov §5.4 的 > 600 警告）。用 `#[path]` 把 `mod tests`
//! 外置后两侧都回到阈值内，而测试**仍然是本模块的私有单元测试** —— `use super::*` 照旧能
//! 访问私有项，这一点与 `tests/` 目录下的集成测试有本质区别，不能混为一谈。
//! 声明处在 `guard.rs` 末尾：`#[cfg(test)] #[path = "guard_tests.rs"] mod tests;`。
//!
//! ## 组织方式
//! 按被测函数分组，每组前有一行 `// --- 函数名 ---` 分隔注释；命名遵循
//! `test_<被测单元>_<条件>_<期望>`（AGENTS.md §5.1）。

// 测试里允许 unwrap/expect/panic：断言失败就该立刻炸出来，包装成 Result 只会掩盖问题
// （AGENTS.md §5.5「tests/ 内可 allow」）。
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::*;

/// 造一条锁记录，只让调用方指定关心的字段。
fn record(owner: &str, acquired_at_unix: u64) -> LockRecord {
    LockRecord {
        target: "MEMORY.md".to_string(),
        owner: owner.to_string(),
        pid: 4242,
        task: "TASK-030".to_string(),
        acquired_at_unix,
        acquired_at_iso: "2026-09-18T14:00:00+08:00".to_string(),
        intent: "追加 LEDGER 行".to_string(),
    }
}

// --- 编解码互逆（不变量 2）---

#[test]
fn test_render_then_parse_lock_record_roundtrips() {
    let original = record("codex-1a2b3c4d", 1_758_172_800);
    let parsed = parse_lock_record(&render_lock_record(&original)).expect("应能解析回来");
    assert_eq!(parsed, original, "不变量 2：编解码互逆");
}

#[test]
fn test_render_lock_record_has_one_line_per_field() {
    let text = render_lock_record(&record("codex-x", 1));
    assert_eq!(text.lines().count(), RECORD_KEYS.len(), "七个字段七行");
    for key in RECORD_KEYS {
        assert!(text.contains(&format!("{key}=")), "缺字段 {key}");
    }
}

#[test]
fn test_render_lock_record_flattens_newlines_in_free_text() {
    // 不变量 3：intent 里的换行会破坏行式格式，必须压成空格
    let mut dirty = record("codex-x", 1);
    dirty.intent = "第一行\n第二行\r\n第三行".to_string();
    let parsed = parse_lock_record(&render_lock_record(&dirty)).expect("仍应能解析");
    assert_eq!(parsed.intent, "第一行 第二行 第三行");
}

#[test]
fn test_parse_lock_record_missing_key_is_error() {
    let error = parse_lock_record("target=MEMORY.md\nowner=codex-x\n")
        .expect_err("缺字段必须报错，不能猜默认值");
    assert!(error.contains("pid"), "应指出缺哪个字段：{error}");
}

#[test]
fn test_parse_lock_record_non_numeric_field_is_error() {
    let text = render_lock_record(&record("codex-x", 1)).replace("pid=4242", "pid=unknown");
    let error = parse_lock_record(&text).expect_err("pid 不是整数必须报错");
    assert!(error.contains("pid"), "实际：{error}");
}

#[test]
fn test_parse_lock_record_rejects_line_without_equals() {
    let error = parse_lock_record("这不是锁记录\n").expect_err("必须报错");
    assert!(error.contains("key=value"), "实际：{error}");
}

#[test]
fn test_parse_lock_record_keeps_equals_sign_inside_intent() {
    // 只在**第一个** `=` 处切分，否则 `intent=a=b` 会被截断
    let mut tricky = record("codex-x", 1);
    tricky.intent = "把 a=b 改成 c=d".to_string();
    let parsed = parse_lock_record(&render_lock_record(&tricky)).expect("应能解析");
    assert_eq!(parsed.intent, "把 a=b 改成 c=d");
}

// --- slug / 摘要（不变量 1）---

#[test]
fn test_slug_for_replaces_separators_and_punctuation() {
    let slug = slug_for("docs/memory/apps/notepad.md");
    assert!(
        slug.starts_with("docs__memory__apps__notepad-md-"),
        "实际：{slug}"
    );
    assert!(slug.ends_with(LOCK_FILE_SUFFIX));
}

#[test]
fn test_slug_for_is_case_insensitive_across_platforms() {
    assert_eq!(
        slug_for("MEMORY.md"),
        slug_for("memory.md"),
        "Windows/macOS 大小写不敏感，必须映射到同一把锁"
    );
}

#[test]
fn test_slug_for_distinguishes_paths_that_fold_to_same_text() {
    // `a-b.md` 与 `a/b.md` 折叠后都是 `a-b-md`，靠摘要区分（否则两个文件共用一把锁）
    assert_ne!(slug_for("a-b.md"), slug_for("a/b.md"));
}

#[test]
fn test_slug_for_is_deterministic_and_bounded() {
    let long_path = format!("docs/{}.md", "x".repeat(400));
    let slug = slug_for(&long_path);
    assert_eq!(slug, slug_for(&long_path), "不变量 1：纯函数");
    assert!(
        slug.chars().count() <= SLUG_VISIBLE_LIMIT + 1 + 8 + 5,
        "slug 必须有界（Windows 路径段上限 255），实际 {} 字符",
        slug.chars().count()
    );
}

#[test]
fn test_fnv1a_hex32_matches_known_vectors() {
    // FNV-1a 32 位的公开测试向量："" = 0x811c9dc5，"a" = 0xe40c292c，"foobar" = 0xbf9cf968
    assert_eq!(fnv1a_hex32(""), "811c9dc5");
    assert_eq!(fnv1a_hex32("a"), "e40c292c");
    assert_eq!(fnv1a_hex32("foobar"), "bf9cf968");
}

// --- normalize_target ---

#[test]
fn test_normalize_target_unifies_separators_and_strips_noise() {
    assert_eq!(normalize_target(".\\docs\\MEMORY.md"), "docs/MEMORY.md");
    assert_eq!(normalize_target("./docs//MEMORY.md"), "docs/MEMORY.md");
    assert_eq!(normalize_target("docs/MEMORY.md/"), "docs/MEMORY.md");
    assert_eq!(normalize_target("  MEMORY.md  "), "MEMORY.md");
}

#[test]
fn test_normalize_target_keeps_letter_case_for_display() {
    assert_eq!(
        normalize_target("MEMORY.md"),
        "MEMORY.md",
        "target 要能原样回显给人看"
    );
}

// --- decide_acquire：五个分支各至少一个用例（ADR-0028 验证方式 7）---

#[test]
fn test_decide_acquire_without_existing_record_grants() {
    assert_eq!(
        decide_acquire(None, 1000, 900, "codex-a", false),
        Acquisition::Grant
    );
}

#[test]
fn test_decide_acquire_same_owner_reuses_instead_of_blocking() {
    let held = record("codex-a", 1000);
    assert_eq!(
        decide_acquire(Some(&held), 1100, 900, "codex-a", false),
        Acquisition::Reuse,
        "同一会话重复 acquire 不该被自己挡住"
    );
}

#[test]
fn test_decide_acquire_fresh_lock_held_by_other_waits() {
    let held = record("codex-b", 1000);
    assert_eq!(
        decide_acquire(Some(&held), 1100, 900, "codex-a", false),
        Acquisition::Wait { held }
    );
}

#[test]
fn test_decide_acquire_stale_lock_is_taken_over() {
    let held = record("codex-b", 1000);
    // ADR-0028 验证方式 3：锁龄 3600 s、阈值 60 s → 接管，且带上被接管者的记录
    assert_eq!(
        decide_acquire(Some(&held), 4600, 60, "codex-a", false),
        Acquisition::TakeOver {
            held: held.clone(),
            reason: TakeOverReason::Stale
        }
    );
    assert_eq!(TakeOverReason::Stale.label(), "STALE");
}

#[test]
fn test_decide_acquire_exactly_at_stale_threshold_takes_over() {
    // 负向边界：判据是「锁龄 >= 阈值」，恰好等于阈值即接管
    let held = record("codex-b", 1000);
    assert!(matches!(
        decide_acquire(Some(&held), 1900, 900, "codex-a", false),
        Acquisition::TakeOver {
            reason: TakeOverReason::Stale,
            ..
        }
    ));
    assert!(matches!(
        decide_acquire(Some(&held), 1899, 900, "codex-a", false),
        Acquisition::Wait { .. }
    ));
}

#[test]
fn test_decide_acquire_force_overrides_everything() {
    let held = record("codex-b", 1000);
    assert_eq!(
        decide_acquire(Some(&held), 1001, 900, "codex-a", true),
        Acquisition::TakeOver {
            held,
            reason: TakeOverReason::Forced
        },
        "人工意图优先于自动判据（含时钟异常）"
    );
    assert_eq!(TakeOverReason::Forced.label(), "FORCED");
}

#[test]
fn test_decide_acquire_future_timestamp_is_clock_anomaly_not_takeover() {
    let held = record("codex-b", 5000);
    // ADR-0028 验证方式 4：获取时刻在未来 → 等待并标注，**不接管**
    assert_eq!(
        decide_acquire(Some(&held), 1400, 60, "codex-a", false),
        Acquisition::ClockAnomaly { held }
    );
}

#[test]
fn test_lock_age_secs_never_underflows() {
    let held = record("codex-b", 5000);
    assert_eq!(
        lock_age_secs(&held, 1400),
        0,
        "时钟异常时锁龄是 0，不是负数或巨大值"
    );
    assert_eq!(lock_age_secs(&held, 6000), 1000);
}

// --- sort_targets（ADR-0028 验证方式 5）---

#[test]
fn test_sort_targets_makes_acquisition_order_independent_of_argument_order() {
    let forward = sort_targets(&["A.md".to_string(), "B.md".to_string()]);
    let backward = sort_targets(&["B.md".to_string(), "A.md".to_string()]);
    assert_eq!(
        forward, backward,
        "两种参数顺序必须得到同一获取顺序（死锁避免）"
    );
    assert_eq!(forward, vec!["A.md", "B.md"]);
}

#[test]
fn test_sort_targets_normalizes_and_dedups() {
    let sorted = sort_targets(&[
        ".\\docs\\MEMORY.md".to_string(),
        "docs/MEMORY.md".to_string(),
        "LEDGER.md".to_string(),
    ]);
    assert_eq!(sorted, vec!["LEDGER.md", "docs/MEMORY.md"]);
}

#[test]
fn test_sort_targets_drops_empty_entries() {
    assert!(sort_targets(&["   ".to_string(), "/".to_string()]).is_empty());
}
