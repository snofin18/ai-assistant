//! Full-ribbon Paint real-machine acceptance for TASK-309.
//!
//! Included by `production_paint.rs`; this module owns the opt-in test that
//! exercises shape, outline, fill, size, brush style, color, and drawing in one
//! production Plan. It writes every captured BGRA frame as a BMP plus a JSON
//! evidence summary under `target/paint-full-toolset-evidence/`.

use super::paint_real_run::{
    REAL_RUN_FOREGROUND_SETTLE, assert_capture_size_matches_window, read_real_canvas_inputs,
    require_real_paint_foreground, settle_then_require_foreground,
};
use super::*;

use serde_json::json;

/// Continue-on-existing-canvas package that exercises the full ribbon toolset
/// before drawing the same rectangle contract.
fn paint_full_toolset_config(data_root: &Path, canvas: PaintCanvasInputs) -> ProductionConfig {
    super::paint_config_for_task(
        data_root,
        "t3.2.full-toolset-path.json",
        canvas,
        (250, 10),
        (350, 50),
    )
}

/// Finds the unique Paint window and explicitly brings it forward.
///
/// The shared real-machine helper intentionally waits for a human to focus
/// Paint, but a Codex-hosted `cargo test` call immediately returns focus to the
/// host UI. This opt-in test therefore performs the single focus operation
/// itself, while every later input still passes `require_real_paint_foreground`.
async fn focus_real_paint_window(
    platform: &WindowsPlatform,
) -> Result<ResolvedWindow, Box<dyn std::error::Error>> {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let windows = platform.list_windows(&WindowFilter::any()).await?;
        if let Some(info) = windows.into_iter().find(|info| {
            let app_id = info.app_id().to_ascii_lowercase();
            (app_id.contains("mspaint") || app_id.contains("paint"))
                && info.window().display_label().contains("MSPaintApp")
        }) {
            if platform.window_state(info.window()).await?.foreground() {
                return Ok(info.window().clone());
            }
            if let Err(error) = platform
                .bring_to_front(info.window(), FocusPolicy::AllowSteal)
                .await
                && error.code() != ErrorCode::TargetUnresponsive
            {
                return Err(error.into());
            }
        }
        if Instant::now() >= deadline {
            return Err("timed out finding and focusing the Paint window".into());
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

#[derive(Debug, Clone)]
struct EvidenceCapture {
    sequence: usize,
    content_address: String,
    width: u32,
    height: u32,
    path: PathBuf,
}

#[derive(Debug)]
struct EvidenceBlobSink {
    output_dir: PathBuf,
    storage: StorageBlobSink,
    records: Mutex<Vec<EvidenceCapture>>,
}

impl EvidenceBlobSink {
    const fn new(output_dir: PathBuf, storage: StorageBlobSink) -> Self {
        Self {
            output_dir,
            storage,
            records: Mutex::new(Vec::new()),
        }
    }

    fn records(&self) -> Vec<EvidenceCapture> {
        self.records.lock().expect("evidence sink records").clone()
    }
}

impl ImageBlobSink for EvidenceBlobSink {
    fn store_bgra(
        &self,
        width: u32,
        height: u32,
        content_address: &str,
        bgra: &[u8],
    ) -> PlatformResult<()> {
        self.storage
            .store_bgra(width, height, content_address, bgra)?;
        let sequence = self.records.lock().expect("evidence sink records").len();
        let file_name = format!("capture-{sequence:03}-{content_address}.bmp");
        let path = self.output_dir.join(file_name);
        write_bmp_bgra(&path, width, height, bgra).map_err(|error| {
            PlatformError::new(
                ErrorCode::Fatal,
                format!("failed to write evidence BMP {}: {error}", path.display()),
            )
        })?;
        self.records
            .lock()
            .expect("evidence sink records")
            .push(EvidenceCapture {
                sequence,
                content_address: content_address.to_owned(),
                width,
                height,
                path,
            });
        Ok(())
    }
}

/// Writes a top-down 32-bit BMP from the BGRA bytes returned by `capture`.
///
/// The test intentionally avoids an image dependency: BMP's 32-bit `BI_RGB`
/// layout is exactly the byte order the capture already produces, and a
/// negative height encodes a top-down image without row reordering.
fn write_bmp_bgra(path: &Path, width: u32, height: u32, bgra: &[u8]) -> std::io::Result<()> {
    let expected = usize::try_from(width)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?
        .checked_mul(
            usize::try_from(height)
                .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?,
        )
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "BMP size overflow"))?;
    if bgra.len() != expected {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!(
                "BGRA length {} does not match {width}x{height}x4 = {expected}",
                bgra.len()
            ),
        ));
    }

    let width_i32 = i32::try_from(width)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    let height_i32 = i32::try_from(height)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?
        .checked_neg()
        .ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "BMP height overflow")
        })?;
    let pixel_bytes = u32::try_from(bgra.len())
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    let file_size = 14_u32
        .checked_add(40)
        .and_then(|value| value.checked_add(pixel_bytes))
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "BMP file overflow"))?;

    let mut bytes = Vec::with_capacity(
        usize::try_from(file_size)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?,
    );
    bytes.extend_from_slice(b"BM");
    bytes.extend_from_slice(&file_size.to_le_bytes());
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    bytes.extend_from_slice(&54_u32.to_le_bytes());
    bytes.extend_from_slice(&40_u32.to_le_bytes());
    bytes.extend_from_slice(&width_i32.to_le_bytes());
    bytes.extend_from_slice(&height_i32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&32_u16.to_le_bytes());
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes.extend_from_slice(&pixel_bytes.to_le_bytes());
    bytes.extend_from_slice(&2835_i32.to_le_bytes());
    bytes.extend_from_slice(&2835_i32.to_le_bytes());
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes.extend_from_slice(bgra);
    std::fs::write(path, bytes)
}

/// TASK-309: one full-ribbon path through the production Paint handlers.
#[tokio::test]
#[ignore = "TASK-309: requires an interactive Paint window with an existing canvas and human observer"]
async fn test_paint_t3_2_full_toolset_path_runs_on_real_paint()
-> Result<(), Box<dyn std::error::Error>> {
    assistant_platform_windows::enable_per_monitor_v2_dpi_awareness()?;
    let directory = TestDirectory::new("production-paint-real-full-toolset")?;
    let evidence_root = workspace_root()
        .join("target")
        .join("paint-full-toolset-evidence");
    let evidence_stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let evidence_dir = evidence_root.join(evidence_stamp.to_string());
    std::fs::create_dir_all(&evidence_dir)?;

    let storage = StorageBlobSink::new();
    let sink: &'static EvidenceBlobSink = Box::leak(Box::new(EvidenceBlobSink::new(
        evidence_dir.clone(),
        storage.clone(),
    )));
    let platform = WindowsPlatform::new().with_blob_sink(sink);
    let paint_window = focus_real_paint_window(&platform).await?;
    settle_then_require_foreground(&platform, &paint_window, REAL_RUN_FOREGROUND_SETTLE).await?;
    assert_capture_size_matches_window(&platform, &paint_window).await?;
    let canvas = read_real_canvas_inputs(&platform, &paint_window).await?;

    let host = assemble_production_host(
        paint_full_toolset_config(&directory.path, canvas),
        platform,
        Arc::new(SystemClock),
    )
    .await?;
    assert!(storage.attach(&host.database_handle()));

    let now_ms = SystemClock.now_unix_ms();
    let approvals = host.approvals();
    for step_id in [
        "select_rectangle_tool",
        "select_shape_outline",
        "select_shape_fill",
        "select_shape_size",
        "select_brush_style",
        "select_rectangle_tool_again",
        "select_foreground_color",
        "select_layer",
        "draw_rectangle",
    ] {
        approvals.grant(&GrantRequest {
            task_id: host.task_id().as_str(),
            step_id,
            scope: ApprovalScope::Once,
            now_ms,
            ttl_ms: 60_000,
            uses: 1,
        })?;
    }

    let plan = host.plan_task()?;
    assert_eq!(plan.steps.len(), 14);
    require_real_paint_foreground(&platform, &paint_window).await?;
    let run = host.execute_plan(plan, now_ms).await?;
    assert_eq!(run.final_snapshot.status, TaskStatus::Completed);
    assert!(
        run.final_snapshot
            .steps
            .iter()
            .all(|step| step.status == assistant_task_engine::StepStatus::Committed),
        "every step of the full-toolset Plan must commit"
    );
    tokio::time::sleep(Duration::from_millis(750)).await;
    let settled_capture = platform
        .capture(&paint_window, &CaptureOptions::new(false))
        .await?;
    host.shutdown().await?;

    let captures = sink.records();
    let before = captures
        .first()
        .ok_or("full-toolset run produced no capture records")?;
    let after = captures
        .last()
        .ok_or("full-toolset run produced no capture records")?;
    if before.content_address == after.content_address {
        return Err(format!(
            "full-toolset run finished without a pixel change: {}",
            before.content_address
        )
        .into());
    }

    let step_statuses = run
        .final_snapshot
        .steps
        .iter()
        .map(|step| {
            json!({
                "id": step.id.to_string(),
                "status": format!("{:?}", step.status),
            })
        })
        .collect::<Vec<_>>();
    let capture_rows = captures
        .iter()
        .map(|capture| {
            json!({
                "sequence": capture.sequence,
                "content_address": capture.content_address,
                "width_px": capture.width,
                "height_px": capture.height,
                "path": capture.path,
            })
        })
        .collect::<Vec<_>>();
    let evidence = json!({
        "task_package": "t3.2.full-toolset-path.json",
        "status": "Completed",
        "step_statuses": step_statuses,
        "capture_count": captures.len(),
        "before_content_address": before.content_address,
        "after_content_address": after.content_address,
        "settled_capture_blob_id": settled_capture.blob_id(),
        "captures": capture_rows,
    });
    std::fs::write(
        evidence_dir.join("evidence.json"),
        serde_json::to_vec_pretty(&evidence)?,
    )?;
    Ok(())
}
