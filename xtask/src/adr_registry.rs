//! # ADR 编号登记表与 ADR 文件的解析原语（ADR-0030 D3 的「测量」一半）
//!
//! 职责：把三处**手工维护**的编号事实解析成结构化数据 ——
//! ① `docs/adr/README.md`（登记表：已存在文件表 / 待建号表 / 退役清单 / 下一个可用编号）；
//! ② `docs/adr/NNNN-*.md`（每个文件的状态行与 `Superseded by`）；
//! ③ `docs/memory/decisions.md`（`[ADR:待建 NNNN]` 条目）。
//!
//! 本模块**只解析，不判定对错**；一致性判定在 `adr_index.rs`。切分理由与
//! `rustscan`（视图）/ `hygiene`（规则）、`memory_table` / `memory_counts` 相同：
//! **解析判据只写一次，规则才能被逐条白盒测试**。
//!
//! ## 为什么这件事值得机器化
//! 编号是 ADR 体系的主键。2026-09-18 发生过 **0019 号被两条不同决策占用**的真实事故
//! （ADR-0026）：建文件时没查预留表，两条决策静默共号，而 git 不会报错、人也不会天天核对。
//! 主键重复会让之后所有 `Supersedes` / `Superseded by` 链条失真。
//!
//! ## 边界（不做什么）
//! - 不做文件 IO：输入是文本与文件名，遍历与读写在 `main.rs`。
//! - 不检查「裸引用」（在 `docs/adr/` 之外写 `ADR-00NN` 而该号没有文件）：现存 2 处违规
//!   在 ADR 正文里，而 ADR 只增不改 → 现在实现就是一条永久红灯（ADR-0030 D4，归 PL-032）。
//! - 不校验 `Supersedes` 链条是否闭环（只查单向标注），ADR 超过 50 份时再评估。
//!
//! ## 不变量
//! 1. 全部函数是**纯函数**。
//! 2. 解析不到某个锚点时**返回空/None，由调用方报 Error**，绝不「当作没有登记项」而静默通过
//!    （铁律 1）：`adr_index` 会把「锚点缺失」单独报成一条 Error。
//! 3. 编号一律用 `u32`（4 位十进制），比较与求最大值都基于数值而非字符串，避免 `0018` 与 `18`
//!    被当成两个号。
//!
//! 相关：`docs/adr/0030-machine-verified-memory-counts-and-adr-index.md`、
//! `docs/adr/0026-adr-number-registry-and-0019-collision.md`、`docs/adr/README.md`

use crate::memory_table::split_table_row;

/// ADR 登记表路径（相对仓库根）。
pub const REGISTRY_PATH: &str = "docs/adr/README.md";

/// ADR 文件所在目录（相对仓库根）。
pub const ADR_DIR: &str = "docs/adr";

/// 待建号条目的落点（`[ADR:待建 NNNN]` 写在这里）。
pub const DECISIONS_PATH: &str = "docs/memory/decisions.md";

/// `decisions.md` 里预留编号的标记前缀（引用规范见 `docs/adr/README.md` §4）。
pub const PENDING_MARKER: &str = "[ADR:待建 ";

/// 退役清单行的标记（ADR-0030 D3 新增的机器可读约定）。
pub const RETIRED_MARKER: &str = "已退役编号：";

/// 「下一个可用编号」所在行的标记。
pub const NEXT_NUMBER_MARKER: &str = "下一个可用编号";

/// 登记表 §1（已存在文件）的章节标题前缀。
const SECTION_EXISTING: &str = "## 1.";

/// 登记表 §2（待建号）的章节标题前缀。
const SECTION_PENDING: &str = "## 2.";

/// 登记表 §3（已知事故 + 退役清单）的章节标题前缀。
const SECTION_INCIDENTS: &str = "## 3.";

/// ADR 状态行里允许出现的状态关键词（gov §9.3 模板）。
const STATUS_KEYWORDS: [&str; 6] = [
    "Accepted",
    "Proposed",
    "Draft",
    "Superseded",
    "Deprecated",
    "Rejected",
];

/// 一个 ADR 文件的机器可读摘要。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdrFileSummary {
    /// 文件名前缀里的 4 位编号（数值形式）。
    pub number: u32,
    /// 文件名（不含目录），例：`0018-nightly-automation-delivery-mechanism.md`。
    pub file_name: String,
    /// 状态行里**最早出现**的状态关键词；找不到状态行时为 `None`。
    pub status: Option<String>,
    /// 状态行的 1 基行号（找不到时为 0）。
    pub status_line_number: usize,
    /// `Superseded by：ADR-NNNN` 里的 NNNN；写 `—` 或没有该字段时为 `None`。
    pub superseded_by: Option<u32>,
}

/// 登记表 §1 表格里的一行（「已存在的 ADR 文件」）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryRow {
    /// 第 1 列的 4 位编号。
    pub number: u32,
    /// 第 3 列（状态列）的原文，保留 `**` 与箭头，供 `adr_index` 判断是否标了 Superseded。
    pub status_cell: String,
    /// 该行在登记表里的 1 基行号。
    pub line_number: usize,
}

/// 登记表的解析结果。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Registry {
    /// §1 的行（已存在文件的编号）。
    pub existing: Vec<RegistryRow>,
    /// §2 的编号（已决定但尚未写成文件的待建号）。
    pub pending: Vec<u32>,
    /// §3 的退役编号清单（曾预留、后被改号，**不得再分配**）。
    pub retired: Vec<u32>,
    /// 「下一个可用编号」；找不到该行为 `None`。
    pub next_available: Option<u32>,
    /// 缺失的锚点名称（供 `adr_index` 报 `adr/registry-section-missing`）。
    pub missing_anchors: Vec<String>,
}

/// 从文件名取 4 位 ADR 编号。
///
/// 判据：文件名以 4 个 ASCII 数字开头且第 5 个字符是 `-`。
/// `README.md` 这类非 ADR 文件返回 `None`（调用方据此把它排除在编号体系之外）。
#[must_use]
pub fn file_number(file_name: &str) -> Option<u32> {
    let head = file_name.get(..4)?;
    if !head.chars().all(|character| character.is_ascii_digit()) {
        return None;
    }
    if file_name.as_bytes().get(4) != Some(&b'-') {
        return None;
    }
    head.parse::<u32>().ok()
}

/// 解析 ADR 文件的状态行（`状态：…　Supersedes：…　Superseded by：…`）。
///
/// 取**最早出现**的状态关键词作为该文件的状态：状态行同时含有 `Superseded by：` 字段，
/// 若按数组顺序匹配而不是按出现位置，`Superseded` 会把 `Accepted` 顶掉。
///
/// # 返回
/// `(状态关键词, 状态行的 1 基行号, Superseded by 的编号)`；找不到状态行时三项分别为
/// `None` / `0` / `None`。
#[must_use]
pub fn parse_status_line(content: &str) -> (Option<String>, usize, Option<u32>) {
    let Some((line_number, line)) = content
        .lines()
        .enumerate()
        .find(|(_, line)| line.contains("状态："))
        .map(|(index, line)| (index + 1, line))
    else {
        return (None, 0, None);
    };
    let status = STATUS_KEYWORDS
        .iter()
        .filter_map(|keyword| line.find(keyword).map(|at| (at, (*keyword).to_string())))
        .min_by_key(|(at, _)| *at)
        .map(|(_, keyword)| keyword);
    let superseded_by = line
        .split("Superseded by：")
        .nth(1)
        .and_then(first_adr_number);
    (status, line_number, superseded_by)
}

/// 汇总一个 ADR 文件；文件名里没有 4 位编号时返回 `None`。
#[must_use]
pub fn summarize_adr_file(file_name: &str, content: &str) -> Option<AdrFileSummary> {
    let number = file_number(file_name)?;
    let (status, status_line_number, superseded_by) = parse_status_line(content);
    Some(AdrFileSummary {
        number,
        file_name: file_name.to_string(),
        status,
        status_line_number,
        superseded_by,
    })
}

/// 解析登记表全文。
///
/// 锚点缺失不会 panic 也不会静默通过：缺失项被收进 `Registry::missing_anchors`，
/// 由 `adr_index` 报成 Error。
#[must_use]
pub fn parse_registry(content: &str) -> Registry {
    let lines: Vec<&str> = content.lines().collect();
    let mut missing_anchors = Vec::new();
    let existing_head = require_anchor(&lines, SECTION_EXISTING, &mut missing_anchors);
    let pending_head = require_anchor(&lines, SECTION_PENDING, &mut missing_anchors);
    let incidents_head = require_anchor(&lines, SECTION_INCIDENTS, &mut missing_anchors);

    let mut registry = Registry {
        missing_anchors,
        ..Registry::default()
    };
    if let (Some(from), Some(until)) = (existing_head, pending_head) {
        registry.existing = parse_existing_rows(&lines, from + 1, until);
    }
    if let (Some(from), Some(until)) = (pending_head, incidents_head) {
        registry.pending = parse_number_column(&lines, from + 1, until);
    }
    // 退役清单与「下一个可用编号」是**整篇范围**的单行标记，不依赖章节边界
    registry.retired = content
        .lines()
        .find(|line| line.contains(RETIRED_MARKER))
        .map_or_else(Vec::new, retired_numbers);
    if !content.lines().any(|line| line.contains(RETIRED_MARKER)) {
        registry
            .missing_anchors
            .push(format!("以 `{RETIRED_MARKER}` 开头的退役清单行"));
    }
    registry.next_available = content
        .lines()
        .find(|line| line.contains(NEXT_NUMBER_MARKER))
        .and_then(first_adr_number);
    if registry.next_available.is_none() {
        registry
            .missing_anchors
            .push(format!("含 `{NEXT_NUMBER_MARKER}` 的行"));
    }
    registry
}

/// 找一个章节标题锚点；找不到就把名称记进 `missing_anchors` 并返回 `None`。
fn require_anchor(lines: &[&str], heading: &str, missing: &mut Vec<String>) -> Option<usize> {
    let found = lines.iter().position(|line| line.starts_with(heading));
    if found.is_none() {
        missing.push(format!("章节标题 `{heading}`"));
    }
    found
}

/// 解析 §1 表的数据行：第 1 列是编号、第 3 列是状态。
fn parse_existing_rows(lines: &[&str], from: usize, until: usize) -> Vec<RegistryRow> {
    let mut rows = Vec::new();
    let mut cursor = from;
    while cursor < until && cursor < lines.len() {
        let line = lines.get(cursor).copied().unwrap_or("");
        if line.starts_with('|') {
            let cells = split_table_row(line);
            if let Some(number) = cells.first().and_then(|cell| parse_number_cell(cell)) {
                rows.push(RegistryRow {
                    number,
                    status_cell: cells.get(2).cloned().unwrap_or_default(),
                    line_number: cursor + 1,
                });
            }
        }
        cursor += 1;
    }
    rows
}

/// 解析 §2 表的数据行：只取第 1 列的编号。
fn parse_number_column(lines: &[&str], from: usize, until: usize) -> Vec<u32> {
    let mut numbers = Vec::new();
    let mut cursor = from;
    while cursor < until && cursor < lines.len() {
        let line = lines.get(cursor).copied().unwrap_or("");
        if line.starts_with('|') {
            let cells = split_table_row(line);
            if let Some(number) = cells.first().and_then(|cell| parse_number_cell(cell)) {
                numbers.push(number);
            }
        }
        cursor += 1;
    }
    numbers.sort_unstable();
    numbers.dedup();
    numbers
}

/// 解析编号单元格：允许 `` `0018` ``、`0018`、`**0018**` 三种写法。
fn parse_number_cell(cell: &str) -> Option<u32> {
    let cleaned = cell.trim().trim_matches('`').trim_matches('*').trim();
    if cleaned.len() != 4 || !cleaned.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    cleaned.parse::<u32>().ok()
}

/// 取一段文本里**按出现顺序**的第一个 `NNNN`（4 位数字，且前后不再紧邻数字）。
///
/// 为什么要「前后不再紧邻数字」：`ADR-0019` 与 `00190` 必须区分开，
/// 否则从长编号里截出来的前 4 位会伪装成一个真实编号。
///
/// **为什么不复用 [`all_adr_numbers`] 再取首元素**：那个函数会升序去重（退役清单与待建号
/// 集合要的是「集合」语义），于是「第一个」会退化成「**最小的**」。对一行里可能出现多个编号的
/// 文本，这是错的，两个实测现场：
/// - `**下一个可用编号：0031**（= 已用最大号 0030 + 1）` → 取最小会读出 `0030`；
/// - `Superseded by：ADR-0029（另见 ADR-0018 的纪律性内容）` → 取最小会读出 `0018`。
///
/// 两处都是 2026-09-18 修 ADR 登记表时真实撞到的，故本函数按**文本顺序**取。
fn first_adr_number(text: &str) -> Option<u32> {
    four_digit_numbers_in_text_order(text).into_iter().next()
}

/// 按**出现顺序**取出全部 4 位编号（不排序、不去重，允许重复）。
///
/// 这是「扫描 4 位编号」的唯一实现：[`all_adr_numbers`]（集合语义）与 [`first_adr_number`]
/// （顺序语义）都建立在它之上，避免两套扫描逻辑各写一遍而口径漂移。
fn four_digit_numbers_in_text_order(text: &str) -> Vec<u32> {
    let characters: Vec<char> = text.chars().collect();
    let mut numbers = Vec::new();
    let mut index = 0usize;
    while index + 4 <= characters.len() {
        // 一律用 get()：crate 顶层 deny(indexing_slicing)，下标访问会 panic，而这里是纯文本解析
        let previous_is_digit = index
            .checked_sub(1)
            .and_then(|previous| characters.get(previous))
            .is_some_and(char::is_ascii_digit);
        let Some(window) = characters.get(index..index + 4) else {
            break; // while 条件已保证窗口存在；防御性退出而不是 panic
        };
        let window_is_digits = window.len() == 4 && window.iter().all(char::is_ascii_digit);
        let next_is_digit = characters.get(index + 4).is_some_and(char::is_ascii_digit);
        if !previous_is_digit && window_is_digits && !next_is_digit {
            let digits: String = window.iter().collect();
            if let Ok(number) = digits.parse::<u32>() {
                numbers.push(number);
            }
            index += 4; // 跳过整个编号：`00190027` 不该被读成两个号
        } else {
            index += 1;
        }
    }
    numbers
}

/// 取一段文本里全部 4 位编号（**升序去重**，集合语义）：退役清单与待建号集合用它。
///
/// 需要「第一个」时**不要**用本函数再 `.next()` —— 那会得到「最小的」，见 [`first_adr_number`]。
fn all_adr_numbers(text: &str) -> Vec<u32> {
    let mut numbers = four_digit_numbers_in_text_order(text);
    numbers.sort_unstable();
    numbers.dedup();
    numbers
}

/// 从退役清单行里取出**真正被退役**的编号。
///
/// 只读 `已退役编号：` 与第一个括号（全角 `（` 或半角 `(`）之间的那一段：
/// 括号里是给人看的说明，例如
/// `已退役编号：0019（原「#[allow] 位置」条目，已由 0027 取代）` ——
/// 其中的 `0027` 是**取代者**，不是退役号。
///
/// 若对整行取号，0027 会被误判成"已退役"，于是 `adr/retired-number-reallocated`
/// 会把一个完全合法的待建号报成「退役号被重新分配」。这类假阳性比漏报更伤：
/// 它让人不再相信护栏（狼来了），最终护栏形同虚设。
fn retired_numbers(line: &str) -> Vec<u32> {
    let after_marker = line.split(RETIRED_MARKER).nth(1).unwrap_or("");
    all_adr_numbers(declaration_before_parenthesis(after_marker))
}

/// 截到第一个括号之前（全角与半角都算）；没有括号就原样返回。
fn declaration_before_parenthesis(text: &str) -> &str {
    text.find(['（', '('])
        .map_or(text, |offset| text.get(..offset).unwrap_or(text))
}

/// 收集 `decisions.md` 里出现的全部 `[ADR:待建 NNNN]` 编号（升序去重）。
///
/// 注意：`decisions.md` 是**只追加**的，被改号的老条目（例如 0019）会永久留在文件里，
/// 因此调用方必须再减去登记表的退役清单（ADR-0030 D3）。
#[must_use]
pub fn collect_pending_numbers(decisions_content: &str) -> Vec<u32> {
    let mut numbers = Vec::new();
    let mut search_from = 0usize;
    while let Some(offset) = decisions_content[search_from..].find(PENDING_MARKER) {
        let absolute = search_from + offset + PENDING_MARKER.len();
        let tail = decisions_content.get(absolute..).unwrap_or("");
        if let Some(number) = first_four_digits(tail) {
            numbers.push(number);
        }
        search_from = absolute;
    }
    numbers.sort_unstable();
    numbers.dedup();
    numbers
}

/// 取文本开头的 4 位数字（`PENDING_MARKER` 之后紧跟的就是编号）。
fn first_four_digits(text: &str) -> Option<u32> {
    let head = text.get(..4)?;
    if !head.chars().all(|character| character.is_ascii_digit()) {
        return None;
    }
    head.parse::<u32>().ok()
}

#[cfg(test)]
// 测试体外置到 `adr_registry_tests.rs`（理由见该文件头）；`#[path]` 让它仍是本模块的私有单测。
#[path = "adr_registry_tests.rs"]
mod tests;
