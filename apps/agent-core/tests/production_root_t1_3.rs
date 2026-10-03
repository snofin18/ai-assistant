//! Production-root T1.3 acceptance tests for the fake Save As path.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::too_many_lines,
    clippy::unwrap_used
)]

#[cfg(windows)]
use std::sync::Arc;
#[cfg(windows)]
use std::time::Duration;

#[path = "support/production_fixture.rs"]
mod fixture;

#[cfg(windows)]
use fixture::FIXED_NOW_MS;
#[cfg(windows)]
use fixture::{FakePlatform, FixedClock, TestDirectory, workspace_root};

#[cfg(windows)]
use assistant_agent_core::{
    ProductionConfig, TaskControlHandler, UiAuthorizationScope, UiCommand, UiCommandHandler,
    UiServerConfig, assemble_production_host,
};
#[cfg(windows)]
use assistant_task_engine::{MemoryCheckpointStore, StepStatus, TaskEngine, TaskStatus};

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
