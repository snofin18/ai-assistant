# assistant-secrets crate (TASK-014)

> 阶段 1A1 基础设施：**OS 密钥库封装**。明文只进 Windows 凭据管理器 / macOS Keychain / Linux Secret Service；**永不**进 SQLite / 日志 / prompt / 崩溃报告 / 审计事件（架构 v2 §12.5）。

## 职责

- **`SecretName`**：密钥名（架构 v2 L2624 的 `key_ref` 取值空间）。构造即校验：非空 / ≤ 128 字节 / 只允许 `[A-Za-z0-9._-]`。名字**不是秘密** —— 它进日志与审计记录
- **`SecretValue`**：明文材料的**唯一**容器。内部 `zeroize::Zeroizing<String>`；无 `Display` / `Serialize` / `Clone`；`Debug` 输出 `SecretValue(<redacted> N bytes)`
- **`SecretStore`**：`get` / `set` / `delete` / `contains` 的接口。**「没有这条」= `Ok(None)`，与「读失败」= `Err` 严格区分**（铁律 1）
- **`KeyringSecretStore`**：`keyring` 4.2 真实后端（Windows 凭据管理器 / macOS Keychain / Linux Secret Service）
- **`InMemorySecretStore`**：进程内 fake，供单测与开发；**不得用于生产**
- **`SecretAccessRecord` / `SecretAccessAuditor`**：访问审计的记录形状与注入点（名字 + 操作 + 结果 + 时间戳）
- **`AuditedSecretStore`**：装饰器 —— 每次访问都记一条；**审计写不进去就拒绝该次访问**（fail-closed）
- 提供**注入点**：审计出口与 `Clock` 都从外面传入（测试用固定时钟 + 记录型出口即可回放）

## 边界（不做什么）

- **不做策略判定**：能不能读某个密钥由 policy 决定（铁律 3 / 6）；本 crate 只执行与记录
- **不做持久化**：不碰 SQLite、不写任何文件 —— 明文不落盘是它存在的理由
- **不定义协议事件类型**：`SecretAccessRecord` 是本 crate 内部类型。映射成 `assistant_protocol::AuditEvent` 需要新增 `event_type` 取值（`protocol/audit-event/audit-event-1.0.json` 是**封闭枚举**，17 项里没有 `secret.*`）= 改 schema → 归后续卡（任务卡 §5 的 `DRIFT-014-1`）
- **不缓存明文**：缓存会制造出 `zeroize` 管不到的副本
- **不做**轮换 / 过期 / 配额 / BYOK 与企业托管装配；**不做** EgressProxy 的密钥注入（架构 v2 §12.5 后半段）→ 后续卡
- **不碰** `crates/core` / `policy` / `task-engine`（铁律 7：core 只经 trait）

## 不变量

1. **明文只有一个出口**：`SecretValue::expose()`。`rg expose` 即可列出全部调用点 —— 这是设计出来的可审计性
2. **明文不进日志**：`SecretValue` 无 `Display`、`Debug` 固定脱敏；`InMemorySecretStore` 的 `Debug` 只报条目数、不列键名
3. **明文不进审计**：`SecretAccessRecord` 的字段只有名字 / 操作 / 结果 / 时间戳 —— 类型层保证
4. **用后清零**：`SecretValue` 内部是 `Zeroizing<String>` 并实现 `Zeroize`，`Drop` 时清零
5. **fail-closed**：审计写不进去 ⇒ 该次访问返回 `Err(AuditRejected)`；读到的明文随之被丢弃并清零
6. **时钟注入**：审计时间戳取自 `assistant_storage::Clock`（AGENTS.md §5.3），测试可回放
7. **无 `unsafe`**：`#![deny(unsafe_code)]` + workspace `[lints]`；平台差异全部封在 `keyring` 里

## 典型用法

```rust
use std::sync::Arc;

use assistant_secrets::{
    AuditedSecretStore, KeyringSecretStore, SecretAccessRecord, SecretAccessAuditor, SecretName,
    SecretResult, SecretStore, SecretValue,
};
use assistant_storage::SystemClock;

// 装配：真后端 + 审计出口（实现方负责落库）+ 注入时钟
let backend = KeyringSecretStore::new("ai-assistant")?;
let store = AuditedSecretStore::new(backend, MyAuditor, Arc::new(SystemClock));

let name = SecretName::new("model.openai.api_key")?;
store.set(&name, &SecretValue::new("sk-...")?)?;
let read_back = store.get(&name)?;

// 审计出口：翻译成协议事件并落进 assistant-audit（归后续卡）
struct MyAuditor;
impl SecretAccessAuditor for MyAuditor {
    fn record_access(&self, record: &SecretAccessRecord) -> SecretResult<()> {
        let _ = record;
        Ok(())
    }
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

## 已知限制

- **CI 上没有任何真实 OS 密钥库**：ubuntu / macos runner 无 Secret Service / 无登录 Keychain，windows runner 的凭据管理器亦不可靠 → **单测一律走 `InMemorySecretStore`**。`KeyringSecretStore` 在 CI 上只做「能构造 + 能编译」；真实读写只做**真机手工验收**：
  1. Windows：`cmdkey /list` 能在写入后看到 `ai-assistant:<name>`；`delete` 后消失
  2. macOS：`security find-generic-password -s ai-assistant -a <name>` 能查到
  3. Linux：`secret-tool lookup service ai-assistant username <name>` 能查到（需 Secret Service 在跑）
- **`zeroize` 只保护本 crate 持有的那份内存**：OS 密钥库、`keyring` 内部缓冲、调用方 `expose()` 之后自己拷出去的那份**不受本 crate 保护**。不要把它读成"端到端零残留"
- **无 `unsafe` ⇒ 无法运行期观测清零**：测试只能做编译期断言（`SecretValue: Zeroize`）+ 显式 `zeroize()` 后为空；"内存里真的被覆盖"依赖 `zeroize` crate 的 volatile 写
- **写 / 删在审计失败时可能已经生效**：本 crate 不假装回滚。调用方看到的语义是"这次操作未被完整记录，故不视为成功"；`set` 幂等、`delete` 重试返回 `Ok(false)`，故重试安全
- **`contains` 在真实后端上会读一次明文**：`keyring` 没有"仅探测存在性"的原语；读回的内容随即清零
- **审计记录尚未落库**：`SecretAccessAuditor` 只有注入点，没有实现（协议 `event_type` 里没有 `secret.*`）→ **`DRIFT-014-1` / PL-049**
- **错误映射用 `ErrorCategory`，不是 `thiserror`**：与 `crates/storage` / `crates/audit` 既有实现保持一致（引入 `thiserror` = 新增依赖 = 漂移触发器 ①）→ 统一化见 PL-050

## 相关文档

- 架构 v2 **§12.5**（密钥与凭据管理：`keyring` / `zeroize` / 访问审计 / 五条禁令）、**L2624**（DB 只存 `key_ref`）、**§15 目录树**（`crates/secrets/`）
- `docs/spec/error-codes.md`（13 类 `ErrorCategory`）、`docs/spec/naming.md` §5（newtype id）
- `docs/DEPENDENCIES.md`（`keyring` 4.2 / `zeroize` 1.9 两行已登记）
- `tasks/TASK-014-secrets-os-keychain-wrapper.md`（本卡正文 + 执行记录）
