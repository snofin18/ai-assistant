//! # 架构护栏检查（arch test）
//!
//! `xtask arch` 子命令（gov §5.1 门禁 #12 子编号 = arch test；TASK-015 实现 = 阶段性）。
//!
//! 职责：把"分层契约"机器化，挡住**违反方向**的内部依赖。
//!
//! ## 边界（不做什么）
//! - 不做语义级依赖图分析（如"`hygiene` 不应依赖 `card_check`"）= 仅按模块级分层规则判定。
//! - 不读 git 历史（仅看当前文件系统）。
//!
//! ## 不变量
//! 1. 零三方依赖：xtask/Cargo.toml [dependencies] 必须为空字符串（ADR-0021 D4 + ADR-0024 D4）。
//! 2. 模块分层：纯规则模块（`hygiene` / `memory_counts` / `adr_index` / `adr_registry` / `card_check` / `refscan` / `docscan` / `rustscan` / `memory_table` / `report` / `guard_model`）**不依赖** IO 模块（`main` / `repowalk` / `guard_store` / `doccheck`）。
//!    反向 = 允许（IO 模块可以调用纯规则）。
//! 3. 跨阶段：本规则是 `stage-0` 的「过渡版本」—— 当 `crates/core` / `crates/platform/*` 真正落地后，arch test 应扩展为「core 不能依赖 platform/windows」等真实分层（xv2 §5 / ADR-0019 元门禁）。
//!    **TASK-015 已把 `crates/core` 那一半落到 `crates/core/tests/arch_layering.rs`**（gov §5.1 #5），
//!    本文件继续负责 xtask 自身的分层 + 零三方依赖（gov §5.1 #12 子编号）。
//! 4. **无静默失败**（铁律 1，TASK-015 修 PL-048）：读不到 `xtask/Cargo.toml` 或读不到某个
//!    待检查的 `.rs` → `run` 必须返回 `Err`（退出码 4），不得像 2026-09-24 之前那样
//!    `let Ok(..) else { return findings }` 静默跳过 —— 那会让「读不到」与「没有依赖」不可区分
//!    （PL-048 的根因之一）。口径与 `memory-counts` / `adr-index` / `check-ledger` 一致：
//!    **读不到必需文件必须报错**，而不是静默得到一个 PASSED。
//!
//! 相关：`docs/governance-ai-agent-execution.md` §5.4、`cross-platform-ai-assistant-architecture-v2.md` §5、ADR-0019 元门禁

use crate::report::{Finding, Severity};

/// Arch 规则名常量。
const RULE_THIRD_PARTY_DEPS: &str = "arch/third-party-dependency";
const RULE_MODULE_LAYERING: &str = "arch/module-layering-violation";

/// 纯规则模块集合（不依赖 IO；引用层 = 0）。
/// 新增模块时**必须**在此表登记；否则会被 `check_module_layering` 误报。
const PURE_RULE_MODULES: &[&str] = &[
    "hygiene",
    "ledger_check",
    "memory_counts",
    "memory_table",
    "migration_registry",
    "adr_registry",
    "adr_index",
    "card_check",
    "refscan",
    "docscan",
    "rustscan",
    "report",
    "guard_model",
];

/// 一个待做分层检查的源文件（相对仓库根的 `/` 分隔路径 + 内容）。
///
/// 为什么把「内容」交给判定函数、而不是让它自己读盘（TASK-015 修 PL-048 的关键）：
/// 原来的写法是判定函数**自己读盘**，于是它的单测必须造临时目录、写文件 —— 而临时目录的
/// 唯一性只靠「进程 id + 时间戳」，两个测试并行撞进同一目录时互相覆盖，产生 5% 的假失败
/// （CI run 35964821459 实测）。把内容当参数传入后，单测**完全不需要文件系统**。
#[derive(Debug, Clone)]
pub struct SourceFile {
    /// 相对仓库根的路径（`/` 分隔）。
    pub rel_path: String,
    /// 文件内容。
    pub content: String,
}

/// 执行 arch 子命令：检查分层 + 零三方依赖。
pub fn run(repo_root: &std::path::Path, output: &mut dyn std::io::Write) -> Result<u8, String> {
    use crate::{EXIT_FINDINGS, EXIT_OK};

    let mut findings = Vec::new();

    // 1. xtask/Cargo.toml [dependencies] 必须为空（读不到 → Err，铁律 1）
    let manifest_path = repo_root.join("xtask/Cargo.toml");
    let manifest = std::fs::read_to_string(&manifest_path)
        .map_err(|error| format!("读 xtask/Cargo.toml 失败：{error}（零三方依赖规则无从判定）"))?;
    findings.extend(check_third_party_deps(&manifest));

    // 2. 跨模块 use 方向约束（IO 集中在 `collect_xtask_sources` 这一处）
    findings.extend(check_module_layering(&collect_xtask_sources(repo_root)?));

    let errors = findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .count();
    let warnings = findings
        .iter()
        .filter(|f| f.severity == Severity::Warning)
        .count();
    let verdict = if errors > 0 { "FAILED" } else { "PASSED" };
    // 输出每条 finding（与 refscan / card-check 一致 = 铁律 ① 无静默失败）
    if !findings.is_empty() {
        // 简版逐条输出：rule path:line message（避免依赖 refscan::render 私有 API）
        for f in &findings {
            writeln!(output, "{} {}:{} {}", f.rule, f.path, f.line, f.message)
                .map_err(|e| e.to_string())?;
        }
    }
    let summary = format!(
        "== arch ==\n-- summary: {errors} error(s), {warnings} warning(s)\n-- verdict: {verdict}\n"
    );
    output
        .write_all(summary.as_bytes())
        .map_err(|e| e.to_string())?;
    if errors > 0 {
        Ok(EXIT_FINDINGS)
    } else {
        Ok(EXIT_OK)
    }
}

/// 收集 `xtask/src/*.rs`（**IO 边界**：读盘只发生在这里）。
///
/// 只收 [`PURE_RULE_MODULES`] 里的模块，并跳过测试外置文件与 `main` / `cli` / `deferred`
/// （它们本来就允许依赖任何东西）。
fn collect_xtask_sources(repo_root: &std::path::Path) -> Result<Vec<SourceFile>, String> {
    use crate::repowalk::{collect_rust_files, relative_display_path};

    let files =
        collect_rust_files(repo_root).map_err(|error| format!("扫描仓库失败：{error:?}"))?;
    let mut sources = Vec::new();
    for path in files {
        let rel_path = relative_display_path(repo_root, &path);
        let Some(file_name) = rel_path
            .rsplit('/')
            .next()
            .and_then(|name| name.strip_suffix(".rs"))
        else {
            continue;
        };
        if file_name.ends_with("_tests") || matches!(file_name, "main" | "cli" | "deferred") {
            continue;
        }
        if !PURE_RULE_MODULES.contains(&file_name) {
            continue;
        }
        // 只关心 xtask 自己的模块（`collect_rust_files` 也会返回 crates/ 与 apps/ 下的文件）
        if !rel_path.starts_with("xtask/src/") {
            continue;
        }
        let content = std::fs::read_to_string(&path)
            .map_err(|error| format!("读 {rel_path} 失败：{error}"))?;
        sources.push(SourceFile { rel_path, content });
    }
    Ok(sources)
}

/// 检查 `xtask/Cargo.toml` 的 `[dependencies]` 节是否为空（ADR-0021 D4）。
///
/// 纯函数（内容由调用方读入）—— 见 [`SourceFile`] 的注释：这是 PL-048 的修法。
#[must_use]
pub fn check_third_party_deps(cargo_toml: &str) -> Vec<Finding> {
    let mut findings = Vec::new();
    // 找 [dependencies] 节，记录到下一个 [xxx] 节为止
    let mut in_deps = false;
    let mut dep_names: Vec<String> = Vec::new();
    for line in cargo_toml.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_deps = trimmed == "[dependencies]";
            continue;
        }
        if !in_deps {
            continue;
        }
        // 形如 "regex = "1.10"" 或 "regex = { version = "1.10" }"
        if let Some(eq) = trimmed.find('=') {
            let name = trimmed.get(..eq).unwrap_or("").trim().to_string();
            // 跳过注释行
            if !name.starts_with('#') {
                dep_names.push(name);
            }
        }
    }
    if !dep_names.is_empty() {
        findings.push(Finding::new(
            RULE_THIRD_PARTY_DEPS,
            Severity::Error,
            "xtask/Cargo.toml",
            0,
            format!(
                "xtask 必须零三方依赖（ADR-0021 D4），但 [dependencies] 含 {} 项：{}",
                dep_names.len(),
                dep_names.join(", ")
            ),
        ));
    }
    findings
}

/// 检查纯规则模块是否 `use crate::<IO 模块>`（分层方向被破坏）。
///
/// 纯函数：待检查的源码由 [`collect_xtask_sources`] 读入（PL-048 的修法）。
#[must_use]
pub fn check_module_layering(sources: &[SourceFile]) -> Vec<Finding> {
    let mut findings = Vec::new();
    for source in sources {
        let module_name = source
            .rel_path
            .rsplit('/')
            .next()
            .and_then(|name| name.strip_suffix(".rs"))
            .unwrap_or(&source.rel_path);
        // 检查 use crate::xxx::xxx 形式
        for (idx, line) in source.content.lines().enumerate() {
            let trimmed = line.trim();
            let Some(rest) = trimmed.strip_prefix("use crate::") else {
                continue;
            };
            let candidates = module_name_candidates(rest);
            let offending = candidates
                .iter()
                .map(|candidate| candidate.split("::").next().unwrap_or(""))
                .find(|first_segment| is_io_module(first_segment));
            if let Some(first_segment) = offending {
                findings.push(Finding::new(
                    RULE_MODULE_LAYERING,
                    Severity::Warning, // stage-0 known: card_check/refscan/docscan/guard_model/migration_registry -> stage-1 refactor
                    &source.rel_path,
                    idx + 1,
                    format!(
                        "纯规则模块 `{module_name}` 不应依赖 IO 模块 `{first_segment}`（分层违反）"
                    ),
                ));
            }
        }
    }
    findings
}

/// 从 `use crate::…` 去掉前缀后的剩余部分，取「一级模块名」候选。
///
/// - `use crate::repowalk::collect` → `[repowalk]`
/// - `use crate::{report, doccheck}` → `[report, doccheck]`（花括号形式必须**逐个看**，
///   否则 `use crate::{report, doccheck}` 会被漏掉 —— 2026-09-24 修 PL-048 时发现的假阴性）
///
/// 已知限制：跨多行的花括号导入（`use crate::{\n  doccheck,\n};`）看不见。
fn module_name_candidates(rest: &str) -> Vec<&str> {
    rest.strip_prefix('{').map_or_else(
        || vec![rest.split([',', ':', ' ', ';', '{']).next().unwrap_or("")],
        |inner| {
            inner
                .split('}')
                .next()
                .unwrap_or("")
                .split(',')
                .map(str::trim)
                .collect()
        },
    )
}

/// 一级模块名是否属于 IO 模块（被纯规则模块**不应**依赖）。
fn is_io_module(name: &str) -> bool {
    matches!(name, "main" | "repowalk" | "guard_store" | "doccheck")
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::uninlined_format_args
)]
mod tests {
    use super::*;

    fn source(rel_path: &str, content: &str) -> SourceFile {
        SourceFile {
            rel_path: rel_path.to_string(),
            content: content.to_string(),
        }
    }

    #[test]
    fn cargo_toml_with_deps_detected() {
        // 纯函数：不再需要临时目录（PL-048 的修法）—— 两个镜像测试也不会再互相覆盖
        let f = check_third_party_deps(
            "[package]\nname = \"x\"\n[dependencies]\nregex = \"1.0\"\nserde = \"1.0\"\n",
        );
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].rule, RULE_THIRD_PARTY_DEPS);
        assert!(f[0].message.contains("regex"));
        assert!(f[0].message.contains("serde"));
    }

    #[test]
    fn cargo_toml_without_deps_passes() {
        let f = check_third_party_deps(
            "[package]\nname = \"x\"\n[dependencies]\n# (intentionally empty)\n",
        );
        assert!(f.is_empty(), "got {:?}", f);
    }

    #[test]
    fn pure_module_importing_io_detected() {
        let f = check_module_layering(&[source(
            "xtask/src/hygiene.rs",
            "use std::fs;\nuse crate::repowalk::collect_repo_files;\n",
        )]);
        assert_eq!(f.len(), 1, "got {:?}", f);
        assert_eq!(f[0].rule, RULE_MODULE_LAYERING);
        assert!(f[0].message.contains("repowalk"));
    }

    #[test]
    fn pure_module_with_pure_imports_passes() {
        let f = check_module_layering(&[source(
            "xtask/src/hygiene.rs",
            "use crate::report::Finding;\nuse crate::memory_table::MemoryRow;\n",
        )]);
        assert!(f.is_empty(), "got {:?}", f);
    }

    #[test]
    fn is_io_module_recognizes_correctly() {
        assert!(is_io_module("main"));
        assert!(is_io_module("repowalk"));
        assert!(is_io_module("guard_store"));
        assert!(is_io_module("doccheck"));
        assert!(!is_io_module("hygiene"));
        assert!(!is_io_module("memory_counts"));
    }

    #[test]
    fn brace_import_of_io_module_is_detected() {
        let f = check_module_layering(&[source(
            "xtask/src/report.rs",
            "use crate::{doccheck, report};\n",
        )]);
        assert_eq!(f.len(), 1, "花括号形式的导入也必须被看见：{f:?}");
    }
}
