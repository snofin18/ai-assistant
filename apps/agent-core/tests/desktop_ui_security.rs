//! Static security checks for the Tauri shell.
#![allow(clippy::expect_used)]

use std::path::Path;

use serde_json::Value;

#[test]
fn test_desktop_ui_has_strict_csp_and_no_system_permissions() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir.join("../..");
    let ui_root = workspace_root.join("apps/desktop-ui");

    let config: Value = serde_json::from_str(
        &std::fs::read_to_string(ui_root.join("src-tauri/tauri.conf.json"))
            .expect("read tauri config"),
    )
    .expect("parse tauri config");
    let csp = config
        .get("app")
        .and_then(|app| app.get("security"))
        .and_then(|security| security.get("csp"))
        .and_then(Value::as_str)
        .expect("strict CSP");
    assert!(csp.contains("default-src 'self'"));
    assert!(csp.contains("script-src 'self'"));
    assert!(!csp.contains("unsafe-eval"));
    assert!(!csp.contains(" *"));

    let capabilities: Value = serde_json::from_str(
        &std::fs::read_to_string(ui_root.join("src-tauri/capabilities/default.json"))
            .expect("read capabilities"),
    )
    .expect("parse capabilities");
    let permissions = capabilities
        .get("permissions")
        .and_then(Value::as_array)
        .expect("permissions array");
    assert!(
        permissions.is_empty(),
        "shell must not grant system permissions"
    );

    let tauri_manifest =
        std::fs::read_to_string(ui_root.join("src-tauri/Cargo.toml")).expect("read tauri manifest");
    for forbidden in [
        "tauri-plugin-shell",
        "tauri-plugin-fs",
        "tauri-plugin-http",
        "tauri-plugin-process",
        "tauri-plugin-dialog",
    ] {
        assert!(
            !tauri_manifest.contains(forbidden),
            "desktop shell must not depend on {forbidden}"
        );
    }

    let package: Value = serde_json::from_str(
        &std::fs::read_to_string(ui_root.join("package.json")).expect("read package.json"),
    )
    .expect("parse package.json");
    let serialized = package.to_string();
    assert!(
        !serialized.contains("@tauri-apps/plugin-"),
        "frontend must not import Tauri system plugins"
    );

    let root_manifest =
        std::fs::read_to_string(workspace_root.join("Cargo.toml")).expect("read root manifest");
    assert!(
        !root_manifest.contains("apps/desktop-ui/src-tauri"),
        "the Tauri shell stays outside the host workspace to keep Linux CI independent of WebKit"
    );
}
