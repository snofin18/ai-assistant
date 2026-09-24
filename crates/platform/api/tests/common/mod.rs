//! 集成测试共用的小工具。
//!
//! 为什么需要它：本 crate 的 `[lints]` 继承 workspace（`clippy::expect_used` / `unwrap_used` /
//! `panic` = deny，AGENTS.md §5.3），而 TASK-016 的 Out of scope 明确禁止 `#[allow]` 放宽 ——
//! 因此测试里**不用** `expect` / `unwrap` / `panic!`，改用"先断言、再解包"。
//! 失败信息与 `expect` 同等可读（上下文 + 完整 `Debug`），且不会把 lint 放宽。
//!
//! 断言风格：优先比较**整个 `Result`**（`.ok()` / `.err()` / `matches!`），
//! 这样"该失败却成功"和"该成功却失败"都会被抓到；确实需要 `Ok` 值时用 [`ok_or_fail`]。

/// 断言 `result` 是 `Ok` 并取出其值。
///
/// # Panics
/// `result` 是 `Err` 时 panic —— 在测试里这正是期望行为（让该测试失败并打印原因）。
#[track_caller]
pub fn ok_or_fail<T: std::fmt::Debug, E: std::fmt::Debug>(
    result: Result<T, E>,
    context: &str,
) -> T {
    assert!(result.is_ok(), "{context}: {result:?}");
    // 上一条 `assert!` 已保证这里是 `Ok`。用 `unreachable!` 而不是 `panic!`：
    // workspace 把 `clippy::panic` 定为 deny（`unreachable!` 是另一个 lint）。
    let Ok(value) = result else {
        unreachable!("`assert!` 已保证是 `Ok`")
    };
    value
}
