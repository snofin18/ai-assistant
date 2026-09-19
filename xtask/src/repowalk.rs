//! # 仓库遍历（xtask 的文件系统层）
//!
//! 职责：确定仓库根、按固定规则收集需要扫描的源文件、生成跨平台一致的相对路径文本。
//! 这是 xtask 里**唯一**接触文件系统的地方（除了 `main.rs` 读文件内容），
//! 把 IO 集中在一处，规则模块才能保持纯函数。
//!
//! ## 边界（不做什么）
//! - 不读文件内容（`main.rs` 负责，因为读失败要带上具体路径报错）。
//! - 不判定规则。
//! - 不写任何文件（xtask 是只读工具，不变量见 `main.rs`）。
//!
//! ## 不变量
//! 1. **确定性**：同一仓库状态两次遍历得到**逐元素相同**的文件列表。
//!    `std::fs::read_dir` 的顺序由文件系统决定，因此必须排序（否则 CI 输出会随机变化）。
//! 2. 结果去重：同一文件不会因为既在仓库根又在扫描根下而出现两次。
//! 3. 相对路径一律用 `/` 分隔，保证 Windows 与 Linux 的报告文本一字不差。
//! 4. 任何 IO 失败都向上返回错误，**不静默跳过**（跳过目录会让检查结果虚假变绿）。
//!
//! ## 扫描范围的决定（为什么不是"整个仓库"）
//! 只扫 workspace 成员所在目录：`spikes/` 与 `fixtures/apps/` 被根 `Cargo.toml` 明确
//! `exclude`，它们是一次性验证代码、不参与产品构建，因此也不受产品卫生规则约束。
//! 否则 Spike 里为快速验证而写的粗糙代码会长期把 CI 染红，进而训练人忽略红灯 ——
//! 那比不检查更糟。
//!
//! 相关：`docs/governance-ai-agent-execution.md` §5.4

use std::path::{Path, PathBuf};

/// 需要扫描的源码根（相对仓库根）。
pub const SCANNED_SOURCE_ROOTS: [&str; 3] = ["crates", "apps", "xtask"];

/// 递归遍历时跳过的目录名（构建产物与版本库元数据）。
pub const SKIPPED_DIRECTORY_NAMES: [&str; 4] = ["target", ".git", "node_modules", "dist"];

/// 遍历失败的原因（带上下文，便于在 CI 日志里直接定位）。
#[derive(Debug, PartialEq, Eq)]
pub enum WalkError {
    /// 仓库根不存在或不是目录。
    RootUnavailable(String),
    /// 无法从编译期常量推导仓库根。
    RootUnderivable,
    /// 读取目录失败。
    ReadDirectory(String),
}

impl std::fmt::Display for WalkError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RootUnavailable(message) => write!(formatter, "仓库根不可用：{message}"),
            Self::RootUnderivable => {
                write!(
                    formatter,
                    "无法从 CARGO_MANIFEST_DIR 推导仓库根（缺少父目录）"
                )
            }
            Self::ReadDirectory(message) => write!(formatter, "读取目录失败：{message}"),
        }
    }
}

/// 确定仓库根目录。
///
/// `override_root` 为 `None` 时用**编译期**的 `CARGO_MANIFEST_DIR` 的父目录，
/// 而不是当前工作目录：这样 `cargo run -p xtask -- hygiene` 无论从哪个子目录调用，
/// 扫描范围都一致（避免"在 xtask 目录下跑就只扫 xtask"这类隐蔽的行为差异）。
///
/// # 错误
/// - `WalkError::RootUnderivable`：编译期常量没有父目录（理论上不会发生）
/// - `WalkError::RootUnavailable`：目录不存在或不是目录
pub fn resolve_repo_root(override_root: Option<&Path>) -> Result<PathBuf, WalkError> {
    let root = match override_root {
        Some(path) => path.to_path_buf(),
        None => Path::new(env!("CARGO_MANIFEST_DIR")).parent().map_or_else(
            || Err(WalkError::RootUnderivable),
            |parent| Ok(parent.to_path_buf()),
        )?,
    };
    if root.is_dir() {
        Ok(root)
    } else {
        Err(WalkError::RootUnavailable(root.display().to_string()))
    }
}

/// 收集需要扫描的 `.rs` 文件（已排序去重，满足不变量 1、2）。
///
/// # 错误
/// 任一目录读取失败即返回 `WalkError::ReadDirectory`（不静默跳过，不变量 4）。
pub fn collect_rust_files(root: &Path) -> Result<Vec<PathBuf>, WalkError> {
    let mut files: Vec<PathBuf> = Vec::new();
    collect_top_level_rust_files(root, &mut files)?;
    for name in SCANNED_SOURCE_ROOTS {
        let sub_root = root.join(name);
        if sub_root.is_dir() {
            collect_rust_files_recursively(&sub_root, &mut files)?;
        }
    }
    files.sort();
    files.dedup();
    Ok(files)
}

/// 收集目录下（**不递归**）的 `.rs` 文件。
///
/// 只用于仓库根：根目录下的散置 `.rs`（例如将来可能出现的 `build.rs`）也应被扫到，
/// 但我们不会去递归 `docs/`、`plans/` 这些非代码目录。
fn collect_top_level_rust_files(directory: &Path, out: &mut Vec<PathBuf>) -> Result<(), WalkError> {
    for entry in read_sorted_entries(directory)? {
        if entry.is_file() && has_rust_extension(&entry) {
            out.push(entry);
        }
    }
    Ok(())
}

/// 递归收集目录下的 `.rs` 文件，跳过 `SKIPPED_DIRECTORY_NAMES`。
fn collect_rust_files_recursively(
    directory: &Path,
    out: &mut Vec<PathBuf>,
) -> Result<(), WalkError> {
    for entry in read_sorted_entries(directory)? {
        if entry.is_dir() {
            if is_skipped_directory(&entry) {
                continue;
            }
            collect_rust_files_recursively(&entry, out)?;
        } else if has_rust_extension(&entry) {
            out.push(entry);
        }
    }
    Ok(())
}

/// 读取目录条目并按路径排序（排序是不变量 1 的前提）。
fn read_sorted_entries(directory: &Path) -> Result<Vec<PathBuf>, WalkError> {
    let entries = std::fs::read_dir(directory)
        .map_err(|error| WalkError::ReadDirectory(format!("{}：{error}", directory.display())))?;
    let mut paths: Vec<PathBuf> = Vec::new();
    for item in entries {
        let item = item.map_err(|error| {
            WalkError::ReadDirectory(format!("{} 的某个条目：{error}", directory.display()))
        })?;
        paths.push(item.path());
    }
    paths.sort();
    Ok(paths)
}

/// 路径的扩展名是否为 `rs`。
#[must_use]
/// 仓库文件条目（相对路径 + 绝对路径）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoFileEntry {
    pub rel_path: String,
    pub abs_path: std::path::PathBuf,
}

/// 递归收集仓库内所有指定扩展名的文件（扩展名小写、不含点）。
/// 跳过 `target/` 与 `.git/`（与 `is_skipped_directory` 一致）。
/// 扩展名为空数组 = 收所有文本文件（不含二进制扩展名）。
pub fn collect_repo_files(
    root: &std::path::Path,
    extensions: &[&str],
) -> Result<Vec<RepoFileEntry>, WalkError> {
    let mut out = Vec::new();
    collect_repo_files_recursively(root, root, extensions, &mut out)?;
    out.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
    Ok(out)
}

/// 递归遍历 `directory`，把扩展名命中 `extensions` 的文件追加到 `out`。
///
/// `extensions` 为空数组 = 「任何文本文件」，实现 = 排除 3 种二进制图像格式
/// （PNG / JPG / GIF；与 `hygiene` 的非代码目录跳过策略一致）。
///
/// TASK-055b 落地：用 `extension_is` 替代原 `lower.ends_with(".ext")` 模式，
/// 消除 fn 级 `#[allow(clippy::case_sensitive_file_extension_comparisons)]`。
/// 语义变化：
/// - 大小写不敏感扩展名匹配（如 `extensions = ["MD"]` 现在能匹配 `a.md`）—— **修复**了
///   原 `lower.ends_with(&format!(".{ext}"))` 在 ext 大写时的逻辑 bug（caller 现状全小写，
///   故实际无回归，只是能力增强）。
/// - 非 UTF-8 扩展名现在按 `to_str()` 失败 = `false`（与原模式在非 UTF-8 `lower: &str` 上
///   同样失败 = 同形）。
fn collect_repo_files_recursively(
    root: &std::path::Path,
    directory: &std::path::Path,
    extensions: &[&str],
    out: &mut Vec<RepoFileEntry>,
) -> Result<(), WalkError> {
    let entries = std::fs::read_dir(directory)
        .map_err(|e| WalkError::ReadDirectory(format!("{}：{e}", directory.display())))?;
    for entry in entries {
        let entry = entry
            .map_err(|e| WalkError::ReadDirectory(format!("{} 条目：{e}", directory.display())))?;
        let path = entry.path();
        if path.is_dir() {
            if is_skipped_directory(&path) {
                continue;
            }
            collect_repo_files_recursively(root, &path, extensions, out)?;
        } else if path.is_file() {
            let matches = if extensions.is_empty() {
                // 「收所有文本文件」模式 = 排除已知的二进制图像格式
                !extension_is(&path, "png")
                    && !extension_is(&path, "jpg")
                    && !extension_is(&path, "gif")
            } else {
                extensions.iter().any(|ext| extension_is(&path, ext))
            };
            if matches {
                let rel = relative_display_path(root, &path);
                out.push(RepoFileEntry {
                    rel_path: rel,
                    abs_path: path,
                });
            }
        }
    }
    Ok(())
}

/// 路径的扩展名（最后一个 `.` 之后的部分）是否等于 `expected`，**大小写不敏感**。
///
/// 与 `has_rust_extension` 等价的通用版本（任何扩展名）。之所以**不**直接复用
/// `has_rust_extension` 的实现模式（`to_string_lossy() == "rs"`），是为了让 clippy
/// 能看到「比较前已做大小写规范化」—— 旧模式（`to_ascii_lowercase().ends_with(".ext")`）
/// 会触发 `clippy::case_sensitive_file_extension_comparisons`，需要 per-line allow。
///
/// ## 语义与边界
/// - **UTF-8 不合法**的扩展名返回 `false`（`OsStr::to_str` 返回 `None`）。
///   这与原 `lower.ends_with(".md")` 在 `lower: &str` 上要求 UTF-8 合法的行为同形。
/// - **大小写不敏感**：MD / md / Md 都算 `.md`。
/// - **无扩展名**（如 `README`）返回 `false`。
///
/// TASK-055b 落地（与 refscan.rs 的 local `ext_is` closure 等价但提到模块级）。
/// ADR-0035 baseline 表同步登记。
#[must_use]
pub fn extension_is(path: &Path, expected: &str) -> bool {
    path.extension()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s.eq_ignore_ascii_case(expected))
}

pub fn has_rust_extension(path: &Path) -> bool {
    extension_is(path, "rs")
}

/// 目录名是否在跳过清单中。
#[must_use]
pub fn is_skipped_directory(path: &Path) -> bool {
    path.file_name()
        .map(std::ffi::OsStr::to_string_lossy)
        .is_some_and(|name| SKIPPED_DIRECTORY_NAMES.contains(&name.as_ref()))
}

/// 生成用于报告的路径文本：相对仓库根、统一用 `/` 分隔（不变量 3）。
///
/// 若路径不在 `root` 之下（理论上不会发生，但 `--repo` 允许任意值），
/// 退化为输出完整路径 —— 宁可输出一个长路径，也不要输出一个错误归因的短路径。
#[must_use]
pub fn relative_display_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_repo_root_defaults_to_workspace_root() {
        // 编译期常量推导：xtask 的父目录就是仓库根，必须包含 AGENTS.md
        let root = resolve_repo_root(None).expect("默认仓库根应可用");
        assert!(
            root.join("AGENTS.md").is_file(),
            "推导出的仓库根不正确：{}",
            root.display()
        );
    }

    #[test]
    fn test_resolve_repo_root_honours_override() {
        // 刻意用「推导出的根 + 子目录」而不是相对路径：cargo test 的工作目录是 package 根
        // （xtask/），用相对路径会让用例的正确性依赖于"从哪里跑测试"，那是不该有的耦合。
        let workspace_root = resolve_repo_root(None).expect("默认仓库根应可用");
        let resolved = resolve_repo_root(Some(&workspace_root.join("xtask"))).expect("应可用");
        assert!(
            resolved.join("Cargo.toml").is_file(),
            "实际：{}",
            resolved.display()
        );
    }

    #[test]
    fn test_resolve_repo_root_rejects_missing_directory() {
        let error = resolve_repo_root(Some(Path::new("Z:/definitely-not-a-directory-xyz")))
            .expect_err("不存在的目录必须报错");
        assert_eq!(
            error,
            WalkError::RootUnavailable("Z:/definitely-not-a-directory-xyz".to_string())
        );
        assert!(error.to_string().contains("仓库根不可用"));
    }

    #[test]
    fn test_resolve_repo_root_rejects_file_as_root() {
        // 负向用例：指向一个文件而不是目录，也必须失败而不是"扫到 0 个文件"
        let workspace_root = resolve_repo_root(None).expect("默认仓库根应可用");
        let file = workspace_root.join("AGENTS.md");
        assert!(file.is_file(), "前置条件：AGENTS.md 应存在");
        let error = resolve_repo_root(Some(&file)).expect_err("文件不是目录");
        assert!(matches!(error, WalkError::RootUnavailable(_)));
    }

    #[test]
    fn test_collect_rust_files_finds_xtask_sources() {
        let root = resolve_repo_root(None).expect("默认仓库根应可用");
        let files = collect_rust_files(&root).expect("遍历不应失败");
        assert!(!files.is_empty(), "至少要扫到 xtask 自己的源码");
        let names: Vec<String> = files
            .iter()
            .map(|file| relative_display_path(&root, file))
            .collect();
        assert!(
            names.iter().any(|name| name == "xtask/src/main.rs"),
            "实际：{names:?}"
        );
    }

    #[test]
    fn test_collect_rust_files_is_sorted_and_deduplicated() {
        let root = resolve_repo_root(None).expect("默认仓库根应可用");
        let files = collect_rust_files(&root).expect("遍历不应失败");
        let mut sorted = files.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(files, sorted, "不变量 1、2：必须有序且无重复");
    }

    #[test]
    fn test_collect_rust_files_excludes_non_code_directories() {
        let root = resolve_repo_root(None).expect("默认仓库根应可用");
        let files = collect_rust_files(&root).expect("遍历不应失败");
        let names: Vec<String> = files
            .iter()
            .map(|file| relative_display_path(&root, file))
            .collect();
        assert!(
            !names
                .iter()
                .any(|name| name.starts_with("spikes/") || name.starts_with("target/")),
            "spikes 与构建产物不在扫描范围内，实际：{names:?}"
        );
    }

    #[test]
    fn test_collect_rust_files_on_missing_directory_reports_error() {
        let error = collect_rust_files(Path::new("Z:/definitely-not-a-directory-xyz"))
            .expect_err("不存在的目录必须报错，不能返回空列表");
        assert!(matches!(error, WalkError::ReadDirectory(_)));
    }

    #[test]
    fn test_has_rust_extension() {
        assert!(has_rust_extension(Path::new("a/b/lib.rs")));
        assert!(!has_rust_extension(Path::new("a/b/lib.rs.bk")));
        assert!(!has_rust_extension(Path::new("a/b/README.md")));
        assert!(!has_rust_extension(Path::new("a/b/noext")));
    }

    // ---- extension_is（TASK-055b 新增）----

    #[test]
    fn test_extension_is_matches_exactly() {
        assert!(extension_is(Path::new("a/b/README.md"), "md"));
        assert!(extension_is(Path::new("a/b/probe.ps1"), "ps1"));
        assert!(!extension_is(Path::new("a/b/README.md"), "rs"));
        assert!(!extension_is(Path::new("a/b/noext"), "md"));
    }

    #[test]
    fn test_extension_is_is_case_insensitive() {
        // MD/md/Md 都算 .md（修复了原 lower.ends_with(&format!(".{ext}")) 在 ext 大写时的逻辑 bug）
        assert!(extension_is(Path::new("a/b/README.MD"), "md"));
        assert!(extension_is(Path::new("a/b/README.Md"), "md"));
        assert!(extension_is(Path::new("a/b/README.md"), "MD"));
        assert!(extension_is(Path::new("a/b/README.md"), "Md"));
    }

    #[test]
    fn test_extension_is_returns_false_for_non_utf8_extension() {
        // OsStr::to_str() 在非 UTF-8 字节序列上返回 None
        // → is_some_and 短路 → false
        // 与原 lower.ends_with(".md") 在 lower: &str 上同样失败的语义同形
        // （Rust 标准库 `Path::new` 在 Linux 上原样保留字节；本机 Windows 路径均 UTF-8，
        //  但理论上 `OsStr` 可含非 UTF-8 字节，跨平台行为仍需契约保证）
        let path = std::path::Path::new("a/b/non_utf8.ÿþ");
        assert!(!extension_is(path, "md"));
    }

    #[test]
    fn test_extension_is_returns_false_for_missing_extension() {
        // 无扩展名的路径（Path::extension 返回 None）
        assert!(!extension_is(Path::new("a/b/README"), "md"));
        assert!(!extension_is(Path::new("Makefile"), "mk"));
        // 以点结尾但点后无字符也算「无扩展名」
        assert!(!extension_is(Path::new("a/b/.hidden"), "hidden"));
    }

    #[test]
    fn test_is_skipped_directory() {
        assert!(is_skipped_directory(Path::new("crates/core/target")));
        assert!(is_skipped_directory(Path::new("x/.git")));
        assert!(is_skipped_directory(Path::new("x/node_modules")));
        assert!(!is_skipped_directory(Path::new("crates/core/src")));
    }

    #[test]
    fn test_relative_display_path_uses_forward_slashes() {
        let root = Path::new("D:/repo");
        let file = root.join("xtask").join("src").join("main.rs");
        assert_eq!(relative_display_path(root, &file), "xtask/src/main.rs");
    }

    #[test]
    fn test_relative_display_path_falls_back_to_full_path_when_outside_root() {
        let root = Path::new("D:/repo");
        let outside = Path::new("C:/elsewhere/a.rs");
        assert_eq!(relative_display_path(root, outside), "C:/elsewhere/a.rs");
    }
}
