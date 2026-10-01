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
use serde_json::Value;

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

/// Committed fixture adapter pack (`PL-097`): the dry run reads the repo artifact
/// instead of assembling adapter data in the test.
fn fixture_adapter_root() -> PathBuf {
    workspace_root().join("adapters/com.example.notepad-like")
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
    let adapter_root = fixture_adapter_root();
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
