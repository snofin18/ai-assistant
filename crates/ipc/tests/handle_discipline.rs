//! Architecture contract for the Host IPC boundary.
//!
//! The transport may serialize only protocol-owned data. Platform implementation
//! dependencies and UI element handles must never enter this crate.

use std::fs;
use std::path::{Path, PathBuf};

const FORBIDDEN_PLATFORM_DEPENDENCIES: [&str; 2] =
    ["assistant-platform-api", "assistant-platform-windows"];
const FORBIDDEN_HANDLE_TYPES: [&str; 5] = [
    "ResolvedElement",
    "ResolvedWindow",
    "HWND",
    "AutomationElement",
    "IUIAutomationElement",
];

#[test]
fn test_arch_ipc_has_no_platform_implementation_dependency() {
    let manifest_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let manifest = fs::read_to_string(&manifest_path);
    assert!(manifest.is_ok());
    let Ok(manifest) = manifest else {
        return;
    };
    for dependency in FORBIDDEN_PLATFORM_DEPENDENCIES {
        assert!(
            !manifest.contains(dependency),
            "assistant-ipc must not depend on {dependency}"
        );
    }
}

#[test]
fn test_arch_ipc_payload_has_no_element_handle_types() {
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut rust_files = Vec::new();
    collect_rust_files(&source_root, &mut rust_files);
    assert!(!rust_files.is_empty(), "assistant-ipc source tree is empty");

    for path in rust_files {
        let source = fs::read_to_string(&path);
        assert!(source.is_ok(), "cannot read {}", path.display());
        let Ok(source) = source else {
            continue;
        };
        for forbidden in FORBIDDEN_HANDLE_TYPES {
            assert!(
                !source.contains(forbidden),
                "{} contains forbidden handle type {forbidden}",
                path.display()
            );
        }
    }
}

fn collect_rust_files(directory: &Path, output: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(directory);
    assert!(entries.is_ok(), "cannot read {}", directory.display());
    let Ok(entries) = entries else {
        return;
    };
    for entry in entries {
        assert!(entry.is_ok(), "cannot read an entry: {entry:?}");
        let Ok(entry) = entry else {
            return;
        };
        let path = entry.path();
        if path.is_dir() {
            collect_rust_files(&path, output);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            output.push(path);
        }
    }
}
