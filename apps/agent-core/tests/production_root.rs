//! Production-root acceptance tests for TASK-214 without a real commercial application.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::too_many_lines,
    clippy::unwrap_used
)]

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
#[cfg(windows)]
use std::time::{SystemTime, UNIX_EPOCH};

#[path = "support/production_fixture.rs"]
mod fixture;

#[cfg(windows)]
use fixture::FIXED_NOW_MS;
use fixture::{FakePlatform, FixedClock, TestDirectory, workspace_root};

use assistant_agent_core::UiServerConfig;
use assistant_agent_core::{ProductionConfig, ProductionError, assemble_production_host};
#[cfg(windows)]
use assistant_agent_core::{TaskControlHandler, UiEvent, UiEventSource, serve_session_with_events};
#[cfg(windows)]
use assistant_ipc::{
    IpcError, NamedPipeTransport, Transport, WireMessage, client_handshake, generate_session_id,
    server_handshake,
};
use assistant_platform_api::ErrorCode;
#[cfg(windows)]
use assistant_platform_api::Fingerprint;
#[cfg(windows)]
use assistant_task_engine::{StepStatus, TaskStatus};

#[cfg(windows)]
const UI_TEST_TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

#[cfg(windows)]
fn unique_ui_pipe_name() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    format!("assistant-agent-core-ui-{}-{nanos}", std::process::id())
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
    let pipe_name = unique_ui_pipe_name();
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
