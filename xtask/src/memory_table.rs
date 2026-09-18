//! # `MEMORY.md` 规模表的解析与计数原语（ADR-0030 D1 的「测量」一半）
//!
//! 职责：提供两类**纯函数**能力 ——
//! ① 计数判据：一个记忆文件的「行数」与「条目数」怎么算；
//! ② 表解析：把 `MEMORY.md`「各文件当前规模」那张手写表解析成结构化数据。
//!
//! 本模块**只测量与解析，不判定对错**。判定（表中数字 vs 实测数字是否一致）在
//! `memory_counts.rs`。这样切分的理由与本 crate 既有的 `rustscan`（视图）/ `hygiene`（规则）
//! 分工完全一致：**判据只写一次，规则才能被逐条白盒测试**。
//!
//! ## 边界（不做什么）
//! - 不做文件 IO：输入是文本，遍历与读写在 `main.rs`。
//! - 不校验条目内容的正确性，只数数。
//! - 不处理表格单元格内的转义竖线（`\|`）：规模表四列都是路径/数字/短句，不含代码跨度。
//!   若将来需要，应与 `hygiene` 的表格竖线规则（PL-031）一起实现，避免两处各写一套解析。
//!
//! ## 不变量
//! 1. 全部函数是**纯函数**：同样输入必得同样输出。
//! 2. 计数判据与 `MEMORY.md`「条目格式」一节的规定一致，**不另立标准**。
//! 3. 解析失败必须返回 `Err`，**绝不返回空表冒充成功**（铁律 1）：
//!    「表不存在」与「表存在但没有数据行」都必须能被区分出来。
//!
//! 相关：`docs/adr/0030-machine-verified-memory-counts-and-adr-index.md`、
//! `docs/adr/0021-memory-layering-and-app-profiles.md`

/// `MEMORY.md` 自身路径（相对仓库根，统一用 `/` 以保证跨平台输出一致）。
pub const INDEX_PATH: &str = "MEMORY.md";

/// L1 记忆文件所在目录（相对仓库根）。规模表里的路径都相对于此目录。
pub const MEMORY_DIR: &str = "docs/memory";

/// 规模表所在章节的标题前缀（唯一的解析锚点，改动它等于改契约）。
pub const SCALE_SECTION_HEADING: &str = "## 各文件当前规模";

/// 记忆条目的行首前缀（完整判据见 [`is_entry_line`]）。
const ENTRY_LINE_PREFIX: &str = "- [";

/// 一个 L1 记忆文件的**实测**规模（由 `main.rs` 读文件后填入）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeasuredFile {
    /// 相对 `docs/memory/` 的路径，统一用 `/` 分隔（例：`facts.md`、`apps/notepad.md`）。
    pub relative_path: String,
    /// 实测行数（判据见 [`count_lines`]）。
    pub line_count: usize,
    /// 实测条目数（判据见 [`count_entries`]）。
    pub entry_count: usize,
}

/// `MEMORY.md` 规模表里的一行**登记值**（可能与实测不符 —— 那正是要检出的东西）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredFile {
    /// 表中第 1 列的路径（已去掉反引号），相对 `docs/memory/`。
    pub relative_path: String,
    /// 表中第 2 列声明的行数；非纯数字时为 `None`（由 `memory_counts` 报为表结构缺陷）。
    pub line_count: Option<usize>,
    /// 表中第 3 列声明的条目数；非纯数字时为 `None`。
    pub entry_count: Option<usize>,
    /// 该行在 `MEMORY.md` 中的 1 基行号（让发现项能定位到具体行）。
    pub line_number: usize,
}

/// 统计文件行数（ADR-0030 D1 的判据）。
///
/// 判据：按 `\n` 切分后的行元素个数。文件以恰好一个 `\n` 结尾时等于常识行数；
/// 末行缺换行时该行仍算一行；**空文件为 0**（不是 1）。
///
/// 为什么不直接用 `str::lines().count()`：`lines()` 对 `"a\n"` 与 `"a"` 都返回 1，
/// 无法区分「末行有换行」与「末行缺换行」，而后者正是 gov §5.4 的卫生问题之一，
/// 本工具的计数必须能与 `hygiene` 的结论对得上。
#[must_use]
pub fn count_lines(content: &str) -> usize {
    if content.is_empty() {
        return 0;
    }
    let newline_count = content.bytes().filter(|byte| *byte == b'\n').count();
    if content.ends_with('\n') {
        newline_count
    } else {
        newline_count + 1
    }
}

/// 统计记忆条目数（ADR-0030 D1 的判据）。
///
/// 判据：**行首**（不允许任何缩进）匹配 `- [YYYY-MM-DD]` 的行数，即
/// `- [YYYY-MM-DD][标签][src:来源] 一句话结论 → 因此怎么做` 这种形态。
#[must_use]
pub fn count_entries(content: &str) -> usize {
    content.lines().filter(|line| is_entry_line(line)).count()
}

/// 判断单行是否是一条记忆条目。
///
/// 严格程度是刻意的：宽松判据（例如「以 `- ` 开头就算」）会把小节标题下的普通列表项
/// 也数进去，导致计数**虚高** —— 那比不校验更糟，因为它给出一个看起来权威的错误数字。
fn is_entry_line(line: &str) -> bool {
    let Some(after_prefix) = line.strip_prefix(ENTRY_LINE_PREFIX) else {
        return false;
    };
    let mut characters = after_prefix.chars();
    // 期望形态：YYYY-MM-DD]（四段之间必须是 `-`，最后必须紧跟 `]`）
    if !take_digits(&mut characters, 4) || characters.next() != Some('-') {
        return false;
    }
    if !take_digits(&mut characters, 2) || characters.next() != Some('-') {
        return false;
    }
    if !take_digits(&mut characters, 2) {
        return false;
    }
    characters.next() == Some(']')
}

/// 从字符迭代器连续取 `count` 个 ASCII 数字；不足或含非数字即返回 `false`。
fn take_digits(characters: &mut std::str::Chars<'_>, count: usize) -> bool {
    for _ in 0..count {
        if !characters
            .next()
            .is_some_and(|character| character.is_ascii_digit())
        {
            return false;
        }
    }
    true
}

/// 解析 `MEMORY.md` 的「各文件当前规模」表。
///
/// 定位方式：先找以 [`SCALE_SECTION_HEADING`] 开头的章节标题，再取该标题之后**第一张**
/// Markdown 表（表头行 + 分隔行 + 若干数据行），逐行取第 1/2/3 列。
/// 第 1 列去反引号当路径，第 2/3 列解析为整数（解析不出记为 `None`，由调用方报错，
/// **不静默当成 0** —— 那会让「表里写了破折号」伪装成「文件真的是 0 行」）。
///
/// # Errors
/// 返回 `Err(说明)` 的四种情况：找不到章节标题、标题之后没有表格、表头后不是分隔行、
/// 表存在但没有任何数据行。四种都必须让调用方失败。
pub fn parse_scale_table(index_content: &str) -> Result<Vec<RegisteredFile>, String> {
    let lines: Vec<&str> = index_content.lines().collect();
    let Some(heading_index) = lines
        .iter()
        .position(|line| line.starts_with(SCALE_SECTION_HEADING))
    else {
        return Err(format!(
            "在 {INDEX_PATH} 中找不到章节标题 `{SCALE_SECTION_HEADING}`（规模表的解析锚点）"
        ));
    };
    let header_index = locate_table_header(&lines, heading_index + 1)?;
    let Some(separator) = lines.get(header_index + 1) else {
        return Err("规模表缺少表头分隔行（`|---|---|---|---|`）".to_string());
    };
    if !separator.starts_with('|') || !separator.contains("---") {
        return Err(format!(
            "规模表表头之后不是分隔行，实际是 `{separator}`（表结构被破坏）"
        ));
    }
    let rows = parse_data_rows(&lines, header_index + 2);
    if rows.is_empty() {
        return Err("规模表存在但没有任何数据行（0 个文件被登记 = 没有路由信息）".to_string());
    }
    Ok(rows)
}

/// 从 `from` 行起向下找第一行以 `|` 开头的行（表头），返回其下标。
///
/// 中间允许有说明文字（现状就是一句「Orchestrator 每次归档/大改后更新本表」的括注）。
///
/// # Errors
/// 一直找到文件末尾都没有表格行时返回 `Err`。
fn locate_table_header(lines: &[&str], from: usize) -> Result<usize, String> {
    let mut cursor = from;
    while let Some(line) = lines.get(cursor) {
        if line.starts_with('|') {
            return Ok(cursor);
        }
        cursor += 1;
    }
    Err(format!(
        "`{SCALE_SECTION_HEADING}` 之后没有 Markdown 表格（规模表被删或改成了别的形式）"
    ))
}

/// 从 `start` 行起解析连续的数据行，遇到第一行不以 `|` 开头的行即停止。
fn parse_data_rows(lines: &[&str], start: usize) -> Vec<RegisteredFile> {
    let mut rows = Vec::new();
    let mut cursor = start;
    while let Some(line) = lines.get(cursor) {
        if !line.starts_with('|') {
            break;
        }
        let cells = split_table_row(line);
        // 列数不足 3（路径/行数/条目数）时两个计数都记为 None，让调用方报「表结构缺陷」，
        // 而不是把唯一那个数字猜成行数
        let (line_count, entry_count) = if cells.len() >= 3 {
            (
                parse_count(cells.get(1).map(String::as_str)),
                parse_count(cells.get(2).map(String::as_str)),
            )
        } else {
            (None, None)
        };
        rows.push(RegisteredFile {
            relative_path: cells
                .first()
                .map_or_else(String::new, |cell| clean_path_cell(cell)),
            line_count,
            entry_count,
            line_number: cursor + 1,
        });
        cursor += 1;
    }
    rows
}

/// 把一行 Markdown 表格切成单元格（去掉首尾竖线产生的空串，并 trim 每格）。
pub fn split_table_row(line: &str) -> Vec<String> {
    let trimmed = line.trim();
    let without_leading = trimmed.strip_prefix('|').unwrap_or(trimmed);
    let body = without_leading.strip_suffix('|').unwrap_or(without_leading);
    body.split('|')
        .map(|cell| cell.trim().to_string())
        .collect()
}

/// 清理路径单元格：去首尾空白、去反引号、把 `\` 归一成 `/`。
fn clean_path_cell(cell: &str) -> String {
    cell.trim().trim_matches('`').trim().replace('\\', "/")
}

/// 解析计数单元格；非纯数字（空、`—`、千分位、带单位）一律 `None`。
fn parse_count(cell: Option<&str>) -> Option<usize> {
    let text = cell?.trim();
    if text.is_empty() {
        return None;
    }
    text.parse::<usize>().ok()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    /// 一份最小但结构完整的规模表（标题后有说明文字，用于验证锚点定位不受干扰）。
    const SAMPLE_INDEX: &str = "\
# MEMORY.md — 项目长期记忆

## 各文件当前规模（Orchestrator 每次归档/大改后更新本表）

| 文件 | 行数 | 条目数 | 读法 |
|---|---|---|---|
| `facts.md` | 3 | 2 | grep 优先 |
| `apps/notepad.md` | 1 | 0 | 全量读 |

## §1 快照
";

    // --- count_lines ---

    #[test]
    fn test_count_lines_empty_content_is_zero() {
        assert_eq!(count_lines(""), 0, "空文件是 0 行，不是 1 行");
    }

    #[test]
    fn test_count_lines_trailing_newline_does_not_add_a_line() {
        assert_eq!(count_lines("a\nb\n"), 2);
        assert_eq!(count_lines("a\n"), 1);
    }

    #[test]
    fn test_count_lines_missing_final_newline_still_counts_last_line() {
        assert_eq!(count_lines("a\nb"), 2, "末行缺换行也算一行");
    }

    // --- count_entries / is_entry_line ---

    #[test]
    fn test_count_entries_counts_only_canonical_entry_lines() {
        let content = "\
- [2026-09-18][FACT][src:x] 结论
- [2026-09-17][PITFALL] 另一条
## 小节标题
- 普通列表项不算条目
";
        assert_eq!(count_entries(content), 2);
    }

    #[test]
    fn test_count_entries_rejects_indented_line() {
        // 判据明写「行首不允许缩进」：缩进的是嵌套列表项，不是记忆条目
        assert_eq!(count_entries("  - [2026-09-18][FACT] x\n"), 0);
    }

    #[test]
    fn test_is_entry_line_rejects_malformed_dates() {
        for line in [
            "- [2026-9-18] 月份只有一位",
            "- [2026/09/18] 分隔符不对",
            "- [26-09-18] 年份只有两位",
            "- [2026-09-18 缺右括号",
            "- [] 空的",
            "* [2026-09-18] 用了星号",
        ] {
            assert!(!is_entry_line(line), "不该被当成条目：{line}");
        }
    }

    #[test]
    fn test_is_entry_line_accepts_canonical_form() {
        assert!(is_entry_line(
            "- [2026-09-18][FACT][src:x] 结论 → 因此怎么做"
        ));
    }

    // --- parse_scale_table ---

    #[test]
    fn test_parse_scale_table_happy_path() {
        let rows = parse_scale_table(SAMPLE_INDEX).expect("应解析成功");
        assert_eq!(rows.len(), 2);
        let first = rows.first().expect("至少一行");
        assert_eq!(first.relative_path, "facts.md", "反引号应被去掉");
        assert_eq!(first.line_count, Some(3));
        assert_eq!(first.entry_count, Some(2));
        assert_eq!(first.line_number, 7, "1 基行号，指向数据行本身");
    }

    #[test]
    fn test_parse_scale_table_normalizes_backslash_paths() {
        let content = SAMPLE_INDEX.replace("`apps/notepad.md`", "`apps\\notepad.md`");
        let rows = parse_scale_table(&content).expect("应解析成功");
        let second = rows.get(1).expect("应有第二行");
        assert_eq!(
            second.relative_path, "apps/notepad.md",
            "路径分隔符必须归一"
        );
    }

    #[test]
    fn test_parse_scale_table_missing_section_is_error() {
        let error = parse_scale_table("# MEMORY.md\n\n没有规模表\n").expect_err("必须报错");
        assert!(
            error.contains(SCALE_SECTION_HEADING),
            "错误信息应指出缺的锚点：{error}"
        );
    }

    #[test]
    fn test_parse_scale_table_section_without_table_is_error() {
        let error =
            parse_scale_table("## 各文件当前规模\n\n只剩一段说明文字\n").expect_err("必须报错");
        assert!(error.contains("没有 Markdown 表格"), "实际：{error}");
    }

    #[test]
    fn test_parse_scale_table_missing_separator_is_error() {
        let content = "## 各文件当前规模\n\n| 文件 | 行数 | 条目数 | 读法 |\n";
        let error = parse_scale_table(content).expect_err("缺分隔行必须报错");
        assert!(error.contains("分隔行"), "实际：{error}");
    }

    #[test]
    fn test_parse_scale_table_without_data_rows_is_error() {
        let content = "\
## 各文件当前规模

| 文件 | 行数 | 条目数 | 读法 |
|---|---|---|---|

后文
";
        let error = parse_scale_table(content).expect_err("空表必须报错");
        assert!(error.contains("没有任何数据行"), "实际：{error}");
    }

    #[test]
    fn test_parse_scale_table_non_numeric_count_becomes_none() {
        let content = SAMPLE_INDEX.replace("| `facts.md` | 3 | 2 |", "| `facts.md` | — | 2 |");
        let rows = parse_scale_table(&content).expect("表结构合法，只是某格非数字");
        let only = rows.first().expect("应有一行");
        assert_eq!(only.line_count, None, "非数字必须记为 None，不能猜成 0");
        assert_eq!(only.entry_count, Some(2));
    }

    #[test]
    fn test_parse_scale_table_too_few_columns_yields_none_counts() {
        let content = "\
## 各文件当前规模

| 文件 | 行数 |
|---|---|
| `facts.md` | 3 |
";
        let rows = parse_scale_table(content).expect("表本身可解析");
        let only = rows.first().expect("应有一行");
        assert_eq!(only.line_count, None, "列数不足时不得把 3 当成行数");
        assert_eq!(only.entry_count, None);
    }

    // --- split_table_row ---

    #[test]
    fn test_split_table_row_drops_edge_pipes_and_trims_cells() {
        let cells = split_table_row("|  `a.md`  | 1 | 2 | 读法 |");
        assert_eq!(cells, vec!["`a.md`", "1", "2", "读法"]);
    }

    #[test]
    fn test_split_table_row_tolerates_missing_edge_pipes() {
        let cells = split_table_row("`a.md` | 1 | 2");
        assert_eq!(cells, vec!["`a.md`", "1", "2"]);
    }
}
