//! Fake-platform coverage for continuing on an existing Paint canvas.
//!
//! Included by `production_paint.rs` via `#[path]`; it reuses that test
//! binary's fake platform and configuration helpers through `super::*`.

use super::*;

#[tokio::test]
async fn test_paint_t3_2_continues_existing_canvas_on_fake_platform()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = TestDirectory::new("production-paint-t3-2")?;
    let sink = StorageBlobSink::new();
    let platform = PaintFakePlatform::new(Some(Arc::new(sink.clone())));
    {
        let mut state = platform.state.lock().expect("paint fake state");
        state.drawn = true;
        state.canvas_revision = 1;
    }
    let host = assemble_production_host(
        paint_continue_config(&directory.path, PaintCanvasInputs::FAKE),
        platform.clone(),
        Arc::new(FixedClock),
    )
    .await?;
    assert!(sink.attach(&host.database_handle()));

    let approvals = host.approvals();
    for step_id in [
        "select_rectangle_tool",
        "select_foreground_color",
        "select_layer",
        "draw_rectangle",
    ] {
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
    assert_eq!(plan.steps.len(), 9);
    assert!(
        !plan
            .steps
            .iter()
            .any(|step| step.id.as_str() == "new_document"),
        "T3.2 must never create a new document"
    );
    let run = host.execute_plan(plan, FIXED_NOW_MS).await?;
    assert_eq!(run.final_snapshot.status, TaskStatus::Completed);
    {
        let state = platform.state.lock().expect("paint fake state");
        assert!(state.drawn, "the existing canvas must have been drawn on");
        assert_eq!(
            state.canvas_revision, 2,
            "the second draw must have produced a new canvas revision"
        );
    }
    host.shutdown().await?;
    Ok(())
}
