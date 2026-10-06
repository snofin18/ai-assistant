//! 截图 blob 写入端（ADR-0076）：把平台层产出的 BGRA 写进 `crates/storage` 的内容寻址 blob 池。
//!
//! 职责：
//! - 实现 `assistant_platform_api::ImageBlobSink`，把「内容地址 + 像素」落盘为 `BlobKind::Screenshot`；
//! - 持有装配根注入的数据库句柄，写入时复用同一个 `Database`（不私开第二条连接）。
//!
//! 边界：
//! - 不截图、不做像素遮挡、不做策略判定 —— 那些在平台层 / 策略引擎；
//! - 平台层只经 trait 调用本实现，因此平台层不依赖 `crates/storage`（铁律 7）。
//!
//! 不变量：
//! 1. 缓冲长度必须恰好等于 `width * height * 4`，否则显式失败；
//! 2. 本实现**不得改写内容地址**：落盘后 `BlobId` 必须与平台层算出的地址逐字相同；
//! 3. `attach` 之前的写入显式失败，绝不静默丢弃截图。
//!
//! 相关：ADR-0076、ADR-0071、ADR-0073、`crates/storage/src/content.rs`。

use std::sync::{Arc, Mutex, OnceLock, Weak};

use assistant_platform_api::{ErrorCode, ImageBlobSink, PlatformError, PlatformResult};
use assistant_storage::{BlobKind, Database};

use crate::adapters::DatabaseHandle;

/// 进程级截图 blob 写入端。
///
/// 由装配根在启动时构造、注入 `WindowsPlatform`，并在 `assemble()` 返回后 `attach` 数据库句柄。
/// 句柄用 `OnceLock` 而非普通 `Mutex`：注入是一次性动作，读取路径因此不需要加锁，也就不会中毒。
#[derive(Debug, Clone, Default)]
pub struct StorageBlobSink {
    /// 只持**弱**引用：写入端被 `leak()` 成进程级单例，若持强引用就会让数据库文件在
    /// `Host` shutdown（及其后的临时目录清理）时仍被占用（实测 os error 32）。
    database: Arc<OnceLock<Weak<Mutex<Database>>>>,
}

impl StorageBlobSink {
    /// 构造一个尚未 `attach` 数据库句柄的写入端。
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// 注入装配根持有的数据库句柄。
    ///
    /// 返回 `false` 表示**此前已经注入过**（接线错误，调用方应显式失败）；已装的句柄不被替换。
    #[must_use]
    pub fn attach(&self, database: &DatabaseHandle) -> bool {
        self.database.set(Arc::downgrade(database)).is_ok()
    }

    /// 把写入端泄漏成 `&'static` 引用（`WindowsPlatform` 需要它才能继续 `Copy`）。
    ///
    /// 进程生命周期内只发生一次，容量有界（ADR-0063）。
    #[must_use]
    pub fn leak(self) -> &'static dyn ImageBlobSink {
        Box::leak(Box::new(self))
    }
}

impl ImageBlobSink for StorageBlobSink {
    fn store_bgra(
        &self,
        width: u32,
        height: u32,
        content_address: &str,
        bgra: &[u8],
    ) -> PlatformResult<()> {
        let expected = u64::from(width) * u64::from(height) * 4;
        if expected != bgra.len() as u64 {
            return Err(sink_error(format!(
                "blob sink: BGRA buffer is {} bytes, but {width}x{height} needs {expected}",
                bgra.len()
            )));
        }
        let database = self.database.get().and_then(Weak::upgrade);
        let Some(database) = database else {
            return Err(sink_error(
                "blob sink: no database handle has been attached yet, or the assembly-owned \
                 database is already gone"
                    .to_owned(),
            ));
        };
        // 互斥锁只包住这一次落盘：地址比对与错误构造都在释放锁之后（clippy::significant_drop_tightening）。
        let blob_id = {
            let store = database.lock().map_err(|_| {
                sink_error("blob sink: the assembly-owned database mutex is poisoned".to_owned())
            })?;
            store
                .blob_store()
                .put(store.connection(), BlobKind::Screenshot, bgra)
                .map_err(|error| {
                    PlatformError::new(
                        error.error_category(),
                        format!("blob sink: {} ({error})", error.reason_code()),
                    )
                })?
        };
        // 内容寻址下地址就是同一性判据：写进去的东西必须就是平台层算出的那一帧。
        if blob_id.as_str() != content_address {
            return Err(sink_error(format!(
                "blob sink: stored address `{}` does not match the platform-computed `{content_address}`",
                blob_id.as_str()
            )));
        }
        Ok(())
    }
}

/// 装配 / 内部一致性类失败一律 `Fatal`（与 ADR-0076 D6 的「未注入 → 显式 Fatal」一致）。
fn sink_error(detail: String) -> PlatformError {
    PlatformError::new(ErrorCode::Fatal, detail)
}
