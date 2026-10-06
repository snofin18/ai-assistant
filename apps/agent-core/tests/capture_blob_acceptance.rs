//! 真机验收（TASK-246 / ADR-0076）：真实窗口截图 → 像素遮挡 → 内容地址 → blob 落盘。
//!
//! 这条用例**只在真机桌面有意义**，默认 `#[ignore]`；跑法：
//! `cargo test -p assistant-agent-core --test capture_blob_acceptance -- --ignored --nocapture`
//!
//! 它验证的是**端到端**链路，而不是纯逻辑：`WindowsPlatform::capture` 走 GDI 真机路径，
//! 把 BGRA 交给注入的 `StorageBlobSink`，后者写进 `crates/storage` 的内容寻址 blob 池。
//! 断言全部可机器复核：地址须是 64 位小写 hex、`width*height*4` 须等于落盘字节数、
//! 且**重算哈希**须与地址逐字相同（`BlobStore::get` 读路径本身也会重算 sha256）。

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::unwrap_used
)]
// 整条链路只在 Windows 真机上存在（GDI 截图 + 桌面窗口）；非 Windows 目标下本测试 crate 为空，
// 否则 TestDirectory / open_handle 等仅被 `#[cfg(windows)]` 用例使用的辅助项会触发 dead_code（-D warnings）。
#![cfg(windows)]

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use assistant_agent_core::StorageBlobSink;
use assistant_storage::{BlobId, Clock, Database, MIGRATIONS, MigrationSet, StoragePaths};

/// Deterministic clock so the blob metadata timestamps are reproducible.
struct FixedClock;

impl Clock for FixedClock {
    fn now_unix_ms(&self) -> i64 {
        1_700_000_000_000
    }
}

/// Temporary data root removed on drop.
struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    fn new(label: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos());
        let path = std::env::temp_dir().join(format!(
            "assistant-capture-acceptance-{label}-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("create temp data root");
        Self { path }
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// Assembly-shaped database handle used by the blob sink.
type Handle = Arc<Mutex<Database>>;

/// Opens the main database of one data root (storage + audit migrations).
fn open_handle(directory: &TestDirectory) -> Handle {
    let mut migrations = MigrationSet::new();
    migrations
        .register_all(MIGRATIONS)
        .expect("storage migrations register");
    migrations
        .register_all(assistant_audit::MIGRATIONS)
        .expect("audit migrations register");
    let paths = StoragePaths::new(&directory.path);
    let database =
        Database::open(&paths, Arc::new(FixedClock), &migrations).expect("open database");
    Arc::new(Mutex::new(database))
}

#[cfg(windows)]
#[tokio::test]
#[ignore = "TASK-246: needs an interactive Windows desktop with at least one titled window"]
async fn test_real_window_capture_lands_in_the_blob_store() -> Result<(), Box<dyn std::error::Error>>
{
    use assistant_platform_api::{CaptureOptions, WindowFilter, WindowProvider};
    use assistant_platform_windows::WindowsPlatform;

    let directory = TestDirectory::new("real-window");
    let handle = open_handle(&directory);

    // ADR-0076 选项 ①：平台层拿到的是注入的写入端；binary 侧用 storage 落盘。
    let blob_sink = StorageBlobSink::new();
    let sink_handle = blob_sink.clone();
    assert!(
        sink_handle.attach(&handle),
        "the blob sink accepts exactly one database handle"
    );
    let platform = WindowsPlatform::new().with_blob_sink(blob_sink.leak());

    // 真机前提：桌面上至少有一个带标题的可见顶层窗口；没有就**显式失败**，不假装通过。
    let windows = platform
        .list_windows(&WindowFilter::default())
        .await
        .map_err(|error| error.to_string())?;
    let target = windows
        .iter()
        .find(|info| !info.title().trim().is_empty())
        .ok_or("no visible titled top-level window on this desktop")?;

    let image = platform
        .capture(target.window(), &CaptureOptions::new(true))
        .await
        .map_err(|error| error.to_string())?;

    // 地址 = 64 位小写 hex 的内容寻址 id。
    let blob_id = BlobId::parse(image.blob_id())?;
    assert_eq!(
        blob_id.as_str(),
        image.blob_id(),
        "blob id must round-trip through BlobId::parse"
    );

    // 字节真的在池里；读路径会重算 sha256（内容与地址不符会报 evidence_corrupt）。
    // 数据库锁只包住这次读取：读完立刻释放，别让 guard 留到断言之后（clippy 的 early-drop 判据）。
    let bytes = {
        let database = handle.lock().map_err(|_| "database mutex poisoned")?;
        database.blob_store().get(database.connection(), &blob_id)?
    };
    assert_eq!(
        u64::from(image.width()) * u64::from(image.height()) * 4,
        bytes.len() as u64,
        "stored bytes must be exactly width*height*4 BGRA"
    );
    assert_eq!(
        BlobId::of_content(&bytes).as_str(),
        image.blob_id(),
        "recomputed content hash must equal the returned content address"
    );

    println!(
        "capture acceptance: window={:?} blob_id={} {}x{} bytes={}",
        target.title(),
        image.blob_id(),
        image.width(),
        image.height(),
        bytes.len()
    );
    Ok(())
}
