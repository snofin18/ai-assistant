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

use assistant_agent_core::{
    GrantRequest, ProductionConfig, UiServerConfig, assemble_production_host,
};
use assistant_hitl::ApprovalScope;
use assistant_platform_windows::WindowsPlatform;
use assistant_storage::{Clock, SystemClock};
use assistant_task_engine::{StepStatus, TaskStatus};
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

fn start_fixture(
    directory: &TestDirectory,
    document: Option<&Path>,
) -> Result<FixtureProcess, Box<dyn std::error::Error>> {
    let state_file = directory.path.join("fixture-state.json");
    let stderr_file = directory.path.join("fixture-stderr.log");
    let stderr = File::create(&stderr_file)?;
    let fixture_script = workspace_root().join("fixtures/apps/notepad-like/notepad-like.ps1");
    let mut command = Command::new(r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe");
    command
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
        .arg(&fixture_script)
        .arg("--state-file")
        .arg(&state_file);
    if let Some(document) = document {
        command.arg("--document").arg(document);
    }
    let child = command
        .arg("--auto-close-ms")
        .arg("120000")
        .current_dir(workspace_root())
        .stdout(Stdio::null())
        .stderr(Stdio::from(stderr))
        .spawn()?;
    wait_for_state_file(&state_file, &stderr_file)?;
    Ok(FixtureProcess { child })
}

#[tokio::test]
#[ignore = "TASK-105: requires an interactive Windows desktop and starts notepad-like"]
async fn test_production_t1_1_dry_run_over_real_uia() -> Result<(), Box<dyn std::error::Error>> {
    if !cfg!(windows) {
        return Err("real UIA dry run is Windows-only".into());
    }
    let directory = TestDirectory::new("production-uia")?;
    let adapter_root = fixture_adapter_root();
    let _fixture = start_fixture(&directory, None)?;

    let ui_config = UiServerConfig::new(
        "assistant-agent-core-production-uia-test",
        "ASSISTANT_AGENT_CORE_UI_TOKEN",
        Duration::from_secs(2),
    )
    .with_allowed_peer("C:\\fixture\\peer.exe");
    let task_inputs = serde_json::json!({
        "file_size_bytes": 10,
        "max_text_bytes": 1024,
        "input.keywords": ["report"],
        "input.max_keyword_paragraphs": 50,
    })
    .as_object()
    .cloned()
    .expect("task input object");
    let config = ProductionConfig::new(
        directory.path.join("data"),
        adapter_root,
        workspace_root().join("adapters/com.microsoft.notepad/tasks/t1.1.open-read-full-text.json"),
        ui_config,
    )
    .with_task_inputs(task_inputs);
    let host =
        assemble_production_host(config, WindowsPlatform::new(), Arc::new(SystemClock)).await?;
    let plan = host.plan_task()?;
    let run = host.execute_plan(plan, SystemClock.now_unix_ms()).await?;
    assert_eq!(run.final_snapshot.status, TaskStatus::Completed);
    assert_eq!(run.snapshots.len(), 1);
    host.shutdown().await?;
    Ok(())
}

#[tokio::test]
#[ignore = "TASK-105: requires an interactive Windows desktop and starts notepad-like"]
async fn test_production_t1_2_dry_run_over_real_uia() -> Result<(), Box<dyn std::error::Error>> {
    if !cfg!(windows) {
        return Err("real UIA dry run is Windows-only".into());
    }
    let directory = TestDirectory::new("production-t1-2-uia")?;
    let document_path = directory.path.join("t1-2.txt");
    std::fs::write(&document_path, "报表 报表")?;
    let _fixture = start_fixture(&directory, Some(&document_path))?;

    let ui_config = UiServerConfig::new(
        "assistant-agent-core-production-t1-2-uia",
        "ASSISTANT_AGENT_CORE_UI_TOKEN",
        Duration::from_secs(2),
    )
    .with_allowed_peer("C:\\fixture\\peer.exe");
    let task_inputs = serde_json::json!({
        "input.file_path": document_path.to_string_lossy(),
        "input.old_text": "报表",
        "input.new_text": "报告",
        "input.expected_replacements": 2,
        "input.target_resolution_policy": "error_if_ambiguous",
        "rollback.replace_recipe": "replace-text-l0-l1",
        "rollback.save_recipe": "save-l0-l1",
        "rollback.required_anchor_levels": ["L0", "L1"],
    })
    .as_object()
    .cloned()
    .expect("task input object");
    let config = ProductionConfig::new(
        directory.path.join("data"),
        fixture_adapter_root(),
        workspace_root()
            .join("adapters/com.microsoft.notepad/tasks/t1.2.replace-save-approval-undo.json"),
        ui_config,
    )
    .with_task_inputs(task_inputs);
    let host =
        assemble_production_host(config, WindowsPlatform::new(), Arc::new(SystemClock)).await?;
    let now_ms = SystemClock.now_unix_ms();
    for step_id in ["approve_replace", "approve_save"] {
        host.approvals().grant(&GrantRequest {
            task_id: host.task_id().as_str(),
            step_id,
            scope: ApprovalScope::Once,
            now_ms,
            ttl_ms: 60_000,
            uses: 1,
        })?;
    }
    let run = host.execute_plan(host.plan_task()?, now_ms).await?;
    assert_eq!(run.final_snapshot.status, TaskStatus::Completed);
    assert!(
        run.final_snapshot
            .steps
            .iter()
            .all(|step| step.status == StepStatus::Committed)
    );
    assert_eq!(std::fs::read_to_string(&document_path)?, "报告 报告");
    host.shutdown().await?;
    Ok(())
}

#[tokio::test]
#[ignore = "TASK-105: requires an interactive Windows desktop and starts notepad-like"]
async fn test_production_t1_3_dry_run_over_real_uia() -> Result<(), Box<dyn std::error::Error>> {
    if !cfg!(windows) {
        return Err("real UIA dry run is Windows-only".into());
    }
    let directory = TestDirectory::new("production-t1-3-uia")?;
    let _fixture = start_fixture(&directory, None)?;
    let target_path = directory.path.join("created-by-t1-3.txt");
    let text = "alpha\nbeta\n";

    let ui_config = UiServerConfig::new(
        "assistant-agent-core-production-t1-3-uia",
        "ASSISTANT_AGENT_CORE_UI_TOKEN",
        Duration::from_secs(2),
    )
    .with_allowed_peer("C:\\fixture\\peer.exe");
    let task_inputs = serde_json::json!({
        "input.text": text,
        "input.target_path": target_path.to_string_lossy(),
        "input.expected_initial_tab_count": 1,
    })
    .as_object()
    .cloned()
    .expect("task input object");
    let config = ProductionConfig::new(
        directory.path.join("data"),
        fixture_adapter_root(),
        workspace_root()
            .join("adapters/com.microsoft.notepad/tasks/t1.3.new-tab-write-save-as.json"),
        ui_config,
    )
    .with_task_inputs(task_inputs);
    let host =
        assemble_production_host(config, WindowsPlatform::new(), Arc::new(SystemClock)).await?;
    let now_ms = SystemClock.now_unix_ms();
    host.approvals().grant(&GrantRequest {
        task_id: host.task_id().as_str(),
        step_id: "approve_save_as",
        scope: ApprovalScope::Once,
        now_ms,
        ttl_ms: 60_000,
        uses: 1,
    })?;
    let run = host.execute_plan(host.plan_task()?, now_ms).await?;
    assert_eq!(run.final_snapshot.status, TaskStatus::Completed);
    assert_eq!(std::fs::read_to_string(&target_path)?, text);
    host.shutdown().await?;
    Ok(())
}
