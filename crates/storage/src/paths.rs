//! 数据目录布局（FS 注入点）。
//!
//! 为什么不抽 FS trait：AGENTS.md §4 漂移触发器 ⑨（想加新抽象层）要求先证明收益。
//! 存储层对文件系统的**决策**只有"根目录在哪"这一件 —— 把它做成值类型 [`StoragePaths`]
//! 就够了：测试传临时目录，生产传平台数据目录。文件内容本身不是决策逻辑，为它再造一层
//! trait 只会增加间接层而没有可回放性收益。若将来出现"必须模拟磁盘满/权限错"的测试需求，
//! 再按 DRIFT 升级。
//!
//! 目录布局（`docs/storage-design.md` §3）：
//! ```text
//! <root>/assistant.db          L1 SQLite 主库（WAL 下还会有 -wal / -shm）
//! <root>/blobs/<前2位>/<sha256>  L2 内容寻址 blob 池（W4 树快照 / 截图）
//! <root>/shadow/<task_id>/      W5 影子副本（本卡只建目录，写入策略归后续卡）
//! ```

use std::path::{Path, PathBuf};

use crate::error::{StorageError, StorageResult};

/// 数据目录布局。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoragePaths {
    root: PathBuf,
}

impl StoragePaths {
    /// 以 `root` 为数据根目录构造布局描述（**不**创建目录，创建见 [`StoragePaths::ensure_layout`]）。
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// 数据根目录。
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// 主库文件路径。
    #[must_use]
    pub fn database_file(&self) -> PathBuf {
        self.root.join("assistant.db")
    }

    /// blob 池根目录。
    #[must_use]
    pub fn blob_root(&self) -> PathBuf {
        self.root.join("blobs")
    }

    /// 影子副本根目录（W5）。
    #[must_use]
    pub fn shadow_root(&self) -> PathBuf {
        self.root.join("shadow")
    }

    /// 创建数据目录骨架（幂等）。
    ///
    /// # Errors
    /// 目录创建失败（权限 / 磁盘满 / 路径被占用）时返回 [`StorageError::Io`]。
    pub fn ensure_layout(&self) -> StorageResult<()> {
        for dir in [self.root.clone(), self.blob_root(), self.shadow_root()] {
            std::fs::create_dir_all(&dir)
                .map_err(|source| StorageError::Io { path: dir, source })?;
        }
        Ok(())
    }
}
