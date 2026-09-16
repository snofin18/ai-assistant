//! # 检查结果报告（Finding / Report）
//!
//! 职责：定义所有护栏子命令共用的「发现项」模型，以及把它渲染到任意 `Write` 的能力。
//!
//! ## 边界（不做什么）
//! - 不做任何文件 IO（IO 在 `main.rs`）；渲染目标由调用方注入 `&mut dyn Write`。
//! - 不做规则判定（规则在 `hygiene.rs` / `comments.rs` / `repocheck.rs`）。
//!
//! ## 不变量
//! 1. `Report::is_failure()` 当且仅当存在 `Severity::Error` 级发现项。
//! 2. 渲染输出必须是确定性的：同一 `Report` 渲染两次得到完全相同的字节。
//! 3. 发现项顺序即插入顺序；调用方负责按 (path, line) 排序后再插入。
//!
//! ## 为什么渲染目标可注入
//! 这是白盒测试的关键接缝：测试把 `Vec<u8>` 当 sink，直接断言输出文本，
//! 不需要捕获进程 stdout（见 docs/spec/testing.md §4.3「输出可注入」）。

use std::fmt::Write as _;

/// 严重级别。
///
/// `Warning` 不阻塞 CI（但会被统计并在连续两次未处理时由 Reviewer 追问）；
/// `Error` 阻塞 CI。级别归属定义在 gov §5.4，修改需 ADR。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// 提示级：不阻塞，但应处理或说明理由。
    Warning,
    /// 阻塞级：CI 失败，禁止合并。
    Error,
}

impl Severity {
    /// 返回用于报告输出的短标签。
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Warning => "WARN",
            Self::Error => "ERROR",
        }
    }
}

/// 一条检查发现项。
///
/// `rule` 使用稳定的规则标识符（如 `hygiene/file-too-long`），便于 CI 输出被机器解析、
/// 也便于在任务卡中引用「本卡不允许触发哪些规则」。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// 规则标识符，形如 `<family>/<rule-name>`。
    pub rule: &'static str,
    /// 严重级别。
    pub severity: Severity,
    /// 相对仓库根的路径（统一使用 `/` 分隔，保证跨平台输出一致）。
    pub path: String,
    /// 1 基行号；仓库级（非行级）发现项使用 0。
    pub line: usize,
    /// 面向人的说明，必须包含「为什么是问题」与「怎么改」。
    pub message: String,
}

impl Finding {
    /// 构造一条发现项。
    ///
    /// `message` 为空视为调用方缺陷，用占位文本兜底而不是 panic（铁律 1：不静默，但也不崩溃）。
    #[must_use]
    pub fn new(
        rule: &'static str,
        severity: Severity,
        path: impl Into<String>,
        line: usize,
        message: impl Into<String>,
    ) -> Self {
        let message = message.into();
        let message = if message.trim().is_empty() {
            format!("(规则 {rule} 未提供说明 —— 这本身是缺陷)")
        } else {
            message
        };
        Self {
            rule,
            severity,
            path: path.into(),
            line,
            message,
        }
    }

    /// 渲染为单行文本：`LEVEL rule path:line message`。
    ///
    /// # 错误
    /// 仅在写入 `sink` 失败时返回该 IO 错误。
    pub fn render(&self, sink: &mut dyn std::io::Write) -> std::io::Result<()> {
        let location = if self.line == 0 {
            self.path.clone()
        } else {
            format!("{}:{}", self.path, self.line)
        };
        writeln!(
            sink,
            "{:<5} {:<34} {} {}",
            self.severity.label(),
            self.rule,
            location,
            self.message
        )
    }
}

/// 一次检查运行的完整结果。
#[derive(Debug, Default, Clone)]
pub struct Report {
    /// 被检查的命令名（用于报告标题）。
    pub command: String,
    /// 扫描的文件数（用于证明检查确实跑了，而不是"没找到文件所以通过"）。
    pub scanned_files: usize,
    /// 发现项列表。
    pub findings: Vec<Finding>,
}

impl Report {
    /// 创建空报告。
    ///
    /// `command` 必须是被执行的子命令名（如 `hygiene`）：报告标题靠它自证
    /// 「这是哪一项检查的结果」，留空会让 CI 输出无法归因（铁律 1：不静默）。
    #[must_use]
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            scanned_files: 0,
            findings: Vec::new(),
        }
    }

    /// 追加一条发现项。
    pub fn push(&mut self, finding: Finding) {
        self.findings.push(finding);
    }

    /// 追加多条发现项。
    pub fn extend(&mut self, findings: impl IntoIterator<Item = Finding>) {
        self.findings.extend(findings);
    }

    /// 是否存在阻塞级发现项。
    #[must_use]
    pub fn is_failure(&self) -> bool {
        self.findings.iter().any(|f| f.severity == Severity::Error)
    }

    /// 统计各级别数量，返回 `(errors, warnings)`。
    #[must_use]
    pub fn counts(&self) -> (usize, usize) {
        let errors = self
            .findings
            .iter()
            .filter(|f| f.severity == Severity::Error)
            .count();
        (errors, self.findings.len() - errors)
    }

    /// 渲染完整报告（含标题、明细、汇总）。
    ///
    /// 输出是确定性的（不变量 2）。
    ///
    /// # 错误
    /// 仅在写入 `sink` 失败时返回该 IO 错误。
    pub fn render(&self, sink: &mut dyn std::io::Write) -> std::io::Result<()> {
        writeln!(sink, "== xtask {} ==", self.command)?;
        writeln!(sink, "scanned_files={}", self.scanned_files)?;
        for finding in &self.findings {
            finding.render(sink)?;
        }
        let (errors, warnings) = self.counts();
        writeln!(sink, "-- summary: {errors} error(s), {warnings} warning(s)")?;
        let verdict = if self.is_failure() {
            "FAILED"
        } else {
            "PASSED"
        };
        writeln!(sink, "-- verdict: {verdict}")?;
        Ok(())
    }

    /// 渲染为便于人读的多行摘要（用于任务卡执行记录粘贴）。
    ///
    /// # 错误
    /// 仅在格式化失败时返回 `std::fmt::Error`。
    pub fn summary_line(&self) -> Result<String, std::fmt::Error> {
        let (errors, warnings) = self.counts();
        let verdict = if self.is_failure() {
            "FAILED"
        } else {
            "PASSED"
        };
        let mut out = String::new();
        write!(
            out,
            "{}: scanned={} errors={} warnings={} verdict={}",
            self.command, self.scanned_files, errors, warnings, verdict
        )?;
        Ok(out)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn sample() -> Report {
        let mut report = Report::new("hygiene");
        report.scanned_files = 3;
        report
    }

    #[test]
    fn test_report_new_keeps_command_name() {
        // 回归测试：new() 曾经把 command 参数丢掉并置空（典型的静默失败），
        // 导致 CI 输出无法归因到具体检查项。这里把该行为锁死。
        assert_eq!(Report::new("hygiene").command, "hygiene");
        assert_eq!(
            Report::new(String::from("check-comments")).command,
            "check-comments"
        );
    }

    #[test]
    fn test_report_empty_is_not_failure() {
        let report = sample();
        assert!(!report.is_failure());
        assert_eq!(report.counts(), (0, 0));
    }

    #[test]
    fn test_report_warning_only_is_not_failure() {
        let mut report = sample();
        report.push(Finding::new("r/w", Severity::Warning, "a.rs", 1, "提示"));
        assert!(!report.is_failure(), "只有 warning 不应阻塞 CI");
        assert_eq!(report.counts(), (0, 1));
    }

    #[test]
    fn test_report_with_error_is_failure() {
        let mut report = sample();
        report.push(Finding::new("r/e", Severity::Error, "a.rs", 2, "阻塞"));
        assert!(report.is_failure());
        assert_eq!(report.counts(), (1, 0));
    }

    #[test]
    fn test_finding_render_is_deterministic() {
        let finding = Finding::new(
            "hygiene/file-too-long",
            Severity::Error,
            "crates/a/src/lib.rs",
            901,
            "文件过长",
        );
        let mut first: Vec<u8> = Vec::new();
        let mut second: Vec<u8> = Vec::new();
        finding.render(&mut first).expect("写入 Vec 不应失败");
        finding.render(&mut second).expect("写入 Vec 不应失败");
        assert_eq!(first, second, "渲染必须确定性（不变量 2）");
        let text = String::from_utf8(first).expect("应为 UTF-8");
        assert!(text.contains("ERROR"));
        assert!(text.contains("crates/a/src/lib.rs:901"));
    }

    #[test]
    fn test_finding_with_zero_line_omits_line_number() {
        let finding = Finding::new(
            "repo/missing-readme",
            Severity::Error,
            "crates/foo",
            0,
            "缺 README",
        );
        let mut sink: Vec<u8> = Vec::new();
        finding.render(&mut sink).expect("写入 Vec 不应失败");
        let text = String::from_utf8(sink).expect("应为 UTF-8");
        assert!(
            text.contains("crates/foo "),
            "仓库级发现项不应带 :0 行号，实际：{text}"
        );
        assert!(!text.contains("crates/foo:0"));
    }

    #[test]
    fn test_finding_with_empty_message_gets_placeholder() {
        let finding = Finding::new("r/x", Severity::Warning, "a.rs", 1, "   ");
        assert!(
            finding.message.contains("未提供说明"),
            "空说明必须被兜底，不能静默"
        );
    }

    #[test]
    fn test_report_render_contains_verdict_and_counts() {
        let mut report = sample();
        report.push(Finding::new("r/e", Severity::Error, "a.rs", 1, "阻塞"));
        report.push(Finding::new("r/w", Severity::Warning, "b.rs", 2, "提示"));
        let mut sink: Vec<u8> = Vec::new();
        report.render(&mut sink).expect("写入 Vec 不应失败");
        let text = String::from_utf8(sink).expect("应为 UTF-8");
        assert!(text.contains("== xtask hygiene =="));
        assert!(text.contains("scanned_files=3"));
        assert!(text.contains("1 error(s), 1 warning(s)"));
        assert!(text.contains("verdict: FAILED"));
    }

    #[test]
    fn test_summary_line_is_single_line() {
        let report = sample();
        let line = report.summary_line().expect("格式化不应失败");
        assert!(!line.contains('\n'));
        assert!(line.contains("verdict=PASSED"));
    }
}
