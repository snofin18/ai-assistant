//! # check-migrations 子命令（PL-047 的机器化：迁移登记表 ↔ 迁移文件 ↔ `MIGRATIONS` 常量）
//!
//! 职责：把 ADR-0038 D4 的「`docs/storage-design.md` §3.4 = 版本号分配的 SSOT」变成**机器判据**。
//!
//! ## 为什么需要它（PL-047 的原话）
//! 装配时 `MigrationSet::register()` / `validate()` 只拦得住「**已经装配进集合**的重号与缺号」——
//! **拦不住「写了迁移文件却忘了在 §3.4 占号」**。那正是 ADR-0030 的教训：
//! 「靠记得回填的护栏会失效」。§3.4 自己写着「本表仍是**手工回填**的」。
//!
//! ## 三条判据（PL-047 原文的三条）
//! | 规则 id | 级别 | 判据 |
//! |---|---|---|
//! | `migration/version-duplicate` | Error | `crates/*/migrations/NNNN_*.sql` 的 **4 位号段全局唯一** |
//! | `migration/registry-mismatch` | Error | 迁移文件集合与 §3.4 登记表**逐行一致**（双向：漏登记 / 多登记 / 路径不符） |
//! | `migration/crate-missing-const` | Error | 每个**含迁移**的 crate 都必须公开 `pub const MIGRATIONS` |
//!
//! 三条都是 **Error**：它们描述的是客观事实（文件在不在、号段重不重），没有「豁免」可豁；
//! 且上线时仓库是绿的（0001/0002/0003 三条都在 §3.4 里，`crates/storage` 与 `crates/audit` 都公开了
//! `MIGRATIONS`）→ 按 ADR-0025 D1 可直接 Error。
//!
//! ## 边界（不做什么）
//! - **不解析 SQL**：只比对「文件/号段/常量」三件事；DDL 内容正确性归各 crate 自己的测试。
//! - 不改任何文件（ADR-0030 选项 1 已否决自动写回：发现不一致时打印可粘贴的正确值）。
//! - 不校验 §3.4 的「拥有者 crate」列（那是路径的一致性；路径一致时它必然一致）。
//!
//! ## 不变量
//! 1. **无静默失败**（铁律 1）：读不到 `docs/storage-design.md`、找不到 §3.4 登记表、
//!    或表里**一行都解析不出** → 必须 `Err`（退出码 4），不得当成「没有迁移 → PASSED」。
//! 2. 输出确定性：同一仓库状态两次运行输出逐字节相同（文件列表由 `repowalk` 排序保证）。
//!
//! ## 与 CI 的关系
//! 本子命令**尚未**接进 `.github/workflows/ci.yml`（改 CI 门禁清单 = 改 gov §5.1，
//! 需要 ADR 与人类批准）→ 见 `docs/PARKING_LOT.md` **PL-056**。
//!
//! 相关：`docs/storage-design.md` §3.4、ADR-0038 D4、ADR-0030、`docs/PARKING_LOT.md` PL-047。

use std::path::Path;

use crate::report::{Finding, Severity};

/// 规则 `migration/version-duplicate`：4 位号段在全部 `crates/*/migrations/*.sql` 里重复。
const RULE_VERSION_DUPLICATE: &str = "migration/version-duplicate";
/// 规则 `migration/registry-mismatch`：迁移文件集合与 §3.4 登记表不一致。
const RULE_REGISTRY_MISMATCH: &str = "migration/registry-mismatch";
/// 规则 `migration/crate-missing-const`：含迁移的 crate 没有公开 `pub const MIGRATIONS`。
const RULE_CRATE_MISSING_CONST: &str = "migration/crate-missing-const";

/// 登记表所在的文档与节标题（SSOT 位置，改动需 ADR —— 见 ADR-0038 D4）。
const STORAGE_DESIGN_PATH: &str = "docs/storage-design.md";
/// §3.4 的节标题前缀（含尾空格，避免误匹配 `### 3.40`）。
const REGISTRY_SECTION_PREFIX: &str = "### 3.4 ";
/// 迁移文件的目录形态：`crates/<owner>/migrations/`。
const MIGRATIONS_DIRECTORY: &str = "/migrations/";
/// 每个含迁移的 crate 必须公开的常量名。
const MIGRATIONS_CONST: &str = "pub const MIGRATIONS";

/// 一个被发现的迁移文件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationFile {
    /// 仓库相对路径（`/` 分隔）。
    pub rel_path: String,
    /// 4 位号段（`0001` / `0002` / `0003`）。
    pub version: String,
}

/// 执行 `check-migrations`：收集迁移文件 + 解析 §3.4 → 跑三条规则 → 渲染。
///
/// 错误语义：读不到 `docs/storage-design.md`、找不到 §3.4、或登记表解析出 0 行 → `Err`（铁律 1）。
pub fn run(repo_root: &Path, output: &mut dyn std::io::Write) -> Result<u8, String> {
    use crate::repowalk::collect_repo_files;
    use crate::{EXIT_FINDINGS, EXIT_OK};

    // ① 实际存在的迁移文件（`repowalk` 只扫 workspace 成员目录，且已排序 → 确定性）
    let entries = collect_repo_files(repo_root, &["sql"])
        .map_err(|error| format!("扫描仓库失败：{error:?}"))?;
    let mut files: Vec<MigrationFile> = Vec::new();
    for entry in &entries {
        let Some(version) = migration_file_version(&entry.rel_path) else {
            continue;
        };
        files.push(MigrationFile {
            rel_path: entry.rel_path.clone(),
            version,
        });
    }

    // ② §3.4 登记表
    let design = std::fs::read_to_string(repo_root.join(STORAGE_DESIGN_PATH))
        .map_err(|error| format!("读 {STORAGE_DESIGN_PATH} 失败：{error}"))?;
    let registry_section = extract_registry_section(&design).ok_or_else(|| {
        format!("{STORAGE_DESIGN_PATH} 里找不到 `{REGISTRY_SECTION_PREFIX}` 节（登记表无法解析）")
    })?;
    let registry = parse_registry_entries(&registry_section);
    if registry.is_empty() {
        return Err(format!(
            "{STORAGE_DESIGN_PATH} 的 §3.4 里一行迁移登记都解析不出（铁律 1：解析不到必须报错，不得静默通过）"
        ));
    }

    // ③ 每个含迁移的 crate 是否公开 `pub const MIGRATIONS`
    let rust_entries = collect_repo_files(repo_root, &["rs"])
        .map_err(|error| format!("扫描仓库失败：{error:?}"))?;
    let mut crate_names: Vec<String> = files
        .iter()
        .filter_map(|file| crate_of_migration_path(&file.rel_path))
        .collect();
    crate_names.sort_unstable();
    crate_names.dedup();

    let mut findings = check_version_uniqueness(&files);
    findings.extend(check_registry_match(&files, &registry));
    for crate_name in &crate_names {
        let has_const = crate_source_exposes_migrations(repo_root, &rust_entries, crate_name);
        findings.extend(check_crate_exposes_migrations(crate_name, has_const));
    }
    // 排序保证输出确定性（report.rs 不变量 3）
    findings.sort_by(|left, right| {
        (&left.path, left.line, left.rule).cmp(&(&right.path, right.line, right.rule))
    });

    let errors = findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .count();
    let warnings = findings
        .iter()
        .filter(|f| f.severity == Severity::Warning)
        .count();
    let verdict = if errors > 0 { "FAILED" } else { "PASSED" };
    for finding in &findings {
        finding.render(output).map_err(|e| e.to_string())?;
    }
    let summary = format!(
        "== check-migrations ==\nscanned_migration_files={}\nregistry_entries={}\ncrates_with_migrations={}\n-- summary: {errors} error(s), {warnings} warning(s)\n-- verdict: {verdict}\n",
        files.len(),
        registry.len(),
        crate_names.len()
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

/// 规则 ①：4 位号段全局唯一。每个重复号段报一条，列出全部占用者。
#[must_use]
pub fn check_version_uniqueness(files: &[MigrationFile]) -> Vec<Finding> {
    use std::collections::BTreeMap;
    let mut by_version: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for file in files {
        by_version
            .entry(file.version.as_str())
            .or_default()
            .push(file.rel_path.as_str());
    }
    let mut findings = Vec::new();
    for (version, paths) in by_version {
        if paths.len() <= 1 {
            continue;
        }
        // 第二个及以后的每个文件各报一条（行号 0 = 文件级）
        for path in paths.iter().skip(1) {
            findings.push(Finding::new(
                RULE_VERSION_DUPLICATE,
                Severity::Error,
                (*path).to_string(),
                0,
                format!(
                    "号段 `{version}` 被 {} 个迁移文件占用：{} —— 版本号全局唯一、从 1 连续（ADR-0038 D4 / §3.4）",
                    paths.len(),
                    paths.join(", ")
                ),
            ));
        }
    }
    findings
}

/// 规则 ②：迁移文件集合与 §3.4 登记表逐行一致（双向）。
#[must_use]
pub fn check_registry_match(
    files: &[MigrationFile],
    registry: &[(String, String)],
) -> Vec<Finding> {
    use std::collections::BTreeMap;
    let registry_by_version: BTreeMap<&str, &str> = registry
        .iter()
        .map(|(version, path)| (version.as_str(), path.as_str()))
        .collect();
    let mut findings = Vec::new();

    // 方向 A：实际文件必须在登记表里，且路径一致
    for file in files {
        match registry_by_version.get(file.version.as_str()) {
            None => findings.push(Finding::new(
                RULE_REGISTRY_MISMATCH,
                Severity::Error,
                file.rel_path.clone(),
                0,
                format!(
                    "号段 `{}` 未在 `{STORAGE_DESIGN_PATH}` §3.4 登记（ADR-0038 D4：新增迁移**先在本表占号**再写 SQL）",
                    file.version
                ),
            )),
            Some(registered_path) => {
                if *registered_path != file.rel_path {
                    findings.push(Finding::new(
                        RULE_REGISTRY_MISMATCH,
                        Severity::Error,
                        file.rel_path.clone(),
                        0,
                        format!(
                            "号段 `{}` 的路径与 §3.4 登记表不一致：登记表写 `{registered_path}`，实际文件是 `{}`",
                            file.version, file.rel_path
                        ),
                    ));
                }
            }
        }
    }

    // 方向 B：登记表里的每一行都必须有对应文件
    let actual_versions: BTreeMap<&str, &str> = files
        .iter()
        .map(|file| (file.version.as_str(), file.rel_path.as_str()))
        .collect();
    for (version, path) in registry {
        if !actual_versions.contains_key(version.as_str()) {
            findings.push(Finding::new(
                RULE_REGISTRY_MISMATCH,
                Severity::Error,
                STORAGE_DESIGN_PATH,
                0,
                format!(
                    "§3.4 登记了号段 `{version}`（`{path}`），但磁盘上找不到该迁移文件 —— 登记表与事实脱钩（PL-047 的原始症状）"
                ),
            ));
        }
    }
    findings
}

/// 规则 ③：含迁移的 crate 必须公开 `pub const MIGRATIONS`。
#[must_use]
pub fn check_crate_exposes_migrations(crate_name: &str, has_const: bool) -> Vec<Finding> {
    if has_const {
        return Vec::new();
    }
    vec![Finding::new(
        RULE_CRATE_MISSING_CONST,
        Severity::Error,
        format!("crates/{crate_name}/src"),
        0,
        format!(
            "`crates/{crate_name}` 有迁移文件，但它的 `src/**/*.rs` 里找不到 `{MIGRATIONS_CONST}` —— 迁移无法被装配点（ADR-0038 D3）合并"
        ),
    )]
}

/// 从仓库相对路径取 4 位号段：`crates/audit/migrations/0002_x.sql` → `Some("0002")`。
///
/// 只认「位于 `crates/<owner>/migrations/` 下、文件名以 4 位数字 + `_` 开头」的 `.sql`；
/// 其他 `.sql`（如将来某个 crate 的测试 fixture）返回 `None`，不参与号段判定。
#[must_use]
pub fn migration_file_version(rel_path: &str) -> Option<String> {
    if !rel_path.contains(MIGRATIONS_DIRECTORY) || !has_sql_extension(rel_path) {
        return None;
    }
    let file_name = rel_path.rsplit('/').next()?;
    let prefix = file_name.get(..4)?;
    if !prefix.chars().all(|character| character.is_ascii_digit()) {
        return None;
    }
    if file_name.as_bytes().get(4) != Some(&b'_') {
        return None;
    }
    Some(prefix.to_string())
}

/// 路径是否以 `.sql` 结尾（大小写不敏感：clippy 的
/// `case_sensitive_file_extension_comparisons` 要求用 `Path::extension`）。
fn has_sql_extension(path: &str) -> bool {
    std::path::Path::new(path)
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("sql"))
}

/// 从仓库相对路径取拥有者 crate 名：`crates/audit/migrations/0002_x.sql` → `Some("audit")`。
#[must_use]
pub fn crate_of_migration_path(rel_path: &str) -> Option<String> {
    let (prefix, _) = rel_path.split_once(MIGRATIONS_DIRECTORY)?;
    let crate_name = prefix.rsplit('/').next()?;
    if crate_name.is_empty() {
        None
    } else {
        Some(crate_name.to_string())
    }
}

/// 截出 `docs/storage-design.md` 的 §3.4 节（标题行到下一个 `##`/`###` 标题之前）。
///
/// 返回 `String`（而不是 `&str`）：节是**跨行**的，必须重新拼接；返回借用需要把拼接结果
/// 存到别处，反而更绕。调用方只做逐行解析，拷贝一次的开销可忽略。
#[must_use]
pub fn extract_registry_section(markdown: &str) -> Option<String> {
    let lines: Vec<&str> = markdown.lines().collect();
    let start = lines
        .iter()
        .position(|line| line.starts_with(REGISTRY_SECTION_PREFIX))?;
    let mut end = lines.len();
    for (offset, line) in lines.iter().enumerate().skip(start + 1) {
        if line.starts_with("## ") || line.starts_with("### ") {
            end = offset;
            break;
        }
    }
    Some(lines.get(start..end)?.join("\n"))
}

/// 解析登记表的表格行，返回（号段, 路径）。
///
/// 只认「第一格是 4 位数字」且「第三格是 `…/migrations/….sql`」的行 ——
/// 表头（`| 版本 | 拥有者 crate | 迁移文件 | … |`）与说明文字自然被跳过。
#[must_use]
pub fn parse_registry_entries(section: &str) -> Vec<(String, String)> {
    let mut entries = Vec::new();
    for line in section.lines() {
        let trimmed = line.trim_start();
        if !trimmed.starts_with('|') {
            continue;
        }
        let cells: Vec<&str> = trimmed
            .trim_matches('|')
            .split('|')
            .map(str::trim)
            .collect();
        let Some(version_cell) = cells.first() else {
            continue;
        };
        if version_cell.len() != 4
            || !version_cell
                .chars()
                .all(|character| character.is_ascii_digit())
        {
            continue;
        }
        let Some(path_cell) = cells.get(2) else {
            continue;
        };
        let path = path_cell.trim_matches('`').trim();
        if !path.contains(MIGRATIONS_DIRECTORY) || !has_sql_extension(path) {
            continue;
        }
        entries.push(((*version_cell).to_string(), path.to_string()));
    }
    entries
}

/// 该 crate 的 `src/**/*.rs` 里是否存在 `pub const MIGRATIONS`。
///
/// 只扫 `crates/<name>/src/` 下的文件：`pub const` 可以写在 `lib.rs`，也可以写在子模块里
/// 再由 `pub use` 重导出（`crates/storage` 就是后者 —— 常量在 `schema.rs`，`lib.rs` 重导出）。
fn crate_source_exposes_migrations(
    repo_root: &Path,
    rust_entries: &[crate::repowalk::RepoFileEntry],
    crate_name: &str,
) -> bool {
    let prefix = format!("crates/{crate_name}/src/");
    for entry in rust_entries {
        if !entry.rel_path.starts_with(&prefix) {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(repo_root.join(&entry.rel_path)) else {
            // 读不到源码 = 无法证明常量存在 → 保守判为「不存在」（报红灯而不是静默放过）
            continue;
        };
        if content.contains(MIGRATIONS_CONST) {
            return true;
        }
    }
    false
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]
mod tests {
    use super::*;

    fn file(path: &str, version: &str) -> MigrationFile {
        MigrationFile {
            rel_path: path.to_string(),
            version: version.to_string(),
        }
    }

    const SECTION: &str = "### 3.4 迁移登记表（版本号分配的 SSOT）\n\n| 版本 | 拥有者 crate | 迁移文件 | 建出的表 / 对象 |\n|---|---|---|---|\n| 0001 | `crates/storage` | `crates/storage/migrations/0001_init.sql` | `tasks` |\n| 0002 | `crates/audit` | `crates/audit/migrations/0002_audit_logs.sql` | `audit_logs` |\n\n## 4. 下一节\n";

    #[test]
    fn extracts_only_the_registry_section() {
        let section = extract_registry_section(SECTION).expect("应截出 §3.4");
        assert!(section.contains("0001_init.sql"));
        assert!(
            !section.contains("下一节"),
            "必须在下个标题处截断：{section}"
        );
    }

    #[test]
    fn parses_registry_rows_and_skips_header() {
        let entries = parse_registry_entries(SECTION);
        assert_eq!(entries.len(), 2, "{entries:?}");
        assert_eq!(entries[0].0, "0001");
        assert_eq!(entries[0].1, "crates/storage/migrations/0001_init.sql");
        assert_eq!(entries[1].1, "crates/audit/migrations/0002_audit_logs.sql");
    }

    #[test]
    fn migration_file_version_accepts_only_numbered_sql() {
        assert_eq!(
            migration_file_version("crates/audit/migrations/0002_audit_logs.sql"),
            Some("0002".to_string())
        );
        assert_eq!(
            migration_file_version("crates/audit/migrations/readme.sql"),
            None
        );
        assert_eq!(migration_file_version("crates/audit/src/0001_x.sql"), None);
        assert_eq!(
            migration_file_version("crates/audit/migrations/0002_x.rs"),
            None
        );
        assert_eq!(
            migration_file_version("crates/audit/migrations/001_x.sql"),
            None
        );
    }

    #[test]
    fn crate_of_path_reads_owner_directory() {
        assert_eq!(
            crate_of_migration_path("crates/audit/migrations/0002_audit_logs.sql").as_deref(),
            Some("audit")
        );
        assert_eq!(crate_of_migration_path("crates/storage/src/lib.rs"), None);
    }

    // ---- 负向用例 1：号段重复必须报 ----
    #[test]
    fn negative_duplicate_version_is_detected() {
        let files = vec![
            file("crates/a/migrations/0001_a.sql", "0001"),
            file("crates/b/migrations/0001_b.sql", "0001"),
        ];
        let findings = check_version_uniqueness(&files);
        assert_eq!(findings.len(), 1, "{findings:?}");
        assert_eq!(findings[0].rule, RULE_VERSION_DUPLICATE);
        assert_eq!(findings[0].severity, Severity::Error);
        assert!(findings[0].message.contains("0001"));
    }

    #[test]
    fn positive_unique_versions_produce_nothing() {
        let files = vec![
            file("crates/a/migrations/0001_a.sql", "0001"),
            file("crates/b/migrations/0002_b.sql", "0002"),
        ];
        assert!(check_version_uniqueness(&files).is_empty());
    }

    // ---- 负向用例 2：登记表与文件不一致必须报（三个方向都测）----
    #[test]
    fn negative_registry_missing_row_is_detected() {
        let files = vec![file("crates/a/migrations/0002_b.sql", "0002")];
        let registry = vec![(
            "0001".to_string(),
            "crates/a/migrations/0001_a.sql".to_string(),
        )];
        let findings = check_registry_match(&files, &registry);
        // 0002 未登记 + 0001 登记了却没有文件 → 两条
        assert_eq!(findings.len(), 2, "{findings:?}");
        assert!(findings.iter().all(|f| f.rule == RULE_REGISTRY_MISMATCH));
    }

    #[test]
    fn negative_registry_path_mismatch_is_detected() {
        let files = vec![file("crates/a/migrations/0001_a.sql", "0001")];
        let registry = vec![(
            "0001".to_string(),
            "crates/other/migrations/0001_a.sql".to_string(),
        )];
        let findings = check_registry_match(&files, &registry);
        assert_eq!(findings.len(), 1, "{findings:?}");
        assert!(findings[0].message.contains("路径与 §3.4 登记表不一致"));
    }

    #[test]
    fn positive_registry_matching_files_produce_nothing() {
        let files = vec![
            file("crates/storage/migrations/0001_init.sql", "0001"),
            file("crates/audit/migrations/0002_audit_logs.sql", "0002"),
        ];
        let registry = vec![
            (
                "0001".to_string(),
                "crates/storage/migrations/0001_init.sql".to_string(),
            ),
            (
                "0002".to_string(),
                "crates/audit/migrations/0002_audit_logs.sql".to_string(),
            ),
        ];
        assert!(check_registry_match(&files, &registry).is_empty());
    }

    // ---- 负向用例 3：crate 没公开 MIGRATIONS 必须报 ----
    #[test]
    fn negative_crate_without_migrations_const_is_detected() {
        let findings = check_crate_exposes_migrations("audit", false);
        assert_eq!(findings.len(), 1, "{findings:?}");
        assert_eq!(findings[0].rule, RULE_CRATE_MISSING_CONST);
        assert_eq!(findings[0].severity, Severity::Error);
        assert!(findings[0].path.contains("crates/audit"));
    }

    #[test]
    fn positive_crate_with_migrations_const_produces_nothing() {
        assert!(check_crate_exposes_migrations("storage", true).is_empty());
    }
}
