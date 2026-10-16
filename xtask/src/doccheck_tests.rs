//! `doccheck.rs` 的私有单元测试：两个 HARD 门禁（`memory-counts` / `adr-index`，CI #12b）
//! 的 IO 边界 —— 读文件 → 调纯函数 → 渲染 → 定退出码。
//!
//! **为什么单独一个文件**：`doccheck.rs` 内联测试会把该文件推过 `hygiene/file-too-long`
//! 的 600 行告警线，而本 crate 的既定做法（PL-033）是用 `#[path]` 把 `mod tests` 外置。
//! 外置不改变可见性语义：`use super::*` 仍能访问父模块的私有项，与 `tests/` 目录下的
//! 集成测试有本质区别。
//! 声明处在 `doccheck.rs` 末尾：`#[cfg(test)] #[path = "doccheck_tests.rs"] mod tests;`。
//!
//! **为什么这里允许真实文件 IO**：`docs/spec/testing.md` §4.1 的「unit 零 IO」针对的是
//! **纯逻辑**单测；本模块要测的恰恰是 IO 边界自己，而两个不可信输入都被注入 ——
//! 仓库根是临时目录（带进程号，用完显式删除），输出是 `&mut dyn Write`（§4.3「输出可注入」，
//! 不捕获进程 stdout）。与 `main.rs` / `write_channel_tests.rs` 的既有做法一致。

// 测试里允许 unwrap/expect/panic：断言失败就该立刻炸出来（ADR-0027 D2）。
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

/// 临时仓库目录名里的序号：同一进程内多个用例并发跑，光靠进程号会互相踩。
static SCRATCH_SEQUENCE: AtomicUsize = AtomicUsize::new(0);

/// 一个有界的临时仓库根目录。
///
/// 不用 `Drop` 清理：Drop 里清理失败只能吞掉 `Result`（铁律 1 禁止）或 panic。
/// 改为显式 [`ScratchRepo::remove`]，与 `main.rs` 的 hygiene 端到端用例同款。
struct ScratchRepo {
    root: PathBuf,
}

impl ScratchRepo {
    fn new(label: &str) -> Self {
        let sequence = SCRATCH_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "xtask-doccheck-{label}-{}-{sequence}",
            std::process::id()
        ));
        if root.exists() {
            std::fs::remove_dir_all(&root).expect("清理上次运行的残留临时仓库");
        }
        std::fs::create_dir_all(&root).expect("创建临时仓库根目录");
        Self { root }
    }

    fn path(&self) -> &Path {
        &self.root
    }

    /// 写一个仓库相对路径的文件（父目录自动创建）。
    fn write(&self, relative: &str, content: &str) {
        let full = self.root.join(path_from_repo_relative(relative));
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent).expect("创建父目录");
        }
        std::fs::write(&full, content).expect("写文件");
    }

    fn remove(self) {
        std::fs::remove_dir_all(&self.root).expect("清理临时仓库");
    }
}

/// 一份与 [`CONSISTENT_FACTS`] 逐格相符的 `MEMORY.md`。
const CONSISTENT_INDEX: &str = "\
# MEMORY.md — 项目长期记忆

## 各文件当前规模

| 文件 | 行数 | 条目数 | 读法 |
|---|---|---|---|
| `facts.md` | 3 | 1 | grep 优先 |
";

/// 与 [`CONSISTENT_INDEX`] 相符的记忆文件内容：3 行、1 条条目。
const CONSISTENT_FACTS: &str = "\
- [2026-01-01][FACT][src:x] 一句话结论
第二行
第三行
";

/// 渲染到 `Vec<u8>` 并把字节当 UTF-8 文本取回（§4.3：不捕获进程 stdout）。
fn rendered(bytes: Vec<u8>) -> String {
    String::from_utf8(bytes).expect("报告应为 UTF-8")
}

/// 一个第一次写入就失败的 sink，用来覆盖「写输出失败 ⇒ 返回 Err」的分支。
struct FailingWriter;

impl Write for FailingWriter {
    fn write(&mut self, _buffer: &[u8]) -> std::io::Result<usize> {
        Err(std::io::Error::other("模拟写失败"))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Err(std::io::Error::other("模拟写失败"))
    }
}

// --- 纯函数：路径与扩展名 ---

#[test]
fn test_is_markdown_file_is_case_insensitive_and_extension_scoped() {
    assert!(is_markdown_file("notes.md"));
    assert!(is_markdown_file("NOTES.MD"));
    assert!(!is_markdown_file("notes.markdown"));
    assert!(!is_markdown_file("md"));
    assert!(!is_markdown_file("notes.md.bak"));
}

#[test]
fn test_path_from_repo_relative_joins_segments_in_order() {
    assert_eq!(
        path_from_repo_relative("docs/memory/facts.md"),
        PathBuf::from("docs").join("memory").join("facts.md")
    );
    assert_eq!(
        path_from_repo_relative("MEMORY.md"),
        PathBuf::from("MEMORY.md")
    );
}

#[test]
fn test_relative_to_uses_forward_slashes_and_falls_back_outside_base() {
    let base = Path::new("repo");
    let inside = base.join("apps").join("notepad.md");
    assert_eq!(relative_to(base, &inside), "apps/notepad.md");
    assert_eq!(
        relative_to(base, Path::new("elsewhere/notes.md")),
        "elsewhere/notes.md",
        "不在 base 之下时应回退成原路径，不得 panic"
    );
}

// --- run_memory_counts ---

#[test]
fn test_run_memory_counts_consistent_repo_passes_with_summary() {
    let repo = ScratchRepo::new("memory-ok");
    repo.write("MEMORY.md", CONSISTENT_INDEX);
    repo.write("docs/memory/facts.md", CONSISTENT_FACTS);

    let mut output: Vec<u8> = Vec::new();
    let code = run_memory_counts(repo.path(), &mut output).expect("不应因 IO 失败");
    let text = rendered(output);
    repo.remove();

    assert_eq!(code, EXIT_OK, "一致状态必须 PASSED：{text}");
    assert!(
        text.contains("== xtask memory-counts =="),
        "缺报告标题：{text}"
    );
    assert!(
        text.contains(
            "-- machine-summary: memory-counts: scanned=2 errors=0 warnings=0 verdict=PASSED"
        ),
        "机器摘要应是可解析的单行：{text}"
    );
    assert!(text.contains("verdict: PASSED"));
}

#[test]
fn test_run_memory_counts_missing_memory_index_is_error_not_silent_pass() {
    // 不变量 1：读不到 MEMORY.md 必须是 Err，不能当成「空内容」得到一个毫无意义的 PASSED。
    let repo = ScratchRepo::new("memory-missing-index");
    repo.write("docs/memory/facts.md", CONSISTENT_FACTS);

    let mut output: Vec<u8> = Vec::new();
    let error = run_memory_counts(repo.path(), &mut output).expect_err("必须报错");
    repo.remove();

    assert!(error.contains("MEMORY.md"), "错误信息应指出路径：{error}");
    assert!(output.is_empty(), "失败前不应写出半份报告");
}

#[test]
fn test_run_memory_counts_count_mismatch_returns_findings_exit_code() {
    let repo = ScratchRepo::new("memory-mismatch");
    repo.write("MEMORY.md", CONSISTENT_INDEX);
    repo.write(
        "docs/memory/facts.md",
        "- [2026-01-01][FACT][src:x] 多了一行\n第二行\n",
    );

    let mut output: Vec<u8> = Vec::new();
    let code = run_memory_counts(repo.path(), &mut output).expect("不应因 IO 失败");
    let text = rendered(output);
    repo.remove();

    assert_eq!(code, EXIT_FINDINGS, "计数不符必须阻塞：{text}");
    assert!(
        text.contains("memory/line-count-mismatch"),
        "缺发现项：{text}"
    );
    assert!(text.contains("verdict: FAILED"));
}

#[test]
fn test_run_memory_counts_zero_files_warns_explicitly() {
    // 不变量 2：扫到 0 个文件时必须显式告警 —— 「什么都没扫到」≠「扫了都没问题」。
    // 本例连规模表也没有，故同一轮还会命中 `memory/scale-table-unparsable`（也是 Error）：
    // 两条同时出现才是诚实的输出，不该为了凑 PASSED 把告警藏起来。
    let repo = ScratchRepo::new("memory-zero-files");
    repo.write("MEMORY.md", "# MEMORY.md\n\n（没有规模表）\n");

    let mut output: Vec<u8> = Vec::new();
    let code = run_memory_counts(repo.path(), &mut output).expect("不应因 IO 失败");
    let text = rendered(output);
    repo.remove();

    assert_eq!(code, EXIT_FINDINGS);
    assert!(
        text.contains("memory/no-files-scanned"),
        "0 文件必须告警：{text}"
    );
    assert!(
        text.contains("scanned_files=1"),
        "只扫到 MEMORY.md 自身：{text}"
    );
}

#[test]
fn test_run_memory_counts_excludes_readme_archive_and_non_markdown() {
    // 不变量 3：发现规则递归但排除 README.md 与 archive/，且只认 .md。
    // 三种文件都放进目录：任一被误收都会变成 `memory/file-unlisted`（Error）→ 退出码不再是 0。
    let repo = ScratchRepo::new("memory-exclusions");
    repo.write("MEMORY.md", CONSISTENT_INDEX);
    repo.write("docs/memory/facts.md", CONSISTENT_FACTS);
    repo.write("docs/memory/README.md", "# 读法路由\n");
    repo.write("docs/memory/archive/old-probes.md", "# 归档\n");
    repo.write("docs/memory/notes.markdown", "# 别的扩展名\n");

    let mut output: Vec<u8> = Vec::new();
    let code = run_memory_counts(repo.path(), &mut output).expect("不应因 IO 失败");
    let text = rendered(output);
    repo.remove();

    assert_eq!(code, EXIT_OK, "排除规则失效时会冒出 file-unlisted：{text}");
    assert!(
        !text.contains("memory/file-unlisted"),
        "不该有未登记发现项：{text}"
    );
    assert!(
        text.contains("scanned_files=2"),
        "只应扫到 MEMORY.md + facts.md：{text}"
    );
}

#[test]
fn test_run_memory_counts_includes_uppercase_extension() {
    // `.MD` 与 `.md` 是同一类文件（Windows/macOS 文件系统），漏掉会让统计少算。
    let repo = ScratchRepo::new("memory-uppercase-ext");
    repo.write(
        "MEMORY.md",
        "# MEMORY.md\n\n## 各文件当前规模\n\n| 文件 | 行数 | 条目数 | 读法 |\n|---|---|---|---|\n| `NOTES.MD` | 1 | 0 | 全量读 |\n",
    );
    repo.write("docs/memory/NOTES.MD", "只有一行\n");

    let mut output: Vec<u8> = Vec::new();
    let code = run_memory_counts(repo.path(), &mut output).expect("不应因 IO 失败");
    let text = rendered(output);
    repo.remove();

    assert_eq!(code, EXIT_OK, "大写扩展名被漏掉时会报 file-missing：{text}");
    assert!(!text.contains("memory/file-missing"));
}

#[test]
fn test_run_memory_counts_write_failure_is_error() {
    let repo = ScratchRepo::new("memory-write-failure");
    repo.write("MEMORY.md", CONSISTENT_INDEX);
    repo.write("docs/memory/facts.md", CONSISTENT_FACTS);

    let error = run_memory_counts(repo.path(), &mut FailingWriter).expect_err("写失败必须报错");
    repo.remove();

    assert!(error.contains("模拟写失败"), "应保留底层 IO 原因：{error}");
}

#[test]
fn test_run_memory_counts_output_is_deterministic() {
    // 报告不变量 2：同一输入两次渲染必须逐字节相同。
    let repo = ScratchRepo::new("memory-deterministic");
    repo.write("MEMORY.md", CONSISTENT_INDEX);
    repo.write("docs/memory/facts.md", CONSISTENT_FACTS);
    repo.write("docs/memory/zzz.md", "多一个未登记文件\n");

    let mut first: Vec<u8> = Vec::new();
    let mut second: Vec<u8> = Vec::new();
    run_memory_counts(repo.path(), &mut first).expect("第一次不应失败");
    run_memory_counts(repo.path(), &mut second).expect("第二次不应失败");
    repo.remove();

    assert_eq!(first, second, "两次运行输出必须逐字节相同");
}

// --- run_adr_index ---

#[test]
fn test_run_adr_index_missing_registry_is_error() {
    let repo = ScratchRepo::new("adr-missing-registry");
    repo.write("docs/memory/decisions.md", "无待建项\n");

    let mut output: Vec<u8> = Vec::new();
    let error = run_adr_index(repo.path(), &mut output).expect_err("必须报错");
    repo.remove();

    // 消息里的路径由 `Path::display()` 渲染，Windows 上是反斜杠：只断言文件名，
    // 免得「在哪个平台跑」变成断言的一部分。
    assert!(error.contains("README.md"), "应指出缺哪个事实源：{error}");
}

#[test]
fn test_run_adr_index_missing_decisions_is_error() {
    let repo = ScratchRepo::new("adr-missing-decisions");
    repo.write("docs/adr/README.md", "## 1. 已存在文件\n");

    let mut output: Vec<u8> = Vec::new();
    let error = run_adr_index(repo.path(), &mut output).expect_err("必须报错");
    repo.remove();

    assert!(
        error.contains("decisions.md"),
        "应指出缺哪个事实源：{error}"
    );
}

#[test]
fn test_run_adr_index_summarizes_adr_files_and_renders_summary() {
    let repo = ScratchRepo::new("adr-index-run");
    repo.write(
        "docs/adr/README.md",
        "## 1. 已存在文件\n\n| 编号 | 标题 |\n|---|---|\n",
    );
    repo.write("docs/memory/decisions.md", "没有待建项\n");
    // 内容里刻意不写 `ADR-NNNN` 字面量：本文件在 `docs/adr/` 之外，裸引用待建号会被
    // `refscan` 判 Error（ADR-0032）。样例只要文件名带 4 位号即可。
    repo.write(
        "docs/adr/0021-sample.md",
        "# 示例 ADR 文件\n\n状态：Accepted\n",
    );
    repo.write("docs/adr/README.md.bak", "不是 ADR 文件\n");

    let mut output: Vec<u8> = Vec::new();
    let code = run_adr_index(repo.path(), &mut output).expect("不应因 IO 失败");
    let text = rendered(output);
    repo.remove();

    assert!(
        code == EXIT_OK || code == EXIT_FINDINGS,
        "adr-index 只应返回 0 或 1，实际 {code}"
    );
    assert!(text.contains("== xtask adr-index =="), "缺报告标题：{text}");
    assert!(
        text.contains("-- machine-summary: adr-index: scanned=3"),
        "扫描口径 = ADR 文件 + 登记表 + decisions.md：{text}"
    );
}

// --- derive_governance_counts ---

#[test]
fn test_derive_governance_counts_missing_fact_source_is_error() {
    let repo = ScratchRepo::new("governance-missing");
    repo.write(".github/workflows/ci.yml", "# gov-gate: 1\n");

    let error = derive_governance_counts(repo.path()).expect_err("缺治理文档必须报错");
    repo.remove();

    assert!(
        error.contains("governance-ai-agent-execution.md"),
        "实际：{error}"
    );
}

#[test]
fn test_derive_governance_counts_on_own_repo_succeeds() {
    // 复用真实仓库做正向证据：本用例所在仓库本身必须通过该门禁。
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask 必在仓库根下一层");
    assert!(
        derive_governance_counts(repo_root).is_ok(),
        "本仓库的治理计数事实源必须可派生"
    );
}
