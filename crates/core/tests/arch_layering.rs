//! # `crates/core` 的分层断言（gov §5.1 门禁 #5，TASK-015 落地）
//!
//! 职责：把**铁律 7**（「core 不得调用平台 API，只能用 `crates/platform/api` 的 trait」）
//! 变成会红的测试，而不是一句口号。
//!
//! ## 为什么这个文件存在
//! `cargo test -p assistant-core arch::` 在 TASK-015 之前跑的是 **`running 0 tests`** ——
//! `crates/core` 当时只有一个 `lib.rs`（TASK-201 骨架），gov §5.1 #5 这条门禁**看着是绿的，
//! 实际什么都没查**（ADR-0019 说的「未验证的信心」）。本文件补上真断言。
//!
//! ## 两条断言
//! ① `crates/core/src/**/*.rs` 里不得出现平台**实现**的引用（`windows::` / `winapi::` /
//!    `objc2::` / `gtk::` / `atspi::` …）；
//! ② `crates/core/Cargo.toml` 的 `[dependencies]` 里不得出现平台实现 crate
//!    （`assistant-platform-windows` / `windows` / `winapi` / …）。
//!
//! ## 边界（不做什么）
//! - 不做完整依赖图分析（那是 `cargo-modules` 那类工具的事）：只查「平台实现」这一类
//!   **最危险且最容易图方便**的越界。
//! - 不查 `use` 的可见性、不查间接依赖（间接依赖由 `cargo deny` / `arch` 子命令覆盖）。
//! - **不查文档注释**：`crates/core/src/lib.rs` 的不变量 3 就写着「禁止 `core → platform/windows`」，
//!   那是对规则的**说明**，不是违规。因此扫描前先剥掉 `//` 之后的注释。
//!
//! ## 不变量
//! 1. **断言不是恒真**：两个正向测试都先断言「真的扫到了源码 / 清单非空」，
//!    否则「一个文件都没扫到」会伪装成通过（铁律 1）。
//! 2. **负向用例必须存在**（ADR-0019 N1）：`scan_flags_*` 三条用例喂已知坏样本，证明扫描器
//!    在违规时真的会报 —— 这是把 gov §5.1 #5 从软门禁转硬门禁的前置条件。
//!
//! 相关：`AGENTS.md` §2 铁律 7、`docs/governance-ai-agent-execution.md` §5.1 #5 / §5.3、
//! `cross-platform-ai-assistant-architecture-v2.md` §3.1、ADR-0019、`tasks/TASK-015-*.md`。
//!
//! 测试内允许 `expect` / `unwrap` / `panic`（AGENTS.md §5.3：`tests/` 内可 allow；
//! 断言失败必须让测试炸掉，而不是被吞掉）。
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};

/// 源码里**禁止**出现的平台实现引用。
///
/// 每条都带 `::`：这样文档注释里提到的**目录名**（`platform/windows`、`platform/api`）
/// 不会被误判 —— 只有真正 `use`/调用某个平台实现 crate 的路径才会命中。
const FORBIDDEN_PLATFORM_REFERENCES: &[&str] = &[
    "windows::",
    "windows_sys::",
    "winapi::",
    "objc2::",
    "cocoa::",
    "core_graphics::",
    "gtk::",
    "x11::",
    "atspi::",
    "zbus::",
    "wayland_client::",
];

/// `Cargo.toml` 的 `[dependencies]` 里**禁止**出现的 crate 名（平台实现层）。
///
/// 注意**不**包含 `assistant-platform-api` —— 那是铁律 7 明确允许的唯一入口（trait 层）。
const FORBIDDEN_PLATFORM_DEPENDENCIES: &[&str] = &[
    "assistant-platform-windows",
    "assistant-platform-macos",
    "assistant-platform-linux",
    "windows",
    "windows-sys",
    "winapi",
    "objc2",
    "cocoa",
    "core-graphics",
    "gtk",
    "x11",
    "zbus",
    "wayland-client",
];

/// 扫描一段 Rust 源码里的平台实现引用；返回人可读的命中清单（空 = 干净）。
///
/// 先剥掉 `//` 之后的注释：规则**说明**里出现平台目录名是正常的，代码里出现才是违规。
fn scan_for_forbidden_platform_references(source: &str) -> Vec<String> {
    let mut hits = Vec::new();
    for (index, line) in source.lines().enumerate() {
        let code = line.split("//").next().unwrap_or("");
        for needle in FORBIDDEN_PLATFORM_REFERENCES {
            if code.contains(needle) {
                hits.push(format!("第 {} 行含 `{needle}`：{}", index + 1, line.trim()));
            }
        }
    }
    hits
}

/// 扫描 `Cargo.toml` 的 `[dependencies]`（含 `[dependencies.<name>]` 形态）里的平台实现依赖。
fn scan_manifest_for_platform_dependency(manifest: &str) -> Vec<String> {
    let mut hits = Vec::new();
    let mut in_dependencies = false;
    for (index, line) in manifest.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            // `[dependencies.winapi]` 这种「表头形态」把依赖名写在表头里，必须单独看
            if let Some(name) = trimmed
                .strip_prefix("[dependencies.")
                .and_then(|inner| inner.strip_suffix(']'))
                && FORBIDDEN_PLATFORM_DEPENDENCIES.contains(&name)
            {
                hits.push(format!("第 {} 行依赖 `{name}`（表头形态）", index + 1));
            }
            in_dependencies = trimmed == "[dependencies]" || trimmed.starts_with("[dependencies.");
            continue;
        }
        if !in_dependencies || trimmed.starts_with('#') {
            continue;
        }
        let Some(name) = trimmed.split('=').next() else {
            continue;
        };
        let name = name.trim();
        if FORBIDDEN_PLATFORM_DEPENDENCIES.contains(&name) {
            hits.push(format!("第 {} 行依赖 `{name}`", index + 1));
        }
    }
    hits
}

/// `crates/core` 的目录（`CARGO_MANIFEST_DIR` 就是这个 crate 的根）。
fn core_crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// 递归收集目录下的 `.rs`（结果排序 → 断言失败时输出稳定）。
fn rust_sources_under(directory: &Path) -> Vec<PathBuf> {
    let mut collected = Vec::new();
    collect_rust_sources(directory, &mut collected);
    collected.sort();
    collected
}

fn collect_rust_sources(directory: &Path, collected: &mut Vec<PathBuf>) {
    let entries = std::fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("读目录 {} 失败：{error}", directory.display()));
    for entry in entries {
        let path = entry.expect("读目录项").path();
        if path.is_dir() {
            collect_rust_sources(&path, collected);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            collected.push(path);
        }
    }
}

mod arch {
    use super::*;

    /// 正向断言 ①：core 源码里不得引用平台实现（铁律 7）。
    #[test]
    fn core_sources_do_not_reference_platform_implementations() {
        let sources = rust_sources_under(&core_crate_dir().join("src"));
        assert!(
            !sources.is_empty(),
            "必须在 crates/core/src 下扫到至少一个 .rs —— 否则这条断言是空断言（铁律 1）"
        );
        let mut violations = Vec::new();
        for path in &sources {
            let content = std::fs::read_to_string(path)
                .unwrap_or_else(|error| panic!("读 {} 失败：{error}", path.display()));
            for hit in scan_for_forbidden_platform_references(&content) {
                violations.push(format!("{}: {hit}", path.display()));
            }
        }
        assert!(
            violations.is_empty(),
            "crates/core 不得直接引用平台实现（铁律 7：只能用 crates/platform/api 的 trait）：\n{violations:#?}"
        );
    }

    /// 正向断言 ②：core 的清单里不得有平台实现依赖。
    #[test]
    fn core_manifest_has_no_platform_implementation_dependency() {
        let manifest_path = core_crate_dir().join("Cargo.toml");
        let manifest = std::fs::read_to_string(&manifest_path)
            .unwrap_or_else(|error| panic!("读 {} 失败：{error}", manifest_path.display()));
        assert!(
            manifest.contains("[dependencies]"),
            "清单里必须能找到 [dependencies] 节 —— 否则这条断言是空断言"
        );
        let violations = scan_manifest_for_platform_dependency(&manifest);
        assert!(
            violations.is_empty(),
            "crates/core 不得依赖平台实现 crate（铁律 7）：\n{violations:#?}"
        );
    }

    // ---- ADR-0019 N1：负向用例（证明断言在该红的时候会红）----

    /// 负向用例 ①：源码扫描器必须能抓到平台实现引用。
    #[test]
    fn scan_flags_a_synthetic_platform_reference() {
        let hits = scan_for_forbidden_platform_references(
            "fn x() { let _ = windows::Win32::UI::Input::KeyboardAndMouse::SendInput; }\n",
        );
        assert!(!hits.is_empty(), "喂 `windows::` 必须报，实际无命中");
        assert!(hits[0].contains("windows::"), "{hits:?}");
    }

    /// 负向用例 ②：清单扫描器必须能抓到平台实现依赖。
    #[test]
    fn scan_flags_a_synthetic_platform_dependency() {
        let hits = scan_manifest_for_platform_dependency(
            "[package]\nname = \"x\"\n[dependencies]\nwindows = \"0.62\"\n",
        );
        assert!(!hits.is_empty(), "喂 `windows = \"0.62\"` 必须报");
        assert!(hits[0].contains("windows"), "{hits:?}");

        let sub_table = scan_manifest_for_platform_dependency(
            "[dependencies]\n[dependencies.winapi]\nversion = \"0.3\"\n",
        );
        assert!(
            !sub_table.is_empty(),
            "`[dependencies.winapi]` 形态也必须报"
        );
    }

    /// 负向用例 ③（反向）：唯一允许的平台入口 `assistant-platform-api` 与
    /// 规则**说明**里提到的目录名都不得被误报。
    #[test]
    fn scan_allows_platform_api_trait_and_doc_mentions() {
        let manifest_hits = scan_manifest_for_platform_dependency(
            "[dependencies]\nassistant-platform-api = { path = \"../platform/api\" }\n",
        );
        assert!(
            manifest_hits.is_empty(),
            "铁律 7 允许 core 依赖 platform/api 的 trait：{manifest_hits:?}"
        );
        let source_hits = scan_for_forbidden_platform_references(
            "//! 禁止 core → platform/windows（只能依赖 platform/api 的 trait）\nlet x = 1;\n",
        );
        assert!(
            source_hits.is_empty(),
            "文档注释里提到平台目录名不是违规：{source_hits:?}"
        );
    }
}
