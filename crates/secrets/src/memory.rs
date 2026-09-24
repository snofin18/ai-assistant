//! 进程内密钥库（**仅供测试与开发**）。
//!
//! 边界：**不是生产实现** —— 明文只存在进程内存里，进程一退就没了，且不跨进程隔离。
//! 存在的理由是让单测与开发环境不必依赖任何真实 OS 密钥库（CI 上没有）。
//!
//! 不变量：
//!   1. 与 [`crate::KeyringSecretStore`] 遵守**同一份** [`SecretStore`] 契约（同一组测试跑两边）
//!   2. `Debug` 只报条目数，**不报内容**
//!   3. 锁被毒化（有线程持锁 panic）时返回错误，**不** `unwrap`（铁律 1）
//!
//! 相关：架构 v2 §12.5、`crates/secrets/README.md`「已知限制」

use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Mutex, MutexGuard};

use zeroize::Zeroizing;

use crate::error::{SecretError, SecretResult};
use crate::{SecretName, SecretStore, SecretValue};

/// 进程内 fake 后端。
///
/// **不得用于生产**：它不做任何平台保护，任何能读到本进程内存的东西都能读到密钥。
#[derive(Default)]
pub struct InMemorySecretStore {
    entries: Mutex<BTreeMap<String, Zeroizing<String>>>,
}

impl InMemorySecretStore {
    /// 建一个空的内存密钥库。
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: Mutex::new(BTreeMap::new()),
        }
    }

    /// 当前条目数（用于断言，不含内容）。
    ///
    /// # Errors
    /// 锁被毒化 → [`SecretError::BackendFailure`]。
    pub fn len(&self) -> SecretResult<usize> {
        Ok(self.lock()?.len())
    }

    /// 是否为空。
    ///
    /// # Errors
    /// 锁被毒化 → [`SecretError::BackendFailure`]。
    pub fn is_empty(&self) -> SecretResult<bool> {
        Ok(self.lock()?.is_empty())
    }

    fn lock(&self) -> SecretResult<MutexGuard<'_, BTreeMap<String, Zeroizing<String>>>> {
        self.entries
            .lock()
            .map_err(|_| SecretError::BackendFailure {
                detail: "内存密钥库的锁被毒化（有线程持锁时 panic 了）".to_string(),
            })
    }
}

impl fmt::Debug for InMemorySecretStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // 只报条目数：`Debug` 会被打进日志，绝不能出现内容
        let count = self.entries.lock().map(|guard| guard.len()).ok();
        write!(f, "InMemorySecretStore(<redacted> entries={count:?})")
    }
}

impl SecretStore for InMemorySecretStore {
    fn get(&self, name: &SecretName) -> SecretResult<Option<SecretValue>> {
        // 锁只在"把值搬出来"这一段持有；`SecretValue` 的构造放在锁外，避免持锁做多余工作。
        // 搬出来的明文装进 `Zeroizing`，所以这次中转副本同样受清零保护。
        let stored = self
            .lock()?
            .get(name.as_str())
            .map(|value| Zeroizing::new(value.as_str().to_string()));
        let Some(stored) = stored else {
            return Ok(None);
        };
        // 值在 `set` 时已过长度校验；这里仍走同一条构造路径 —— 若上限后来被调小，
        // 读回旧值会**显式**报 `CorruptEntry`，而不是被静默截断。
        match SecretValue::new(stored.as_str()) {
            Ok(value) => Ok(Some(value)),
            Err(error) => Err(SecretError::CorruptEntry {
                detail: format!("内存库中的值不满足当前上限：{error}"),
            }),
        }
    }

    fn set(&self, name: &SecretName, value: &SecretValue) -> SecretResult<()> {
        self.lock()?.insert(
            name.as_str().to_string(),
            Zeroizing::new(value.expose().to_string()),
        );
        Ok(())
    }

    fn delete(&self, name: &SecretName) -> SecretResult<bool> {
        let mut guard = self.lock()?;
        Ok(guard.remove(name.as_str()).is_some())
    }
}
