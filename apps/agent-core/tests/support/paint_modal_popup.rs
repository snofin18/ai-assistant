//! Fake-platform coverage for the Paint modal-popup fail-closed path.
//!
//! Included by `production_paint.rs` via `#[path]`; it reuses that test
//! binary's helpers through `super::*` and keeps the parent file below the
//! repository's 900-line hygiene hard limit.

use super::*;

#[tokio::test]
async fn test_paint_document_new_fails_closed_when_modal_popup_is_present()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new("production-paint-modal-popup")?;
    let platform = PaintFakePlatform::new(None);
    {
        let mut state = platform.state.lock().expect("paint fake state");
        state.modal_popup = true;
    }
    let host = assemble_production_host(
        paint_config(&directory.path),
        platform.clone(),
        Arc::new(FixedClock),
    )
    .await?;

    host.approvals().grant(&GrantRequest {
        task_id: host.task_id().as_str(),
        step_id: "new_document",
        scope: ApprovalScope::Once,
        now_ms: FIXED_NOW_MS,
        ttl_ms: 60_000,
        uses: 1,
    })?;

    let plan = host.plan_task()?;
    let error = match host.execute_plan(plan, FIXED_NOW_MS).await {
        Ok(run) => panic!(
            "an open modal popup must fail the run, got {:?}",
            run.final_snapshot.status
        ),
        Err(error) => error,
    };
    assert!(
        error.to_string().contains("modal/popup surface"),
        "error must explain the modal popup: {error}"
    );
    {
        let state = platform.state.lock().expect("paint fake state");
        assert_eq!(state.revision, 1, "no synthetic input may be sent");
        assert_eq!(state.pointer_actions, 0, "no pointer input may be sent");
    }
    host.shutdown().await?;
    Ok(())
}
