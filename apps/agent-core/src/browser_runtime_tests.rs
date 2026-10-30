//! Offline tests for the private CLI browser-runtime assembly.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use assistant_agent_core::browser_registry::build_browser_registry;
use assistant_platform_api::{ResolvedCdpSession, ResolvedCdpTarget};
use assistant_platform_windows::cdp::runtime::RuntimeCdpProvider;

use super::{BrowserRuntimeOptions, BrowserSelection};

fn arguments(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

fn base_edge_arguments() -> Vec<String> {
    arguments(&[
        "--browser",
        "edge",
        "--browser-executable",
        r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
        "--browser-profile-root",
        r"C:\assistant\browser-profiles",
    ])
}

#[test]
fn test_parse_edge_defaults_to_automation_profile_and_adapter()
-> Result<(), Box<dyn std::error::Error>> {
    let options = BrowserRuntimeOptions::parse(&base_edge_arguments())?;
    assert_eq!(options.browser, BrowserSelection::Edge);
    assert_eq!(options.profile_id, "automation");
    assert_eq!(options.adapter_root, PathBuf::from("adapters/browser.edge"));
    assert_eq!(options.session_timeout_ms, 5_000);
    assert_eq!(
        options.tools_path(),
        Path::new("adapters/browser.edge/tools/tools.json")
    );
    Ok(())
}

#[test]
fn test_parse_chrome_explicit_values() -> Result<(), Box<dyn std::error::Error>> {
    let options = BrowserRuntimeOptions::parse(&arguments(&[
        "--browser",
        "chrome",
        "--browser-executable",
        r"C:\Program Files\Google\Chrome\Application\chrome.exe",
        "--browser-profile-root",
        r"D:\assistant\chrome-profiles",
        "--browser-profile-id",
        "round-two",
        "--browser-adapter-root",
        r"D:\assistant\browser.chrome",
        "--browser-session-timeout-ms",
        "7500",
    ]))?;
    assert_eq!(options.browser, BrowserSelection::Chrome);
    assert_eq!(options.profile_id, "round-two");
    assert_eq!(
        options.adapter_root,
        PathBuf::from(r"D:\assistant\browser.chrome")
    );
    assert_eq!(options.session_timeout_ms, 7_500);
    Ok(())
}

#[test]
fn test_parse_rejects_missing_browser_and_profile_root() {
    let missing_browser = arguments(&[
        "--browser-executable",
        r"C:\Edge\msedge.exe",
        "--browser-profile-root",
        r"C:\assistant\profiles",
    ]);
    assert!(
        BrowserRuntimeOptions::parse(&missing_browser)
            .err()
            .is_some_and(|error| error.to_string().contains("--browser"))
    );

    let missing_profile = arguments(&[
        "--browser",
        "edge",
        "--browser-executable",
        r"C:\Edge\msedge.exe",
    ]);
    assert!(
        BrowserRuntimeOptions::parse(&missing_profile)
            .err()
            .is_some_and(|error| error.to_string().contains("--browser-profile-root"))
    );
}

#[test]
fn test_parse_rejects_relative_paths_duplicate_options_and_invalid_timeout() {
    let relative = arguments(&[
        "--browser",
        "edge",
        "--browser-executable",
        r"C:\Edge\msedge.exe",
        "--browser-profile-root",
        "relative/profile",
    ]);
    assert!(BrowserRuntimeOptions::parse(&relative).is_err());

    let duplicate = arguments(&[
        "--browser",
        "edge",
        "--browser",
        "chrome",
        "--browser-executable",
        r"C:\Edge\msedge.exe",
        "--browser-profile-root",
        r"C:\assistant\profiles",
    ]);
    assert!(BrowserRuntimeOptions::parse(&duplicate).is_err());

    let invalid_timeout = arguments(&[
        "--browser",
        "edge",
        "--browser-executable",
        r"C:\Edge\msedge.exe",
        "--browser-profile-root",
        r"C:\assistant\profiles",
        "--browser-session-timeout-ms",
        "99",
    ]);
    assert!(BrowserRuntimeOptions::parse(&invalid_timeout).is_err());

    let invalid_profile_id = arguments(&[
        "--browser",
        "edge",
        "--browser-executable",
        r"C:\Edge\msedge.exe",
        "--browser-profile-root",
        r"C:\assistant\profiles",
        "--browser-profile-id",
        "..",
    ]);
    assert!(BrowserRuntimeOptions::parse(&invalid_profile_id).is_err());
}

#[test]
fn test_registry_assembly_contains_six_tools_without_launching_browser()
-> Result<(), Box<dyn std::error::Error>> {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (family, adapter_name, tool_prefix) in [
        (BrowserSelection::Edge, "browser.edge", "edge.cdp."),
        (BrowserSelection::Chrome, "browser.chrome", "chrome.cdp."),
    ] {
        let provider = Arc::new(RuntimeCdpProvider::new());
        let target = ResolvedCdpTarget::new(ResolvedCdpSession::new(1), 1);
        let tools_path = workspace_root
            .join("adapters")
            .join(adapter_name)
            .join("tools")
            .join("tools.json");
        let build =
            build_browser_registry(provider, target, family.registry_family(), &tools_path)?;
        assert_eq!(build.registry().len(), 6);
        assert_eq!(
            build
                .tool_schemas()
                .iter()
                .filter(|schema| schema.name.starts_with(tool_prefix))
                .count(),
            6
        );
        assert_eq!(
            build
                .tool_schemas()
                .iter()
                .filter(|schema| schema.name.starts_with(tool_prefix))
                .count(),
            6
        );
    }
    Ok(())
}
