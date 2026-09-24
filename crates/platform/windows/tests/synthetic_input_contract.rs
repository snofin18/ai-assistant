//! TASK-018 的**契约**回归（`tests/` 层，与 `src/**` 里的单测互补）。
//!
//! 为什么需要它：`src/**` 的单测只覆盖**纯函数**（键名映射、组合键顺序、DPI 换算…），
//! 但 TASK-018 的 `DoD` 里还有两条**结构性**断言无法在单测里表达：
//! 1. `pointer_action` / `key_action` 的 `// STUB(TASK-018):` 占位**已被真实实现替换**
//!    （源码里不该再出现这个标记）；
//! 2. 两个方法**不再返回 `CapabilityMissing`** —— 而这是"占位实现还在"的典型症状。
//!
//! ## 不变量
//! 1. 断言**不是恒真**：先断言"真的扫到了预期的那些文件"，否则目录改名会让扫描变成空转。
//! 2. **负向用例必须存在**（ADR-0019 N1）：扫描器喂已知坏样本，证明它真会报。
//! 3. 真机相关的断言（需要显示器）只在 Windows 上跑，且**不依赖任何真实应用**。

use std::path::{Path, PathBuf};

/// `src/**` 里**不允许**再出现的占位标记（TASK-018 `DoD` 第 1 条）。
const FORBIDDEN_STUB: &str = "STUB(TASK-018)";

/// 期望扫到的源码文件数**下界**（2026-09-25 实测 19 个 `.rs`）。
///
/// 取一个明显低于实测值的数：它只用来证明"扫描真的发生了"，不是版本计数器。
const MINIMUM_SOURCE_FILES: usize = 12;

fn crate_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// 递归收集 `dir` 下所有 `.rs` 文件（相对 crate 根，**排序**后返回 → 失败可复现）。
fn collect_rust_sources(root: &Path, dir: &Path, collected: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rust_sources(root, &path, collected);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            let relative = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
            collected.push(relative);
        }
    }
    collected.sort();
}

/// 文本里是否还有 TASK-018 的占位标记（**不**剥注释：源码任何位置都不该再出现它）。
fn contains_forbidden_stub(text: &str) -> bool {
    text.contains(FORBIDDEN_STUB)
}

#[test]
fn test_no_task_018_stub_marker_remains_in_sources() {
    let root = crate_root();
    let mut sources = Vec::new();
    collect_rust_sources(&root, &root.join("src"), &mut sources);
    assert!(
        sources.len() >= MINIMUM_SOURCE_FILES,
        "只扫到 {} 个源码文件（下界 {MINIMUM_SOURCE_FILES}）—— 扫描面可能已经失效",
        sources.len()
    );
    let mut offenders = Vec::new();
    for relative in &sources {
        let Ok(text) = std::fs::read_to_string(root.join(relative)) else {
            continue;
        };
        if contains_forbidden_stub(&text) {
            offenders.push(relative.display().to_string());
        }
    }
    assert!(
        offenders.is_empty(),
        "TASK-018 的占位标记仍留在源码里（占位必须已被真实实现替换）: {offenders:?}"
    );
}

#[test]
fn test_stub_scanner_flags_a_synthetic_marker() {
    // 负向用例（ADR-0019 N1）：扫描器必须真的会报 —— 否则上一条断言可能是恒真。
    assert!(contains_forbidden_stub("// STUB(TASK-018): 占位"));
    assert!(contains_forbidden_stub("STUB(TASK-018)"));
    // 正向对照：只提到卡号、没有占位标签前缀 → 不算占位。
    assert!(!contains_forbidden_stub("// TASK-018 已落地"));
    assert!(!contains_forbidden_stub(""));
}

/// Windows 上的**行为**断言：两个方法必须真的走实现，而不是继续返回 `CapabilityMissing`。
#[cfg(windows)]
mod windows_behaviour {
    use assistant_platform_api::{
        ErrorCode, KeyChord, KeyTarget, NormalizedPoint, PlatformResult, PointerAction,
        UiAutomationProvider,
    };
    use assistant_platform_windows::WindowsPlatform;

    /// 极简 executor：本 crate 不依赖 async runtime，测试只需要把"同步返回的 future"跑完。
    fn block_on<T>(future: impl std::future::Future<Output = T>) -> T {
        use std::task::{Context, Poll, Waker};
        let mut future = Box::pin(future);
        let waker = Waker::noop();
        let mut context = Context::from_waker(waker);
        loop {
            match future.as_mut().poll(&mut context) {
                Poll::Ready(value) => return value,
                Poll::Pending => std::thread::yield_now(),
            }
        }
    }

    /// 测试夹具：规范点（坐标非法就直接判夹具坏掉，而不是让断言静默通过）。
    fn point(x: f64, y: f64) -> NormalizedPoint {
        match NormalizedPoint::new(x, y) {
            Ok(point) => point,
            Err(failure) => unreachable!("测试夹具必须合法: {failure}"),
        }
    }

    /// 把一个 `PlatformResult` 压成"错误码或成功"，便于断言。
    fn code_of(outcome: PlatformResult<()>) -> Result<(), ErrorCode> {
        outcome.map_err(|failure| failure.code())
    }

    #[test]
    fn test_pointer_action_reports_target_not_found_for_a_point_off_every_display() {
        // 一个远在任何显示器之外的逻辑点：真实实现会走「枚举显示器 → 换算 → 归一化」，
        // 并在归一化处**明确**报 `TargetNotFound`。若占位实现还在，这里会得到
        // `CapabilityMissing` —— 那正是本用例要拦住的回归。
        let platform = WindowsPlatform::new();
        let outcome = block_on(UiAutomationProvider::pointer_action(
            &platform,
            point(10_000_000.0, 10_000_000.0),
            &PointerAction::Move,
        ));
        assert_eq!(
            code_of(outcome),
            Err(ErrorCode::TargetNotFound),
            "远点必须报 TargetNotFound（CapabilityMissing 说明占位实现还在）"
        );
    }

    #[test]
    fn test_key_action_rejects_an_unknown_key_name_before_touching_any_window() {
        // 键名校验是**纯逻辑**且排在解析窗口之前：所以这条用例不需要任何真实窗口，
        // 也不受"当前前台窗口是谁"影响（不受 CI 桌面状态影响的确定性断言）。
        let platform = WindowsPlatform::new();
        let chord = KeyChord::new("NotAKey".to_string(), Vec::new());
        let outcome = block_on(UiAutomationProvider::key_action(
            &platform,
            &chord,
            &KeyTarget::Foreground,
        ));
        assert_eq!(code_of(outcome), Err(ErrorCode::ToolInvalidArgs));
    }
}
