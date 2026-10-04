//! # `hygiene` 的文件 IO 辅助边界
//!
//! 职责：读取文本文件、Cargo manifest、依赖登记表与顶层目录白名单，并把失败包装成
//! `main::Failure`。判定逻辑仍在 `hygiene` / `duplicate_code` / `top_level_dirs`。
//!
//! ## 边界（不做什么）
//! - 不定义规则、不改写任何文件。
//! - 不静默跳过缺失的白名单：ADR-0069 的缺失 / 不可解析必须变成 Error Finding。
//!
//! 相关：`docs/adr/0068-cross-file-duplicate-code-hygiene.md`、
//! `docs/adr/0069-top-level-directory-adr-whitelist.md`

use std::collections::BTreeSet;
use std::path::Path;

use crate::Failure;
use crate::hygiene;
use crate::report::{Finding, Report, Severity};
use crate::repowalk::{
    RepoFileEntry, parse_cargo_dependency_names, parse_cargo_package_name,
    parse_registered_cargo_dependencies,
};
use crate::top_level_dirs;

/// 已读取的 Rust 源文件与单文件规则结果，供跨文件规则继续消费。
#[derive(Debug)]
pub struct RustHygieneData {
    /// 单文件 Rust 规则产生的发现项。
    pub findings: Vec<Finding>,
    /// `(相对路径, 源码文本)`，保持 `collect_rust_files` 的排序。
    pub sources: Vec<(String, String)>,
    /// 扫描到的函数总数。
    pub function_count: usize,
}

/// 读取 Rust 源文件并运行单文件 hygiene 规则。
pub fn collect_rust_hygiene_data(
    root: &Path,
    rust_files: &[std::path::PathBuf],
) -> Result<RustHygieneData, Failure> {
    let mut data = RustHygieneData {
        findings: Vec::new(),
        sources: Vec::new(),
        function_count: 0,
    };
    for file in rust_files {
        let source = std::fs::read_to_string(file)
            .map_err(|error| Failure::from_io(&format!("读取 {}", file.display()), &error))?;
        let relative = crate::repowalk::relative_display_path(root, file);
        data.function_count += hygiene::count_functions(&source);
        data.findings
            .extend(hygiene::check_rust_source(&relative, &source));
        data.sources.push((relative, source));
    }
    Ok(data)
}

/// 对「扫到 0 个对象」产生显式 Warning，避免空仓库得到虚假 PASSED。
pub fn append_zero_scan_warnings(
    report: &mut Report,
    root: &Path,
    rust_file_count: usize,
    text_file_count: usize,
    function_count: usize,
) {
    if rust_file_count == 0 {
        report.push(Finding::new(
            "xtask/no-source-files",
            Severity::Warning,
            "xtask",
            0,
            format!(
                "在 {} 下没有找到任何 .rs 文件（扫描根：{}）。PASSED 只代表没有代码可查，不代表代码合规。",
                root.display(),
                crate::repowalk::SCANNED_SOURCE_ROOTS.join(", ")
            ),
        ));
    }
    if text_file_count == 0 {
        report.push(Finding::new(
            "xtask/no-text-files",
            Severity::Warning,
            "xtask",
            0,
            "扫描集里没有任何 ADR-0025 文本文件；换行与依赖登记规则没有可判定对象。",
        ));
    }
    if function_count == 0 {
        report.push(Finding::new(
            "hygiene/no-functions-scanned",
            Severity::Warning,
            "xtask",
            0,
            "扫描集里没有任何函数；结构规则没有可判定对象，PASSED 不代表函数结构合规。",
        ));
    }
}

/// 对文本文件执行 CRLF 与末行换行规则；读取失败必须带路径向上失败。
pub fn collect_text_hygiene_findings(
    text_files: &[RepoFileEntry],
) -> Result<Vec<Finding>, Failure> {
    let mut findings = Vec::new();
    for file in text_files {
        let bytes = std::fs::read(&file.abs_path).map_err(|error| {
            Failure::from_io(&format!("读取 {}", file.abs_path.display()), &error)
        })?;
        findings.extend(hygiene::check_text_file_bytes(&file.rel_path, &bytes));
    }
    Ok(findings)
}

/// 双向比对 Cargo 直接依赖与 `docs/DEPENDENCIES.md`。
pub fn collect_dependency_hygiene_findings(
    root: &Path,
    text_files: &[RepoFileEntry],
) -> Result<Vec<Finding>, Failure> {
    let manifests: Vec<(&str, String)> = text_files
        .iter()
        .filter(|entry| {
            entry
                .abs_path
                .file_name()
                .and_then(std::ffi::OsStr::to_str)
                .is_some_and(|name| name == "Cargo.toml")
                && !["spikes/", "fixtures/", "tools/"]
                    .iter()
                    .any(|prefix| entry.rel_path.starts_with(prefix))
        })
        .map(|entry| {
            std::fs::read_to_string(&entry.abs_path)
                .map(|source| (entry.rel_path.as_str(), source))
                .map_err(|error| {
                    Failure::from_io(&format!("读取 {}", entry.abs_path.display()), &error)
                })
        })
        .collect::<Result<_, _>>()?;
    if manifests.is_empty() {
        return Ok(Vec::new());
    }

    let internal_names: BTreeSet<String> = manifests
        .iter()
        .filter_map(|(_path, source)| parse_cargo_package_name(source))
        .collect();
    let manifest_dependencies: Vec<(String, BTreeSet<String>)> = manifests
        .iter()
        .map(|(path, source)| {
            let mut dependencies = parse_cargo_dependency_names(source);
            dependencies.retain(|name| !internal_names.contains(name));
            ((*path).to_string(), dependencies)
        })
        .collect();

    let registry_path = root.join("docs").join("DEPENDENCIES.md");
    let registry_source = std::fs::read_to_string(&registry_path)
        .map_err(|error| Failure::from_io(&format!("读取 {}", registry_path.display()), &error))?;
    Ok(
        parse_registered_cargo_dependencies(&registry_source).map_or_else(
            || {
                vec![Finding::new(
                    "hygiene/dependency-registry-unparsable",
                    Severity::Error,
                    "docs/DEPENDENCIES.md",
                    0,
                    "缺少 `## Rust（cargo）` 或表格行不足九列".to_string(),
                )]
            },
            |registered| hygiene::check_dependency_registry(&manifest_dependencies, &registered),
        ),
    )
}

/// 对仓库根目录运行 ADR-0069 的顶层目录白名单规则。
pub fn collect_top_level_directory_hygiene_findings(root: &Path) -> Result<Vec<Finding>, Failure> {
    let directories = crate::repowalk::collect_top_level_directory_names(root)
        .map_err(|error| Failure::from_walk(&error))?;
    let whitelist_path = root.join(top_level_dirs::TOP_LEVEL_WHITELIST_PATH);
    let whitelist_source = match std::fs::read_to_string(&whitelist_path) {
        Ok(source) => source,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(vec![top_level_dirs::unparsable_whitelist_finding()]);
        }
        Err(error) => {
            return Err(Failure::from_io(
                &format!("读取 {}", whitelist_path.display()),
                &error,
            ));
        }
    };
    let Some(whitelist) = top_level_dirs::parse_top_level_whitelist(&whitelist_source) else {
        return Ok(vec![top_level_dirs::unparsable_whitelist_finding()]);
    };
    Ok(top_level_dirs::check_top_level_directories(
        &directories,
        &whitelist,
    ))
}
