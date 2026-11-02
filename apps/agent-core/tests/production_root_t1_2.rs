//! Production-root T1.2 acceptance tests with bounded approvals and rollback paths.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::too_many_lines,
    clippy::unwrap_used
)]

#[cfg(windows)]
use std::path::Path;
#[cfg(windows)]
use std::sync::Arc;
#[cfg(windows)]
use std::time::Duration;

#[cfg(windows)]
#[path = "support/production_fixture.rs"]
mod fixture;

#[cfg(windows)]
use fixture::FIXED_NOW_MS;
#[cfg(windows)]
use fixture::{FakePlatform, FixedClock, TestDirectory, workspace_root};

#[cfg(windows)]
use assistant_agent_core::GrantRequest;
#[cfg(windows)]
use assistant_agent_core::{
    ProductionConfig, ProductionError, UiAuthorizationScope, UiCommand, UiCommandHandler,
    UiCommandOutcome, UiServerConfig, assemble_production_host,
};
#[cfg(windows)]
use assistant_hitl::ApprovalScope;
#[cfg(windows)]
use assistant_task_engine::{StepStatus, TaskStatus};

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
    let mut approved_steps = Vec::new();
    let run = host
        .drive_ui_approvals(run, |_config, pending, handler, events| {
            approved_steps.push(pending.step_id.to_string());
            let emitted = events.drain();
            assert!(emitted.iter().any(|event| matches!(
                event,
                assistant_agent_core::UiEvent::StepStateChanged { step_id, .. }
                    if step_id == pending.step_id.as_str()
            )));
            handler
                .handle(UiCommand::ApproveRequest {
                    request_id: pending.request_id.clone(),
                    scope: UiAuthorizationScope::Once,
                })
                .map_err(|error| ProductionError::UiServer {
                    reason: error.to_string(),
                })
        })
        .await?;
    assert_eq!(approved_steps, vec!["approve_replace", "approve_save"]);
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
async fn test_drive_ui_approvals_denies_with_the_run_engine()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new("production-t1-2-deny")?;
    let document_path = directory.path.join("t1-2-deny.txt");
    std::fs::write(&document_path, "报表 报表")?;
    let (host, platform) = assemble_t1_2_fake_host(&directory, &document_path).await?;

    let run = host.execute_plan(host.plan_task()?, FIXED_NOW_MS).await?;
    let denied_step = run
        .pending_approval()
        .map(|pending| pending.step_id.to_string())
        .expect("the replace step must pause");
    let run = host
        .drive_ui_approvals(run, |_config, pending, handler, events| {
            assert!(events.drain().iter().any(|event| matches!(
                event,
                assistant_agent_core::UiEvent::StepStateChanged { step_id, .. }
                    if step_id == pending.step_id.as_str()
            )));
            handler
                .handle(UiCommand::DenyRequest {
                    request_id: pending.request_id.clone(),
                    reason: "human rejected the production test".to_owned(),
                })
                .map_err(|error| ProductionError::UiServer {
                    reason: error.to_string(),
                })
        })
        .await?;

    assert_eq!(denied_step, "approve_replace");
    assert_ne!(run.final_snapshot.status, TaskStatus::Completed);
    assert!(!run.is_awaiting_approval());
    assert_eq!(platform.state.lock().expect("fake state").text, "报表 报表");
    assert_eq!(platform.state.lock().expect("fake state").key_calls, 0);
    host.shutdown().await?;
    Ok(())
}

#[cfg(windows)]
#[tokio::test]
async fn test_drive_ui_approvals_rejects_a_decision_without_a_grant()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new("production-t1-2-no-grant")?;
    let document_path = directory.path.join("t1-2-no-grant.txt");
    std::fs::write(&document_path, "报表 报表")?;
    let (host, _platform) = assemble_t1_2_fake_host(&directory, &document_path).await?;

    let run = host.execute_plan(host.plan_task()?, FIXED_NOW_MS).await?;
    let result = host
        .drive_ui_approvals(run, |_config, pending, _handler, _events| {
            Ok(UiCommandOutcome::ApprovalGranted {
                request_id: pending.request_id.clone(),
                scope: UiAuthorizationScope::Once,
            })
        })
        .await;
    let Err(error) = result else {
        return Err("an approval outcome without a grant must fail closed".into());
    };
    let reason = error.to_string();
    assert!(
        reason.contains("already pending") || reason.contains("paused again"),
        "{reason}"
    );
    host.shutdown().await?;
    Ok(())
}

#[cfg(windows)]
#[tokio::test]
async fn test_drive_ui_approvals_rejects_a_mismatched_request()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new("production-t1-2-wrong-request")?;
    let document_path = directory.path.join("t1-2-wrong-request.txt");
    std::fs::write(&document_path, "报表 报表")?;
    let (host, _platform) = assemble_t1_2_fake_host(&directory, &document_path).await?;

    let run = host.execute_plan(host.plan_task()?, FIXED_NOW_MS).await?;
    let result = host
        .drive_ui_approvals(run, |_config, _pending, _handler, _events| {
            Ok(UiCommandOutcome::ApprovalGranted {
                request_id: "runtime:another_task:another_step".to_owned(),
                scope: UiAuthorizationScope::Once,
            })
        })
        .await;
    let Err(error) = result else {
        return Err("a mismatched approval request must fail closed".into());
    };
    assert!(error.to_string().contains("does not match pending request"));
    host.shutdown().await?;
    Ok(())
}

#[cfg(windows)]
#[tokio::test]
async fn test_drive_ui_approvals_rejects_a_denial_without_engine_transition()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new("production-t1-2-fake-deny")?;
    let document_path = directory.path.join("t1-2-fake-deny.txt");
    std::fs::write(&document_path, "报表 报表")?;
    let (host, _platform) = assemble_t1_2_fake_host(&directory, &document_path).await?;

    let run = host.execute_plan(host.plan_task()?, FIXED_NOW_MS).await?;
    let result = host
        .drive_ui_approvals(run, |_config, pending, _handler, _events| {
            Ok(UiCommandOutcome::ApprovalDenied {
                request_id: pending.request_id.clone(),
                reason: "forged outcome without engine transition".to_owned(),
                task_status: "failed".to_owned(),
            })
        })
        .await;
    let Err(error) = result else {
        return Err("a denial outcome without an engine transition must fail closed".into());
    };
    assert!(
        error
            .to_string()
            .contains("was not applied by the run engine")
    );
    host.shutdown().await?;
    Ok(())
}
