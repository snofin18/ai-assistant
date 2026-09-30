//! Interactive Windows UIA dry run for the production root.
//!
//! This test is ignored by default because it requires a desktop session and
//! starts the repository's `notepad-like` fixture. Run it explicitly with:
//!
//! `cargo test -p assistant-agent-core --test production_root_uia -- --ignored`

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::too_many_lines,
    clippy::unwrap_used
)]

use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use assistant_agent_core::{ProductionConfig, UiServerConfig, assemble_production_host};
use assistant_platform_windows::WindowsPlatform;
use assistant_storage::{Clock, SystemClock};
use assistant_task_engine::TaskStatus;
use serde_json::{Value, json};

struct FixtureProcess {
    child: Child,
}

impl Drop for FixtureProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    fn new(label: &str) -> std::io::Result<Self> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(std::io::Error::other)?
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "assistant-agent-core-{label}-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path)?;
        Ok(Self { path })
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn workspace_root() -> PathBuf {
    let canonical = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root");
    let text = canonical.to_string_lossy();
    PathBuf::from(text.strip_prefix(r"\\?\").unwrap_or(&text))
}

fn write_fixture_adapter(root: &Path) -> std::io::Result<()> {
    let tools_dir = root.join("tools");
    let selectors_dir = root.join("selectors");
    std::fs::create_dir_all(&tools_dir)?;
    std::fs::create_dir_all(&selectors_dir)?;

    let source_tools = std::fs::read_to_string(
        workspace_root().join("adapters/com.microsoft.notepad/tools/tools.json"),
    )?;
    let mut tools: Value = serde_json::from_str(&source_tools).map_err(std::io::Error::other)?;
    let object = tools
        .as_object_mut()
        .ok_or_else(|| std::io::Error::other("tools.json must be an object"))?;
    object.insert(
        "app_id".to_owned(),
        Value::String("powershell.exe".to_owned()),
    );
    std::fs::write(
        tools_dir.join("tools.json"),
        serde_json::to_vec_pretty(&tools).map_err(std::io::Error::other)?,
    )?;

    let target = json!({
        "schema_version": 1,
        "app_id": "powershell.exe",
        "app_version_range": "any",
        "policy": {
            "on_ambiguous": "error_and_ask",
            "on_not_found": "escalate_to_human",
            "min_score_to_try": 0.1,
            "max_resolve_ms": 3000
        },
        "targets": [
            {
                "id": "main_window",
                "scope": "window",
                "candidates": [{
                    "id": "fixture-window-aid",
                    "kind": "automation_id",
                    "value": {"text": "MainWindow"},
                    "score": 1.0,
                    "locale_dependent": false
                }]
            },
            {
                "id": "editor",
                "scope": "element",
                "candidates": [{
                    "id": "fixture-editor-aid",
                    "kind": "automation_id",
                    "value": {"text": "EditorTextBox"},
                    "score": 1.0,
                    "locale_dependent": false
                }]
            },
            {
                "id": "add_tab_button",
                "scope": "element",
                "candidates": [{
                    "id": "fixture-add-tab",
                    "kind": "automation_id",
                    "value": {"text": "DoesNotExist"},
                    "score": 1.0,
                    "locale_dependent": false
                }]
            },
            {
                "id": "save_as_dialog",
                "scope": "window",
                "candidates": [{
                    "id": "fixture-save-as-dialog",
                    "kind": "automation_id",
                    "value": {"text": "DoesNotExist"},
                    "score": 1.0,
                    "locale_dependent": false
                }]
            },
            {
                "id": "save_as_filename",
                "scope": "element",
                "candidates": [{
                    "id": "fixture-save-as-filename",
                    "kind": "automation_id",
                    "value": {"text": "DoesNotExist"},
                    "score": 1.0,
                    "locale_dependent": false
                }]
            },
            {
                "id": "save_as_save_button",
                "scope": "element",
                "candidates": [{
                    "id": "fixture-save-as-button",
                    "kind": "automation_id",
                    "value": {"text": "DoesNotExist"},
                    "score": 1.0,
                    "locale_dependent": false
                }]
            }
        ]
    });
    std::fs::write(
        selectors_dir.join("targets.json"),
        serde_json::to_vec_pretty(&target).map_err(std::io::Error::other)?,
    )
}

fn wait_for_state_file(path: &Path, stderr_path: &Path) -> std::io::Result<()> {
    for _attempt in 0..300 {
        if let Ok(body) = std::fs::read_to_string(path)
            && let Ok(state) = serde_json::from_str::<Value>(body.trim_start_matches('\u{feff}'))
            && state.get("status").and_then(Value::as_str) == Some("ready")
        {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let stderr = std::fs::read_to_string(stderr_path).unwrap_or_default();
    Err(std::io::Error::other(format!(
        "notepad-like fixture did not report ready state; stderr={stderr}"
    )))
}

#[tokio::test]
#[ignore = "requires an interactive Windows desktop and starts notepad-like"]
async fn test_production_t1_1_dry_run_over_real_uia() -> Result<(), Box<dyn std::error::Error>> {
    if !cfg!(windows) {
        return Err("real UIA dry run is Windows-only".into());
    }
    let directory = TestDirectory::new("production-uia")?;
    let adapter_root = directory.path.join("adapter");
    write_fixture_adapter(&adapter_root)?;
    let state_file = directory.path.join("fixture-state.json");
    let stderr_file = directory.path.join("fixture-stderr.log");
    let stderr = File::create(&stderr_file)?;
    let fixture_script = workspace_root().join("fixtures/apps/notepad-like/notepad-like.ps1");
    let child = Command::new(r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe")
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
        .arg(&fixture_script)
        .arg("--state-file")
        .arg(&state_file)
        .arg("--auto-close-ms")
        .arg("60000")
        .current_dir(workspace_root())
        .stdout(Stdio::null())
        .stderr(Stdio::from(stderr))
        .spawn()?;
    let _fixture = FixtureProcess { child };
    wait_for_state_file(&state_file, &stderr_file)?;

    let ui_config = UiServerConfig::new(
        "assistant-agent-core-production-uia-test",
        "ASSISTANT_AGENT_CORE_UI_TOKEN",
        Duration::from_secs(2),
    )
    .with_allowed_peer("C:\\fixture\\peer.exe");
    let config = ProductionConfig::new(
        directory.path.join("data"),
        adapter_root,
        workspace_root().join("adapters/com.microsoft.notepad/tasks/t1.1.open-read-full-text.json"),
        ui_config,
    );
    let host =
        assemble_production_host(config, WindowsPlatform::new(), Arc::new(SystemClock)).await?;
    let plan = host.plan_task()?;
    let run = host.execute_plan(plan, SystemClock.now_unix_ms()).await?;
    assert_eq!(run.final_snapshot.status, TaskStatus::Completed);
    assert_eq!(run.snapshots.len(), 1);
    host.shutdown().await?;
    Ok(())
}
