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

fn scan_workspace_dependencies(manifest: &str) -> Vec<String> {
    let mut findings = Vec::new();
    let mut in_dependencies = false;

    for (index, line) in manifest.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            if let Some(name) = trimmed
                .strip_prefix("[dependencies.")
                .and_then(|inner| inner.strip_suffix(']'))
            {
                inspect_dependency(name, index + 1, &mut findings);
            }
            in_dependencies = trimmed == "[dependencies]" || trimmed.starts_with("[dependencies.");
            continue;
        }
        if !in_dependencies || trimmed.starts_with('#') {
            continue;
        }
        let Some((name, _)) = trimmed.split_once('=') else {
            continue;
        };
        inspect_dependency(name.trim(), index + 1, &mut findings);
    }
    findings
}

fn inspect_dependency(name: &str, line: usize, findings: &mut Vec<String>) {
    if name.starts_with("assistant-") && !ALLOWED_WORKSPACE_DEPENDENCIES.contains(&name) {
        findings.push(format!("line {line}: workspace dependency {name}"));
    }
}

mod arch {
    use super::*;

    #[test]
    fn core_manifest_workspace_dependencies_stay_within_adr_0053_whitelist() {
        let manifest = std::fs::read_to_string(core_manifest_path()).unwrap();
        assert!(
            manifest.contains("assistant-protocol"),
            "positive check must see at least one workspace dependency"
        );
        let findings = scan_workspace_dependencies(&manifest);
        assert!(
            findings.is_empty(),
            "Core workspace dependencies exceed ADR-0053 D2:\n{findings:#?}"
        );
    }

    #[test]
    fn scan_workspace_dependencies_flags_blacklisted_workspace_crate() {
        let findings = scan_workspace_dependencies(
            "[dependencies]\nassistant-policy = { path = \"../policy\" }\n",
        );
        assert_eq!(findings.len(), 1);
        assert!(findings[0].contains("assistant-policy"));
    }

    #[test]
    fn scan_workspace_dependencies_allows_whitelisted_workspace_crate() {
        let findings = scan_workspace_dependencies(
            "[dependencies.assistant-task-engine]\npath = \"../task-engine\"\n",
        );
        assert!(findings.is_empty(), "{findings:#?}");
    }
}
