//! # Rust 源码扫描（纯函数，无 IO）
//!
//! 职责：把一份 Rust 源码文本拆成两个视图 —— **注释列表** 与 **降噪后的代码**，
//! 供护栏规则（`hygiene.rs` / 未来的 `comments.rs`）做判定。
//!
//! ## 边界（不做什么）
//! - 不做任何文件 IO：输入是 `&str`，输出是值（IO 只在 `main.rs`）。
//! - 不是完整的 Rust 解析器：不构建 AST、不做名字解析、不校验语法。
//!   它只需要"足够准"地区分 **代码 / 注释 / 字符串字面量 / 字符字面量 / 生命周期**，
//!   因为护栏规则关心的是"某段文本到底是注释还是代码"。
//! - 不判定规则：本模块只回答"这是什么"，不回答"这算不算违规"。
//!
//! ## 不变量
//! 1. **行号保真**：`Scan::code` 与原源码的换行位置完全一致，因此 `code` 的第 N 行
//!    就是原文件的第 N 行（这样规则报出的行号可以直接给人看）。
//! 2. **长度保真**：`code.chars().count() == source.chars().count()`。被"抹掉"的
//!    注释/字面量内容一律替换为等量空格（换行除外），所以列号也保真。
//! 3. **确定性**：同一输入必得同一输出（无随机、无时间、无环境依赖）。
//! 4. **注释顺序**：`comments` 按出现顺序排列，行号单调不减。
//!
//! ## 为什么要有 `code`（降噪视图）
//! 直接对原始文本做子串匹配会误判：`let url = "http://x";` 里的 `//` 不是注释，
//! 文档里提到的「待办」字样若出现在字符串中也不该触发规则。抹平字面量与注释之后，
//! 剩下的就是"真正的代码"，规则可以放心地做模式匹配（TASK-015 的函数扫描也依赖它）。
//!
//! 相关：`docs/governance-ai-agent-execution.md` §5.4、`docs/spec/naming.md` §8/§10

/// 注释的种类。
///
/// 区分种类是必要的：`//` 与 `///`/`//!` 的语义完全不同 ——
/// 文档注释是**给人看的说明书**，被注释掉的代码只可能是 `//` 形式，
/// 所以「注释掉的代码块」规则只看 `Line`，不看文档注释。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentKind {
    /// 普通行注释：`// ...`
    Line,
    /// 条目文档注释：`/// ...` 或 `/** ... */`
    LineDoc,
    /// 模块文档注释：`//! ...` 或 `/*! ... */`
    ModuleDoc,
    /// 普通块注释：`/* ... */`
    Block,
}

/// 源码中的一条注释。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comment {
    /// 注释起始行，1 基（与编辑器显示一致）。
    pub line: usize,
    /// 注释种类。
    pub kind: CommentKind,
    /// 去掉起始标记（`//`、`///`、`//!`、`/*`）后的正文。
    ///
    /// 块注释的正文可能包含换行；行注释的正文不含换行。
    /// 结尾的 `*/` 不计入正文。
    pub text: String,
}

/// 一次扫描的完整结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scan {
    /// 全部注释，按出现顺序（不变量 4）。
    pub comments: Vec<Comment>,
    /// 降噪后的源码：注释与字面量内容被替换为等量空格（不变量 1、2）。
    pub code: String,
    /// 源码行数，与 `str::lines().count()` 一致（空串为 0，`"a\n"` 为 1）。
    pub line_count: usize,
}

/// 扫描 Rust 源码，得到注释列表与降噪代码。
///
/// 这是本模块唯一的对外入口；其余都是实现细节。
///
/// # 未闭合的注释/字面量
/// 扫描到文件尾即停止，**不报错**（语法错误由 `rustc` 负责报告，护栏工具不重复管这件事）。
/// 这样设计是为了让"半写坏的文件"也能被扫出已有问题，而不是让整个检查静默跳过。
#[must_use]
pub fn scan(source: &str) -> Scan {
    let mut scanner = Scanner::new(source);
    scanner.run();
    Scan {
        comments: scanner.comments,
        code: scanner.code,
        line_count: source.lines().count(),
    }
}

/// 一个可执行函数在源码中的稳定形状。
///
/// 这是 TASK-085 的五条源码结构规则共用的只读视图。字段全部来自**降噪后的代码**
/// （注释与字面量已抹平），因此函数体判据不会被字符串或注释中的 `if` / `?` 污染。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionSpan {
    /// 函数名；不包含泛型参数或参数列表。
    pub name: String,
    /// `fn` 所在行，1 基。
    pub start_line: usize,
    /// 函数体右花括号所在行，1 基。
    pub end_line: usize,
    /// 顶层参数个数；`self` / `&self` 各计一个。
    pub parameter_count: usize,
    /// 起始复杂度 1 + 分支计数（`if` / `match` / 循环 / `&&` / `||` / `?`）。
    pub cyclomatic_complexity: usize,
    /// 是否为测试函数（`#[test]` / `#[tokio::test]` / `#[rstest]` / `#[test_case]`）。
    pub is_test: bool,
    /// 紧邻函数的 `#[ignore]` 或 `.skip` 属性原文；没有则为 `None`。
    pub ignore_attribute: Option<String>,
    /// 降噪后的函数体，包含左右花括号。
    pub body: String,
}

/// 扫描源码中的可执行函数与紧邻属性。
///
/// 扫描器刻意只做**结构性近似**：它不构建 AST，也不尝试解析宏展开。目标是把
/// `fn` 边界、顶层参数、函数体和紧邻属性切成稳定结构，供纯规则函数判定。
/// 不含函数体的 trait 声明会被跳过，因为后续五条规则只关心可执行实现。
#[must_use]
pub fn scan_functions(source: &str) -> Vec<FunctionSpan> {
    let scanned = scan(source);
    let code_lines: Vec<&str> = scanned.code.lines().collect();
    let source_lines: Vec<&str> = source.lines().collect();
    let mut functions = Vec::new();

    for (index, code_line) in code_lines.iter().enumerate() {
        let Some((name, fn_offset)) = function_name(code_line) else {
            continue;
        };
        let Some((body_line, body_column)) = body_start(&code_lines, index, fn_offset) else {
            continue;
        };
        let Some((end_line, end_column)) = body_end(&code_lines, body_line, body_column) else {
            continue;
        };

        let signature = code_fragment(&code_lines, index, fn_offset, body_line, body_column);
        let body = code_fragment(
            &code_lines,
            body_line,
            body_column,
            end_line,
            end_column + 1,
        );
        let attributes = preceding_attributes(&source_lines, &code_lines, index + 1);
        let is_test = attributes.as_deref().is_some_and(|text| {
            text.contains("#[test")
                || text.contains("#[tokio::test")
                || text.contains("#[async_std::test")
                || text.contains("#[rstest")
                || text.contains("#[test_case")
        });
        let ignore_attribute =
            attributes.filter(|text| text.contains("#[ignore") || text.contains(".skip"));

        functions.push(FunctionSpan {
            name,
            start_line: index + 1,
            end_line: end_line + 1,
            parameter_count: count_parameters(&signature),
            cyclomatic_complexity: 1 + count_complexity(&body),
            is_test,
            ignore_attribute,
            body,
        });
    }

    functions
}

/// 返回函数名与 `fn` 关键字在行内的字节偏移。
fn function_name(line: &str) -> Option<(String, usize)> {
    let mut cursor = 0;
    while let Some(found) = line.get(cursor..)?.find("fn") {
        let offset = cursor + found;
        let before = line.get(..offset).and_then(|text| text.chars().next_back());
        let after = line.get(offset + 2..).and_then(|text| text.chars().next());
        let is_token = before
            .is_none_or(|character| !(character.is_alphanumeric() || character == '_'))
            && after.is_none_or(|character| !(character.is_alphanumeric() || character == '_'));
        if is_token {
            let remainder = line.get(offset + 2..).unwrap_or_default().trim_start();
            let name: String = remainder
                .chars()
                .take_while(|character| {
                    !matches!(character, '(' | '<' | ':' | ' ' | ';' | '{' | ',' | ')')
                })
                .collect();
            if !name.is_empty() {
                return Some((name, offset));
            }
        }
        cursor = offset + 2;
    }
    None
}

/// 从 `fn` 所在行开始寻找函数体左花括号。
fn body_start(code_lines: &[&str], start_line: usize, fn_offset: usize) -> Option<(usize, usize)> {
    for (line_index, line) in code_lines.iter().enumerate().skip(start_line) {
        let start = if line_index == start_line {
            fn_offset + 2
        } else {
            0
        };
        if let Some(offset) = line.get(start..)?.find('{') {
            return Some((line_index, start + offset));
        }
    }
    None
}

/// 从函数体左花括号开始做大括号配对，返回右花括号位置。
fn body_end(code_lines: &[&str], start_line: usize, start_column: usize) -> Option<(usize, usize)> {
    let mut depth = 0usize;
    for (line_index, line) in code_lines.iter().enumerate().skip(start_line) {
        let start = if line_index == start_line {
            start_column
        } else {
            0
        };
        let segment = line.get(start..)?;
        for (offset, character) in segment.char_indices() {
            match character {
                '{' => depth += 1,
                '}' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        return Some((line_index, start + offset));
                    }
                }
                _ => {}
            }
        }
    }
    None
}

/// 收集从 `start` 到 `end` 的降噪代码片段，换行保留。
fn code_fragment(
    code_lines: &[&str],
    start_line: usize,
    start_column: usize,
    end_line: usize,
    end_column: usize,
) -> String {
    let mut fragment = String::new();
    for (line_index, line) in code_lines.iter().enumerate() {
        if line_index < start_line || line_index > end_line {
            continue;
        }
        let start = if line_index == start_line {
            start_column
        } else {
            0
        };
        let end = if line_index == end_line {
            end_column.min(line.len())
        } else {
            line.len()
        };
        if let Some(piece) = line.get(start..end) {
            fragment.push_str(piece);
        }
        if line_index < end_line {
            fragment.push('\n');
        }
    }
    fragment
}

/// 收集紧邻函数的属性块；字符串内容可能已被降噪，但属性关键字仍在。
fn preceding_attributes(
    source_lines: &[&str],
    code_lines: &[&str],
    function_line: usize,
) -> Option<String> {
    let mut index = function_line.checked_sub(1)?;
    let mut lines = Vec::new();
    let mut steps = 0usize;
    while index > 0 && steps < 12 {
        index -= 1;
        steps += 1;
        let code = code_lines.get(index).map_or("", |line| line.trim());
        if code.is_empty() {
            continue;
        }
        if code.starts_with('#') || code.ends_with(']') {
            if let Some(line) = source_lines.get(index) {
                lines.push((*line).to_string());
            }
            continue;
        }
        break;
    }
    lines.reverse();
    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
    }
}

/// 数顶层参数；支持跨行签名、泛型与 `self`。
fn count_parameters(signature: &str) -> usize {
    let Some(open_offset) = signature.find('(') else {
        return 0;
    };
    let mut depth = 0usize;
    let mut commas = 0usize;
    let mut has_parameter = false;
    let mut last_non_whitespace = None;
    for character in signature.get(open_offset + 1..).unwrap_or_default().chars() {
        match character {
            '(' | '[' | '{' => {
                depth += 1;
                has_parameter = true;
                last_non_whitespace = Some(character);
            }
            ')' if depth == 0 => break,
            ')' | ']' | '}' => {
                depth = depth.saturating_sub(1);
                has_parameter = true;
                last_non_whitespace = Some(character);
            }
            ',' if depth == 0 => {
                commas += 1;
                has_parameter = true;
                last_non_whitespace = Some(character);
            }
            character if !character.is_whitespace() => {
                has_parameter = true;
                last_non_whitespace = Some(character);
            }
            _ => {}
        }
    }
    if !has_parameter {
        return 0;
    }
    if last_non_whitespace == Some(',') {
        commas
    } else {
        commas + 1
    }
}

/// 估算函数体分支数；复杂度的基数由 `scan_functions` 加 1。
fn count_complexity(body: &str) -> usize {
    let mut complexity = 0usize;
    for token in body.split(|character: char| !(character.is_alphanumeric() || character == '_')) {
        if matches!(token, "if" | "match" | "while" | "for" | "loop") {
            complexity += 1;
        }
    }
    complexity += body.match_indices("=>").count();
    complexity += body.match_indices("&&").count();
    complexity += body.match_indices("||").count();
    complexity += body.matches('?').count();
    complexity
}

// ---------------------------------------------------------------------------
// 实现：单遍字符状态机
// ---------------------------------------------------------------------------

/// 单遍扫描器。
///
/// 用 `Vec<char>` 而不是字节切片，是为了 ① 天然支持 UTF-8（注释里全是中文）
/// ② 完全避开下标切片（`clippy::indexing_slicing` 是 deny 级），所有访问都走 `get()`。
struct Scanner {
    /// 源码字符序列。
    chars: Vec<char>,
    /// 当前读取位置（`chars` 的下标）。
    pos: usize,
    /// 当前行号，1 基。
    line: usize,
    /// 降噪代码输出缓冲。
    code: String,
    /// 已收集的注释。
    comments: Vec<Comment>,
}

impl Scanner {
    /// 创建扫描器（不消费任何字符）。
    fn new(source: &str) -> Self {
        Self {
            chars: source.chars().collect(),
            pos: 0,
            line: 1,
            code: String::with_capacity(source.len()),
            comments: Vec::new(),
        }
    }

    /// 向前看第 `offset` 个字符（越界返回 `None`，不 panic）。
    fn peek(&self, offset: usize) -> Option<char> {
        self.chars.get(self.pos + offset).copied()
    }

    /// 当前位置是否处于一个 token 的起点（前一个字符不是标识符字符）。
    ///
    /// 用于避免把标识符尾部的 `b` / `r` 误认成字面量前缀（如 `number"..."`）。
    fn is_at_token_start(&self) -> bool {
        match self.pos {
            0 => true,
            n => self
                .chars
                .get(n - 1)
                .is_none_or(|c| !(c.is_alphanumeric() || *c == '_')),
        }
    }

    /// 原样输出当前字符（用于代码区）。
    fn emit_self(&mut self) {
        if let Some(c) = self.chars.get(self.pos).copied() {
            self.pos += 1;
            if c == '\n' {
                self.line += 1;
            }
            self.code.push(c);
        }
    }

    /// 把当前字符替换为空格输出（用于注释与字面量区）；换行原样保留以维持行号。
    ///
    /// 返回被消费的字符，供调用方累积注释正文。
    fn emit_blank(&mut self) -> Option<char> {
        let c = self.chars.get(self.pos).copied()?;
        self.pos += 1;
        if c == '\n' {
            self.line += 1;
            self.code.push('\n');
        } else {
            self.code.push(' ');
        }
        Some(c)
    }

    /// 主循环：不断尝试各种 token，都不匹配则按普通代码字符处理。
    fn run(&mut self) {
        while self.pos < self.chars.len() {
            if self.try_line_comment()
                || self.try_block_comment()
                || self.try_raw_string()
                || self.try_char_literal()
                || self.try_string()
            {
                continue;
            }
            self.emit_self();
        }
    }

    /// 尝试识别行注释 `//`、`///`、`//!`。命中返回 true。
    fn try_line_comment(&mut self) -> bool {
        if self.peek(0) != Some('/') || self.peek(1) != Some('/') {
            return false;
        }
        let start_line = self.line;
        self.emit_blank();
        self.emit_blank();
        // `////` 在 Rust 里是普通行注释（正文以 `//` 开头），只有 `///` 后不接 `/` 才是文档注释。
        let kind = if self.peek(0) == Some('/') && self.peek(1) != Some('/') {
            self.emit_blank();
            CommentKind::LineDoc
        } else if self.peek(0) == Some('!') {
            self.emit_blank();
            CommentKind::ModuleDoc
        } else {
            CommentKind::Line
        };
        let mut text = String::new();
        while let Some(c) = self.peek(0) {
            if c == '\n' {
                break;
            }
            text.push(c);
            self.emit_blank();
        }
        self.comments.push(Comment {
            line: start_line,
            kind,
            text,
        });
        true
    }

    /// 尝试识别块注释 `/* ... */`（支持 Rust 的嵌套块注释）。命中返回 true。
    fn try_block_comment(&mut self) -> bool {
        if self.peek(0) != Some('/') || self.peek(1) != Some('*') {
            return false;
        }
        let start_line = self.line;
        self.emit_blank();
        self.emit_blank();
        let kind = if self.peek(0) == Some('!') {
            self.emit_blank();
            CommentKind::ModuleDoc
        } else if self.peek(0) == Some('*') && self.peek(1) != Some('/') {
            self.emit_blank();
            CommentKind::LineDoc
        } else {
            CommentKind::Block
        };
        let mut depth = 1usize;
        let mut text = String::new();
        while depth > 0 {
            match self.peek(0) {
                None => break,
                Some('/') if self.peek(1) == Some('*') => {
                    depth += 1;
                    self.emit_blank();
                    self.emit_blank();
                    text.push_str("/*");
                }
                Some('*') if self.peek(1) == Some('/') => {
                    depth -= 1;
                    self.emit_blank();
                    self.emit_blank();
                    // 最外层的 `*/` 是标记，不算正文（不变量：Comment::text 不含结尾标记）
                    if depth > 0 {
                        text.push_str("*/");
                    }
                }
                Some(_) => {
                    if let Some(c) = self.emit_blank() {
                        text.push(c);
                    }
                }
            }
        }
        self.comments.push(Comment {
            line: start_line,
            kind,
            text,
        });
        true
    }

    /// 尝试识别原始字符串 `r"..."` / `r#"..."#` / `br#"..."#`。命中返回 true。
    fn try_raw_string(&mut self) -> bool {
        if !self.is_at_token_start() {
            return false;
        }
        // 字节串前缀 `b` 会把 `r` 后移一位；用 usize::from 表达「0 或 1」比 if/else 更直白
        let prefix_offset = usize::from(self.peek(0) == Some('b'));
        if self.peek(prefix_offset) != Some('r') {
            return false;
        }
        let mut hashes = 0usize;
        while self.peek(prefix_offset + 1 + hashes) == Some('#') {
            hashes += 1;
        }
        if self.peek(prefix_offset + 1 + hashes) != Some('"') {
            return false;
        }
        // 消费前缀：可选 `b`、`r`、若干 `#`、以及开引号
        for _ in 0..=(prefix_offset + 1 + hashes) {
            self.emit_blank();
        }
        // 消费正文，直到遇到「`"` 后紧跟 hashes 个 `#`」
        loop {
            match self.peek(0) {
                None => break,
                Some('"') => {
                    let mut matched = 1usize;
                    while matched <= hashes && self.peek(matched) == Some('#') {
                        matched += 1;
                    }
                    if matched == hashes + 1 {
                        for _ in 0..matched {
                            self.emit_blank();
                        }
                        break;
                    }
                    self.emit_blank();
                }
                Some(_) => {
                    self.emit_blank();
                }
            }
        }
        true
    }

    /// 尝试识别普通字符串 `"..."` / `b"..."`（含 `\` 转义）。命中返回 true。
    fn try_string(&mut self) -> bool {
        // 只有处在 token 起点时才可能是 `b"..."`，否则 `b` 只是某个标识符的尾字符
        let prefix_offset = usize::from(
            self.is_at_token_start() && self.peek(0) == Some('b') && self.peek(1) == Some('"'),
        );
        if self.peek(prefix_offset) != Some('"') {
            return false;
        }
        for _ in 0..=prefix_offset {
            self.emit_blank();
        }
        loop {
            match self.peek(0) {
                None => break,
                Some('"') => {
                    self.emit_blank();
                    break;
                }
                Some('\\') => {
                    // 转义序列：整体抹掉两个字符；行尾续行（`\` + 换行）也在此路径内，
                    // emit_blank 会保留换行，因此行号仍然保真。
                    self.emit_blank();
                    self.emit_blank();
                }
                Some(_) => {
                    self.emit_blank();
                }
            }
        }
        true
    }

    /// 尝试识别字符字面量 `'x'` / `b'x'` / `'\''`。命中返回 true。
    ///
    /// **生命周期不是字面量**：`'a`、`'static` 后面不会紧跟 `'`，此时返回 false，
    /// 让主循环把 `'` 当普通代码字符输出（否则会误抹掉类型签名的一部分）。
    fn try_char_literal(&mut self) -> bool {
        let prefix_offset = usize::from(
            self.is_at_token_start() && self.peek(0) == Some('b') && self.peek(1) == Some('\''),
        );
        if self.peek(prefix_offset) != Some('\'') {
            return false;
        }
        let close = match self.peek(prefix_offset + 1) {
            None => return false,
            Some('\\') => match self.escape_end(prefix_offset + 2) {
                Some(index) => index,
                None => return false,
            },
            Some(_) => prefix_offset + 2,
        };
        if self.peek(close) != Some('\'') {
            return false;
        }
        for _ in 0..=close {
            self.emit_blank();
        }
        true
    }

    /// 已知 `at` 指向转义符 `\` 之后的第一个字符，求转义序列结束后的位置。
    ///
    /// 覆盖 Rust 的全部字符转义：`\n \r \t \\ \0 \' \"`、`\xNN`、`\u{...}`。
    fn escape_end(&self, at: usize) -> Option<usize> {
        match self.chars.get(at)? {
            'x' => Some(at + 3),
            'u' => {
                let mut i = at + 1;
                loop {
                    match self.chars.get(i)? {
                        '}' => return Some(i + 1),
                        _ => i += 1,
                    }
                }
            }
            _ => Some(at + 1),
        }
    }
}

#[cfg(test)]
#[path = "rustscan_tests.rs"]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests;
