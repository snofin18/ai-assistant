//! Architecture assertions for the ADR-0053 Core dependency whitelist.
//!
//! The platform test proves Core cannot reference a platform implementation.
//! This test closes the adjacent gap: another workspace crate cannot silently
//! enter `assistant-core` without an ADR-0053 change.
//!
//! The scanner intentionally checks only dependency names in the
//! `assistant-*` namespace. Third-party crates remain governed separately by
//! `docs/DEPENDENCIES.md` and `cargo deny`.
#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unwrap_used
)]

use std::collections::BTreeSet;
use std::path::PathBuf;

const ALLOWED_WORKSPACE_DEPENDENCIES: &[&str] = &[
    "assistant-protocol",
    "assistant-storage",
    "assistant-platform-api",
    "assistant-task-engine",
    "assistant-model-gateway",
];

fn core_manifest_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml")
}

#[derive(Default)]
struct DependencyScan {
    findings: Vec<String>,
    workspace_dependencies: BTreeSet<String>,
}

fn scan_workspace_dependencies(manifest: &str) -> DependencyScan {
    let mut scan = DependencyScan::default();
    let mut in_dependencies = false;

    for (index, line) in manifest.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            if let Some(name) = dependency_section_name(trimmed) {
                inspect_dependency(name, index + 1, &mut scan);
            }
            in_dependencies = is_dependency_table(trimmed);
            continue;
        }
        if !in_dependencies || trimmed.starts_with('#') {
            continue;
        }
        let Some((name, value)) = trimmed.split_once('=') else {
            continue;
        };
        inspect_dependency(name.trim(), index + 1, &mut scan);
        if let Some(package) = package_override(value) {
            inspect_dependency(package, index + 1, &mut scan);
        }
    }
    scan
}

fn dependency_section_name(header: &str) -> Option<&str> {
    let inner = header.strip_prefix('[')?.strip_suffix(']')?;
    if let Some(rest) = inner.strip_prefix("dependencies.") {
        return Some(rest);
    }
    let marker = ".dependencies.";
    inner
        .rfind(marker)
        .map(|index| &inner[index + marker.len()..])
}

fn is_dependency_table(header: &str) -> bool {
    header == "[dependencies]"
        || header.starts_with("[dependencies.")
        || header.ends_with(".dependencies]")
        || header.contains(".dependencies.")
}

fn package_override(value: &str) -> Option<&str> {
    let after_keyword = value.split_once("package")?.1.trim_start();
    let after_equals = after_keyword.strip_prefix('=')?.trim_start();
    let after_quote = after_equals.strip_prefix('"')?;
    Some(after_quote.split_once('"')?.0)
}

fn inspect_dependency(name: &str, line: usize, scan: &mut DependencyScan) {
    if !name.starts_with("assistant-") {
        return;
    }
    scan.workspace_dependencies.insert(name.to_owned());
    if !ALLOWED_WORKSPACE_DEPENDENCIES.contains(&name) {
        scan.findings
            .push(format!("line {line}: workspace dependency {name}"));
    }
}

mod arch {
    use super::*;

    #[test]
    fn core_manifest_workspace_dependencies_stay_within_adr_0053_whitelist() {
        let manifest = std::fs::read_to_string(core_manifest_path()).unwrap();
        assert!(
            !scan_workspace_dependencies(&manifest)
                .workspace_dependencies
                .is_empty(),
            "positive check must see at least one workspace dependency"
        );
        let findings = scan_workspace_dependencies(&manifest).findings;
        assert!(
            findings.is_empty(),
            "Core workspace dependencies exceed ADR-0053 D2:\n{findings:#?}"
        );
    }

    #[test]
    fn scan_workspace_dependencies_flags_blacklisted_workspace_crate() {
        let findings = scan_workspace_dependencies(
            "[dependencies]\nassistant-policy = { path = \"../policy\" }\n",
        )
        .findings;
        assert_eq!(findings.len(), 1);
        assert!(findings[0].contains("assistant-policy"));
    }

    #[test]
    fn scan_workspace_dependencies_allows_whitelisted_workspace_crate() {
        let findings = scan_workspace_dependencies(
            "[dependencies.assistant-task-engine]\npath = \"../task-engine\"\n",
        )
        .findings;
        assert!(findings.is_empty(), "{findings:#?}");
    }

    #[test]
    fn scan_workspace_dependencies_flags_renamed_blacklisted_crate() {
        let findings = scan_workspace_dependencies(
            "[dependencies]\npolicy_alias = { package = \"assistant-policy\", path = \"../policy\" }\n",
        )
        .findings;
        assert_eq!(findings.len(), 1);
        assert!(findings[0].contains("assistant-policy"));
    }

    #[test]
    fn scan_workspace_dependencies_flags_target_specific_blacklisted_crate() {
        let findings = scan_workspace_dependencies(
            "[target.'cfg(windows)'.dependencies]\nassistant-policy = { path = \"../policy\" }\n",
        )
        .findings;
        assert_eq!(findings.len(), 1);
        assert!(findings[0].contains("assistant-policy"));
    }
}
