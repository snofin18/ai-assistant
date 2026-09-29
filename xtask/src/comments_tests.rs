//! `comments.rs` 的单元测试：naming §10 八条规则的白盒测试。
//!
//! **为什么单独一个文件**：`comments.rs` 连同测试会超过 gov §5.4 的 600 行警告阈值。
//! 用 `#[path]` 把 `mod tests` 外置后两侧都回到阈值内，而测试**仍然是本模块的私有单元测试** ——
//! `use super::` 照旧能访问私有项，这与 `tests/` 目录下的集成测试有本质区别。
//! 声明处在 `comments.rs` 末尾：`#[cfg(test)] #[path = "comments_tests.rs"] mod tests;`。

use super::{
    TOTAL_COMMENT_RULE_COUNT, check_abbreviations, check_controlled_vocabulary,
    check_module_header, check_pitfall_format, check_public_api_docs, check_rust_source,
    check_unsafe_safety, rule_coverage,
};
use crate::report::Severity;
use crate::rustscan::scan;

fn rules_of(findings: &[crate::report::Finding]) -> Vec<&'static str> {
    findings.iter().map(|finding| finding.rule).collect()
}

#[test]
fn test_rule_coverage_covers_every_numbered_rule() {
    let coverage = rule_coverage();
    assert!(!coverage.is_empty());
    for number in 1..=TOTAL_COMMENT_RULE_COUNT {
        assert!(
            coverage.iter().any(|entry| usize::from(entry.0) == number),
            "规则 {number} 必须在 rule_coverage() 里有实现位置，否则就是静默跳过"
        );
    }
}

#[test]
fn test_public_fn_without_doc_is_error() {
    let source = "pub fn list_windows() {}\n";
    let findings = check_public_api_docs("a.rs", &scan(source));
    assert_eq!(rules_of(&findings), vec!["comments/public-api-missing-doc"]);
    assert_eq!(
        findings.first().map(|finding| finding.severity),
        Some(Severity::Error)
    );
}

#[test]
fn test_public_fn_with_doc_comment_passes() {
    let source = "/// 列出窗口。\npub fn list_windows() {}\n";
    assert!(check_public_api_docs("a.rs", &scan(source)).is_empty());
}

#[test]
fn test_attribute_between_doc_and_item_still_counts_as_documented() {
    let source = "/// 列出窗口。\n#[must_use]\npub fn list_windows() {}\n";
    assert!(check_public_api_docs("a.rs", &scan(source)).is_empty());
}

#[test]
fn test_doc_attribute_counts_as_documented() {
    // 宏里只能用 `#[doc = ...]`，不能挂 `///`。
    let source = "#[doc = concat!(\"Validated \", \"id\")]\npub struct TaskId(String);\n";
    assert!(check_public_api_docs("a.rs", &scan(source)).is_empty());
}

#[test]
fn test_previous_item_doc_does_not_leak_to_next_item() {
    // 反向用例：第二项没有文档，且与上一项之间只有空行 —— 不能借用上一项的文档。
    let source = "/// 第一项。\npub fn first() {}\n\npub fn second() {}\n";
    let findings = check_public_api_docs("a.rs", &scan(source));
    assert_eq!(findings.len(), 1);
    assert_eq!(findings.first().map(|finding| finding.line), Some(4));
}

#[test]
fn test_private_fn_does_not_need_doc() {
    let source = "fn helper() {}\n";
    assert!(check_public_api_docs("a.rs", &scan(source)).is_empty());
}

#[test]
fn test_module_header_missing_is_error_in_strict_crate() {
    let source = "pub fn run() {}\n";
    let findings = check_module_header("crates/core/src/lib.rs", source, &scan(source));
    assert_eq!(rules_of(&findings), vec!["comments/module-header-missing"]);
    assert_eq!(
        findings.first().map(|finding| finding.severity),
        Some(Severity::Error)
    );
}

#[test]
fn test_module_header_missing_is_warning_outside_strict_crate() {
    let source = "pub fn run() {}\n";
    let findings = check_module_header("xtask/src/comments.rs", source, &scan(source));
    assert_eq!(
        findings.first().map(|finding| finding.severity),
        Some(Severity::Warning)
    );
}

#[test]
fn test_module_header_present_passes() {
    let source = "//! 职责说明。\n\npub fn run() {}\n";
    assert!(check_module_header("crates/core/src/lib.rs", source, &scan(source)).is_empty());
}

#[test]
fn test_empty_file_is_not_missing_module_header() {
    assert!(check_module_header("crates/core/src/empty.rs", "", &scan("")).is_empty());
}

#[test]
fn test_unsafe_without_safety_comment_is_error() {
    let source = "fn run() {\n    unsafe { do_thing(); }\n}\n";
    let findings = check_unsafe_safety("a.rs", &scan(source));
    assert_eq!(rules_of(&findings), vec!["comments/unsafe-without-safety"]);
    assert_eq!(findings.first().map(|finding| finding.line), Some(2));
}

#[test]
fn test_unsafe_with_safety_comment_passes() {
    let source =
        "fn run() {\n    // SAFETY: 指针来自已验证的缓冲区\n    unsafe { do_thing(); }\n}\n";
    assert!(check_unsafe_safety("a.rs", &scan(source)).is_empty());
}

#[test]
fn test_multi_line_safety_comment_passes() {
    // 回归用例：`// SAFETY:` 的续行紧邻 unsafe，只看上一行会误报。
    let source = "fn run() {\n    // SAFETY: 结论在首行\n    // 理由写在续行。\n    unsafe { do_thing(); }\n}\n";
    assert!(check_unsafe_safety("a.rs", &scan(source)).is_empty());
}

#[test]
fn test_safety_comment_group_covers_consecutive_statements() {
    // 仓库既有写法：一条 `// SAFETY:` 说明紧跟其后的连续 unsafe 语句组。
    let source = "fn run() {\n    // SAFETY: 以下均为只读查询。\n    let a = unsafe { read_a() };\n    let b = unsafe { read_b() };\n}\n";
    assert!(check_unsafe_safety("a.rs", &scan(source)).is_empty());
}

#[test]
fn test_blank_line_breaks_the_safety_group() {
    // 反向用例：空行之后是新的语句组，上一条 SAFETY 不能覆盖它。
    let source = "fn run() {\n    // SAFETY: 上一组的说明\n    let a = unsafe { read_a() };\n\n    unsafe { do_thing(); }\n}\n";
    assert_eq!(check_unsafe_safety("a.rs", &scan(source)).len(), 1);
}

#[test]
fn test_identifier_containing_unsafe_is_not_flagged() {
    let source = "fn run() {\n    let unsafe_code_allowed = true;\n}\n";
    assert!(check_unsafe_safety("a.rs", &scan(source)).is_empty());
}

#[test]
fn test_pitfall_without_scope_is_warning() {
    let scanned = scan("// PITFALL: 没有作用域\n");
    let findings = check_pitfall_format("a.rs", &scanned.comments);
    assert_eq!(rules_of(&findings), vec!["comments/pitfall-format"]);
    assert_eq!(
        findings.first().map(|finding| finding.severity),
        Some(Severity::Warning)
    );
}

#[test]
fn test_pitfall_with_app_scope_passes() {
    let scanned = scan("// PITFALL(app=excel): COM 会清空 undo 栈\n");
    assert!(check_pitfall_format("a.rs", &scanned.comments).is_empty());
}

#[test]
fn test_pitfall_with_platform_scope_passes() {
    let scanned = scan("// PITFALL(platform=windows): 前台校验\n");
    assert!(check_pitfall_format("a.rs", &scanned.comments).is_empty());
}

#[test]
fn test_banned_synonym_in_declaration_is_warning() {
    let source = "pub fn target_element() {}\n";
    let findings = check_controlled_vocabulary("a.rs", &scan(source));
    assert_eq!(rules_of(&findings), vec!["comments/controlled-vocabulary"]);
    assert_eq!(
        findings.first().map(|finding| finding.severity),
        Some(Severity::Warning)
    );
}

#[test]
fn test_camel_case_synonym_is_detected() {
    let source = "pub struct TargetElement;\n";
    assert_eq!(
        check_controlled_vocabulary("a.rs", &scan(source)).len(),
        1,
        "CamelCase 也要切词，否则只会抓 snake_case"
    );
}

#[test]
fn test_controlled_term_without_synonym_passes() {
    let source = "pub fn resolve_target_descriptor() {}\n";
    assert!(check_controlled_vocabulary("a.rs", &scan(source)).is_empty());
}

#[test]
fn test_vowelless_stem_is_warning() {
    let source = "pub fn fetch_tgt() {}\n";
    let findings = check_abbreviations("a.rs", &scan(source));
    assert_eq!(rules_of(&findings), vec!["comments/abbreviation"]);
}

#[test]
fn test_const_fn_keyword_is_not_treated_as_name() {
    // 回归用例：`pub const fn` 曾把 `fn` 当成名字，产出 500+ 条噪声。
    let source = "/// 构造。\npub const fn new() -> Self {}\n";
    assert!(
        check_abbreviations("a.rs", &scan(source)).is_empty(),
        "`pub const fn new` 的名字是 new，不是 fn"
    );
    assert!(check_public_api_docs("a.rs", &scan(source)).is_empty());
}

#[test]
fn test_async_fn_keyword_is_not_treated_as_name() {
    let source = "/// 装配。\npub async fn assemble() {}\n";
    assert!(check_abbreviations("a.rs", &scan(source)).is_empty());
    assert!(check_public_api_docs("a.rs", &scan(source)).is_empty());
}

#[test]
fn test_const_declaration_keeps_const_keyword() {
    let source = "/// 上限。\npub const MAX_LEN: usize = 4;\n";
    assert!(check_public_api_docs("a.rs", &scan(source)).is_empty());
}

#[test]
fn test_banned_abbreviation_list_is_warning() {
    let source = "pub fn read_buf() {}\n";
    assert_eq!(check_abbreviations("a.rs", &scan(source)).len(), 1);
}

#[test]
fn test_allowlisted_abbreviation_passes() {
    let source = "pub fn read_json_url() {}\n";
    assert!(check_abbreviations("a.rs", &scan(source)).is_empty());
}

#[test]
fn test_ordinary_short_english_words_are_not_flagged() {
    // 反向用例：长度 ≤4 的普通英文词不该被启发式命中（旧判据会误报这一整类）。
    let source = "pub fn max_len() {}\npub fn read_name() {}\npub fn path_for_row() {}\n";
    assert!(
        check_abbreviations("a.rs", &scan(source)).is_empty(),
        "普通英文短词不应产生噪声"
    );
}

#[test]
fn test_check_rust_source_reuses_hygiene_tag_rules() {
    let source = "//! 头\n\n// TODO 没有卡号\npub fn run() {}\n";
    let findings = check_rust_source("xtask/src/a.rs", source);
    assert!(
        findings
            .iter()
            .any(|finding| finding.rule == "hygiene/missing-card-reference"),
        "规则 ① 必须复用 hygiene 的判据与规则 id：{findings:?}"
    );
}

#[test]
fn test_check_rust_source_is_deterministic() {
    let source = "pub fn target_element() {}\n// PITFALL: x\n";
    let first = check_rust_source("a.rs", source);
    let second = check_rust_source("a.rs", source);
    assert_eq!(first, second);
}
