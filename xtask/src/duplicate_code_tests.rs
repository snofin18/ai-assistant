//! `duplicate_code.rs` 的私有单测（ADR-0068）。

use super::*;
use std::fmt::Write as _;

fn repeated_source(prefix: &str, count: usize) -> String {
    let mut source = String::from("fn sample() {\n");
    for index in 0..count {
        writeln!(source, "    let {prefix}_{index} = {index} + 1;").expect("写入 String 不应失败");
    }
    source.push_str("}\n");
    source
}

fn source_pair(left: String, right: String) -> Vec<DuplicateSource<'static>> {
    // 测试字符串必须活到调用结束；用 Box::leak 避免为每个用例引入生命周期噪声。
    vec![
        DuplicateSource {
            relative_path: "crates/left/src/lib.rs",
            source: Box::leak(left.into_boxed_str()),
        },
        DuplicateSource {
            relative_path: "crates/right/src/lib.rs",
            source: Box::leak(right.into_boxed_str()),
        },
    ]
}

#[test]
fn test_identical_sources_are_reported() {
    let findings = check_duplicate_code(&source_pair(
        repeated_source("value", 30),
        repeated_source("value", 30),
    ));
    assert_eq!(findings.len(), 1);
    let finding = findings.first().expect("应有发现项");
    assert_eq!(finding.rule, "hygiene/duplicate-code");
    assert_eq!(finding.severity, Severity::Warning);
    assert_eq!(finding.path, "crates/left/src/lib.rs");
}

#[test]
fn test_identifier_renames_still_match() {
    let findings = check_duplicate_code(&source_pair(
        repeated_source("left", 30),
        repeated_source("right", 30),
    ));
    assert_eq!(findings.len(), 1, "局部变量改名不应绕过 token 规范化");
}

#[test]
fn test_dissimilar_sources_are_not_reported() {
    let mut right = String::from("fn other() {\n");
    for index in 0..30 {
        writeln!(right, "    println!(\"item-{index}\");").expect("写入 String 不应失败");
    }
    right.push_str("}\n");
    assert!(check_duplicate_code(&source_pair(repeated_source("value", 30), right)).is_empty());
}

#[test]
fn test_ignored_paths_are_not_reported() {
    let mut sources = source_pair(repeated_source("value", 30), repeated_source("value", 30));
    if let Some(source) = sources.get_mut(0) {
        source.relative_path = "fixtures/apps/left.rs";
    }
    if let Some(source) = sources.get_mut(1) {
        source.relative_path = "fixtures/apps/right.rs";
    }
    assert!(check_duplicate_code(&sources).is_empty());
    assert!(is_ignored_duplicate_path(
        "crates/example/src/generated/code.rs"
    ));
    assert!(is_ignored_duplicate_path("xtask/src/example_tests.rs"));
    assert!(!is_ignored_duplicate_path("crates/example/src/lib.rs"));
}

#[test]
fn test_similarity_threshold_boundary() {
    assert!(meets_similarity_threshold(4, 5));
    assert!(!meets_similarity_threshold(3, 5));
    assert!(!meets_similarity_threshold(7, 10));
    assert!(meets_similarity_threshold(8, 10));
}

#[test]
fn test_short_sources_are_not_eligible() {
    let findings = check_duplicate_code(&source_pair(
        "fn short() {}\n".to_string(),
        "fn short() {}\n".to_string(),
    ));
    assert!(findings.is_empty());
}
