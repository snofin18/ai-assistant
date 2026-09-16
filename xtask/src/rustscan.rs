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
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    /// 取出第 `index` 条注释（越界时给出可读的失败信息，便于定位）。
    fn nth(scan: &Scan, index: usize) -> &Comment {
        scan.comments.get(index).unwrap_or_else(|| {
            panic!(
                "期望至少 {} 条注释，实际 {} 条：{:?}",
                index + 1,
                scan.comments.len(),
                scan.comments
            )
        })
    }

    // --- 不变量 1/2：行号与长度保真 ---

    #[test]
    fn test_scan_blank_replacement_preserves_char_count() {
        let source = "let s = \"abcd\";\n// 12345\n";
        let result = scan(source);
        assert_eq!(
            result.code.chars().count(),
            source.chars().count(),
            "不变量 2：长度必须保真"
        );
    }

    #[test]
    fn test_scan_newline_positions_are_preserved() {
        let source = "a\n// c\nb\n";
        let result = scan(source);
        let code_lines: Vec<&str> = result.code.lines().collect();
        assert_eq!(
            code_lines.len(),
            source.lines().count(),
            "不变量 1：行数必须一致"
        );
        assert_eq!(
            code_lines.get(1).copied(),
            Some("    "),
            "注释整行应被抹成等量空格（`// c` = 4 字符）"
        );
        assert_eq!(result.line_count, 3);
    }

    #[test]
    fn test_scan_empty_source_yields_empty_result() {
        let result = scan("");
        assert_eq!(result.line_count, 0);
        assert!(result.comments.is_empty());
        assert!(result.code.is_empty());
    }

    // --- 行注释种类 ---

    #[test]
    fn test_line_comment_kinds_are_distinguished() {
        let source = "// 普通\n/// 文档\n//! 模块\n//// 四个斜杠\n";
        let result = scan(source);
        assert_eq!(result.comments.len(), 4);
        assert_eq!(nth(&result, 0).kind, CommentKind::Line);
        assert_eq!(nth(&result, 1).kind, CommentKind::LineDoc);
        assert_eq!(nth(&result, 2).kind, CommentKind::ModuleDoc);
        assert_eq!(
            nth(&result, 3).kind,
            CommentKind::Line,
            "`////` 是普通注释，正文以 // 开头"
        );
        assert_eq!(nth(&result, 3).text, "// 四个斜杠");
    }

    #[test]
    fn test_line_comment_reports_one_based_line_number() {
        let source = "fn main() {}\n\n// 第三行\n";
        let result = scan(source);
        assert_eq!(nth(&result, 0).line, 3);
    }

    // --- 字符串中的假注释 ---

    #[test]
    fn test_double_slash_inside_string_is_not_a_comment() {
        let source = "let url = \"http://example.com\";\n";
        let result = scan(source);
        assert!(result.comments.is_empty(), "字符串里的 // 不能被当成注释");
        assert!(result.code.contains("let url ="), "字符串外的代码应保留");
        assert!(!result.code.contains("http"), "字符串内容应被抹掉");
    }

    #[test]
    fn test_escaped_quote_does_not_end_string() {
        let source = "let s = \"a\\\"// b\";\n// 真注释\n";
        let result = scan(source);
        assert_eq!(result.comments.len(), 1, "只应识别出末尾那一条真注释");
        assert_eq!(nth(&result, 0).text, " 真注释");
    }

    #[test]
    fn test_raw_string_with_hashes_swallows_slashes() {
        let source = "let s = r#\"// 不是注释 /* 也不是 */\"#;\n// 真注释\n";
        let result = scan(source);
        assert_eq!(result.comments.len(), 1);
        assert_eq!(nth(&result, 0).line, 2);
    }

    // --- 块注释 ---

    #[test]
    fn test_block_comment_is_captured_with_text() {
        let source = "/* 第一行\n第二行 */\nlet x = 1;\n";
        let result = scan(source);
        assert_eq!(result.comments.len(), 1);
        assert_eq!(nth(&result, 0).kind, CommentKind::Block);
        assert!(nth(&result, 0).text.contains("第二行"));
        assert!(!nth(&result, 0).text.contains("*/"), "结尾标记不应计入正文");
    }

    #[test]
    fn test_nested_block_comments_are_balanced() {
        let source = "/* 外 /* 内 */ 仍在外 */\nlet x = 1;\n";
        let result = scan(source);
        assert_eq!(result.comments.len(), 1, "嵌套块注释应算一条");
        assert!(result.code.contains("let x = 1;"), "嵌套结束后应回到代码区");
    }

    #[test]
    fn test_doc_block_comment_kinds() {
        let result = scan("/** 文档 */\n/*! 模块 */\n");
        assert_eq!(nth(&result, 0).kind, CommentKind::LineDoc);
        assert_eq!(nth(&result, 1).kind, CommentKind::ModuleDoc);
    }

    #[test]
    fn test_unterminated_block_comment_does_not_panic() {
        let result = scan("/* 没有结尾");
        assert_eq!(
            result.comments.len(),
            1,
            "扫到文件尾即停止，不报错（见 scan 文档）"
        );
    }

    // --- 字符字面量 vs 生命周期 ---

    #[test]
    fn test_char_literal_is_blanked() {
        let result = scan("let c = 'x';\n");
        assert!(!result.code.contains("'x'"));
        assert!(result.code.contains("let c ="));
    }

    #[test]
    fn test_lifetime_is_kept_as_code() {
        let source = "fn f<'a>(x: &'a str) -> &'a str { x }\n";
        let result = scan(source);
        assert!(result.code.contains("'a"), "生命周期属于代码，不能被抹掉");
        assert!(result.comments.is_empty());
    }

    #[test]
    fn test_escaped_quote_char_literal_is_recognized() {
        let source = "let c = '\\'';\nlet d = 'y';\n";
        let result = scan(source);
        assert!(
            !result.code.contains("'y'"),
            "转义引号之后的字面量仍应被正确识别"
        );
    }

    #[test]
    fn test_byte_string_and_byte_char_prefixes() {
        let result = scan("let a = b\"//x\";\nlet b = b'y';\n");
        assert!(result.comments.is_empty(), "字节字面量中的 // 不是注释");
    }

    // --- 便捷封装 ---

    #[test]
    fn test_scan_separates_comment_from_adjacent_code() {
        let source = "// c\nlet s = \"x\";\n";
        let result = scan(source);
        assert_eq!(result.comments.len(), 1);
        assert_eq!(nth(&result, 0).text, " c");
        assert!(result.code.contains("let s ="), "代码部分应原样保留");
    }
}
