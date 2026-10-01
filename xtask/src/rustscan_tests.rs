//! `rustscan.rs` 的私有单元测试（TASK-085 外置）：扫描器与助手。

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

// --- TASK-085：函数与属性扫描 ---

#[test]
fn test_scan_functions_extracts_name_and_bounds() {
    let source = "fn outer() {\n    fn inner() {}\n}\n";
    let functions = scan_functions(source);
    assert_eq!(functions.len(), 2);
    let outer = functions.first().expect("outer should exist");
    assert_eq!(outer.name, "outer");
    assert_eq!(outer.start_line, 1);
    assert_eq!(outer.end_line, 3);
    assert!(outer.body.contains("fn inner"));
    let inner = functions.get(1).expect("nested function should exist");
    assert_eq!(inner.name, "inner");
    assert_eq!(inner.start_line, 2);
}

#[test]
fn test_scan_functions_counts_parameters_across_lines() {
    let source = "fn many(\n    first: u8,\n    second: u8,\n    third: u8,\n) -> u8 { first + second + third }\n";
    let functions = scan_functions(source);
    let function = functions.first().expect("function should exist");
    assert_eq!(function.parameter_count, 3);
    assert_eq!(function.cyclomatic_complexity, 1);
}

#[test]
fn test_scan_functions_counts_complexity_and_test_attribute() {
    let source = "#[test]\n#[ignore = \"TASK-085: windows-only\"]\nfn sample(value: bool) -> Result<(), ()> {\n    if value && value { return Ok(()); }\n    match value { true => Ok(()), false => Err(())? }\n}\n";
    let functions = scan_functions(source);
    let function = functions.first().expect("function should exist");
    assert!(function.is_test);
    assert!(
        function
            .ignore_attribute
            .as_deref()
            .is_some_and(|text| text.contains("TASK-085"))
    );
    assert!(function.cyclomatic_complexity >= 5);
}

#[test]
fn test_scan_functions_skips_trait_declaration() {
    let source = "trait Example { fn declared(&self); }\n";
    assert!(scan_functions(source).is_empty());
}

#[test]
fn test_scan_functions_empty_source_returns_empty() {
    assert!(scan_functions("").is_empty());
}
