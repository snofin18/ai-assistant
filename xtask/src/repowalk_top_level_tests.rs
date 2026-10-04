//! `repowalk` 顶层目录收集的补充单测（ADR-0069）。

use super::*;

#[test]
fn test_collect_top_level_directory_names_contains_controlled_roots() {
    let root = resolve_repo_root(None).expect("默认仓库根应可用");
    let directories = collect_top_level_directory_names(&root).expect("遍历不应失败");
    assert!(directories.iter().any(|name| name == ".github"));
    assert!(directories.iter().any(|name| name == "crates"));
    assert!(directories.iter().any(|name| name == "xtask"));
}

#[test]
fn test_collect_top_level_directory_names_skips_build_artifacts() {
    let root = resolve_repo_root(None).expect("默认仓库根应可用");
    let directories = collect_top_level_directory_names(&root).expect("遍历不应失败");
    assert!(!directories.iter().any(|name| name == "target"));
    assert!(!directories.iter().any(|name| name == ".git"));
}
