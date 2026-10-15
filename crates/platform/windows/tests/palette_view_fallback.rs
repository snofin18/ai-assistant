//! ADR-0094 的真机验证：`Selection::ByIndex` 必须能在「子项只存在于 control view」的列表上工作。
//!
//! 一手证据（`DRIFT-296-1`）：Paint 调色板的色块是 **control 元素但不是 content 元素** ——
//! 该 `List` 在 content view 下 0 个子项、在 control view 下有全部色块。ADR-0087 的单一
//! `ContentViewWalker` 口径因此对调色板系统性失能；ADR-0094 改为 `content → control` 有序回退。
//!
//! 默认 `#[ignore]`：需要真实桌面 + 正在运行的 Paint 11。本用例会**改变前景色**（选中一个色块）。
#![cfg(windows)]
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::future::Future;
use std::task::{Context, Poll, Waker};

use assistant_platform_api::{
    ErrorCode, Selection, SelectorCandidate, SelectorChain, SelectorKind, SelectorValue,
    UiAutomationProvider, WindowFilter, WindowProvider,
};
use assistant_platform_windows::WindowsPlatform;

/// 驱动平台 provider 的 future（它们都是立即就绪的 `poll_fn`，见 `uia/mod.rs` 的说明）。
fn ready<F: Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    let waker = Waker::noop();
    let mut context = Context::from_waker(waker);
    match future.as_mut().poll(&mut context) {
        Poll::Ready(output) => output,
        Poll::Pending => panic!("platform provider future is not immediately ready"),
    }
}

/// 与 `adapters/com.microsoft.paint/selectors/targets.json` 的 `foreground_color_button` 同链。
fn palette_chain() -> SelectorChain {
    SelectorChain::new(vec![
        SelectorCandidate::new(
            "color-group",
            SelectorKind::ExactNameAndRole,
            SelectorValue::ExactNameAndRole {
                name: "颜色".to_owned(),
                role: "Group".to_owned(),
            },
            0.3,
            true,
        )
        .expect("color group candidate"),
        SelectorCandidate::new(
            "palette-list",
            SelectorKind::RoleAndParent,
            SelectorValue::RoleAndParent {
                role: "List".to_owned(),
                parent_id: "color-group".to_owned(),
            },
            0.66,
            false,
        )
        .expect("palette list candidate"),
    ])
}

#[test]
#[ignore = "ADR-0094: needs a real desktop with a running Paint 11 window; changes the foreground color"]
fn test_select_by_index_falls_back_to_the_control_view_for_the_paint_palette() {
    let platform = WindowsPlatform::new();
    let windows = ready(platform.list_windows(&WindowFilter::any())).expect("list windows");
    let window = windows
        .into_iter()
        .find(|info| info.app_id().to_ascii_lowercase().contains("mspaint"))
        .map(|info| info.window().clone())
        .expect("a running Paint window");
    let palette = ready(platform.resolve_element(&window, &palette_chain()))
        .expect("the Paint color palette resolves inside the window scope");

    // 负向先跑（不改 UI）：两个视图都越界 → 显式 `TargetNotFound`，绝不猜第一个元素。
    let error = ready(platform.select(&palette, &Selection::ByIndex(5_000)))
        .expect_err("an index beyond both views must fail closed");
    assert_eq!(
        error.code(),
        ErrorCode::TargetNotFound,
        "out-of-range select must be TargetNotFound, got {error:?}"
    );

    // 正向：色块只在 control view 里，`content → control` 回退必须命中（本用例会改前景色）。
    ready(platform.select(&palette, &Selection::ByIndex(3)))
        .expect("ByIndex(3) must reach a swatch through the control-view fallback");
}
