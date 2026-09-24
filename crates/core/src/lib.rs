//! # assistant-core crate（TASK-201 骨架）
//!
//! 阶段 1A1 的**编排层**落点。本文件目前**只有文档**：不含任何公开类型、函数或常量。
//!
//! ## 职责（骨架声明；实际能力归 TASK-020~028）
//!
//! 会话管理与上下文管理（树裁剪 / 压缩 / 预算）、Planner、Memory（App Map 加载 + FTS5 检索）、装配。
//!
//! ## 边界（不做什么）
//!
//! - **不调用任何平台 API**（铁律 7）：平台能力只经 `crates/platform/api` 的 trait
//! - 不直接持有 SQLite 连接：持久化经 `assistant-storage`（TASK-012）
//! - 不含 UI、不含 Adapter、不含审计 hash chain（TASK-013）、不含密钥访问（TASK-014）
//! - 骨架期**不发明接口**：公共 API 的形状属接口设计，归 TASK-020~028（本卡刻意不写 `pub` 项）
//!
//! ## 不变量
//!
//! 1. 骨架期**零第三方依赖**；新增依赖必须先登记 `docs/DEPENDENCIES.md`（漂移触发器 ①）
//! 2. `unsafe` 永久禁止（`#![deny(unsafe_code)]` + workspace `[lints]`）
//! 3. 依赖方向单向：`core → {protocol, storage, platform/api 的 trait}`；
//!    **禁止** `core → platform/windows`（分层断言归 TASK-015 的 `crates/core/tests/arch*`）
//! 4. 时钟 / 随机 / UUID / FS / 网络一律 trait 注入（AGENTS.md §5.3），以便回放
//!
//! ## 已知限制
//!
//! - 本 crate 目前**只有文档**：`cargo test -p assistant-core arch::` 的语义是"0 个测试通过"，
//!   而**不是**"分层规则已生效"。真正的 arch 断言归 **TASK-015**（它拥有 `crates/core/tests/arch*`）。
//!   本卡只把"包不存在"这个硬阻塞消掉（`docs/PARKING_LOT.md` PL-037）。
//!
//! ## 相关文档
//!
//! `cross-platform-ai-assistant-architecture-v2.md` §3（架构）/§5（工具）/§13（平台）、
//! `plans/stage-1-pilots.md`、`tasks/TASK-201-core-crate-skeleton.md`、`docs/PARKING_LOT.md` PL-037。

#![deny(unsafe_code)]
