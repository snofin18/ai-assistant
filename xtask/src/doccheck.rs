//! # 文档一致性子命令的 IO 层（`memory-counts` 与 `adr-index`）
//!
//! 职责：把磁盘上的文件读进来、把 `docs/memory/` 与 `docs/adr/` 的目录列出来，交给
//! `memory_counts` / `adr_index` 的纯函数判定，再用 `report` 渲染并决定退出码。
//!
//! 本 crate 的分层规矩是「规则判定一律纯函数，IO 集中在边界」（`xtask/Cargo.toml` 不变量 2），
//! 本文件就是这两个子命令的**边界**。
//!
//! ## 为什么两个子命令共用一个文件
//! 它们的 IO 形状完全一样（读若干 md → 调纯函数 → 渲染 Report），分开写会复制三遍同样的样板。
//! 判定逻辑仍然是两个独立模块，互不依赖。
//!
//! ## 边界（不做什么）
//! - 不做判定：计数与编号一致性的规则在 `memory_counts.rs` / `adr_index.rs`。
//! - 不改写任何文件：发现不一致时**打印正确值让人粘贴**（ADR-0030 选项 1 已否决自动改写）。
//! - 不访问网络。
//!
//! ## 不变量
//! 1. 读不到必需文件（`MEMORY.md` / `docs/adr/README.md` / `docs/memory/decisions.md`）
//!    必须返回 `Err`，**不得**当成"空内容"继续（那会得到一个毫无意义的 PASSED，铁律 1）。
//! 2. 扫到 0 个记忆文件时必须显式告警：与 `main.rs` 的 hygiene 不变量 4 同理，
//!    "什么都没扫到"和"扫了都没问题"必须可区分。
//! 3. `docs/memory/` 的发现是**递归**的，排除 `README.md` 与 `archive/`（归档区按定义不再计入
//!    当前规模）。新增档案不需要改代码，漏登记会被 `memory/file-unlisted` 抓到。
//!
//! 相关：`docs/adr/0030-machine-verified-memory-counts-and-adr-index.md`、`.github/workflows/ci.yml` 的 `[HARD #12b]`

use std::io::Write;
use std::path::{Path, PathBuf};

use crate::adr_index;
use crate::adr_registry::{
    ADR_DIR, DECISIONS_PATH, REGISTRY_PATH, file_number, summarize_adr_file,
};
use crate::memory_counts;
use crate::memory_table::{INDEX_PATH, MEMORY_DIR, MeasuredFile, count_entries, count_lines};
use crate::report::{Finding, Report, Severity};
use crate::{EXIT_FINDINGS, EXIT_OK};

/// 记忆文件的发现规则：这些名字不计入「当前规模」。
const MEMORY_FILE_EXCLUSIONS: [&str; 1] = ["README.md"];

/// 记忆文件的发现规则：这些子目录不计入「当前规模」（归档区按定义已不是"当前"）。
const MEMORY_DIR_EXCLUSIONS: [&str; 1] = ["archive"];

/// Markdown 扩展名（小写形式）。
///
/// 为什么抽成常量而不是就地写 `".md"`：① 两处判定（ADR 目录、记忆目录）必须同口径；
/// ② clippy 的 `case_sensitive_file_extension_comparison` 要求扩展名比较大小写无关 ——
/// Windows/macOS 文件系统里 `.MD` 与 `.md` 是同一类文件，漏掉它们会让统计少算。
const MARKDOWN_EXTENSION: &str = ".md";

/// 文件名是否是 Markdown 文件（大小写无关）。
fn is_markdown_file(file_name: &str) -> bool {
    file_name.to_ascii_lowercase().ends_with(MARKDOWN_EXTENSION)
}

/// 执行 `memory-counts`，返回退出码。
///
/// # Errors
/// `MEMORY.md` 读不到、`docs/memory/` 不可列、或写输出失败时返回原因。
pub fn run_memory_counts(repo_root: &Path, output: &mut dyn Write) -> Result<u8, String> {
    let index_path = repo_root.join(path_from_repo_relative(INDEX_PATH));
    let index_content = read_required_file(&index_path)?;
    let measured = measure_memory_files(repo_root)?;

    let mut report = Report::new("memory-counts");
    report.scanned_files = measured.len() + 1; // +1 = MEMORY.md 自身
    if measured.is_empty() {
        // 不变量 2：0 个文件时 PASSED 是假信号
        report.push(Finding::new(
            "memory/no-files-scanned",
            Severity::Warning,
            MEMORY_DIR,
            0,
            format!(
                "在 {MEMORY_DIR}/ 下没有找到任何记忆文件。PASSED 只代表没有东西可比对，\
                 不代表计数是对的（不变量 2）。"
            ),
        ));
    }
    let mut findings = memory_counts::check(&index_content, &measured);
    findings.sort_by(|left, right| {
        (&left.path, left.line, left.rule).cmp(&(&right.path, right.line, right.rule))
    });
    report.extend(findings);
    render(&report, output)
}

/// 执行 `adr-index`，返回退出码。
///
/// # Errors
/// 登记表 / `decisions.md` 读不到、`docs/adr/` 不可列、或写输出失败时返回原因。
pub fn run_adr_index(repo_root: &Path, output: &mut dyn Write) -> Result<u8, String> {
    let registry_content =
        read_required_file(&repo_root.join(path_from_repo_relative(REGISTRY_PATH)))?;
    let decisions_content =
        read_required_file(&repo_root.join(path_from_repo_relative(DECISIONS_PATH)))?;
    let adr_files = summarize_adr_files(repo_root)?;

    let mut report = Report::new("adr-index");
    report.scanned_files = adr_files.len() + 2; // +2 = 登记表与 decisions.md
    let mut findings = adr_index::check(&registry_content, &adr_files, &decisions_content);
    findings.sort_by(|left, right| {
        (&left.path, left.line, left.rule).cmp(&(&right.path, right.line, right.rule))
    });
    report.extend(findings);
    render(&report, output)
}

/// 汇总 `docs/adr/NNNN-*.md`；文件名里没有 4 位编号的（如 `README.md`）自动排除。
fn summarize_adr_files(
    repo_root: &Path,
) -> Result<Vec<crate::adr_registry::AdrFileSummary>, String> {
    let directory = repo_root.join(path_from_repo_relative(ADR_DIR));
    let mut summaries = Vec::new();
    for entry in list_directory(&directory)? {
        let file_name = entry.file_name().to_string_lossy().to_string();
        if file_number(&file_name).is_none() || !is_markdown_file(&file_name) {
            continue;
        }
        let content = std::fs::read_to_string(entry.path())
            .map_err(|error| format!("读取 {} 失败：{error}", entry.path().display()))?;
        if let Some(summary) = summarize_adr_file(&file_name, &content) {
            summaries.push(summary);
        }
    }
    // 输出确定性：按编号排序（BTreeMap 不适用于结构体，这里显式排）
    summaries.sort_by_key(|summary| summary.number);
    Ok(summaries)
}

/// 递归测量 `docs/memory/` 下的记忆文件（不变量 3 的发现规则）。
fn measure_memory_files(repo_root: &Path) -> Result<Vec<MeasuredFile>, String> {
    let root = repo_root.join(path_from_repo_relative(MEMORY_DIR));
    let mut measured = Vec::new();
    collect_memory_files(&root, &root, &mut measured)?;
    // 输出确定性：按相对路径排序
    measured.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(measured)
}

/// 递归收集记忆文件；`base` 用于算出相对 `docs/memory/` 的路径（与规模表第 1 列同口径）。
fn collect_memory_files(
    base: &Path,
    directory: &Path,
    measured: &mut Vec<MeasuredFile>,
) -> Result<(), String> {
    if !directory.is_dir() {
        // 目录不存在不是错误：由 MEMORY.md 的规模表登记项去报 `memory/file-missing`
        return Ok(());
    }
    for entry in list_directory(directory)? {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            if MEMORY_DIR_EXCLUSIONS.contains(&name.as_str()) {
                continue;
            }
            collect_memory_files(base, &path, measured)?;
            continue;
        }
        if !is_markdown_file(&name) || MEMORY_FILE_EXCLUSIONS.contains(&name.as_str()) {
            continue;
        }
        let content = std::fs::read_to_string(&path)
            .map_err(|error| format!("读取 {} 失败：{error}", path.display()))?;
        measured.push(MeasuredFile {
            relative_path: relative_to(base, &path),
            line_count: count_lines(&content),
            entry_count: count_entries(&content),
        });
    }
    Ok(())
}

/// 算出相对 `base` 的路径，统一用 `/`（跨平台输出一致）。
fn relative_to(base: &Path, path: &Path) -> String {
    path.strip_prefix(base)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// 把仓库相对路径（统一用 `/`）转成本机路径。
fn path_from_repo_relative(relative: &str) -> PathBuf {
    relative
        .split('/')
        .fold(PathBuf::new(), |accumulated, segment| {
            accumulated.join(segment)
        })
}

/// 读一个**必须存在**的文件（不变量 1）。
///
/// # Errors
/// 读不到时返回带路径的原因。
fn read_required_file(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|error| format!("读取 {} 失败：{error}", path.display()))
}

/// 列目录。
///
/// # Errors
/// 目录不可读时返回原因。
fn list_directory(directory: &Path) -> Result<Vec<std::fs::DirEntry>, String> {
    match std::fs::read_dir(directory) {
        Ok(entries) => {
            let mut collected = Vec::new();
            for entry in entries {
                collected.push(entry.map_err(|error| {
                    format!("读取 {} 的目录项失败：{error}", directory.display())
                })?);
            }
            Ok(collected)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(format!("列目录 {} 失败：{error}", directory.display())),
    }
}

/// 渲染报告并给出退出码（与 `main.rs::run_hygiene` 同样的两段式输出）。
fn render(report: &Report, output: &mut dyn Write) -> Result<u8, String> {
    let summary = report
        .summary_line()
        .map_err(|error| format!("生成报告摘要失败：{error}"))?;
    writeln!(output, "-- machine-summary: {summary}").map_err(|error| error.to_string())?;
    writeln!(
        output,
        "-- 说明：本工具**只读**，不会自动改写文档；发现项的消息里给出了可直接粘贴的正确值（ADR-0030 选项 1）。"
    )
    .map_err(|error| error.to_string())?;
    report.render(output).map_err(|error| error.to_string())?;
    Ok(if report.is_failure() {
        EXIT_FINDINGS
    } else {
        EXIT_OK
    })
}
