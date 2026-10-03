//! Production-root acceptance tests for TASK-214 without a real commercial application.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::too_many_lines,
    clippy::unwrap_used
)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[path = "support/production_fixture.rs"]
mod fixture;

#[cfg(windows)]
use fixture::FIXED_NOW_MS;
use fixture::{FakePlatform, FixedClock};

#[cfg(windows)]
use assistant_agent_core::GrantRequest;
use assistant_agent_core::UiServerConfig;
use assistant_agent_core::{ProductionConfig, ProductionError, assemble_production_host};
#[cfg(windows)]
use assistant_agent_core::{
    TaskControlHandler, UiAuthorizationScope, UiCommand, UiCommandHandler, UiEvent, UiEventSource,
    serve_session_with_events,
};
#[cfg(windows)]
use assistant_hitl::ApprovalScope;
#[cfg(windows)]
use assistant_ipc::{
    IpcError, NamedPipeTransport, Transport, WireMessage, client_handshake, generate_session_id,
    server_handshake,
};
use assistant_platform_api::ErrorCode;
#[cfg(windows)]
use assistant_platform_api::Fingerprint;
#[cfg(windows)]
use assistant_task_engine::{MemoryCheckpointStore, StepStatus, TaskEngine, TaskStatus};

#[cfg(windows)]
const UI_TEST_TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

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
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root")
}

fn production_config(data_root: &Path) -> ProductionConfig {
    let adapter_root = workspace_root().join("adapters/com.microsoft.notepad");
    let task_package_path = adapter_root
        .join("tasks")
        .join("t1.1.open-read-full-text.json");
    let ui_config = UiServerConfig::new(
        "assistant-agent-core-production-test",
        "ASSISTANT_AGENT_CORE_TEST_TOKEN",
        Duration::from_secs(1),
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
    ProductionConfig::new(data_root, adapter_root, task_package_path, ui_config)
        .with_task_inputs(task_inputs)
}

#[cfg(windows)]
fn read_step_event(
    transport: &mut NamedPipeTransport,
    expected_step_id: &str,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    loop {
        match transport.recv(Duration::from_secs(2))? {
            WireMessage::UiEvent(event)
                if event.event.get("kind").and_then(serde_json::Value::as_str)
                    == Some("step_state_changed")
                    && event
                        .event
                        .get("step_id")
                        .and_then(serde_json::Value::as_str)
                        == Some(expected_step_id)
                    && event
                        .event
                        .get("status")
                        .and_then(serde_json::Value::as_str)
                        == Some("committed") =>
            {
                return Ok(event.event);
            }
            WireMessage::UiEvent(_) | WireMessage::Heartbeat(_) => {}
            other => {
                return Err(format!(
                    "unexpected {} on the production UI event pipe",
                    other.kind()
                )
                .into());
            }
        }
    }
}

#[cfg(windows)]
fn assert_step_event(
    step_event: &serde_json::Value,
    task_id: &str,
    expected_step_id: &str,
    expected_fingerprint: &Fingerprint,
) {
    assert_eq!(step_event.as_object().map(serde_json::Map::len), Some(6));
    assert_eq!(
        step_event
            .get("task_id")
            .and_then(serde_json::Value::as_str),
        Some(task_id)
    );
    assert_eq!(
        step_event
            .get("step_id")
            .and_then(serde_json::Value::as_str),
        Some(expected_step_id)
    );
    assert_eq!(
        step_event.get("status").and_then(serde_json::Value::as_str),
        Some("committed")
    );
    assert!(
        step_event
            .get("phase")
            .is_some_and(serde_json::Value::is_null)
    );
    assert_eq!(
        step_event
            .get("post_fingerprint")
            .and_then(serde_json::Value::as_str),
        Some(expected_fingerprint.as_str())
    );
}

#[cfg(windows)]
fn join_ui_server(server: std::thread::JoinHandle<Result<(), IpcError>>) {
    let disconnected = std::time::Instant::now() + Duration::from_secs(2);
    while !server.is_finished() && std::time::Instant::now() < disconnected {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(
        server.is_finished(),
        "UI server did not stop after disconnect"
    );
    let ended = server.join().expect("UI server thread");
    assert!(
        matches!(ended, Err(IpcError::Disconnected { .. })),
        "expected explicit disconnect, got {ended:?}"
    );
}

#[cfg(windows)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_production_step_event_over_real_ui_pipe() -> Result<(), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new("production-ui-pipe")?;
    let platform = FakePlatform::new("alpha\nbeta\n");
    let mut config = production_config(&directory.path);
    let pipe_name = fixture::unique_ui_pipe_name();
    config.ui_config.pipe_name.clone_from(&pipe_name);
    let host = assemble_production_host(config, platform.clone(), Arc::new(FixedClock)).await?;
    let plan = host.plan_task()?;
    let run = host.execute_plan(plan, FIXED_NOW_MS).await?;
    let expected_fingerprint = platform.fingerprint();

    let mut source = host.snapshot_event_source();
    let mut handler = TaskControlHandler::new(run.into_engine(), Arc::new(FixedClock));
    let server_pipe = pipe_name.clone();
    let server = std::thread::spawn(move || -> Result<(), IpcError> {
        let mut transport = NamedPipeTransport::server(&server_pipe)?;
        transport.accept(Duration::from_secs(5))?;
        let session_id = generate_session_id()?;
        let handshake = server_handshake(
            &mut transport,
            UI_TEST_TOKEN,
            Vec::new(),
            &session_id,
            Duration::from_secs(5),
        )?;
        serve_session_with_events(
            &mut transport,
            &handshake.server_hello,
            Duration::from_secs(2),
            &mut handler,
            &mut source,
        )
    });

    let mut transport = NamedPipeTransport::client(&pipe_name)?;
    transport.connect(Duration::from_secs(5))?;
    let _server_hello = client_handshake(
        &mut transport,
        UI_TEST_TOKEN,
        Vec::new(),
        Duration::from_secs(5),
    )?;
    // ADR-0065: the unconditional preamble is now the first step the timeline carries, and
    // `read_text` follows it. Both must reach the UI over the real pipe as committed events
    // carrying the injected platform's fingerprint.
    let task_id = host.task_id().as_str().to_owned();
    let preamble_event = read_step_event(&mut transport, "capture_initial_fingerprint")?;
    assert_step_event(
        &preamble_event,
        &task_id,
        "capture_initial_fingerprint",
        &expected_fingerprint,
    );
    let read_event = read_step_event(&mut transport, "read_text")?;
    assert_step_event(&read_event, &task_id, "read_text", &expected_fingerprint);

    drop(transport);
    join_ui_server(server);
    host.shutdown().await?;
    Ok(())
}

#[cfg(windows)]
#[tokio::test]
async fn test_production_t1_1_commits_through_real_tool_bus_and_receipt()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new("production-t1-1")?;
    let platform = FakePlatform::new("alpha\nbeta\n");
    let host = assemble_production_host(
        production_config(&directory.path),
        platform.clone(),
        Arc::new(FixedClock),
    )
    .await?;
    assert_eq!(host.task_id().as_str(), "notepad_t1_1_open_read_full_text");
    let plan = host.plan_task()?;
    let run = host.execute_plan(plan, FIXED_NOW_MS).await?;

    assert_eq!(run.final_snapshot.status, TaskStatus::Completed);
    // TASK-219 / ADR-0064 / ADR-0065: the unconditional fingerprint preamble, read_text, the
    // skipped file-channel branch, and the pure `analyze` step all commit a state transition,
    // so four snapshots are observed.
    assert_eq!(run.snapshots.len(), 4);
    let steps = &run.final_snapshot.steps;
    let step = steps.first().expect("one committed step");
    let expected_fingerprint = platform.fingerprint();
    assert_eq!(step.status, StepStatus::Committed);
    assert_eq!(
        step.post_fingerprint.as_deref(),
        Some(expected_fingerprint.as_str())
    );
    // ADR-0065 D2: that first fingerprint is the injected fake platform's own reading, produced
    // by the same code path production runs through `WindowsPlatform`.
    assert_eq!(step.id.to_string(), "capture_initial_fingerprint");
    assert_eq!(platform.state.lock().expect("fake state").read_calls, 1);

    let mut events = host.snapshot_event_source();
    let projected = events.drain();
    assert!(
        projected
            .iter()
            .any(|event| matches!(event, UiEvent::StepStateChanged { .. }))
    );
    assert!(
        projected
            .iter()
            .any(|event| matches!(event, UiEvent::TaskStateChanged { .. }))
    );
    let serialized = serde_json::to_string(&projected)?;
    assert!(serialized.contains("step_state_changed"));
    host.shutdown().await?;
    Ok(())
}

/// ADR-0065 / DRIFT-223-1: on the large-file branch the conditional first step evaluates false,
/// and the Plan still runs to `Completed` through the reserved file channel because the
/// unconditional preamble already published a real platform fingerprint.
#[cfg(windows)]
#[tokio::test]
async fn test_production_t1_1_large_file_skips_read_text_and_uses_the_file_channel()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new("production-t1-1-large")?;
    let document_path = directory.path.join("t1-1-large.txt");
    // 5200 bytes: comfortably over the 1024-byte budget, so the prefix read must truncate.
    std::fs::write(&document_path, "report line\n".repeat(400))?;
    let task_inputs = serde_json::json!({
        "input.file_path": document_path.to_string_lossy(),
        "file_size_bytes": 5200,
        "max_text_bytes": 1024,
        "input.keywords": ["report"],
        "input.max_keyword_paragraphs": 50,
    })
    .as_object()
    .cloned()
    .expect("task input object");
    // The same T1.1 package, adapter root, and UI config as the small-file run; only the task
    // inputs differ, which is exactly what selects the branch at runtime.
    let config = production_config(&directory.path).with_task_inputs(task_inputs);
    // The editor holds text that must never be analyzed: if the UIA branch ran, `analyze` would
    // see this string instead of the file prefix.
    let platform = FakePlatform::new("editor text that must never be read\n");
    let host = assemble_production_host(config, platform.clone(), Arc::new(FixedClock)).await?;

    let plan = host.plan_task()?;
    let run = host.execute_plan(plan, FIXED_NOW_MS).await?;

    assert_eq!(run.final_snapshot.status, TaskStatus::Completed);
    let steps = &run.final_snapshot.steps;
    let ids: Vec<String> = steps.iter().map(|step| step.id.to_string()).collect();
    assert_eq!(
        ids,
        vec![
            "capture_initial_fingerprint".to_owned(),
            "read_text".to_owned(),
            "read_file_channel".to_owned(),
            "analyze".to_owned(),
        ]
    );
    assert!(
        steps
            .iter()
            .all(|step| step.status == StepStatus::Committed),
        "条件为假的步骤按 ADR-0061 D8 仍提交，任何一步都不得停在失败态"
    );
    assert_eq!(run.snapshots.len(), 4);
    // `read_text` never touched UIA, so the small-file branch really was skipped instead of run;
    // `analyze` could only have committed on text the file channel published.
    assert_eq!(platform.state.lock().expect("fake state").read_calls, 0);
    // ADR-0065 D2: the preamble's fingerprint is the injected platform's own reading.
    assert_eq!(
        steps
            .first()
            .expect("preamble step")
            .post_fingerprint
            .as_deref(),
        Some(platform.fingerprint().as_str())
    );
    host.shutdown().await?;
    Ok(())
}

#[cfg(windows)]
#[tokio::test]
async fn test_production_t1_2_runs_with_bounded_approvals() -> Result<(), Box<dyn std::error::Error>>
{
    let directory = TestDirectory::new("production-t1-2")?;
    let adapter_root = workspace_root().join("adapters/com.microsoft.notepad");
    let task_package_path = adapter_root
        .join("tasks")
        .join("t1.2.replace-save-approval-undo.json");
    let document_path = directory.path.join("t1-2.txt");
    std::fs::write(&document_path, "报表 报表")?;
    let ui_config = UiServerConfig::new(
        "assistant-agent-core-production-t1-2",
        "ASSISTANT_AGENT_CORE_TEST_TOKEN",
        Duration::from_secs(1),
    )
    .with_allowed_peer("C:\\fixture\\peer.exe");
    let task_inputs = serde_json::json!({
        "input.file_path": document_path.to_string_lossy(),
        "input.old_text": "报表",
        "input.new_text": "报告",
        "input.expected_replacements": 2,
        "rollback.replace_recipe": "replace-text-l0-l1",
        "rollback.save_recipe": "save-l0-l1",
        "rollback.required_anchor_levels": ["L0", "L1"],
    })
    .as_object()
    .cloned()
    .expect("task input object");
    let config = ProductionConfig::new(
        directory.path.join("data"),
        adapter_root,
        task_package_path,
        ui_config,
    )
    .with_task_inputs(task_inputs);
    let platform = FakePlatform::new("报表 报表");
    let host = assemble_production_host(config, platform.clone(), Arc::new(FixedClock)).await?;
    let approvals = host.approvals();
    for step_id in ["approve_replace", "approve_save"] {
        approvals.grant(&GrantRequest {
            task_id: host.task_id().as_str(),
            step_id,
            scope: ApprovalScope::Once,
            now_ms: FIXED_NOW_MS,
            ttl_ms: 60_000,
            uses: 1,
        })?;
    }

    let plan = host.plan_task()?;
    let run = host.execute_plan(plan, FIXED_NOW_MS).await?;
    assert_eq!(run.final_snapshot.status, TaskStatus::Completed);
    assert!(
        run.final_snapshot
            .steps
            .iter()
            .all(|step| step.status == StepStatus::Committed)
    );
    assert_eq!(platform.state.lock().expect("fake state").text, "报告 报告");
    assert_eq!(platform.state.lock().expect("fake state").key_calls, 1);
    host.shutdown().await?;
    Ok(())
}

#[cfg(windows)]
#[tokio::test]
async fn test_production_rollback_without_a_captured_anchor_is_refused()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new("production-rollback-missing-anchor")?;
    let target_path = directory.path.join("missing-anchor.txt");
    std::fs::write(&target_path, "报表 报表")?;
    let (host, _platform) = assemble_t1_2_fake_host(&directory, &target_path).await?;
    let error = host
        .observe_rollback_state()
        .expect_err("observation must fail before any anchor is captured");
    assert!(
        error.contains("no captured anchor"),
        "unexpected missing-anchor error: {error}"
    );
    let error = host
        .execute_rollback(true)
        .expect_err("rollback must fail before any anchor is captured");
    assert!(
        error.contains("no captured anchor"),
        "unexpected missing-anchor rollback error: {error}"
    );
    host.shutdown().await?;
    Ok(())
}

#[cfg(windows)]
#[tokio::test]
async fn test_production_rollback_whole_anchor_overwrites_later_file_change()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new("production-rollback-digest-mismatch")?;
    let target_path = directory.path.join("digest-mismatch.txt");
    std::fs::write(&target_path, "报表 报表")?;
    let (host, platform) = assemble_t1_2_fake_host(&directory, &target_path).await?;
    let approvals = host.approvals();
    for step_id in ["approve_replace", "approve_save"] {
        approvals.grant(&GrantRequest {
            task_id: host.task_id().as_str(),
            step_id,
            scope: ApprovalScope::Once,
            now_ms: FIXED_NOW_MS,
            ttl_ms: 60_000,
            uses: 1,
        })?;
    }
    let run = host.execute_plan(host.plan_task()?, FIXED_NOW_MS).await?;
    assert_eq!(run.final_snapshot.status, TaskStatus::Completed);
    // A later user change to the file is detected as a conflict, but an explicit whole-anchor
    // restore (ADR-0062 D7, `RestoreOverall`) deliberately restores the captured snapshot.
    std::fs::write(&target_path, "user changed the file")?;
    let restored = host.execute_rollback(true)?;
    assert_eq!(
        restored
            .get("file_matches_snapshot")
            .and_then(serde_json::Value::as_bool),
        Some(true),
        "an explicit whole-anchor restore must restore the captured bytes: {restored}"
    );
    assert_eq!(std::fs::read_to_string(&target_path)?, "报表 报表");
    assert_eq!(platform.state.lock().expect("fake state").text, "报表 报表");
    host.shutdown().await?;
    Ok(())
}

#[cfg(windows)]
#[tokio::test]
async fn test_production_rollback_user_change_is_an_incident()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new("production-rollback-user-change")?;
    let target_path = directory.path.join("user-change.txt");
    std::fs::write(&target_path, "报表 报表")?;
    let (host, _platform) = assemble_t1_2_fake_host(&directory, &target_path).await?;
    let approvals = host.approvals();
    for step_id in ["approve_replace", "approve_save"] {
        approvals.grant(&GrantRequest {
            task_id: host.task_id().as_str(),
            step_id,
            scope: ApprovalScope::Once,
            now_ms: FIXED_NOW_MS,
            ttl_ms: 60_000,
            uses: 1,
        })?;
    }
    let run = host.execute_plan(host.plan_task()?, FIXED_NOW_MS).await?;
    assert_eq!(run.final_snapshot.status, TaskStatus::Completed);
    // The editor-only path is fail-closed: the observed state is neither the anchor nor the
    // recorded post-step state, so it must stop rather than guess.
    let error = host
        .execute_rollback(false)
        .expect_err("a later user change must block the fail-closed editor path");
    assert!(
        error.contains("current state changed") || error.contains("rollback"),
        "unexpected user-change error: {error}"
    );
    host.shutdown().await?;
    Ok(())
}

#[cfg(windows)]
#[tokio::test]
async fn test_production_rollback_uses_l1_fallback_when_l0_misses()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new("production-rollback-fallback")?;
    let target_path = directory.path.join("fallback.txt");
    std::fs::write(&target_path, "报表 报表")?;
    let (host, platform) = assemble_t1_2_fake_host(&directory, &target_path).await?;
    let approvals = host.approvals();
    for step_id in ["approve_replace", "approve_save"] {
        approvals.grant(&GrantRequest {
            task_id: host.task_id().as_str(),
            step_id,
            scope: ApprovalScope::Once,
            now_ms: FIXED_NOW_MS,
            ttl_ms: 60_000,
            uses: 1,
        })?;
    }
    let run = host.execute_plan(host.plan_task()?, FIXED_NOW_MS).await?;
    assert_eq!(run.final_snapshot.status, TaskStatus::Completed);
    // Force the L0 undo to land on a state that is not the anchor: an out-of-band change with
    // no matching undo history, so Ctrl+Z cannot return to the captured anchor and the executor
    // has to use the L1 snapshot.
    {
        let mut state = platform.state.lock().expect("fake state");
        state.text_history.clear();
        state.text = "third-party change".to_owned();
    }
    let fallback = host.execute_rollback(true)?;
    assert_eq!(
        fallback
            .get("used_fallback")
            .and_then(serde_json::Value::as_bool),
        Some(true),
        "L0 miss must be recovered through the L1 snapshot: {fallback}"
    );
    assert_eq!(
        fallback
            .get("file_matches_snapshot")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert_eq!(std::fs::read_to_string(&target_path)?, "报表 报表");
    host.shutdown().await?;
    Ok(())
}

/// Builds a production host around the fake platform for the T1.2 rollback paths.
#[cfg(windows)]
async fn assemble_t1_2_fake_host(
    directory: &TestDirectory,
    target_path: &Path,
) -> Result<
    (
        assistant_agent_core::ProductionHost<FakePlatform>,
        FakePlatform,
    ),
    Box<dyn std::error::Error>,
> {
    let adapter_root = workspace_root().join("adapters/com.microsoft.notepad");
    let adapter_root_clone = adapter_root.clone();
    let ui_config = UiServerConfig::new(
        "assistant-agent-core-production-rollback",
        "ASSISTANT_AGENT_CORE_TEST_TOKEN",
        Duration::from_secs(1),
    )
    .with_allowed_peer("C:\\fixture\\peer.exe");
    let task_inputs = serde_json::json!({
        "input.file_path": target_path.to_string_lossy(),
        "input.old_text": "报表",
        "input.new_text": "报告",
        "input.expected_replacements": 2,
        "rollback.replace_recipe": "replace-text-l0-l1",
        "rollback.save_recipe": "save-l0-l1",
        "rollback.required_anchor_levels": ["L0", "L1"],
    })
    .as_object()
    .cloned()
    .expect("task input object");
    let config = ProductionConfig::new(
        directory.path.join("data"),
        adapter_root_clone,
        adapter_root.join("tasks/t1.2.replace-save-approval-undo.json"),
        ui_config,
    )
    .with_task_inputs(task_inputs);
    let platform = FakePlatform::new("报表 报表");
    let host = assemble_production_host(config, platform.clone(), Arc::new(FixedClock)).await?;
    Ok((host, platform))
}

#[cfg(windows)]
#[tokio::test]
async fn test_production_t1_2_pauses_and_resumes_with_ui_approval()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new("production-t1-2-resume")?;
    let adapter_root = workspace_root().join("adapters/com.microsoft.notepad");
    let task_package_path = adapter_root
        .join("tasks")
        .join("t1.2.replace-save-approval-undo.json");
    let document_path = directory.path.join("t1-2-resume.txt");
    std::fs::write(&document_path, "报表 报表")?;
    let ui_config = UiServerConfig::new(
        "assistant-agent-core-production-t1-2-resume",
        "ASSISTANT_AGENT_CORE_TEST_TOKEN",
        Duration::from_secs(1),
    )
    .with_allowed_peer("C:\\fixture\\peer.exe");
    let task_inputs = serde_json::json!({
        "input.file_path": document_path.to_string_lossy(),
        "input.old_text": "报表",
        "input.new_text": "报告",
        "input.expected_replacements": 2,
        "rollback.replace_recipe": "replace-text-l0-l1",
        "rollback.save_recipe": "save-l0-l1",
        "rollback.required_anchor_levels": ["L0", "L1"],
    })
    .as_object()
    .cloned()
    .expect("task input object");
    let config = ProductionConfig::new(
        directory.path.join("data"),
        adapter_root,
        task_package_path,
        ui_config,
    )
    .with_task_inputs(task_inputs);
    let platform = FakePlatform::new("报表 报表");
    let host = assemble_production_host(config, platform.clone(), Arc::new(FixedClock)).await?;

    let run = host.execute_plan(host.plan_task()?, FIXED_NOW_MS).await?;
    let first_pause = run
        .pending_approval()
        .cloned()
        .expect("the replace step must pause for approval");
    assert_eq!(first_pause.step_id.as_str(), "approve_replace");
    let mut handler = TaskControlHandler::new(
        TaskEngine::new(MemoryCheckpointStore::new()),
        Arc::new(FixedClock),
    )
    .with_pending(host.pending_approvals())
    .with_approvals(host.approvals());
    handler
        .handle(UiCommand::ApproveRequest {
            request_id: first_pause.request_id,
            scope: UiAuthorizationScope::Once,
        })
        .expect("UI approval for replace");

    let run = host
        .resume_plan(run, FIXED_NOW_MS.saturating_add(10))
        .await?;
    let second_pause = run
        .pending_approval()
        .cloned()
        .expect("the save step must pause for approval");
    assert_eq!(second_pause.step_id.as_str(), "approve_save");
    assert_eq!(
        run.final_snapshot
            .step(&assistant_task_engine::StepId::new(
                "capture_pre_replace_text"
            )?)
            .map(|step| step.attempts),
        Some(1),
        "resuming an approval must not replay committed read steps"
    );
    assert_eq!(platform.state.lock().expect("fake state").text, "报告 报告");

    handler
        .handle(UiCommand::ApproveRequest {
            request_id: second_pause.request_id,
            scope: UiAuthorizationScope::Once,
        })
        .expect("UI approval for save");
    let run = host
        .resume_plan(run, FIXED_NOW_MS.saturating_add(20))
        .await?;

    assert_eq!(run.final_snapshot.status, TaskStatus::Completed);
    assert!(!run.is_awaiting_approval());
    assert!(
        run.final_snapshot
            .steps
            .iter()
            .all(|step| step.status == StepStatus::Committed)
    );
    assert_eq!(platform.state.lock().expect("fake state").key_calls, 1);
    host.shutdown().await?;
    Ok(())
}

#[cfg(windows)]
#[tokio::test]
async fn test_production_t1_3_creates_a_new_file_through_fake_platform()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new("production-t1-3")?;
    let adapter_root = workspace_root().join("adapters/com.example.notepad-like");
    let task_package_path = workspace_root()
        .join("adapters/com.microsoft.notepad/tasks/t1.3.new-tab-write-save-as.json");
    let target_path = directory.path.join("created-by-t1-3.txt");
    let text = "alpha\nbeta\n";
    let ui_config = UiServerConfig::new(
        "assistant-agent-core-production-t1-3",
        "ASSISTANT_AGENT_CORE_TEST_TOKEN",
        Duration::from_secs(1),
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
        adapter_root,
        task_package_path,
        ui_config,
    )
    .with_task_inputs(task_inputs);
    let platform = FakePlatform::new("original");
    let host = assemble_production_host(config, platform.clone(), Arc::new(FixedClock)).await?;

    let run = host.execute_plan(host.plan_task()?, FIXED_NOW_MS).await?;
    let pause = run
        .pending_approval()
        .cloned()
        .expect("Save As must pause for approval");
    assert_eq!(pause.step_id.as_str(), "approve_save_as");

    let mut handler = TaskControlHandler::new(
        TaskEngine::new(MemoryCheckpointStore::new()),
        Arc::new(FixedClock),
    )
    .with_pending(host.pending_approvals())
    .with_approvals(host.approvals());
    handler
        .handle(UiCommand::ApproveRequest {
            request_id: pause.request_id,
            scope: UiAuthorizationScope::Once,
        })
        .expect("UI approval for Save As");

    let run = host
        .resume_plan(run, FIXED_NOW_MS.saturating_add(10))
        .await?;
    assert_eq!(run.final_snapshot.status, TaskStatus::Completed);
    assert!(!run.is_awaiting_approval());
    assert_eq!(std::fs::read_to_string(&target_path)?, text);
    assert_eq!(platform.state.lock().expect("fake state").tab_count, 2);
    assert_eq!(platform.state.lock().expect("fake state").text, text);
    assert!(
        run.final_snapshot
            .steps
            .iter()
            .all(|step| step.status == StepStatus::Committed)
    );
    host.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn test_production_missing_task_package_fails_closed()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new("production-missing-task")?;
    let mut config = production_config(&directory.path);
    config.task_package_path = directory.path.join("missing.json");
    let platform = FakePlatform::new("text");
    let Err(error) = assemble_production_host(config, platform, Arc::new(FixedClock)).await else {
        panic!("missing task package must fail");
    };
    assert!(error.to_string().contains("could not be read"));
    Ok(())
}

#[tokio::test]
async fn test_production_missing_plan_provider_reports_capability_missing()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new("production-missing-provider")?;
    let mut config = production_config(&directory.path);
    config.task_package_path = directory.path.join("missing-provider.json");
    let platform = FakePlatform::new("text");
    let Err(error) = assemble_production_host(config, platform, Arc::new(FixedClock)).await else {
        panic!("missing plan provider must fail");
    };
    assert!(matches!(error, ProductionError::TaskPackage(_)));
    assert_eq!(error.error_code(), ErrorCode::CapabilityMissing);
    Ok(())
}

#[tokio::test]
async fn test_production_handler_count_mismatch_fails_closed()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new("production-handler-count")?;
    let adapter_root = directory.path.join("adapter");
    let source_adapter_root = workspace_root().join("adapters/com.microsoft.notepad");
    std::fs::create_dir_all(adapter_root.join("selectors"))?;
    std::fs::create_dir_all(adapter_root.join("tools"))?;
    std::fs::copy(
        source_adapter_root.join("selectors").join("targets.json"),
        adapter_root.join("selectors").join("targets.json"),
    )?;
    let tools_body = std::fs::read_to_string(source_adapter_root.join("tools").join("tools.json"))?;
    let mut tools: serde_json::Value = serde_json::from_str(&tools_body)?;
    let declared_tools = tools
        .get_mut("tools")
        .and_then(serde_json::Value::as_array_mut)
        .ok_or_else(|| std::io::Error::other("tools.json has no tools array"))?;
    declared_tools.pop();
    std::fs::write(
        adapter_root.join("tools").join("tools.json"),
        serde_json::to_vec(&tools)?,
    )?;

    let mut config = production_config(&directory.path);
    config.adapter_root = adapter_root;
    let platform = FakePlatform::new("text");
    let Err(error) = assemble_production_host(config, platform, Arc::new(FixedClock)).await else {
        panic!("handler count mismatch must fail");
    };
    assert!(matches!(
        error,
        ProductionError::InvalidConfiguration {
            field: "adapter_root.tools.tools",
            ..
        }
    ));
    assert_eq!(error.error_code(), ErrorCode::ToolInvalidArgs);
    assert!(
        error
            .to_string()
            .contains("expected 5 declared tools, found 4")
    );
    Ok(())
}

#[tokio::test]
async fn test_production_empty_ui_peer_allowlist_fails_closed()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new("production-empty-peer")?;
    let mut config = production_config(&directory.path);
    config.ui_config.allowed_peer_images.clear();
    let platform = FakePlatform::new("text");
    let Err(error) = assemble_production_host(config, platform, Arc::new(FixedClock)).await else {
        panic!("empty UI peer allow-list must fail");
    };
    assert!(error.to_string().contains("allowed_peer_images"));
    Ok(())
}
