//! `top_level_dirs.rs` 的私有单测（ADR-0069）。

use super::*;

const SAMPLE: &str = r"
# 顶层目录 ADR 白名单

## 允许的顶层目录

| 目录 | 用途 | 授权 ADR |
|---|---|---|
| `apps/` | applications | ADR-0053 |
| `crates/` | crates | ADR-0053 |
| `.github/` | automation | ADR-0019 |
";

#[test]
fn test_parse_whitelist_positive() {
    let whitelist = parse_top_level_whitelist(SAMPLE).expect("应解析成功");
    assert_eq!(
        whitelist,
        BTreeSet::from([
            ".github".to_string(),
            "apps".to_string(),
            "crates".to_string(),
        ])
    );
}

#[test]
fn test_parse_whitelist_rejects_missing_heading_or_rows() {
    assert!(parse_top_level_whitelist("# no table\n").is_none());
    assert!(parse_top_level_whitelist("## 允许的顶层目录\n").is_none());
}

#[test]
fn test_registered_directory_passes() {
    let whitelist = parse_top_level_whitelist(SAMPLE).expect("应解析成功");
    let findings = check_top_level_directories(
        &[
            "apps".to_string(),
            "crates".to_string(),
            ".github".to_string(),
        ],
        &whitelist,
    );
    assert!(findings.is_empty());
}

#[test]
fn test_unregistered_directory_is_error() {
    let whitelist = parse_top_level_whitelist(SAMPLE).expect("应解析成功");
    let findings = check_top_level_directories(
        &[
            "apps".to_string(),
            "crates".to_string(),
            ".github".to_string(),
            "scratch".to_string(),
        ],
        &whitelist,
    );
    assert_eq!(findings.len(), 1);
    let finding = findings.first().expect("应有发现项");
    assert_eq!(finding.rule, "hygiene/unregistered-top-level-directory");
    assert_eq!(finding.severity, Severity::Error);
    assert_eq!(finding.path, "scratch/");
}

#[test]
fn test_stale_whitelist_entry_is_warning() {
    let whitelist = parse_top_level_whitelist(SAMPLE).expect("应解析成功");
    let findings = check_top_level_directories(&["apps".to_string()], &whitelist);
    assert!(findings.iter().any(|finding| {
        finding.rule == "hygiene/stale-top-level-directory" && finding.severity == Severity::Warning
    }));
}

#[test]
fn test_unparsable_finding_is_error() {
    let finding = unparsable_whitelist_finding();
    assert_eq!(finding.rule, "hygiene/top-level-whitelist-unparsable");
    assert_eq!(finding.severity, Severity::Error);
}
