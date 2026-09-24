//! 密钥存取接口（trait）与其语义契约。
//!
//! 边界：本模块**只**声明接口；具体后端在 `memory`（fake）与 `keyring_store`（真后端）。
//!
//! 不变量：
//!   1. **「没有这条」与「读失败」必须可区分**：`get` 用 `Ok(None)` 表达前者、`Err` 表达后者（铁律 1）
//!   2. 每个写操作都有 postcondition：`set` 后 `get` 能读回同一值；`delete` 后 `contains` 为 false（铁律 4）
//!   3. 实现**不得**把明文写进日志 / 错误文本 / 磁盘（架构 v2 §12.5）
//!
//! 相关：架构 v2 §12.5、`docs/spec/error-codes.md`

use crate::error::SecretResult;
use crate::{SecretName, SecretValue};

/// 密钥存取接口。
///
/// 为什么是**同步**接口：OS 密钥库调用（DPAPI / Keychain / Secret Service）本身是阻塞的短操作；
/// 引入 async 会把运行时契约压给每一个实现（`keyring` 4.x 的 `v1` 接口也是同步的）。
///
/// 为什么 `Send + Sync`：实现要能被注入进 Host 的装配图并被多线程共享。
pub trait SecretStore: Send + Sync {
    /// 读取。
    ///
    /// 语义：**没有这条密钥 → `Ok(None)`**；读取本身失败 → `Err(..)`。两者不可合并。
    ///
    /// # Errors
    /// 后端不可用 / 拒绝访问 / 条目损坏 → [`crate::SecretError`] 的对应变体。
    fn get(&self, name: &SecretName) -> SecretResult<Option<SecretValue>>;

    /// 写入（覆盖语义，**幂等**）。
    ///
    /// postcondition：`get(name)` 能读回同一值。
    ///
    /// # Errors
    /// 后端不可用 / 拒绝访问 / 值超长（由 [`SecretValue`] 构造期拦掉）→ 对应变体。
    fn set(&self, name: &SecretName, value: &SecretValue) -> SecretResult<()>;

    /// 删除。
    ///
    /// 语义：返回**是否真的删掉了一条** —— 原本就不存在时返回 `Ok(false)`，那是正常结果不是错误。
    /// postcondition：`contains(name)` 为 false。
    ///
    /// # Errors
    /// 后端不可用 / 拒绝访问 → 对应变体。
    fn delete(&self, name: &SecretName) -> SecretResult<bool>;

    /// 是否存在。
    ///
    /// 默认实现走 `get`（实现可在后端提供更省的探测原语时覆写）。
    ///
    /// # Errors
    /// 同 [`SecretStore::get`]。
    fn contains(&self, name: &SecretName) -> SecretResult<bool> {
        Ok(self.get(name)?.is_some())
    }
}
