//! # codegen 子命令（TASK-011）
//!
//! 职责：把 `protocol/**/*.json`（协议**单一事实源**）渲染成
//! `crates/protocol/src/generated/*.rs`，并支持"只检测不写盘"的 `--check` 模式。
//!
//! ## 边界（不做什么）
//! - 不做 schema 语义校验 —— 那是 `verify-schemas` 的职责
//! - 不引入第三方依赖 —— 渲染在 [`crate::render`]，JSON 解析在 [`crate::serde_json_lite`]
//! - 不做格式化 —— 渲染器直接产出 rustfmt 稳定形态，否则 `cargo fmt --check` 会红灯
//!
//! ## 不变量
//! 1. `--check` **只读**：绝不写盘；有 drift → 退出码 1（CI 硬门禁），无 drift → 0
//! 2. 写盘模式只重写真正 drift 的文件，未 drift 的文件不碰
//! 3. 比对前两侧都过 [`crate::render::normalize`]，故"仅尾随空白差异"不算 drift
//! 4. 任何渲染失败都必须让退出码非 0（铁律 1：无静默失败）
//!
//! ## 退出码
//! 0 通过 | 1 有 drift（`--check` 的阻塞级发现项）| 4 IO/渲染错误（由 `main.rs` 映射）

use std::fs;
use std::path::Path;

/// 一次比对的结果：要么一致，要么记录首个差异行用于输出摘要。
struct DriftReport {
    /// 生成物相对仓库根的路径（写盘与输出都用它）。
    relative_path: String,
    /// 本次渲染出的完整内容（写盘模式直接落盘）。
    generated: String,
}

/// 首个差异：1-based 行号 + 两侧该行原文。
struct Difference {
    line_number: usize,
    generated_line: String,
    on_disk_line: String,
}

/// 渲染全部 schema 并与磁盘比对，按需写盘。
///
/// # 错误
/// 仅在 IO 失败（读 schema / 写生成物失败）时返回 `Err`；渲染失败会记入
/// `error_count` 并最终以 `Err` 收尾，绝不静默吞掉。
pub fn run(
    repo_root: &Path,
    check_only: bool,
    output: &mut dyn std::io::Write,
) -> Result<u8, String> {
    writeln!(output, "== codegen ==").map_err(|error| error.to_string())?;
    writeln!(output, "mode={}", mode_label(check_only)).map_err(|error| error.to_string())?;

    let (drifts, error_count) = render_all(repo_root, output)?;
    if !check_only {
        write_drifts(repo_root, &drifts)?;
    }
    report(output, &drifts, error_count, check_only)
}

/// `--check` / 写盘两种模式的标签（只为了输出可读）。
const fn mode_label(check_only: bool) -> &'static str {
    if check_only { "check" } else { "write" }
}

/// 逐份 schema 渲染 + 与磁盘比对，返回（drift 列表，渲染失败数）。
fn render_all(
    repo_root: &Path,
    output: &mut dyn std::io::Write,
) -> Result<(Vec<DriftReport>, usize), String> {
    use crate::render::{SCHEMAS, normalize, render};

    let mut drifts: Vec<DriftReport> = Vec::new();
    let mut error_count = 0usize;
    for (schema_relative_path, generated_relative_path) in SCHEMAS {
        let schema_path = repo_root.join(schema_relative_path);
        let schema_text = match fs::read_to_string(&schema_path) {
            Ok(text) => text,
            Err(error) => {
                writeln!(output, "  [ERROR] {schema_relative_path}  read: {error}")
                    .map_err(|error| error.to_string())?;
                error_count += 1;
                continue;
            }
        };
        let generated = match render(schema_relative_path, &schema_text) {
            Ok(text) => text,
            Err(reason) => {
                writeln!(output, "  [ERROR] {schema_relative_path}  render: {reason}")
                    .map_err(|error| error.to_string())?;
                error_count += 1;
                continue;
            }
        };

        // 生成物缺失 = 必须生成（不是错误）；存在则按 normalize 后的内容比对。
        let on_disk = if repo_root.join(generated_relative_path).exists() {
            let path = repo_root.join(generated_relative_path);
            Some(
                fs::read_to_string(&path)
                    .map_err(|error| format!("read {generated_relative_path}: {error}"))?,
            )
        } else {
            None
        };
        let difference = on_disk
            .as_deref()
            .and_then(|disk| first_difference_line(&normalize(disk), &normalize(&generated)));
        match (&difference, &on_disk) {
            (Some(difference), _) => {
                writeln!(
                    output,
                    "  [DRIFT] {generated_relative_path}  first difference at line {}",
                    difference.line_number
                )
                .map_err(|error| error.to_string())?;
                writeln!(output, "      generated: {}", difference.generated_line)
                    .map_err(|error| error.to_string())?;
                writeln!(output, "      on disk  : {}", difference.on_disk_line)
                    .map_err(|error| error.to_string())?;
            }
            (None, None) => {
                writeln!(output, "  [DRIFT] {generated_relative_path}  file missing")
                    .map_err(|error| error.to_string())?;
            }
            (None, Some(_)) => {
                writeln!(output, "  [OK] {generated_relative_path}")
                    .map_err(|error| error.to_string())?;
            }
        }
        if difference.is_some() || on_disk.is_none() {
            drifts.push(DriftReport {
                relative_path: (*generated_relative_path).to_string(),
                generated,
            });
        }
    }
    Ok((drifts, error_count))
}

/// 把 drift 的生成物写回磁盘（不变量 2：只碰 drift 的文件）。
fn write_drifts(repo_root: &Path, drifts: &[DriftReport]) -> Result<(), String> {
    for drift in drifts {
        let full_path = repo_root.join(&drift.relative_path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).map_err(|error| format!("mkdir: {error}"))?;
        }
        fs::write(&full_path, &drift.generated)
            .map_err(|error| format!("write {}: {error}", drift.relative_path))?;
    }
    Ok(())
}

/// 打印小结与 verdict，并给出本子命令的退出码（不变量 1 / 4）。
fn report(
    output: &mut dyn std::io::Write,
    drifts: &[DriftReport],
    error_count: usize,
    check_only: bool,
) -> Result<u8, String> {
    if !check_only && !drifts.is_empty() {
        writeln!(output, "-- wrote {} drifted file(s)", drifts.len())
            .map_err(|error| error.to_string())?;
    }
    writeln!(
        output,
        "-- summary: {} drift(s), {error_count} error(s)",
        drifts.len()
    )
    .map_err(|error| error.to_string())?;
    let verdict = if error_count > 0 {
        "FAILED (render error)"
    } else if drifts.is_empty() {
        "PASSED"
    } else if check_only {
        "FAILED (drift)"
    } else {
        "WRITTEN"
    };
    writeln!(output, "-- verdict: {verdict}").map_err(|error| error.to_string())?;

    if error_count > 0 {
        return Err(format!("{error_count} schema(s) failed to render"));
    }
    // 不变量 1：`--check` 有 drift 必须是阻塞级发现项（退出码 1），否则门禁形同虚设。
    if check_only && !drifts.is_empty() {
        return Ok(1);
    }
    Ok(0)
}

/// 找出两侧内容第一处不同的行。
///
/// 入参必须是 [`crate::render::normalize`] 过的文本（行尾统一为 `\n`），否则行号会漂移。
/// 返回 `None` 表示两侧逐行完全一致。
fn first_difference_line(left: &str, right: &str) -> Option<Difference> {
    let mut left_lines = left.lines();
    let mut right_lines = right.lines();
    let mut line_number = 1usize;
    loop {
        match (left_lines.next(), right_lines.next()) {
            (None, None) => return None,
            (left_line, right_line) => {
                let left_text = left_line.unwrap_or("<missing line>");
                let right_text = right_line.unwrap_or("<missing line>");
                if left_text != right_text {
                    return Some(Difference {
                        line_number,
                        generated_line: right_text.to_string(),
                        on_disk_line: left_text.to_string(),
                    });
                }
                line_number += 1;
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    //! 负向验证（ADR-0019 形式 N1）：**生成物被手改 / schema 读不到时，`--check` 必须变红**。
    //!
    //! 这条门禁的价值全在"该红的时候真的红"：`codegen --check` 若恒返回 0，
    //! 生成物就可以被随意手改而无人发现（铁律 1：无静默失败）。

    use super::*;
    use crate::render::normalize;

    /// 正向基线：两侧完全一致 → 没有差异（证明下面的负向断言不是恒真）。
    #[test]
    fn test_first_difference_line_returns_none_when_identical() {
        let text = normalize("line one\nline two\n");
        assert!(first_difference_line(&text, &text).is_none());
    }

    /// 负向：第 2 行被手改 → 必须报出**首个**差异行（1-based 行号 + 两侧原文）。
    #[test]
    fn test_first_difference_line_reports_first_differing_line() {
        let on_disk = normalize("line one\nHAND EDITED\nline three\n");
        let generated = normalize("line one\nline two\nline three\n");
        let difference =
            first_difference_line(&on_disk, &generated).expect("第 2 行不同必须被检测到");
        assert_eq!(difference.line_number, 2);
        assert_eq!(difference.on_disk_line, "HAND EDITED");
        assert_eq!(difference.generated_line, "line two");
    }

    /// 负向：生成物末尾被追加一行 → 必须被检测到（不能因为前缀相同就判"一致"）。
    #[test]
    fn test_first_difference_line_detects_appended_trailing_line() {
        let on_disk = normalize("a\nb\n// injected\n");
        let generated = normalize("a\nb\n");
        let difference =
            first_difference_line(&on_disk, &generated).expect("多出的尾行必须被检测到");
        assert_eq!(difference.line_number, 3);
        assert_eq!(difference.generated_line, "<missing line>");
    }

    /// 负向：仓库根不存在 → schema 读不到 → 必须以 `Err` 收尾，
    /// 绝不允许"什么都没读到 → 报告 0 drift → exit 0"。
    #[test]
    fn test_run_fails_loudly_when_schemas_unreadable() {
        let missing = Path::new("Z:/definitely-not-a-directory-codegen");
        let mut output: Vec<u8> = Vec::new();
        let result = run(missing, true, &mut output);
        assert!(
            result.is_err(),
            "读不到 schema 必须是错误，不能静默返回成功"
        );
        let text = String::from_utf8(output).expect("输出必须是 UTF-8");
        assert!(
            text.contains("FAILED (render error)"),
            "必须显式打印渲染失败，实际输出：{text}"
        );
    }
}
