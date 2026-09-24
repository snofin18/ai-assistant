//! # assistant-secrets crate（TASK-014）
//!
//! 阶段 1A1 基础设施：**OS 密钥库封装**。把模型 API Key / 主程序凭据 / MCP 令牌只存进
//! Windows 凭据管理器 / macOS Keychain / Linux Secret Service，并保证明文**永不**落
//! SQLite / 日志 / prompt / 崩溃报告 / 审计事件（架构 v2 §12.5）。
//!
//! ## 职责
//!
//! - [`SecretName`]：密钥名（架构 v2 L2624 的 `key_ref` 取值空间），构造即校验
//! - [`SecretValue`]：明文材料的唯一容器，`Zeroizing` + 脱敏 `Debug` + 无 `Serialize` / `Clone`
//! - [`SecretStore`]：存取接口；**「没有这条」= `Ok(None)`，与「读失败」= `Err` 严格区分**（铁律 1）
//! - [`KeyringSecretStore`]：`keyring` 4.2 真实后端；[`InMemorySecretStore`]：测试用 fake
//! - [`SecretAccessAuditor`] + [`SecretAccessRecord`]：访问审计的注入点与记录形状
//! - [`AuditedSecretStore`]：装饰器 —— 每次访问都记一条；**审计写不进去就拒绝该次访问**（fail-closed）
//!
//! ## 边界（不做什么）
//!
//! - **不做策略判定**：能不能读某个密钥由 policy 决定（铁律 3 / 6），本 crate 只执行与记录
//! - **不做持久化**：本 crate 不碰 SQLite、不写任何文件（明文不落盘是它的存在理由）
//! - **不定义协议事件类型**：`SecretAccessRecord` 是本 crate 内部类型；映射成
//!   `assistant_protocol::AuditEvent` 需要新增 `event_type` 取值（封闭枚举，改 schema）
//!   → 归后续卡（任务卡 §5 的 `DRIFT-014-1`）
//! - **不做轮换 / 过期 / 配额 / BYOK 装配**，也不做 `EgressProxy` 的密钥注入（架构 v2 §12.5 后半段）→ 后续卡
//! - **不缓存明文**：缓存会制造出 `zeroize` 管不到的副本
//!
//! ## 不变量
//!
//! 1. **明文只有一个出口**：[`SecretValue::expose`]；`rg expose` 即可列出全部调用点
//! 2. **明文不进日志**：`SecretValue` 无 `Display`、`Debug` 固定输出 `<redacted> N bytes`；
//!    [`InMemorySecretStore`] 的 `Debug` 只报条目数
//! 3. **明文不进审计**：`SecretAccessRecord` 的字段只有名字 / 操作 / 结果 / 时间戳
//! 4. **用后清零**：`SecretValue` 内部是 `Zeroizing<String>`，实现 `Zeroize`，`Drop` 时清零
//! 5. **fail-closed**：审计写不进去 ⇒ 该次访问返回 `Err(AuditRejected)`，读到的明文不外泄
//! 6. **时钟注入**：审计时间戳取自 [`assistant_storage::Clock`]（AGENTS.md §5.3），测试可回放
//!
//! ## 典型用法
//!
//! ```no_run
//! use std::sync::Arc;
//!
//! use assistant_secrets::{
//!     AuditedSecretStore, KeyringSecretStore, SecretName, SecretStore, SecretValue,
//! };
//! use assistant_storage::SystemClock;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // 装配：真后端 + 审计出口（实现方负责落库）+ 注入时钟
//! let backend = KeyringSecretStore::new("ai-assistant")?;
//! let store = AuditedSecretStore::new(backend, MyAuditor, Arc::new(SystemClock));
//!
//! let name = SecretName::new("model.openai.api_key")?;
//! let value = SecretValue::new("sk-...")?;
//!
//! // 写 → 读；每次访问都会产生一条审计记录
//! store.set(&name, &value)?;
//! let read_back = store.get(&name)?;
//! assert_eq!(read_back.map(|v| v.len()), Some(5));
//! # Ok(())
//! # }
//! # struct MyAuditor;
//! # impl assistant_secrets::SecretAccessAuditor for MyAuditor {
//! #     fn record_access(
//! #         &self,
//! #         record: &assistant_secrets::SecretAccessRecord,
//! #     ) -> assistant_secrets::SecretResult<()> {
//! #         // 装配点：翻译成协议事件并落进 assistant-audit
//! #         let _ = record;
//! #         Ok(())
//! #     }
//! # }
//! ```
//!
//! ## 相关 spec / 文档
//!
//! 架构 v2 **§12.5**（密钥与凭据管理：`keyring` / `zeroize` / 访问审计 / 五条禁令）与
//! **L2624**（DB 只存 `key_ref`）、`docs/spec/error-codes.md`、`docs/DEPENDENCIES.md`、
//! `crates/secrets/README.md`、`tasks/TASK-014-secrets-os-keychain-wrapper.md`（本卡正文 + 执行记录）。

#![deny(unsafe_code)]

mod access_audit;
mod audited;
mod error;
mod keyring_store;
mod memory;
mod secret_name;
mod secret_value;
mod store;

pub use access_audit::{
    SecretAccessAuditor, SecretAccessOperation, SecretAccessOutcome, SecretAccessRecord,
};
pub use audited::AuditedSecretStore;
pub use error::{SecretError, SecretResult};
pub use keyring_store::{KeyringSecretStore, MAX_SERVICE_NAME_LEN};
pub use memory::InMemorySecretStore;
pub use secret_name::{MAX_SECRET_NAME_LEN, SecretName};
pub use secret_value::{MAX_SECRET_VALUE_LEN, SecretValue};
pub use store::SecretStore;
