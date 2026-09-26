//! # assistant-core crate（TASK-201 骨架 / TASK-028 拆卡）
//!
//! 阶段 1A1 的**编排层**落点。本文件目前**只有文档**：不含任何公开类型、函数或常量。
//! 契约见 `docs/spec/core-orchestration.md`；分层决策见 `docs/adr/0053-core-orchestration-layer-interface.md`。
//!
//! ## 职责（骨架声明；实际能力归 TASK-028 / 207 / 208）
//!
//! 会话管理与上下文管理（树裁剪 / 压缩 / 预算，TASK-028）、Planner（TASK-207）、
//! Memory（App Map 加载 + 消费 `assistant-storage` 的检索 API，TASK-208）。
//! **不装配**：装配点唯一在 binary 层（`apps/agent-core`，TASK-029）。
//!
//! ## 边界（不做什么）
//!
//! - **不调用任何平台 API**（铁律 7）：平台能力只经 `crates/platform/api` 的 trait
//! - 不直接持有 SQLite 连接：持久化经 `assistant-storage`（TASK-012）
//! - **不判定权限**（铁律 3）：工具是否放行归 `crates/policy`
//! - **不装配**：不 new 出 `tool-bus` / `policy` / `audit` / `hitl` / `verify` / `undo` / `lease` 的实现
//! - 不含 UI、不含 Adapter、不含审计 hash chain（TASK-013）、不含密钥访问（TASK-014）
//! - **不重定义**别家的类型：`Plan` / `PlanStep` 等归 `assistant-task-engine`；
//!   跨进程与持久化结构归 `assistant-protocol` / `assistant-storage`
//!
//! ## 不变量
//!
//! 1. 骨架期**零第三方依赖**；新增依赖必须先登记 `docs/DEPENDENCIES.md`（漂移触发器 ①）
//! 2. `unsafe` 永久禁止（`#![deny(unsafe_code)]` + workspace `[lints]`）
//! 3. **依赖白名单（ADR-0053 D2）**：`core` 只许依赖 `protocol` / `storage` /
//!    `platform/api`（仅 trait）/ `task-engine`（Plan/Step DAG 类型）/ `model-gateway`；
//!    **黑名单（ADR-0053 D3）**：`platform/{windows,macos,linux}`、`tool-bus`、`policy`、`audit`、
//!    `hitl`、`verify`、`undo`、`lease`、`secrets`、`ipc`、`apps/*`、UI —— 一律禁止
//! 4. 时钟 / 随机 / UUID / FS / 网络一律 trait 注入（AGENTS.md §5.3），以便回放
//! 5. **可装配**：每个组件都可由 binary 构造 + 注入；不依赖全局单例
//! 6. **无静默失败**（铁律 1）：裁剪 / 压缩 / 检索的每一次「少给了东西」都必须在返回值里显式标注
//!
//! ## 已知限制
//!
//! - 本 crate 目前**只有文档**：`cargo test -p assistant-core arch::` 的语义是"0 个测试通过"，
//!   而**不是**"分层规则已生效"。真正的 arch 断言归 **TASK-015**（它拥有 `crates/core/tests/arch*`）。
//!   本卡只把"包不存在"这个硬阻塞消掉（`docs/PARKING_LOT.md` PL-037）。
//! - 原 TASK-028（五合一）已于 2026-09-26 **拆卡**：会话 + 上下文 → TASK-028；Planner → TASK-207；
//!   Memory → TASK-208；「组装」→ TASK-029（ADR-0053）。
//!
//! ## 相关文档
//!
//! `cross-platform-ai-assistant-architecture-v2.md` §3 / §5 / §7 / §11 / §13、
//! `docs/spec/core-orchestration.md`、`docs/adr/0053-core-orchestration-layer-interface.md`、
//! `plans/stage-1-pilots.md`、`tasks/TASK-201-core-crate-skeleton.md`、
//! `tasks/TASK-028-core-session-context.md`、`tasks/TASK-207-core-planner-plan-step-dag.md`、
//! `tasks/TASK-208-core-memory-app-map-fts-retrieval.md`、`docs/PARKING_LOT.md` PL-037。

#![deny(unsafe_code)]
