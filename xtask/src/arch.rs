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
//! 3. 跨阶段：本规则是 `stage-0` 的「过渡版本」—— 当 `crates/core` / `crates/platform/*` 真正落地后，
//!    arch test 应扩展为「core 不能依赖 platform/windows」等真实分层（xv2 §5 / ADR-0019 元门禁）。
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
    "memory_counts",
    "memory_table",
    "adr_registry",
    "adr_index",
    "card_check",
    "refscan",
    "docscan",
    "rustscan",
    "report",
    "guard_model",
];

/// 执行 arch 子命令：检查分层 + 零三方依赖。
pub fn run(repo_root: &std::path::Path, output: &mut dyn std::io::Write) -> Result<u8, String> {
    use crate::report::Severity;
    use crate::{EXIT_FINDINGS, EXIT_OK};

    let mut findings = Vec::new();
    // 1. xtask/Cargo.toml [dependencies] 必须为空
    findings.extend(check_third_party_deps(repo_root));
    // 2. 跨模块 use 方向约束
    findings.extend(check_module_layering(repo_root));

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

/// 检查 xtask/Cargo.toml 的 [dependencies] 节是否为空（ADR-0021 D4）。
fn check_third_party_deps(repo_root: &std::path::Path) -> Vec<Finding> {
    let mut findings = Vec::new();
    let cargo_toml = repo_root.join("xtask/Cargo.toml");
    let Ok(content) = std::fs::read_to_string(&cargo_toml) else {
        return findings;
    };
    // 找 [dependencies] 节，记录到下一个 [xxx] 节为止
    let mut in_deps = false;
    let mut dep_names: Vec<String> = Vec::new();
    for line in content.lines() {
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
            let name = trimmed[..eq].trim().to_string();
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

/// 检查 xtask/src/*.rs 的 `use crate::...` 方向：纯规则模块**不能依赖** IO 模块。
/// 用 `repowalk` 收集源文件 + 手动 grep（零三方依赖）。
fn check_module_layering(repo_root: &std::path::Path) -> Vec<Finding> {
    let mut findings = Vec::new();
    let Ok(entries) = crate::repowalk::collect_repo_files(repo_root, &["rs"]) else {
        return findings;
    };
    for entry in &entries {
        if !entry.rel_path.starts_with("xtask/src/") {
            continue;
        }
        let file_name = std::path::Path::new(&entry.rel_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        // 跳过测试文件与 main 入口
        if file_name.ends_with("_tests") || file_name == "main" || file_name == "cli" || file_name == "deferred" {
            continue;
        }
        if !PURE_RULE_MODULES.contains(&file_name) {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(&entry.abs_path) else {
            continue;
        };
        // 检查 use crate::xxx::xxx 形式
        for (idx, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            if !trimmed.starts_with("use ") {
                continue;
            }
            // 形如 "use crate::repowalk::..." 或 "use crate::{guard_store, doccheck}"
            if let Some(rest) = trimmed.strip_prefix("use crate::") {
                let module_path = rest.split([',', ':', ' ', ';', '{']).next().unwrap_or("");
                // 找一级模块名
                let first_segment = module_path.split("::").next().unwrap_or("");
                if is_io_module(first_segment) {
                    findings.push(Finding::new(
                        RULE_MODULE_LAYERING,
                        Severity::Warning, // stage-0 known: card_check/refscan/docscan/guard_model -> stage-1 refactor
                        &entry.rel_path,
                        idx + 1,
                        format!("纯规则模块 `{file_name}` 不应依赖 IO 模块 `{first_segment}`（分层违反）"),
                    ));
                }
            }
        }
    }
    findings
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

    #[test]
    fn cargo_toml_with_deps_detected() {
        let tmp = tempdir();
        std::fs::create_dir_all(tmp.join("xtask")).unwrap();
        std::fs::write(
            tmp.join("xtask/Cargo.toml"),
            "[package]\nname = \"x\"\n[dependencies]\nregex = \"1.0\"\nserde = \"1.0\"\n",
        )
        .unwrap();
        let f = check_third_party_deps(&tmp);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].rule, RULE_THIRD_PARTY_DEPS);
        assert!(f[0].message.contains("regex"));
        assert!(f[0].message.contains("serde"));
    }

    #[test]
    fn cargo_toml_without_deps_passes() {
        let tmp = tempdir();
        std::fs::create_dir_all(tmp.join("xtask")).unwrap();
        std::fs::write(
            tmp.join("xtask/Cargo.toml"),
            "[package]\nname = \"x\"\n[dependencies]\n# (intentionally empty)\n",
        )
        .unwrap();
        let f = check_third_party_deps(&tmp);
        assert!(f.is_empty(), "got {:?}", f);
    }

    #[test]
    fn pure_module_importing_io_detected() {
        let tmp = tempdir();
        std::fs::create_dir_all(tmp.join("xtask/src")).unwrap();
        std::fs::write(
            tmp.join("xtask/src/hygiene.rs"),
            "use std::fs;\nuse crate::repowalk::collect_repo_files;\n",
        )
        .unwrap();
        let f = check_module_layering(&tmp);
        assert_eq!(f.len(), 1, "got {:?}", f);
        assert_eq!(f[0].rule, RULE_MODULE_LAYERING);
        assert!(f[0].message.contains("repowalk"));
    }

    #[test]
    fn pure_module_with_pure_imports_passes() {
        let tmp = tempdir();
        std::fs::create_dir_all(tmp.join("xtask/src")).unwrap();
        std::fs::write(
            tmp.join("xtask/src/hygiene.rs"),
            "use crate::report::Finding;\nuse crate::memory_table::MemoryRow;\n",
        )
        .unwrap();
        let f = check_module_layering(&tmp);
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

    fn tempdir() -> std::path::PathBuf {
        let base = std::env::temp_dir();
        let unique = format!(
            "xtask-arch-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0_u128, |d| d.as_nanos())
        );
        let p = base.join(unique);
        std::fs::create_dir_all(&p).unwrap();
        p
    }
}
