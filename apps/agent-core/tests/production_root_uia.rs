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
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::unwrap_used
)]
// The leak-audit convergence test needs Win32 process-memory queries; every unsafe call is
// wrapped at the call site with a SAFETY note. Test-only, no product code takes this path.
#![allow(unsafe_code)]

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
        terminate_process_tree(&mut self.child);
    }
}

/// Terminates a fixture process tree and reaps the direct child.
///
/// `Child::kill` only signals the direct PowerShell process, so a WPF child or any helper it
/// spawned could survive and keep holding memory and windows. `taskkill /T /F` covers the whole
/// tree; `wait()` then reaps the direct child so no zombie entry is left behind.
fn terminate_process_tree(child: &mut Child) {
    let pid = child.id().to_string();
    let _ = Command::new("taskkill")
        .args(["/PID", &pid, "/T", "/F"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let _ = child.kill();
    let _ = child.wait();
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

/// Leak-audit: run real UIA tasks repeatedly in **this** process and sample its own resources.
///
/// Unlike the external harness, this measures the agent test process itself, so the working-set
/// trend is attributable to the code under test. Run with:
/// `cargo test -p assistant-agent-core --test production_root_uia test_production_resource_convergence -- --ignored --nocapture`
#[cfg(windows)]
#[tokio::test]
#[ignore = "TASK-222: requires an interactive Windows desktop and starts notepad-like"]
async fn test_production_resource_convergence_over_real_uia()
-> Result<(), Box<dyn std::error::Error>> {
    const ITERATIONS: usize = 6;
    const MAX_WORKING_SET_GROWTH_BYTES: usize = 32 * 1024 * 1024;
    let mut samples = vec![sample_process_resources(0)];
    for iteration in 1..=ITERATIONS {
        let directory = TestDirectory::new("production-convergence")?;
        let _fixture = start_fixture(&directory, None)?;
        let ui_config = UiServerConfig::new(
            "assistant-agent-core-production-convergence",
            "ASSISTANT_AGENT_CORE_UI_TOKEN",
            Duration::from_secs(2),
        )
        .with_allowed_peer("C:\\fixture\\peer.exe");
        let task_inputs = serde_json::json!({
            "file_size_bytes": 11,
            "max_text_bytes": 1_048_576,
            "input.keywords": ["alpha"],
            "input.max_keyword_paragraphs": 50,
        })
        .as_object()
        .cloned()
        .expect("task input object");
        let config = ProductionConfig::new(
            directory.path.join("data"),
            fixture_adapter_root(),
            workspace_root()
                .join("adapters/com.microsoft.notepad/tasks/t1.1.open-read-full-text.json"),
            ui_config,
        )
        .with_task_inputs(task_inputs);
        let host =
            assemble_production_host(config, WindowsPlatform::new(), Arc::new(SystemClock)).await?;
        let run = host
            .execute_plan(host.plan_task()?, SystemClock.now_unix_ms())
            .await?;
        assert_eq!(run.final_snapshot.status, TaskStatus::Completed);
        host.shutdown().await?;
        samples.push(sample_process_resources(iteration));
        drop(directory);
    }
    println!(
        "resource samples: {}",
        samples
            .iter()
            .map(|sample| format!(
                "#{} ws={}MB handles={}",
                sample.iteration,
                sample.working_set_bytes / (1024 * 1024),
                sample.handles
            ))
            .collect::<Vec<_>>()
            .join(" | ")
    );
    let half = samples.len() / 2;
    let first_half_last = samples
        .iter()
        .take(half)
        .next_back()
        .map_or(0, |sample| sample.working_set_bytes);
    let last = samples.last().map_or(0, |sample| sample.working_set_bytes);
    let growth = last.saturating_sub(first_half_last);
    assert!(
        growth <= MAX_WORKING_SET_GROWTH_BYTES,
        "in-process working set kept growing across real UIA runs: second half +{growth} bytes \
         (samples: {:?})",
        samples
            .iter()
            .map(|sample| sample.working_set_bytes)
            .collect::<Vec<_>>()
    );
    Ok(())
}

/// Control for the convergence test: sample the process **without** spawning any fixture.
///
/// If handle growth only appears with a fixture, the leak is in fixture/child-process handling;
/// if it appears here too, it is in the process's own lazy initialization.
#[cfg(windows)]
#[tokio::test]
#[ignore = "TASK-222: manual leak-audit control"]
async fn test_production_resource_convergence_control_no_fixture()
-> Result<(), Box<dyn std::error::Error>> {
    const ITERATIONS: usize = 20;
    let mut samples = vec![sample_process_resources(0)];
    for iteration in 1..=ITERATIONS {
        tokio::task::yield_now().await;
        samples.push(sample_process_resources(iteration));
    }
    println!(
        "control samples: {}",
        samples
            .iter()
            .map(|sample| format!("#{} handles={}", sample.iteration, sample.handles))
            .collect::<Vec<_>>()
            .join(" | ")
    );
    Ok(())
}

/// Teardown control: build and shut down a production host N times with **no fixture and no task
/// execution**, and sample handles. Isolates whether the assembly/shutdown path itself leaks.
#[cfg(windows)]
#[tokio::test]
#[ignore = "TASK-222: manual leak-audit control"]
async fn test_production_resource_convergence_assembly_only()
-> Result<(), Box<dyn std::error::Error>> {
    const ITERATIONS: usize = 12;
    let mut samples = vec![sample_process_resources(0).handles];
    for _ in 0..ITERATIONS {
        let directory = TestDirectory::new("production-assembly-only")?;
        let ui_config = UiServerConfig::new(
            "assistant-agent-core-assembly-only",
            "ASSISTANT_AGENT_CORE_UI_TOKEN",
            Duration::from_secs(1),
        )
        .with_allowed_peer("C:\\fixture\\peer.exe");
        let config = ProductionConfig::new(
            directory.path.join("data"),
            fixture_adapter_root(),
            workspace_root()
                .join("adapters/com.microsoft.notepad/tasks/t1.1.open-read-full-text.json"),
            ui_config,
        )
        .with_task_inputs(
            serde_json::json!({
                "file_size_bytes": 11,
                "max_text_bytes": 1_048_576,
                "input.keywords": ["alpha"],
                "input.max_keyword_paragraphs": 50,
            })
            .as_object()
            .cloned()
            .expect("task input object"),
        );
        let host =
            assemble_production_host(config, WindowsPlatform::new(), Arc::new(SystemClock)).await?;
        host.shutdown().await?;
        samples.push(sample_process_resources(0).handles);
    }
    println!("assembly-only handles: {samples:?}");
    let first = samples.first().copied().unwrap_or(0);
    let last = samples.last().copied().unwrap_or(0);
    assert!(
        last.saturating_sub(first)
            <= u32::try_from(ITERATIONS * 2).expect("iteration budget fits in u32"),
        "assembly/shutdown path leaks handles: {samples:?}"
    );
    Ok(())
}

/// Fixture-only control: start and terminate the PowerShell fixture N times with no host.
#[cfg(windows)]
#[tokio::test]
#[ignore = "TASK-222: manual leak-audit control"]
async fn test_production_resource_convergence_fixture_only()
-> Result<(), Box<dyn std::error::Error>> {
    const ITERATIONS: usize = 12;
    let mut samples = vec![sample_process_resources(0).handles];
    for _ in 0..ITERATIONS {
        let directory = TestDirectory::new("production-fixture-only")?;
        let fixture = start_fixture(&directory, None)?;
        drop(fixture);
        samples.push(sample_process_resources(0).handles);
    }
    println!("fixture-only handles: {samples:?}");
    let first = samples.first().copied().unwrap_or(0);
    let last = samples.last().copied().unwrap_or(0);
    assert!(
        last.saturating_sub(first)
            <= u32::try_from(ITERATIONS * 2).expect("iteration budget fits in u32"),
        "fixture start/terminate path leaks handles: {samples:?}"
    );
    Ok(())
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
    let fixture = FixtureProcess { child };
    if let Err(error) = wait_for_state_file(&state_file, &stderr_file) {
        // The ready probe may time out; the process tree must not be left running on the
        // failure path either. `fixture` drops here and performs the tree kill + reap.
        return Err(error.into());
    }
    Ok(fixture)
}

/// One in-process resource sample for the leak-audit convergence test (ADR-0063).
#[cfg(windows)]
struct ProcessResourceSample {
    iteration: usize,
    working_set_bytes: usize,
    handles: u32,
}

/// Samples the calling process's working set and handle count.
#[cfg(windows)]
fn sample_process_resources(iteration: usize) -> ProcessResourceSample {
    use windows::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
    use windows::Win32::System::Threading::{GetCurrentProcess, GetProcessHandleCount};

    // SAFETY: `GetCurrentProcess` returns a pseudo-handle for the calling process; no ownership
    // is transferred and the handle must not be closed by the caller.
    let process = unsafe { GetCurrentProcess() };
    let mut counters = PROCESS_MEMORY_COUNTERS::default();
    let size = u32::try_from(std::mem::size_of::<PROCESS_MEMORY_COUNTERS>())
        .expect("counter struct fits in u32");
    // SAFETY: `counters` is a stack value of the size passed in; the API only writes within it.
    let counters_ptr: *mut PROCESS_MEMORY_COUNTERS = &raw mut counters;
    let memory_ok = unsafe { GetProcessMemoryInfo(process, counters_ptr, size) }.is_ok();
    let mut handles = 0_u32;
    // SAFETY: `handles` is a valid `u32` out-parameter for the pseudo-handle of this process.
    let handles_ptr: *mut u32 = &raw mut handles;
    let handles_ok = unsafe { GetProcessHandleCount(process, handles_ptr) }.is_ok();
    ProcessResourceSample {
        iteration,
        working_set_bytes: if memory_ok {
            counters.WorkingSetSize
        } else {
            0
        },
        handles: if handles_ok { handles } else { 0 },
    }
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
    let before_rollback = host.observe_rollback_state()?;
    assert_eq!(
        before_rollback.get("editor_matches_anchor"),
        Some(&serde_json::Value::Bool(false))
    );
    let rollback = host.execute_rollback(true)?;
    assert!(
        rollback.get("used_fallback").is_some(),
        "rollback must report the fallback decision: {rollback}"
    );
    assert_eq!(
        rollback.get("editor_matches_anchor"),
        Some(&serde_json::Value::Bool(true))
    );
    assert_eq!(
        rollback.get("file_matches_snapshot"),
        Some(&serde_json::Value::Bool(true))
    );
    let after_rollback = host.observe_rollback_state()?;
    assert_eq!(
        after_rollback.get("editor_matches_anchor"),
        Some(&serde_json::Value::Bool(true))
    );
    assert_eq!(
        after_rollback
            .get("file")
            .and_then(|file| file.get("file_matches_snapshot")),
        Some(&serde_json::Value::Bool(true))
    );
    assert_eq!(std::fs::read_to_string(&document_path)?, "报表 报表");
    let second_rollback = host.execute_rollback(true)?;
    assert!(
        matches!(
            second_rollback
                .get("status")
                .and_then(serde_json::Value::as_str),
            Some("already_at_anchor" | "restored")
        ),
        "a repeated rollback must stay at the anchor: {second_rollback}"
    );
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
