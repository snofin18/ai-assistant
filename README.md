# 跨平台本地 AI 助理（Cross-Platform Local AI Assistant）

让大模型**受控地**操作你电脑上已有的常用应用（记事本、画图、Edge/Chrome、Excel、
Photoshop…）：模型负责理解与规划，所有动作都通过**注册的工具**执行，
全过程可审计、可撤销、可回放。**默认拒绝**，不可逆动作必须人工确认。

> 状态：**TASK-244 Done（PL-022 机器派生计数收口；ADR-0075；CI 11/11 SUCCESS）**；**TASK-041 Done（拆分 A；CI 11/11 SUCCESS）**；**TASK-241 Done（ADR-0072 派生值指针化 + PL-035 闭环；PL-022 机器派生余项已由 TASK-244 闭环）**；**阶段 1（三试点闭环：Notepad → Paint → Edge/Chrome）** —— **TASK-236 Done：`xtask replay --suite core` 已支持 Recording v2 真实 UIA 树快照序列与 expected-vs-actual 树级 diff，`--list-deferred` replay 未实现项清零**；**TASK-235 Done：ADR-0070 将 `RoleAndParent` 父候选限定为子树作用域，父命中但子缺失显式 `TargetNotFound`，`PL-094` 闭环**；**TASK-234 Done（PR #218 / merge `e417a2a`）：ADR-0068 / 0069 落地，`xtask hygiene` 13/13、0E/95W，`PL-060` 闭环**；阶段 0 已于 2026-09-20 closeout；阶段 1a 补救卡 **TASK-102 / 103 / 104 / 105 / 210 / 212 / 087 / 211 / 213 / 214 / 216 / 217 / 218 / 219 均 Done**（TASK-087：fmt/clippy/build 负向 canary + `check-comments` CI 硬门禁 + gate-selftest run `36598959358` 全绿，**PL-018 关闭**；TASK-211：阶段 1a 复验清单与证据矩阵，关闭 **PL-056 / PL-058**）。**ADR-0057 / ADR-0058 / ADR-0061 已 Accepted**；**TASK-213** 完成 UI↔Core 真管道与生产事件源，**PL-095 闭环**；**TASK-214** 完成生产装配根、确定性 task-package Plan 来源、5 个 Notepad handler、真 UIA T1.1 干跑、真 UI 管道事件与 fail-closed 收口，**PL-096 闭环**；**TASK-205** 完成 `tool-bus` schema 校验模块拆分，`hygiene` 长文件 warning 4→3；**TASK-216** 运行时补齐 A/B 片由 TASK-217 落地并完成收口；**TASK-217** 完成 T1.2/T1.3 fake-platform 执行、无授权暂停与 UI 批准恢复，`DRIFT-216-4` 闭环；**TASK-085 / TASK-086** 完成 Rust 源码结构、末行换行、CRLF 与 Cargo 依赖登记 hygiene 规则，覆盖推进到 11/13。**TASK-039** 的独立复验硬门禁全绿，**TASK-105** 已取得 T1.1~T1.3 各 10 次真实运行证据，阶段 1a 翻转为 **GO**；**TASK-223 Done** —— ADR-0064 的 `assistant.runtime.host_read_utf8_prefix` 文件通道 + ADR-0065 的 `assistant.runtime.host_capture_initial_fingerprint` 前置初始指纹步骤，T1.1 大文件 Plan 在 fake 平台与真机 UIA 各取证跑通，`DRIFT-223-1` / `PL-100` 闭环；**TASK-220 Done** —— 四类资源泄露防护（UIA 元素表 4096 / anchor 64 / grant 1024 上限与淘汰、`taskkill /T /F` + `wait()` reap、可调用 `clear_thread_elements()`）真机 ignored 全套复跑 **8 passed / 0 failed**（含 6 轮收敛测量与三个对照），`DRIFT-220-1` 闭环；**TASK-225 Done** —— 合成输入目标租约独占已接入 Host 装配层，两任务冲突稳定 `Transient`、释放后重试、失败回基线与 pointer-shaped 专项 5 passed，`PL-101` 闭环；**TASK-226 Done** —— PL-104 / PL-105 闭环，ignored 真机用例以异步互斥串行化 fixture 启动，`production_root.rs` 拆分后 3 个测试文件均 <600 行且断言零放宽；**TASK-227 Done** —— TASK-040 / TASK-225 的状态行按实更正为 Done、`PL-103` 给出可直接粘贴的 spec 改法（待 Orchestrator 落笔）、`PL-106` 开卡 TASK-228（Ready、待派单）；`plans/stage-1-pilots.md` 的「当前进度」句已不再声称「真机点击校准仍需人工授权」。**TASK-229 Done** —— `PL-103` 落笔：`docs/spec/runtime-execution.md` §3 不再手写保留工具个数与名单，改为以 `RESERVED_RUNTIME_TOOLS` 为唯一事实源；`docs/memory/pitfalls.md` 落「收口类提交必须逐张比对状态行与 LEDGER」四步硬规则；轮次 A 的残留目录已清理（先清 git 打包对象只读位再删）。**TASK-224 Done** —— ADR-0066 命令行 `xtask write` 通道 + Windows `share_mode(0)` 占用探测 + 有界退避，持锁读取不阻塞、占用超时退出 5，`PL-107` 闭环。复验证据见 `docs/audits/stage-1a-reaudit-2026-10-02.md`；**TASK-231 Done** —— ADR-0067 把 pointer 起点与 `DragTo` 释放点的 `CoordinateSpace` 显式化，删除混合 DPI 收敛启发式，`PL-074` 闭环；**TASK-232 Done** —— 四轮自动化（TASK-228 / 224 / 230 / 231）审计收口：`TASK-224` 的滞后状态行按实修正、「状态行比对」规则补强（`正文只读 ≠ 状态行只读`，PR 贴比对为必做）、storage 轮遗留的「生产 `SessionStore` 适配器仍缺」转成 `PL-108`。
> **TASK-041 Done（拆分 A + B）**：ADR-0073 冻结「像素遮挡归平台层 / `NeverPersist` 不保留 `ImageRef` / 滚动清理有界」，`crates/capture` + `crates/dlp/src/redact` 零依赖纯逻辑（15 专项测试）；ADR-0076 落地 Windows GDI `PrintWindow` 单窗口截图 + UIA `IsPassword` 像素遮挡 + `ImageBlobSink` 注入（平台层算内容地址、binary `StorageBlobSink` 用 `crates/storage` 落盘并核对地址，未注入 → 显式 `Fatal`），`DRIFT-041-2` / `PL-112` 闭环；真机截图与真机 blob 落盘由 **TASK-246** 取证。
> **TASK-230 Done**：storage `0005` 新增 `conversations` + `conversation_messages` 与会话 / 消息树公开记录 API；真实关闭并重开 SQLite 后逐字段一致，缺失会话、非法父子关系和 revision 跳号显式失败，`PL-092` 闭环。
> **TASK-231 Done**：ADR-0067 将 `pointer_action` 起始点与 `DragTo` 释放点的 `CoordinateSpace` 显式化；旧混合 DPI 收敛启发式已删除，起点 / 终点分别按各自显示器 scale 换算，未知设备 / DPI 不一致 / 越界点显式失败，`PL-074` 闭环。
> **TASK-228 Done**：四个 Windows 真机 `#[ignore]` 用例改为覆盖写结构化记录（`pass|skip|fail` + 非空原因 + 测量值），`tools/acceptance-report` 严格校验并区分全 pass / 含 skip-fail / 非法记录；真机一轮 **4 pass / 0 skip / 0 fail**，`PL-106` 闭环。
> **TASK-233 Done**：生产装配接入 storage 会话持久化 —— 新增 `StorageSessionStore` 适配器与 `with_storage_session_store()` 开关，生产 `production.rs` / `main.rs` 两处不再注入 `MemorySessionStore`；适配器级跨重开逐字段一致、负向四类显式失败、装配级关库重开读回均取证，`PL-108` 闭环。
> **TASK-237 Done**：三连发自动化审计收口 —— `PL-062` 闭环（`xtask replay --suite core` 交付后与 `AGENTS.md` §6 一致，实测 EXIT 0）、孤儿自动化目录删除、`TASK-235` 状态行归一；TASK-234 / 235 / 236 独立复核全绿。
> **TASK-238 Done**：ADR-0071 冻结截图管线 / 出域脱敏的 crate 边界 —— 新增 `crates/capture`（平台无关窗口截图管线，唯一原语 = `platform/api` 的 `WindowProvider::capture`）与 `crates/dlp`（出域策略 + 脱敏 + 截图遮挡）两个零第三方依赖骨架，架构 §3 布局追加 `capture/`；`PL-102` 闭环，解锁 1b TASK-041 / 042 与 1c TASK-050。
> **TASK-239 Done**：停车位存量复核收口 —— 新增 `docs/audits/parking-lot-review-2026-10-05.md`，`docs/PARKING_LOT.md` 仅追加 48 行复核结论；补记 13 条已闭环/取代、保留 35 条待治理或人工裁决。
> **TASK-240 Done**：任务卡状态行全量清扫完成 —— 18 张台账已 Done 但卡面仍 `Ready` / `InProgress` 的卡按实更正；只改状态行，卡正文、执行记录与产品代码零改动。
> **TASK-242 Done**：三连发审计收口 —— 补正 TASK-076 的滞后状态行（LEDGER L88 早已声称 Done，commit `0f05ef7` 实际没改），并把 TASK-002 的卡面 / PLAN / LEDGER 三方不一致开成 **PL-109**（待人类裁决）。
> **TASK-245 Done**：2026-10-06 四连发审计收口 —— `LEDGER.md` 里 TASK-041 的 merge-hash 行原**重复 5 次**且插错位置，已去重为 **1 条**（404 → 400 行、完全重复行组归零）；`PL-111` 落点 = `DRIFT-041-2`（storage sink 待裁决）+ ADR-0074 号冲突 + `DRIFT-042-1` / `PL-110`。
> **TASK-042 Done**：视觉验证纯逻辑 —— ADR-0074 冻结 `visual_assert` 扁平结构化形状（`field` + `op` + 具名容差 + `confidence_min`）与 pHash / dHash 64-bit 口径（`max_hamming_distance` 上限 24，随机图对 `P(≤24)=2.997%<5%`）；`crates/verify/src/visual/**` 零依赖落地 21 个专项测试；接入既有 `Postcondition` / `Observation` 另立卡（`DRIFT-042-1` / `PL-110`）。
> **TASK-243 Done**：**PL-109 裁决收口 —— 人类 2026-10-05 定 `TASK-002 = Done`**；`PLAN.md` 阻塞项删去「TASK-002 仍 Blocked」、`MEMORY.md` §1 改写、`LEDGER.md` 记录裁决、`docs/PARKING_LOT.md` 标 PL-109 闭环；SPIKE-A banner 与 2026-09-30 审计快照加前向标注。
> 产品代码自阶段 1 起才落地（`crates/protocol` / `crates/storage` / `crates/audit` / `crates/core` 骨架 / `crates/secrets` /
> `xtask` 护栏 / **`crates/platform/api`**（平台抽象层：4 个纯类型 + 3 个 trait 形状 + 能力矩阵）/
> **`crates/platform/windows`**（Win32 / UIA provider：树快照 / selector 链解析 / 读写 / 指纹 / 窗口枚举）已完成，
> TASK-017 的 7 条 DRIFT 已全部裁决落地（**ADR-0043** 元素解析加 scope / **ADR-0044** 歧义策略收敛为唯一
> `ErrorAndAsk` / **ADR-0045** 非宿主平台编译门禁进 `AGENTS.md` §6；PL-068 / 069 / 070 闭环），
> 同 crate 的合成输入（`SendInput`）+ 坐标归一化（DPI / 多屏）+ IME 也已落地（TASK-018 —— 铁律 5 的 **L4** 层；
> 真机验收 2/2；新提 PL-074）；`apps/automation-host` + `crates/ipc`（TASK-019：帧 / 握手 token / NamedPipe / 对端身份白名单 / 双向心跳 / 看门狗）也已落地并经真实进程 kill 断连验收；**`crates/tool-bus`**（TASK-020：MCP client(`rmcp`) + **同进程** MCP server + draft-07 子集参数校验（不支持即拒绝） + 统一返回信封（`untrusted` / `truncated`） + 工具集指纹 + 动态挂载（> 40 告警））、**`crates/policy`**（TASK-021：唯一放行点 / 默认拒绝 / deny 优先 / DSL v0 / 参数护栏）、**`crates/undo`**（TASK-024：L0~L3 + 三类锚点 + 回滚剧本 + 冲突检测 + incident）、**`crates/lease`**（TASK-025：三模式矩阵 + TTL / 续租 / 用户抢占 + 零提交批量获取）、**`crates/model-gateway`**（TASK-026：同步拉取式流 / 路由 / fallback / backoff / cache hint / 成本）与 **`crates/hitl`**（TASK-027：ADR-0048 无损 confirmation 投影 / 审批与四维授权 / 接管与暂停恢复 / diff 数据）均已落地；TASK-036 已完成并合并 PR #88（merge `0b64f85`）；TASK-037 已合并 PR #92（merge `6e9c85a`）；TASK-038 已合并 PR #94（merge `5860840`），下一张是 TASK-039。**当前阶段详情以 `PLAN.md` 为准**。

---

## 这个项目最特别的一点

它主要由 **AI coding agent**（Codex / opencode / Claude Code）实现，人类负责规划、审阅与裁决。
因此仓库里有一半的"代码"其实是**约束 agent 的机制**：
任务卡、write scope、约束回执、漂移触发器、机器护栏、长期记忆文件。
如果你只想看架构，直接读架构文档；如果你想让 agent 在这个仓库里干活，**必须先读 `AGENTS.md`**。

---

## 产品层 vs 工程元层（读任何文档前先分清这一条）

本项目由 AI coding agent 实现、人类审阅，因此仓库里有大量**不属于产品**的东西。
判定标准只有一句话：**「删掉它，产品的行为会变吗？」不会 → 工程元层。**

| 层 | 包含 | 会被构建进发布物吗 | 受什么约束 |
|---|---|---|---|
| **产品层** | `crates/`、`protocol/`、`adapters/`、`adapters-private/`、`apps/`、`fixtures/`、`eval/` | ✅ 是 | `docs/spec/*` 的契约 ＋ `AGENTS.md` |
| **工程元层** | `AGENTS.md`、`docs/governance-ai-agent-execution.md`、`docs/subagent-orchestration.md`、`docs/automation-charter.md`、`docs/automations/*`、`MEMORY.md` ＋ `docs/memory/*`、`LEDGER.md`、`docs/PARKING_LOT.md`、`docs/adr/*`、`xtask/`、`.github/workflows/*` | ❌ 否 | 只受 `AGENTS.md` 约束 |

**为什么要正式区分**（ADR-0029 D5）：这条边界此前只存在于口头，造成三个真实症状 ——
① 文档地图把「自动化章程」与「存储设计」并列，读者（尤其是将来开源后的外部读者）
分不清哪些是产品的设计、哪些是「我们怎么干活」；② 每次讨论自动化都要重新解释一遍
「它不影响 `crates/` 里的任何代码」；③ 护栏工具与治理文档的体积已接近产品文档，
若不分类，「阶段 0 零产品代码」这个事实会被掩盖（此前只能靠一句免责声明打补丁，那是补丁不是结构）。
边界不清的真实代价是**注意力错配**：工程元层的讨论会被误当成产品需求变更，反之亦然。

> **推论**：工程元层的 ADR（自动化机制、护栏工具口径、ADR 编号治理…）**不构成产品决策**，
> 也不改变任何 `docs/spec/*`。审阅它们不需要产品上下文；反过来，审阅产品 spec 也不需要读它们。

---

## 文档地图（按需读，不要全读）

| 你想做什么 | 读什么 |
|---|---|
| 让 AI agent 在本仓库干活 | **`AGENTS.md`**（唯一入口，含十条铁律与会话协议） |
| 了解整体架构（分层、进程边界、安全、平台差异） | `cross-platform-ai-assistant-architecture-v2.md` |
| 了解"某个应用能不能接、怎么接" | `target-apps-feasibility.md` |
| 了解现在做什么、不做什么 | `PLAN.md` → `plans/<当前阶段>.md` |
| 回忆项目已知什么 / 否决过什么 / 踩过什么坑 | `MEMORY.md`（**L0 索引**，≤150 行，含路由表）→ 按路由跳读 `docs/memory/*` |
| 了解治理机制（任务卡、CI 门禁、代码规范、验收分离） | `docs/governance-ai-agent-execution.md` |
| 了解多 agent 如何分工 | `docs/subagent-orchestration.md` |
| 了解存储方案与性能预算 | `docs/storage-design.md` |
| 了解全项目拆解顺序 | `docs/wbs-overview.md` |
| 了解无人值守自动化的**纪律与边界** | `docs/automation-charter.md`（章程 v1.4 §11：一主一备 —— 主 = Codex 原生 scheduled tasks，备 = 任务计划程序 + `codex exec`） |
| 查自动化的**具体操作**（建 / 改 / 删 / 立即运行 / 暂停 / 停止 / 恢复） | `docs/automations/codex-automations-operations.md`（ADR-0029 的主交付物；每条结论都带 `[官方]` / `[实测]` / `[未验证]` 证据标签） |
| 查契约细节 | `docs/spec/`（naming 已就位，其余陆续产出） |
| 查"为什么当初这么决定" | `docs/adr/README.md`（**编号登记表**：哪些号已有文件 / 哪些只是待建）→ 具体 `docs/adr/NNNN-*.md` |
| 查台账 / 停车位 / 依赖登记 | `LEDGER.md` / `docs/PARKING_LOT.md` / `docs/DEPENDENCIES.md` |

---

## 当前阶段

**阶段 1 — 三试点闭环**（Notepad → Paint → Edge/Chrome；TASK-011 ~ TASK-058）。
**2026-10-06 TASK-244 机器派生计数收口**：PL-022 闭环 —— ADR-0075 让 xtask 从 gov §5.4 表格运行时派生 hygiene 规则总数，并机器校验 gov §5.1 与 ci.yml 的 # gov-gate 标记集合；缺失 / 重复 / 额外 / 不可解析均为 exit 1。
**2026-10-06 TASK-041 拆分 A + 拆分 B**：ADR-0073 冻结像素遮挡归平台层、`NeverPersist` 不返回 / 不保留 `ImageRef`、滚动清理有界；`crates/capture` 注入式编排与 `crates/dlp/src/redact` 纯规则模型落地，15 个专项测试通过。**拆分 B**（ADR-0076）：平台层用 GDI `PrintWindow` 截单个已解析窗口（未遮挡时 `BitBlt` 回退）、按 UIA `IsPassword` 矩形做像素遮挡、算 SHA-256 内容地址后交给**注入的** `ImageBlobSink`；binary `StorageBlobSink` 用 `crates/storage` 的 `BlobStore`（`BlobKind::Screenshot`）落盘并核对地址，未注入 → 显式 `Fatal`。真实 GUI 截图与真机 blob 落盘由 **TASK-246** 取证。
**2026-10-04 replay 完整版收口**：**TASK-236 Done** —— Recording v2 序列 fixture + 节点 / 属性 / 文本树级 diff + core suite + 两类篡改负向证据；--list-deferred replay 遗留清零。
**2026-10-04 三连发审计收口**：**TASK-237 Done** —— 对 10:30 / 13:00 / 15:30 三轮自动化做事后独立复核：`PL-062` 已闭环（`xtask replay --suite core` 交付后与 `AGENTS.md` §6 一致，实测 EXIT 0）、孤儿自动化目录删除、`TASK-235` 状态行归一；TASK-234 / 235 / 236 的 `hygiene 13/13` / `PL-094` / replay 完整版证据本机重跑全绿。
**2026-10-04 crate 边界解锁**：**TASK-238 Done** —— ADR-0071 冻结截图管线 / 出域脱敏的 crate 边界：`crates/capture` = 平台无关窗口截图管线（唯一截图原语 = `platform/api` 的 `WindowProvider::capture` trait）、`crates/dlp` = 出域策略 + 脱敏 + 截图遮挡；依赖方向单向、铁律 7 不破、零第三方依赖起步。两个零依赖骨架已落地，架构 v2 §3 布局追加 `capture/`；`PL-102` 闭环，1b 的 TASK-041 / 042 与 1c 的 TASK-050 前置解锁。
**2026-10-05 停车位复核**：**TASK-239 Done** —— `docs/audits/parking-lot-review-2026-10-05.md` 逐条复核仍开放或存在部分关闭余项的停车位；`docs/PARKING_LOT.md` 只追加 48 行结论，补记 13 条已闭环/取代、保留 35 条待治理或人工裁决。
**2026-10-05 派生值指针化**：**TASK-241 Done** —— ADR-0072 冻结动态派生值只指向唯一事实源；AGENTS.md / MEMORY.md §1 / plans/stage-1-pilots.md / 派单文档行数副本已指针化，PL-035 闭环；PL-022 机器派生余项已由 TASK-244 闭环。
**2026-10-05 三连发审计收口**：**TASK-242 Done** —— 独立复核 TASK-239 / 240 / 241 发现两处残余：TASK-076 的滞后状态行（LEDGER L88 早在 2026-09-22 就声称改成 Done，但 commit `0f05ef7` 未落盘，卡面至今 InProgress）已按实补正；TASK-002 的卡面 `Done` / `PLAN` 阻塞项 `Blocked` / `LEDGER` 无 Done 事件三方不一致，按规则开成 **PL-109（待裁决）**。
**2026-10-05 PL-109 裁决收口**：**TASK-243 Done** —— 人类 2026-10-05 裁决 **`TASK-002` = Done**（SPIKE-A 已归档 GO，§11~§15 在 main）。据此把活的事实源全部对齐：`PLAN.md` 阻塞项删去「TASK-002 仍 Blocked」（只余 gov #9 / #11 SOFT 门禁）、`MEMORY.md` §1「下一步 ③」改写、`LEDGER.md` 追加裁决事件、`docs/PARKING_LOT.md` 标 PL-109 闭环；`SPIKE-A.md` banner 与 2026-09-30 审计快照各加 `[supersedes:2026-10-05]` 前向标注（历史正文按 ADR-0051/0052 保留不改）。
**2026-10-06 四连发审计收口**：**TASK-245 Done** —— 四连发（00:30 TASK-041 拆分 A 已合并 / 03:00 TASK-042 WIP / 05:30 Windows `capture()` WIP / 08:00 TASK-244 已合并）审计发现三处账面问题并收口：① `LEDGER.md` 里 TASK-041 的 merge-hash 行被重复追加 **5 次**且插在 2026-10-04 行之间 → 去重为 1 条、归位到 2026-10-06 段；② 两个 WIP 的 `DRIFT-041-2`（storage sink 未接入，**待人类裁决**）/ `DRIFT-042-1` + `PL-110`（`visual_assert` 未接入）此前只存在于 PR 分支 → 补进 `PL-111`；③ ADR-0074 号被 PR #241 与 #242 同时占用（main 上 0074 空缺）→ 登记待改号（建议 #241 留 0074、#242 改 0076）。
**2026-10-05 状态行清扫**：**TASK-240 Done** —— 全量比对卡面状态与 `LEDGER.md` 最后状态，18 张确定滞后的卡只改状态行；`TASK-105` 因台账仍为 `InProgress` 且 L0/L1 回滚项未闭环保持原状。
阶段 1a 补救治理已把运行链路、typed IPC、UI/commit 质量门禁与 `check-comments` 门禁收口；`gate-selftest` 的 fmt / clippy / build canary 已有成功 run 证据（`36598959358`）。
**2026-10-04 hygiene 13/13 收口**：**TASK-234 Done** —— ADR-0068 冻结跨文件重复代码规则（40-token 规范化 shingle、包含度 ≥ 80%、共享 ≥ 4、忽略测试 / 生成 / fixture、Warning），ADR-0069 建立 `docs/adr/top-level-directories.md` 白名单（未登记目录 Error，`scripts/` 继续留给 PL-023）；两条规则均有正负样本，`--list-deferred` 的 hygiene 未实现项为 0，`PL-060` 闭环。
**2026-10-04 RoleAndParent 解析契约收口**：**TASK-235 Done** —— ADR-0070 将链内被 `parent_id` 引用的候选限定为子树作用域；`resolve_element` 顶层过滤 parent scope candidate，父候选单独命中不再作为结果返回，父命中但子缺失显式 `TargetNotFound`，`PL-094` 闭环。
**2026-10-04 会话持久化装配收口**：**TASK-233 Done** —— binary 装配层新增 `StorageSessionStore`（实现 `SessionStore` 三方法，经装配层 `DatabaseHandle` 调 TASK-230 的记录原语），生产 `production.rs` / `main.rs` 两处由 `MemorySessionStore` 改为 `with_storage_session_store()`；适配器级跨重开逐字段一致（含分支消息）、负向四类显式失败、装配级关库重开读回均取证，`PL-108` 闭环。
**2026-10-03 资源生命周期收口**：**TASK-220 Done** —— 人类要求的全仓泄露审计落地：UIA 元素表 4096 上限 + FIFO 淘汰 + `clear_thread_elements()`；生产根 anchor 注册表 64 上限、`ApprovalGrants` 1024 上限与过期清理；靶机 PowerShell 子进程在正常结束 / 启动失败 / 被杀三种路径都走 `taskkill /T /F` + `wait()` reap。真机 ignored 全套（`--test-threads=1`）**8 passed / 0 failed**：四个 T1 干跑 + 6 轮真实 UIA 收敛测量 + 三个对照；对照 `no_fixture` 句柄全程 125、`fixture_only` 稳定 130，而真实 UIA 迭代路径每轮约 +4（130→271，6 次）→ 增长只出现在真实使用路径上，有界性由元素表 4096 上限 + FIFO 淘汰保证（6 次不足以证明收敛平台）。`DRIFT-220-1` 闭环。
**2026-10-03 收口**：**TASK-223 Done** —— ADR-0064 让 T1.1 大文件走 binary 层保留 `host_service` 的有界 UTF-8 前缀读取（显式 `truncated`、16 MiB 硬上限、不整文件入内存）；ADR-0065 新增恒执行的前置步骤 `assistant.runtime.host_capture_initial_fingerprint`，指纹只来自**注入平台**（fake 与 `WindowsPlatform` 同一段代码、同一条路径），显式否决 `cfg!(debug_assertions)` 分叉与伪造指纹。大文件输入下 `read_text` 被条件跳过（UIA read 调用数 0）、文件通道接管、4 步全提交到 `Completed`；`runtime_binding` 的 fail-closed 判据一字未改。`DRIFT-223-1` / `PL-100` 闭环。
**2026-10-03 跨层租约收口**：**TASK-225 Done** —— Host 装配层新增可克隆共享的 `TargetLeaseRegistry` / `TargetLeaseGate`；`notepad.file.save`、`save_as` 与 rollback `Ctrl+Z` 的 `key_action` 在平台发送前取得窗口 exclusive lease，冲突复用既有 `Transient`，成功/失败都释放，只读路径不接入。两任务共享 registry 的冲突、释放后重试、失败回基线与 pointer-shaped 专项 **5 passed**；`PL-101` 闭环。
**2026-10-03 测试卫生收口**：**TASK-226 Done** —— PL-104 的 ignored 真机用例改为在同一测试二进制内用 `tokio::sync::Mutex` 串行化 fixture 启动，修复前 `--ignored --test-threads=4` 为 6 failed / 2 passed（`TargetAmbiguous`），修复后为 **8 passed / 0 failed**；PL-105 把 888 行 `production_root.rs` 拆成 433 / 390 / 101 行的 3 个 `production_root*` 文件，公共 fixture helper 下沉到 `tests/support/production_fixture.rs`，断言只移动、未删未放宽。
**2026-10-03 状态与停车位收口**：**TASK-227 Done** —— 只改账面、零代码改动：`TASK-040`（真机四用例取证 + 跨层 lease 由 TASK-225 闭环）与 `TASK-225` 的状态行按实更正为 Done，`TASK-040` 正文 DoD 里那条 lease 复选框因正文只读仍空但事实已由记录区承担；`PL-103` 在停车位给出可直接粘贴的 `docs/spec/runtime-execution.md` §3 改法（指向 `RESERVED_RUNTIME_TOOLS` 唯一事实源，待 Orchestrator 落笔）；`PL-106` 开卡 `TASK-228`（真机验收 PASS/SKIP/FAIL 结构化输出，Ready 待派单）。
**2026-10-03 spec 与规则收口**：**TASK-229 Done**（人类逐条授权）—— `PL-103` 闭环：spec §3 改为「保留工具的闭集与完整名单以 `apps/agent-core/src/runtime_tools.rs` 的 `RESERVED_RUNTIME_TOOLS` 为唯一事实源」，避免手写计数再次漂移；`docs/memory/pitfalls.md` 新增可执行硬规则（收口类提交必须先用 `git diff --name-only` 列出涉及的卡与本卡，逐张用 `^- 状态` 正则取状态行并与 LEDGER 比对，把结果贴进 PR）；轮次 A 的残留目录已删除（真实阻塞是 git 打包对象的只读属性，不是文件锁）。
**2026-10-03 真机验收结构化收口**：**TASK-228 Done** —— 四个 Windows ignored 用例统一写 `target/acceptance/windows-input-acceptance.json` 的 `pass|skip|fail` 记录；`tools/acceptance-report` 对缺字段、未知字段、未知状态、用例集合不完整显式拒绝，全 pass 才退 0。真机一轮 **4 pass / 0 skip / 0 fail**，坐标误差与点击命中误差均为 0 px，`PL-106` 闭环。
**2026-10-04 命令行写通道收口**：**TASK-224 Done** —— ADR-0066 落档 Accepted：`xtask write` 先复用 ADR-0028 的 guard 锁，再用 Windows `share_mode(0)` 探测目标占用并退避重试；写句柄只共享 `FILE_SHARE_READ`，所以读取路径不阻塞。`apply_patch` 明确不纳入强制通道，`FILE_SHARE_WRITE` 盲区与非 Windows fail-closed 写进 ADR；目标占用/guard 超时均退出码 5，实机无残留进程与锁，`PL-107` 闭环。
**2026-10-04 会话持久化收口**：**TASK-230 Done** —— storage 新增全局迁移 `0005`，以 `conversations` 与 `conversation_messages` 保存 Core `SessionSnapshot` 语义；公开插入 / 读取 / 整树替换 API，父子同会话、父 sequence 更小、revision 单调 +1 全部 fail-closed。跨重启测试真实关闭并重开同一数据库，`PL-092` 闭环。
**2026-10-04 pointer DPI 收口**：**TASK-231 Done** —— ADR-0067 删除 `coordinate_space_for_logical_point` 的收敛启发式；`pointer_action` 显式携带起始 `CoordinateSpace`，`DragTo` 的释放点携带自己的 `drop_coordinate_space`。平台只接受真实设备名、`PhysicalPixels` 单位与匹配 DPI，并要求物理点落在任一显示器；混合 DPI 归属 6 passed、跨屏拖拽双 scale 1 passed，`PL-074` 闭环。
**2026-10-04 四轮自动化审计收口**：**TASK-232 Done** —— 23:20 / 01:50 / 04:20 / 06:50 四轮自动化的产物（`TASK-228` 真机结构化记录、`TASK-224` ADR-0066 写通道、`TASK-230` storage 会话持久化、`TASK-231` ADR-0067 pointer 显式坐标空间）已逐项复核通过并全部并入 main；审计发现的账面问题就地收口：`TASK-224` 状态行按实改 Done、状态行比对规则补强为「默认必须本批改掉，且 PR 必须贴比对」、新增 `PL-108` 记录 storage 轮遗留（生产 `SessionStore` 适配器仍缺，装配层仍在用内存 store）。
**2026-10-02 收口**：**TASK-216 Done** —— A/B 片的任务输入绑定、`hitl` / `host_service` / `verify` 保留步骤、`notepad.tab.new` 计数观测与可恢复审批由 TASK-217 落地；T1.2/T1.3 已在 fake platform 到 `Completed`，`DRIFT-216-4` 闭环。**TASK-105 / TASK-218 / TASK-219 Done**：真实 UIA 主路径 3×10、物理快照回滚与 sweep 证据已就位；独立复验硬门禁全绿，阶段 1a 翻转为 **GO**。
**2026-10-02 护栏推进**：**TASK-085 Done** —— `hygiene` 新增单函数行数、参数个数、圈复杂度、空 stub、跳过测试 5 条源码结构规则；现有 81 处存量按 ADR-0025 D1 先以 Warning 上线，未实现规则从 10 项降到 5 项，覆盖推进到 **8/13**。
**2026-10-02 护栏推进**：**TASK-086 Done** —— CRLF、末行换行与 Cargo 直接依赖登记三条 hygiene 规则已落地；未实现规则降到 2 项，覆盖推进到 **11/13**。末行换行按 ADR-0025 D1 保持 Warning，10 个存量文件留给后续清扫卡。
**2026-10-01 并行治理**：**TASK-205 Done** —— `crates/tool-bus/src/schema.rs`（892 行）纯搬移到 `schema/{mod,registration,instance,shared}.rs`，公共项路径与行为不变；`hygiene` 的 `file-too-long` warning 从 4 降到 3，所有卡面门禁全绿。
**2026-10-01 运行时收口**：**TASK-217 Done** —— 在 binary 层实现任务包 `$name` 数据流、提交后输出发布、白名单 `pure`/host operation、封闭 `when` 谓词与审批暂停/恢复；T1.2/T1.3 在 fake platform 执行到 `Completed`，无授权停在 `AwaitingApproval`，UI 批准后有界授权从同一快照恢复且不重放已提交步骤，`DRIFT-216-4` 闭环。
子阶段 1a 已开工：地基层的 `crates/protocol`（schema + codegen）/ `crates/storage`（SQLite WAL + 迁移注册表 + blob + **TASK-206 `memory_fts` FTS5 检索**）/
`crates/audit`（append-only hash chain）/ `crates/core`（会话 + 上下文 + Planner + Memory）/ `crates/secrets`（OS keychain 封装）/
`xtask` 护栏（`docscan` 4 条结构规则 + `check-ledger` + `check-migrations` + `crates/core` 分层断言）/
**`crates/platform/api`**（铁律 7 的唯一平台入口：`TargetDescriptor` / `NormalizedPoint` / `Fingerprint` /
`CapabilityMatrix` + `PlatformService` / `WindowProvider` / `UiAutomationProvider` 三个 trait 形状）/
**`crates/platform/windows`**（Win32 / UIA provider：树快照 / selector 链解析 / `read_text` / `set_value` /
`edit_text` / `invoke_action` / `bounds` / `fingerprint` / 窗口枚举与状态 / **合成输入（`SendInput`）+ 坐标归一化（DPI / 多屏）+ IME**）**均已落地**；
跨阶段治理卡 TASK-200 / 201 / 202 / 203 / 204 已 Done；TASK-016 的能力命名歧义已由 **ADR-0042** 定案
（`CapabilityCatalog` = 稳定标识目录 / `CapabilityMatrix` = 运行时探测结果 / `CapabilityEntry` = 风险·审批元数据）；
TASK-017 的 7 条 DRIFT 遗留已由 **ADR-0043 / 0044 / 0045** 裁决落地（元素解析加 scope / 歧义策略收敛为唯一
`ErrorAndAsk` / 非宿主平台编译门禁进 `AGENTS.md` §6；PL-068 / 069 / 070 闭环）；
TASK-018 把铁律 5 的 **L4（合成输入）** 层补齐（`SendInput` VK 路径 + `KEYEVENTF_UNICODE` 文本路径 + 发送前 100% 前台校验 +
显示器枚举 / DPI 换算 / 虚拟屏幕归一化 + `is_ime_open`；真机验收 2/2 —— 坐标误差 0 px、记事本 Unicode + Ctrl+S 磁盘回读），
新提 **PL-074**（`pointer_action` 不带目标窗口 → 混合 DPI 多屏下逻辑点无法唯一归属显示器）。
TASK-019 把 Host IPC 传输层落地：纯函数帧编解码（magic / 16 MiB 上限 / CRC32）+ 版本化认证握手（token 常数时间比较）+
Windows NamedPipe 传输（`PIPE_REJECT_REMOTE_CLIENTS` + 客户端 PID / 镜像路径白名单）+ 双向心跳 / 看门狗；
真实验收启动 host、完成 `ServerHello` 与心跳后 kill host，客户端在 2 s 内检测断连。
TASK-020 把**工具通道**落地（架构 v2 §5.4「MCP 是唯一工具协议」）：`crates/tool-bus` 用 `rmcp` 在**同一进程**内起 MCP server、
`tokio::io::duplex` 作传输（无 socket / 无子进程 / 无网络），`tools/list` 与 `tools/call` 全链路走**真实 MCP 往返**；调用身份经 MCP `_meta` 跨边界传递。
参数按 `ToolSchema.input` 做 draft-07 **子集**校验（**不支持即拒绝注册**），不合法直接拒（`ToolInvalidArgs`）且**不进 handler**；返回统一为 `ToolEnvelope`（`untrusted` / `source` / `truncated` / `metrics` / `error` 齐全），超预算截断显式标注、截不动则 fail-closed；
工具集指纹（SHA-256，与挂载顺序无关）+ `toolset.list` / `toolset.search` 两个元工具 + 单次挂载 > 40 个工具的结构化告警（含未串链审计事件）。
TASK-021 把**唯一策略放行点**落地：`crates/policy` 用 JSON DSL v0 完整表达架构 v2 §12.2 的五条示例规则，
求值为无 IO / 无时钟 / 无随机的纯函数；deny 优先，未命中一律 `default_deny`，且 `L3 + unattended` 与
污点上下文中的 L3 / High / Critical 由不可绕过的硬底线拒绝。路径穿越 / UNC / 设备路径 / 符号链接逃逸、
URL scheme-host-IP、文本长度与行数、数值边界、正则 ReDoS 形状均走 fail-closed 护栏；每个 deny 都带
非空 `rule_id` 与 reason。新增 29 个测试 + 1 个 doctest，`policy` 行覆盖 **93.60%**，判定中位数
**3.6~5.6 µs**。`assistant_protocol::PolicyDecision` 无法携带 confirmation scope / `show_diff`，故完整决策
保留在 policy 内、协议映射只作审计摘要，协议扩展记为 **PL-082** / `DRIFT-021-1`。
TASK-022 把任务编排层落地：12 个 Task 状态与完整 Step 生命周期、确定性 DAG Frontier、每次成功迁移先写检查点、SQLite 重开恢复、暂停 / 取消 / 接管、四维预算和分阶段看门狗；恢复证据缺失或 `Unknown` 一律进入 `NeedsHuman`，绝不猜测写操作是否发生过。新增 43 个测试 + 1 个 doctest，行覆盖 **87.03%**。
TASK-023 把**后置断言引擎**落地：架构 v2 §7.4 的 11 种断言（= 12 种减 `visual_assert`，后者归 TASK-042）全部 fail-closed 解析与三值求值 —— `Verified` 要求全部满足，任一 `Falsified` 即 `Violated`，无 `Falsified` 但有 `NotEvaluable` 即 `Inconclusive`，**两种非成功结果都带 `VerifyFailed`**，结构上不可能出现「验证失败却 ok」。另含 §7.3 状态指纹（canonical form + SHA-256；忽略字段由 per-adapter 显式给出，无全局默认）、§8.5 幂等判定（证据不全一律 `Unknown`，应用自报优先）与 `on_violation` 分派（`retry_once` 只对 `Transient` / `TargetNotFound`）。新增 66 个测试 + 1 个 doctest。
TASK-024 把**撤销闭环**落地：`assistant-undo` 用纯逻辑建模 L0~L3、内容快照 / 影子副本 / undo 预算 / 补偿剧本 / evidence-only 锚点；L0 可带 L1 snapshot fallback。冲突检测默认最保守：当前状态偏离 post 指纹即阻断，缺 post 或当前指纹时即使 `RestoreOverall` 也不放行。回滚动作通过注入 `RollbackExecutor` 执行，只有最终指纹等于锚点 pre 指纹才算成功；执行失败、指纹不一致与冲突都变成带 expected/observed/evidence/recovery guidance 的结构化 incident，上报失败显式返回 `Fatal`。新增 23 个测试。

TASK-025 把**目标租约与并发控制**落地：`assistant-lease` 建模 `Shared` / `Intent` / `Exclusive` 三模式，跨 owner 只允许一个写租约；`Intent` 可并发规划但阻止另一 owner 的 `Exclusive`。TTL 以 `now == expires_at_ms` 为失效边界，支持显式续租、懒清理和 `reap_expired`；用户抢占按 target key 强制释放全部 agent 租约，并把自然过期项单列报告。`acquire_many` 在任何状态写入前按规范 key 排序、去重并完成冲突 / TTL / 溢出预检，任一冲突即零提交，消除“先持有再回滚”窗口。新增 20 个测试，行覆盖 89.53%，零新增第三方依赖。
TASK-026 把**模型网关**落地：`assistant-model-gateway` 定义同步拉取式 Provider 流契约（每轮 poll 必须检查取消并遵守 timeout）、类型化 stage/context/sensitivity/budget/tool-count 路由、指数退避与注入 jitter、可审计降级链、prompt-cache 稳定前缀提示透传，以及按模型聚合的整数 micro-USD 成本记录。第一个非空 text / tool-call delta 之后失败一律返回 `PartialOutput`，禁止重试或降级以避免重复输出；成功流必须恰有一次 usage + finish。新增 20 个契约测试 + 1 doctest，行覆盖 75.23%，零新增第三方依赖。
TASK-204 把 `tool-bus` 的 draft-07 判据**显式化**：三张带理由的拒绝表（永久放弃 12 条 / 暂未实现 4 条 / 非 draft-07 方言 15 条）+ 未知关键字兜底 + `$schema` **方言校验**（声明非 draft-07 = 拒绝），报错信息直接给出「为何不可用 / 该用什么代替」。
TASK-030 把审批与时间线 feature 落地：审批卡片覆盖动作、目标、影响范围、五类 diff、来源归因、工具理由、风险/可逆性、撤销方式、授权范围与证据；`app_content` 标红并默认拒绝，高风险/不可逆只允许 `once`，L3 需输入二次确认词。执行时间线覆盖状态、前后指纹、验证、证据、耗时、成本与可逆性；撤销/重放仅在证据和 Anchor 完整且调用方接线时可用，禁用原因直接显示。所有输入经 fail-closed 运行时校验，零新增依赖，13 个专项测试覆盖关键负向路径。
TASK-031 把元素拾取器与目标绑定向导 v0 落地：严格校验 Host 注入快照，高亮 workspace 内元素，展示 role/name/AutomationId/ClassName/RuntimeId/状态/actions/patterns/bounds/父路径；候选生成保证严格降序、稳定首选、本地化上限与 `kind/value` 一致，并生成 `TargetDescriptor` 形状的 Adapter selector 草稿。独立 review 发现的 `RoleAndParent` 父候选误选风险已以 fail-closed 省略处理，P0/P1 清零；19 个专项测试与现有 Rust/xtask 门禁全绿。PL-094 跟踪后续平台契约治理。
TASK-032 把策略、能力与成本三个 UI feature 落地：出域策略按 `local_only` / `redacted` / `full` 三档提供全局与逐应用覆盖，缺少本地模型时拒绝 `local_only`，升级到更宽级别必须逐次确认；常驻状态栏呈现当前有效级别。Capability Matrix 视图严格解析运行时的 probe / channels / capabilities / degradations，并拒绝审批与风险不一致；成本面板使用整数 micro-USD，按 task / today / month 与模型、应用聚合，拒绝非安全整数和非法日期以免静默舍入或错位。新增 26 个 Node 专项测试，UI typecheck/build 与 Rust/xtask 门禁全绿。
TASK-033 交付 PowerShell WPF `notepad-like` 故障注入靶机；TASK-034 交付 Recording v1、离线 `WindowProvider` / `UiAutomationProvider` 与 `xtask replay`，让逻辑层回归可在无真机环境使用受控树快照；TASK-035 交付 `adapters/com.microsoft.notepad/**` 声明式 Adapter 包；TASK-036/037/038 交付 T1.1~T1.3 声明式任务包与静态评测集。
TASK-039 的集成审计后，2026-10-02 独立复验已复跑门禁矩阵；TASK-105 的真实 T1.1/T1.2/T1.3 各 10 次证据已就位，且 TASK-218/219 补齐回滚与 pure 步骤真实性，因此 **阶段 1a = GO**。
阶段 0（文档与 Spike）已于 2026-09-20 closeout —— 它的产出是 Spike 报告，**不是**产品代码。
详见 `plans/stage-1-pilots.md`。

2026-09-26 的 **DRIFT-028 五点落地**（**ADR-0053**）把 `crates/core` 的编排层范围定死：`core` = **可装配的编排组件库**
（依赖白名单 = `protocol` / `storage` / `platform/api`（仅 trait）/ `task-engine` / `model-gateway`；黑名单含 `tool-bus` / `policy` / `audit` …），
**装配点唯一在 binary**；原 TASK-028（五合一）拆为 **TASK-028**（会话 + 上下文）/ **TASK-207**（Planner）/ **TASK-208**（Memory），
FTS5 检索（`memory_fts`）归 `crates/storage` 的前置卡 **TASK-206**，「组装」下沉 **TASK-029**；契约见 `docs/spec/core-orchestration.md`。

TASK-103 把 **ADR-0056 的最小执行闭环**落地：`crates/verify` 新增不透明 `VerificationReceipt`（字段私有 / 无公开构造器 / 不实现反序列化，只有 `Verified` 能铸出），`crates/task-engine` 的成功提交入口改为消费持有该 receipt 的 `StepCommit`（不提供未验证提交重载）；`apps/agent-core` 新增 binary 装配层 `RuntimeExecutor`，按 Policy → Approval → Execute → Observe → Verify → Commit 单步驱动，并提供真实 `ToolBusInvoker`（`rmcp` 同进程 MCP 往返）与 `EnvelopeObservationCollector`；`apps/automation-host` 新增 `RequestDispatcher` / `run_host_with_dispatcher` / correlation 校验，收到 `Request` 返回 `Response`，其余消息 fail-closed。工具返回 `ok=false` → `ToolFailed`（带信封 `ErrorCode`），传输失败 / 未知边界 → `NeedsHuman`，两条路径都不进入 commit。

TASK-104 把 **UI 与 Core 的边界**落地：`apps/agent-core` 新增 `ui_ipc`（版本化 `UiCommand` / `UiCommandOutcome` / `UiEvent` 契约、未知字段与未知 kind 的 fail-closed 解析、`project_snapshot_events`）与 `ui_control`（`TaskControlHandler` 把 approve / deny / pause / cancel / take-over 应用到真实 task-engine；deny 会把步骤置 `PolicyDenied`、任务置 `Failed`）；`apps/desktop-ui` 用 **zod** 在发送前与渲染前双向校验（`contract.ts` / `client.ts`），并新增审批决定 → 命令映射、Core 事件 → 时间线投影与第一个真实调用点 `IntentLauncher`；`apps/desktop-ui/src-tauri` 的 `send_ui_command` 本地 fail-closed 校验后经注入的 `CoreCommandTransport` 转发。两侧共享 `apps/agent-core/tests/fixtures/ui_ipc/*.json` 黄金样本，各解析一遍。**真实 UI↔Core 传输与事件推送尚未接通**（未注入传输时显式返回 `core_transport_unavailable`），已记 **PL-095**。

TASK-210 把**非 Rust 侧的质量门禁**补齐：`apps/desktop-ui` 接入 **Prettier**（`format:check`，首次归一化 44 个既有文件）、**ESLint** flat config（`lint` 由 `tsc --noEmit` 改为 `eslint . --max-warnings 0`，`no-explicit-any` / `no-console` 为 error，并抓到一行真实死变量）、**Vitest + jsdom + Testing Library**（`*.test.tsx`，与既有 `node --test` 的 `*.test.mjs` 按扩展名分工）与 **commitlint**（`type-enum` / `type-case` / `scope-case` / `header-max-length`；`subject-case` 与 `body-max-line-length` 因中英混排关闭）。CI 侧：`desktop-ui` job 增加 `format:check` + `lint` + 三条 N2 负向验证（注入坏格式 / 注入 `no-console` / commitlint 正负样本，均断言具体退出码）；新增 `commit-lint` job（仅 PR，校验 `origin/<base>..HEAD`，merge 提交由 `defaultIgnores` 跳过）；新增 `desktop-ui-tauri` job（Windows）跑 `src-tauri` 的 fmt / clippy / test —— 补上 TASK-104 记录的门禁盲区（该 crate 是独立 workspace，根 `cargo test --workspace` 覆盖不到）。

TASK-087/TASK-212 把注释与 CI 元门禁补齐：`check-comments` 真实现 naming §10 的 8 条规则，TASK-212 只加 30 行注释把首次实跑的 9 处 Error 清零；`gate-selftest.yml` 新增 fmt（exit 1 + `Diff in`）、clippy（exit 101 + `unwrap_used` / `dbg_macro`）、build（exit 101 + `error[E0425]`）三类 canary，主 CI 新增 `[HARD #15] xtask check-comments`。gate-selftest 的成功 run number 待推送 PR #104 后触发并回填。

**PL-095（UI↔Core 真实传输）** 已完成到「待裁决」这一步：新增 `docs/adr/0057-ui-core-ipc-transport-contract.md`（**Proposed**）与配套 `docs/spec/ui-ipc-protocol.md`（Draft），并派单实现卡 `tasks/TASK-213-ui-core-ipc-transport.md`。ADR 的关键取舍：Core 与 UI **保持独立进程**（架构 v2 §12.7），但**不复用** `crates/ipc` 的工具形状信封去承载 UI 命令 —— 新增 UI 专属的 `UiIpcRequest` / `UiIpcResponse` / `UiIpcEvent`，传输复用 NamedPipe 帧 + 一次性 token + 对端进程镜像白名单，命令带 `correlation_id`、事件单向推送。**该 ADR 未转 Accepted 前 TASK-213 不得开工，TASK-105 也随之仍被阻塞。**

平台基线：**Windows 11 24H2+**（唯一正式基线；Windows 10 已 EOL，仅 C 级尽力）。
Linux 侧 **Wayland-first**（GNOME 50 已移除 X11 后端）。

---

## 构建与验证

前置：Rust stable（由 `rust-toolchain.toml` 固定）。Windows 需要 MSVC 生成工具 + Windows SDK。

```powershell
# 安装工具链（Windows）
winget install Rustlang.Rustup

# 三项基本门禁
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace

# 仓库卫生护栏（gov §5.4；当前实现 3/13 项，工具会自己声明 —— ADR-0025）
cargo run -p xtask -- hygiene
cargo run -p xtask -- --list-deferred   # 查看"还缺哪些检查、归属哪张卡"

# 文档一致性护栏（ADR-0030；派生计数与 ADR 编号不再靠人记）
cargo run -p xtask -- memory-counts     # MEMORY.md 规模表 ↔ docs/memory/ 实测
cargo run -p xtask -- adr-index         # ADR 登记表 ↔ docs/adr/*.md ↔ decisions.md

# 改写公共热点文件（LEDGER.md / docs/memory/* / docs/PARKING_LOT.md）前先取锁（ADR-0028）
cargo run -p xtask -- guard acquire MEMORY.md --owner <会话级唯一标识> --intent "<要干什么>"
cargo run -p xtask -- guard status
cargo run -p xtask -- guard release MEMORY.md --owner <同上>
```

`xtask` 是**零第三方依赖**的只读护栏工具。它对未实现的子命令**显式返回失败**
（退出码 3）并指出归属卡号 —— 本项目不允许任何形式的静默失败。

CI：`.github/workflows/ci.yml`（三平台矩阵；**硬门禁 10 项**已上线 —— fmt / clippy / test / verify-schemas(#6) /
codegen --check(#7) / deny / build / hygiene / spike-deny(#8b) / doc-consistency(#12b)；另有 **7 项软门禁**标
`continue-on-error` 并注明启用卡号；#6/#7 的注入式负向验证在 `gate-negative` job，单测侧负向用例在
`xtask/src/{verify_schemas,codegen}.rs`）。

---

## 最近进展
> **2026-10-06 TASK-244 Done（PL-022 机器派生计数收口；ADR-0075；CI 11/11 SUCCESS）**：xtask 不再硬编码 hygiene 总数，gov §5.1 与 ci.yml 的 # gov-gate 标记集合机器一致；负向 canary 缺标记 exit 1。
> **2026-10-06 TASK-041 Done（拆分 A + B；CI 11/11 SUCCESS）**：拆分 A = ADR-0073 与零依赖纯截图管线 / 脱敏规则模型；拆分 B = ADR-0076 的 Windows GDI 单窗口截图 + UIA `IsPassword` 像素遮挡 + `ImageBlobSink` 注入（binary `StorageBlobSink` 落盘并核对内容地址）。未改 `WindowProvider` / `CaptureOptions` / `ImageRef` 形状，未引入正则引擎或新的第三方依赖；真实 GUI 截图与真机 blob 落盘由 **TASK-246** 验收。
> 历史（2026-10-05：**TASK-241 —— ADR-0072 派生值指针化 + PL-035 闭环，Done；PL-022 机器派生余项已由 TASK-244 闭环**；2026-10-04：**TASK-236 —— replay 完整版（Recording v2 树快照序列 + 树级 diff + `--suite core`），Done；`--list-deferred` replay 未实现项清零**；**TASK-232 —— 四轮自动化审计收口（TASK-224 状态行 + 规则补强 + PL-108），Done**；**TASK-224 —— ADR-0066 命令行 `write` 唯一通道 + Windows 独占占用探测 + 有界退避，Done；PL-107 闭环**；2026-10-03：**TASK-228 —— Windows 真机 PASS/SKIP/FAIL 结构化记录 + 严格汇总入口，Done；真机 4 pass / 0 skip / 0 fail，PL-106 闭环**；**TASK-229 —— PL-103 spec 落笔 + 状态行硬规则 + 残留目录清理，Done**；**TASK-227 —— TASK-040 / TASK-225 状态行收口 + PL-103 改法 + PL-106 开卡，Done**；**TASK-220 —— 资源泄露四类防护 + 真机 ignored 全套 8 passed，Done；`DRIFT-220-1` 闭环**；**TASK-223 —— L1 文件通道 `read_utf8_prefix` + ADR-0065 前置初始指纹步骤，Done；T1.1 大文件 Plan fake 与真机双取证，`DRIFT-223-1` / `PL-100` 闭环**；2026-10-02：**TASK-039 —— 阶段 1a 独立复验全绿，Done / GO**；**TASK-105 —— T1.1/T1.2/T1.3 各 10 次真实 UIA 运行证据，Done**；**TASK-218 / TASK-219 —— 真实回滚链与 pure 步骤真实性修复，Done**；**TASK-086 —— CRLF / 末行换行 / 依赖登记三条 hygiene 规则，Done；实现覆盖 11/13**；**TASK-085 —— Rust 源码结构 5 条 hygiene 规则，Done；实现覆盖 8/13**；**TASK-216 —— 运行时补齐状态收口，Done；DRIFT-216-4 闭环**；2026-10-01：**TASK-217 —— 运行时任务数据流、确定性本地操作与可恢复审批，Done；DRIFT-216-4 闭环**；**TASK-215 —— `notepad-like` 靶机能力扩展 + `com.example.notepad-like` 适配包，Done；PL-097 闭环**（文件读写 + 标签页 + 跨进程 Save As 对话框，真 UIA 实测）；**TASK-214 —— 生产装配根 + Notepad Host handler + 1a Plan 来源，Done；PL-096 闭环**；**TASK-205 —— `tool-bus` schema 模块拆分，Done**；**ADR-0058 Accepted**；2026-09-30：**TASK-213 —— UI↔Core 真实传输，Done（PL-095 闭环）**；**TASK-087 —— CI 负向验证 + `check-comments` 硬门禁，Done**；**TASK-212 —— check-comments 9 处真实违规修复，Done**；2026-09-29：**TASK-210 —— UI 与提交质量门禁（Prettier / ESLint / Vitest / commitlint），Done**；**TASK-104 —— UI ↔ Core typed IPC 与审批接线，Done**；**TASK-103 —— 真实任务执行器（Host 分发 + VerifyReceipt 接线），Done**；**TASK-102 —— 运行执行链路契约，Done；ADR-0056 Accepted**；**TASK-038 —— T1.3 任务包与评测集**；**TASK-037 —— T1.2 任务包与评测集**；2026-09-28：**TASK-036 —— T1.1 任务包与评测集**；**TASK-035 —— Notepad Adapter 声明式包 v0**；**TASK-034 —— 录制回放框架 v0**；**TASK-033 —— notepad-like 靶机应用**；**TASK-032 —— 策略 / 能力 / 成本面板**；**TASK-031 —— 元素拾取器 + 目标绑定**；2026-09-27：**TASK-208 —— `core` Memory（App Map + FTS5 检索消费）**；**TASK-206 —— `crates/storage` `memory_fts` FTS5 检索**；**TASK-207 —— `core` Planner（模型输出 → Plan / Step DAG）**；2026-09-26：TASK-028 —— 会话 + 上下文；TASK-027 —— HITL 审批与接管；TASK-026 —— 模型网关；TASK-025 —— 目标租约与并发控制；TASK-024 —— 撤销与补偿闭环；2026-09-25：TASK-023 —— 后置断言引擎；TASK-204 —— 工具 schema 关键字判据硬化；TASK-022 —— 任务引擎；TASK-021 —— 唯一策略放行点；TASK-020 —— 工具通道；TASK-019 —— Host IPC；TASK-018 —— 合成输入 + 坐标；2026-09-24：地基层 + 平台抽象 + Windows UIA + 治理池）

**2026-10-06 —— TASK-042 视觉验证纯逻辑，Done（ADR-0074；DRIFT-042-1 / PL-110）**

- ADR-0074 Accepted：`visual_assert` 只有结构化形状（`field` + `op` + 具名容差/阈值 + 必填 `confidence_min`），不引入表达式语言；pHash = 32×32 下采样 + raw 8×8 DCT-II 中位阈值、dHash = 9×8 下采样 + 相邻差分，均 64-bit；`max_hamming_distance` 硬上限 24；低置信 / 尺寸不符 → `NeedsHuman` → `NotEvaluable`，不得单独判成功。
- `crates/verify/src/visual/**` 零第三方依赖落地 `GrayImage` 校验、两种感知哈希、像素容差与 21 个确定性合成图专项单测；未改 `Postcondition` / `Observation` / `parse_postconditions` 既有形状。
- 已知限制入 ADR / README / pitfalls：低细节或高频塌缩图会让 64-bit 哈希退化，「没画上」优先用 `pixels` 容差 + 低 `confidence`。

**2026-10-06 —— TASK-245 四连发自动化审计收口（LEDGER 去重 + WIP 落点），Done**

- `LEDGER.md`：TASK-041 的 merge-hash 行从 **5 条重复**（L377/L386/L390/L394/L402，逐字节相同、4 条插在 10-04 行之间）去重为 **1 条**（现 L398，10-06 段）；文件 404 → 400 行，完全重复行组 = 0。
- `docs/PARKING_LOT.md` 仅追加 1 行：`PL-111` = `DRIFT-041-2`（storage sink 待裁决）+ ADR-0074 号冲突 + `DRIFT-042-1` / `PL-110`（原本只在 PR 分支上）。
- 未裁决 storage sink、未合并 PR #242（它明确要求裁决前不得合并）。零产品代码改动。

**2026-10-05 —— TASK-243 PL-109 裁决收口（TASK-002 = Done），Done**

- 人类 2026-10-05 裁决 `TASK-002 = Done`；SPIKE-A 已归档 GO（§11~§15 在 main）。
- 活的事实源全部对齐：`PLAN.md` 阻塞项删去「TASK-002 仍 Blocked」、`MEMORY.md` §1「下一步 ③」改写、`LEDGER.md` 追加裁决事件、`docs/PARKING_LOT.md` 标 `PL-109` 闭环。
- `docs/spike-reports/SPIKE-A.md` banner 与 `docs/audits/stage-1a-reaudit-checklist-2026-09-30.md` 各加 `[supersedes:2026-10-05]` 前向标注；历史正文不改。零产品代码改动。

**2026-10-05 —— TASK-242 三连发审计收口（TASK-076 状态行 + TASK-002 落点），Done**

- 独立复核 TASK-239 / 240 / 241：三轮产物均已在 main、门禁全绿；第 2 轮（状态行清扫）漏改了 TASK-076。
- TASK-076 状态行按实改 `Done`（依据 LEDGER L88 + 卡内「9 节执行记录填写完毕」；`git show 0f05ef7` 证明状态行从未落盘）。
- TASK-002 三方不一致（卡面 `Done` / `PLAN.md` 阻塞项 `Blocked` / `LEDGER.md` 无 Done 事件）开成 **PL-109**（待裁决），补上第 2 轮漏掉的停车位落点。

**2026-10-05 —— TASK-239 停车位存量复核收口，Done**

**2026-10-05 —— TASK-240 任务卡状态行全量清扫，Done**

- 18 张台账最后状态为 `Done` 的卡从 `Ready` / `InProgress` 更正为 `Done`，每行附 PR / merge / 产物证据。
- `git diff --unified=0` 证明本批任务卡只修改 `- 状态：` 行；产品代码与卡正文/记录区零改动。
- `TASK-105`、`TASK-002` 等反向或未闭环不一致项保持原状并进入审阅说明。

- 新增 `docs/audits/parking-lot-review-2026-10-05.md`，逐条列出仍开放条目、补记闭环/取代条目、证据命令与人工判断标记。
- `docs/PARKING_LOT.md` 只在末尾追加 48 行复核行，原 216 行逐字未改；补记 13 条已闭环/取代，保留 35 条待治理或人工裁决。
- 重点纠正了三类“表面关闭但仍有剩余项”的形态：`PL-019` 的 spike 依赖登记、`PL-022` 的派生计数、`PL-035` 的 AGENTS 行数手抄仍开放。

**2026-10-04 —— TASK-238 `capture` / `dlp` 边界 crate 骨架，Done（PL-102 闭环）**

- ADR-0071（Accepted）冻结两个边界 crate：`crates/capture`（平台无关的窗口截图管线，唯一截图原语 = `platform/api` 的 `WindowProvider::capture`）与 `crates/dlp`（出域策略 + 脱敏 + 截图遮挡）；依赖方向单向、铁律 7 不破、零第三方依赖起步。
- 两个零依赖骨架落地（`cargo test -p assistant-capture -p assistant-dlp` 全绿），架构 v2 §3 布局追加 `capture/`；未改根 `Cargo.toml`（`crates/*` glob 自动纳入）、未改公共 trait / schema。
- `PL-102` 闭环：TASK-041 / 042 与 TASK-050 前置解锁。

**2026-10-04 —— TASK-237 三连发自动化审计收口，Done（PL-062 闭环）**

- 独立复核 10:30 / 13:00 / 15:30 三轮产物：本机重跑 `hygiene` = scanned 359 / 0E / PASSED（13/13）、`--list-deferred` 未实现 0 项、`replay --suite core` = 2 step / 5 change / 0 mismatch、`cargo test -p xtask` 459 passed、`cargo test -p assistant-replay` 16 passed，全绿。
- `PL-062` 闭环：`xtask replay --suite core` 完整版交付后与 `AGENTS.md` §6 一致（实测 EXIT 0），触发条件消失；`AGENTS.md` 未改。
- 删除孤儿自动化目录 `...\.codex\automations\ai-assistant-task-round-0420-storage-session-api\`（1 文件 870 B；删前断言在 `.codex\automations\` 下）；`TASK-235` 状态行归一。

**2026-10-04 —— TASK-235 收口 `RoleAndParent` 父候选语义，Done（ADR-0070；PL-094 闭环）**

- `resolve_element` 以链内 `parent_id` 引用关系识别 parent scope candidate，并在顶层目标尝试前过滤；父候选仍可用于 `RoleAndParent` 递归子树搜索，但绝不作为结果返回。
- 专项证据：父 + 子链只选择子候选；父命中但子缺失返回 `TargetNotFound`；只有 parent scope 候选的链在碰 COM 前返回 `ToolInvalidArgs`。
- 变更只触及 `crates/platform/windows/src/uia/{resolve,search}.rs` 与 ADR / 记忆 / 台账，不新增依赖、不新增 `#[allow]`、不改公共 wire 类型；`min_score_to_try`、降权和歧义策略保持不变。

**2026-10-04 —— TASK-234 补齐 gov §5.4 最后两条 hygiene 规则，Done（hygiene 13/13；PL-060 闭环；PR #218 / `e417a2a`）**

**2026-10-04 —— TASK-233 生产装配接入 storage 会话持久化（`SessionStore` 适配器），Done（PL-108 闭环）**

- 新增 `apps/agent-core/src/storage_session_store.rs`：`StorageSessionStore` 实现 `SessionStore` 三方法，只经装配层 `DatabaseHandle` 调 TASK-230 记录原语；`insert` → `insert_conversation_snapshot`、`update` → 先自查 revision 恰好 +1 再 `replace_conversation_snapshot`、`load` → `load_conversation_snapshot` + `SessionSnapshot::restore` 复校验。
- 生产 `production.rs` / `main.rs` 两处改为 `.with_storage_session_store()`；新增开关默认 `false`、注入 store 优先，二者皆无则 `MissingComponent{session_store}` fail-closed。
- 证据：`session_store_persistence` 3 passed（跨重开逐字段一致 + 重复 insert / revision 非 +1 / 缺失会话 / 篡改行负向）、`assembly_contract` 6 passed（含装配级关库重开读回）；fmt / clippy / workspace tests / arch / xtask gates 全绿。

**2026-10-04 —— TASK-224 命令行唯一写通道，Done（PL-107 闭环）**

**2026-10-04 —— TASK-230 会话 / 消息树持久化，Done（PL-092 闭环）**

- 新增 `0005_conversations_message_tree.sql`，不改 `0001_init.sql`；`MIGRATIONS` 与 §3.4 登记同步，`check-migrations` 5/5。
- 公开 `ConversationRecord` / `ConversationMessageRecord` 与 `insert_conversation_snapshot` / `load_conversation_snapshot` / `replace_conversation_snapshot`。
- 跨重启测试真实 `close()` 后重开同一 SQLite 并逐字段比对；缺失会话、非法父子关系、revision 跳号均显式失败且无半棵树。

- `xtask write <仓库相对路径>` 先复用 ADR-0028 的 guard 锁，再在 Windows 上用 `share_mode(0)` 探测目标占用；占用时固定退避轮询，默认 5 秒超时并退出码 5 + 放弃日志。
- 写入句柄只共享 `FILE_SHARE_READ`，持锁读取实测 `read_ms=77`；`FileShare::None` 占用测试输出 `sharing_violation(raw_os_error=32)`、目标未改、锁已释放、残留 xtask 进程 0。
- `apply_patch` 明确不纳入强制通道；`FILE_SHARE_WRITE` 盲区、临时进程无真 FIFO、非 Windows fail-closed 均由 ADR-0066 明写。

**2026-10-03 —— TASK-226 测试卫生清理，Done（PL-104 / PL-105 闭环）**

- `production_root_uia.rs` 用 `tokio::sync::Mutex` 只串行化会启动 `notepad-like` 的 ignored 用例；同一 `--ignored --test-threads=4` 从 6 failed / 2 passed 变为 8 passed / 0 failed。
- `production_root.rs` 888 行拆为 433 + 390 + 101 行，共享 `TestDirectory` / `workspace_root` 进入 `tests/support/production_fixture.rs`；断言只移动、未删未放宽。
- 验收：`cargo test -p assistant-agent-core --test production_root*` = 7 + 6 + 1 passed；fmt / clippy / workspace tests / arch / deny / xtask gates 全绿。

**2026-10-03 —— TASK-225 合成输入目标租约独占，Done（PL-101 闭环）**

- `TargetLeaseRegistry` 由 `ProductionConfig` 克隆共享；`key_action` / pointer-shaped 输入发送前必须取得窗口 exclusive lease，冲突稳定 `Transient`，成功与失败都释放。
- 验收：TASK-225 专项 5 passed；`cargo test --workspace` / arch / clippy 全绿；`PL-101` 关闭，TASK-040 正文复选框因正文只读留待 Orchestrator。

**2026-10-01 —— TASK-205 `tool-bus` schema 模块拆分，Done**

- `crates/tool-bus/src/schema.rs`（892 行）纯搬移到 `schema/{mod,registration,instance,shared}.rs`；注册期检查与运行期校验分责，所有文件低于 600 行建议线。
- 验收：`fmt` / `clippy -D warnings` / workspace tests / tool-bus 22 tests / schema-codegen / xtask 门禁 / `cargo deny` 全绿；`hygiene` 长文件 warning 4→3。

**2026-10-01 —— TASK-214 生产装配根与 1a Plan 来源，Done（PL-096 闭环）**

- `apps/agent-core`：`--production` 显式装配 `TaskEngine`、`RuntimeExecutor`、5 个 Notepad handler、`ui_server` 与 `SnapshotEventSource`；`ProductionError` 新增稳定 `ErrorCode`。
- 验收：真实 `notepad-like` + UIA T1.1 干跑 1/1 通过；真实 NamedPipe 推出 committed `step_state_changed`（strict 六字段 + 真 post fingerprint）；缺 provider / 空 registry / handler 数不符 / 空 peer 白名单均 fail-closed。全量 fmt / clippy / workspace test / xtask 门禁 / `cargo deny` / UI lint-format-test 全绿。
- 遗留：`PL-097` 仍需扩靶机以提供 T1.2 / T1.3 文件、标签页与跨进程对话框证据；阶段 1a 仍 **NO-GO**。

**2026-09-30 —— TASK-213 UI↔Core 真实传输，Done（PL-095 闭环）**

- `crates/ipc`：新增 UI 专属 wire 信封（`UiIpcRequest`/`UiIpcResponse`/`UiIpcEvent` + `UiIpcResult::Rejected{code,message}`），`WireMessage` 增三个 UI 变体；**工具形状信封一个字段未动**（ADR-0057 D2）。
- `apps/agent-core`：Core 侧监听端 `ui_server`（pipe 接受 + 对端镜像白名单 + token 握手 + 会话循环 + **阻塞读之前先推事件**）、生产事件源 `ui_events::SnapshotEventSource`（`project_snapshot_events` + 按 `revision` 去重）。
- `apps/desktop-ui/src-tauri`：UI 侧真实 client `core_pipe`（连接 + 握手 + correlation 匹配 + `subscribe()` 长连接消费事件）；**不链接 Core**（架构 v2 §12.7）。
- **真管道端到端验收**：真实 NamedPipe 上断言 `Heartbeat → UiEvent → UiResponse` 顺序、请求恰好触达处理器一次、断开后 2s 内显式收尾。
- 验收：`cargo test --workspace` **1062 passed / 0 failed**；`src-tauri` 13 条；UI（model 76 + DOM 3）；xtask 八道门禁全 PASS；hygiene 0E/4W（基线）。
- 留给 TASK-105 的装配一行：把 executor 的引擎接成事件 provider。

**2026-09-30 —— ADR-0057 转 Accepted（人类裁决）**

- 人类明确接受 ADR-0057（UI↔Core 传输契约）：Core 与 UI 分进程、**新增 UI 专属 wire 信封**（不复用工具形状的 `RequestMessage`/`ResponseMessage`）、传输复用 NamedPipe 帧 + 一次性 token + 对端镜像白名单、命令带 `correlation_id`、事件单向推送。
- ADR 文件与 `docs/adr/README.md` 登记表、`docs/memory/decisions.md`、`docs/PARKING_LOT.md` 同步为 **Accepted**；`docs/spec/ui-ipc-protocol.md` 由「Draft（ADR Proposed）」成为**实现契约**。
- **后果**：PL-095 不再阻塞 **TASK-213**（已解锁，当前应开工的卡）；**TASK-105 仍不可开工**（还需 TASK-213 完成 + 真实 ModelProvider + Notepad Host handler）。
- 同一轮按人类指示**删除 6 条定时 automation**（`ai-assistant-1-6` … `6-6`），此前它们只剩 2 条真正跑过、且都被上游 429 掐死。

**2026-09-30 —— TASK-211：阶段 1a 复验准备与停车位收口**

- 新增 `docs/audits/stage-1a-reaudit-checklist-2026-09-30.md`：gov §5.1 逐行复验矩阵（标注首轮 → 当前的变化）+ ADR-0019 负向验证登记（PL-018 证据齐备）+ **仍 open 事项清单** + 给独立复验会话留空的「复验实测」列。
- 判定基线写死：**门禁全绿也不够 —— 缺 TASK-105 的运行证据就不能把 1a 转 GO**；阶段 1a 结论保持 **NO-GO**。
- 关闭两个停车位：**PL-056**（`check-migrations` 已在 `doc-consistency` 每次执行）、**PL-058**（`refscan` 已入 CI 且基线 151 error → 0E/0W）。
- **DRIFT-211-1**：卡面声明依赖含 TASK-105（未完成，被 PL-095 阻塞），故本卡只交付不依赖运行证据的部分，并在卡内显式声明"不宣布 1a 通过"。

**2026-09-30 —— PL-095 推进：ADR-0057（UI↔Core 传输契约，Proposed）+ TASK-213 派单**

- 人类指示「把自动化任务没有做完的都做掉」→ 执行原 6 轮自动化 prompt 的第 3 项（PL-095：先立 ADR 并停下等人类接受）。
- `docs/adr/0057-ui-core-ipc-transport-contract.md`（**Proposed**）：分进程 + UI 专属 wire 信封（不复用工具信封）+ NamedPipe/token/对端镜像白名单 + 命令带 correlation / 事件单向推送；含 5 条被否决选项与 4 项验证方式。
- `docs/spec/ui-ipc-protocol.md`（Draft）：传输/握手、三个信封、六个 fail-closed 点、错误映射、7 条不变量、5 项验证清单。
- `tasks/TASK-213-ui-core-ipc-transport.md`（Ready）：Core 侧监听端 + UI 侧 client + 事件推送；**开工前置 = ADR-0057 转 Accepted**。
- 登记同步：`docs/adr/README.md` 下一个可用编号 → **0058**；`docs/memory/decisions.md` 追加 Proposed 决策条目；`docs/PARKING_LOT.md` PL-095 状态更新为「待人类接受 ADR」。
- 结论不变：**TASK-105 仍被 PL-095 阻塞，阶段 1a 仍为 NO-GO**。

**2026-09-30 —— TASK-087 CI 负向验证 + `check-comments` 硬门禁**

- `gate-selftest.yml`：新增 fmt / clippy / build 三类 N3 canary，正向基线 + 注入坏样本 + 具体退出码与故障文本断言 + 还原；首跑发现 Clippy 诊断渲染 lint 名用连字符，已把断言钉为 `-D clippy::unwrap-used` / `-D clippy::dbg-macro`，未放宽 exit 101 要求。
- `ci.yml`：`check-comments` 以 `[HARD #15]` 接入；TASK-212 已把 9 处真实 Error 清零，避免永久红灯。
- 验收：gate-selftest run `36598959358` 五个 canary job 全绿；PR #104 的 push / pull_request CI 全部 success；PL-018 关闭。

**2026-09-29 —— TASK-210 UI 与提交质量门禁**

- `apps/desktop-ui`：Prettier（`format:check`）、ESLint flat config（`lint` 改为 `eslint . --max-warnings 0`）、Vitest + jsdom + Testing Library（`*.test.tsx`）、commitlint 配置；13 个新 devDependency 已登记。
- CI：`desktop-ui` job 增加 format:check / lint / 三条 N2 负向验证；新增 `commit-lint`（PR 区间）与 `desktop-ui-tauri`（Windows，补 `src-tauri` 门禁盲区）。
- 顺带修掉两处真实缺陷：`egressPolicy.ts` 的死变量（ESLint 抓到）、`src-tauri/src/commands.rs` 未格式化（新 fmt 门禁抓到）。
- 验收：UI 五道门禁全绿（model 76 + DOM 3）；commitlint 正负样本 exit 0 / 1；`cargo test --workspace` 1009 passed / 0 failed。

**2026-09-29 —— TASK-104 UI ↔ Core typed IPC 与审批接线**

- `apps/agent-core`：`ui_ipc`（1.0 契约 + 未知字段/kind/版本 fail-closed + 事件投影）与 `ui_control`（approve/deny/pause/cancel/takeover → 真实 task-engine）。
- `apps/desktop-ui`：zod 双向校验、`sendUiCommand`、审批决定映射、时间线事件投影、`IntentLauncher` 首个真实调用点；`zod@4` 已登记批准。
- `apps/desktop-ui/src-tauri`：`send_ui_command` 本地校验 + `CoreCommandTransport` 边界（未注入传输时显式失败）。
- 两侧共享 7 个黄金样本；新增 **PL-095**（真实 UI↔Core 传输与事件推送待立卡 + ADR）。
- 验收：`cargo test --workspace` **1009 passed / 0 failed**；UI lint/typecheck/test（76）/build 全绿；src-tauri `cargo check/test` 本地通过。

**2026-09-29 —— TASK-103 真实任务执行器（Host 分发 + VerifyReceipt 接线）**

- `crates/verify`：新增不透明 `VerificationReceipt`（字段私有 / 无公开构造器 / 不实现反序列化，只有 `Verified` 可铸）与 `verify_postconditions_with_receipt`。
- `crates/task-engine`：`commit_step` 改为消费持有 receipt 的 `StepCommit`，不提供未验证提交重载。
- `apps/agent-core`：新增 `RuntimeExecutor`（Policy → Approval → Execute → Observe → Verify → Commit）、真实 `ToolBusInvoker`（`rmcp` 同进程 MCP 往返）与 `EnvelopeObservationCollector`。
- `apps/automation-host`：新增 `RequestDispatcher` / `run_host_with_dispatcher` / correlation 校验；默认 dispatcher 对 `Request` fail-closed。
- 验收：`cargo test --workspace` **999 passed / 0 failed**；fmt / clippy / hygiene / check-migrations / verify-schemas / codegen 全绿。

**2026-09-29 —— TASK-038 T1.3 新建标签 / 写入 / 跨进程另存为**

新增 `adapters/com.microsoft.notepad/tasks/t1.3.new-tab-write-save-as.json`：声明新建标签、Host `set_editor_value` 写入后读回、高-risk `once` Save As 审批、`#32770` 跨进程对话框身份校验，以及目标已存在/竞态创建时 `PolicyDenied` 且绝不覆盖。新增 `eval/tasks/notepad/t1.3/**`：14 个用例、确定性 expected、Python fixture 生成器与静态校验器；DRIFT-038-1/2 记录缺少注册 write 工具与自动安全覆盖路径。PR #94 以 merge `5860840` 合并，9/9 CI 全绿。

**2026-09-29 —— TASK-037 T1.2 替换 / 保存 / 审批 / L0+L1 撤销**

新增 `adapters/com.microsoft.notepad/tasks/t1.2.replace-save-approval-undo.json`：声明字面量全文替换、两段 `once` + `show_diff` 审批、`error_if_ambiguous` 目标解析、保存后标题/磁盘读回后置条件，以及 L0 保存前撤销与 L1 保存后恢复。新增 `eval/tasks/notepad/t1.2/**`：11 个用例、确定性 expected、Python fixture 生成器与静态校验器；静态校验同时确认任务只引用现有注册工具。PR #92 以 merge `6e9c85a` 合并，9/9 CI 全绿。

**2026-09-28 —— TASK-036 T1.1 任务包与评测集**

新增 `adapters/com.microsoft.notepad/tasks/t1.1.open-read-full-text.json`：只读任务声明包含输入/输出、普通 UIA 读与大文件 L1 降级、行数/关键词段落分析、postconditions 与错误映射。新增 `eval/tasks/notepad/t1.1/**`：10 个用例、确定性 expected、Python fixture 生成器和静态校验器；1 MB 目标准备为 1,295,000 bytes，要求 `text_source=file_channel`、`truncated=true`、全文件行数 105000 与 `truncated_prefix` 分析范围。DRIFT-036-1 记录缺少 `notepad.file.open` 注册工具。PR #88 以 merge `0b64f85` 合并，8/8 CI 全绿。

**2026-09-28 —— TASK-035 Notepad Adapter 声明式包 v0**

新增 `adapters/com.microsoft.notepad/**`：`adapter.toml` 声明 Win11 新版版本范围、`L3_a11y` 与 UIA 通道；`app_map.json` 提供 commands/ui_map/known_pitfalls/undo_capability，另有 core v1 投影 `memory/app_map.v1.json`；`selectors/targets.json` 覆盖 9 个目标且每个 ≥3 候选；`tools/tools.json` 声明 read/replace/save/new_tab/save_as 五个工具；rollback 同时声明 L0 `Ctrl+Z` 与 L1 快照；interrupts 对未保存三态框默认 Cancel。DRIFT-035-1/2 记录当前 ToolSchema 与富 App Map 的 schema 缺口。PR #86 以 merge `9a979f4` 合并，8/8 CI 全绿。

**2026-09-28 —— TASK-034 录制回放框架 v0**

新增 `crates/replay/**`：Recording v1 从 JSON 严格加载，拒绝未知版本、重复 handle、重复文本结果、孤儿父节点、环与悬空引用；离线 `WindowProvider` / `UiAutomationProvider` 复用 `assistant-platform-api` trait，歧义/缺失返回 `TargetAmbiguous` / `TargetNotFound`，未录制写动作与不支持的裁剪/过滤显式返回 `CapabilityMissing`。新增 `fixtures/recordings/core/notepad-like-basic.json` 与 16 个专项测试；`xtask replay` 兼容 v1 / legacy 形状并输出确定性摘要。PR #84 以 merge `167c3f2` 合并，16/16 CI 全绿；无新增第三方依赖，`Cargo.lock` 仅自动登记 workspace package。

**2026-09-28 —— TASK-033 notepad-like 靶机应用**

新增 `fixtures/apps/notepad-like/**`：使用 Windows PowerShell 5.1 内置 WPF，不引入 .NET SDK 或第三方依赖；主窗口、编辑区、工具栏、状态栏、busy 遮罩与注入弹窗均有稳定 `AutomationId`。CLI 支持 `none` / `disappear` / `timeout` / `ambiguous` / `dialog` / `busy` 六种故障，`--self-check` 真实解析 XAML 并逐控件校验 AutomationId。专项测试用 UIA 检查可见性、重复候选、busy 禁用状态和 dialog 运行时 ID，timeout 用 dispatcher 探针证明 UI 线程阻塞；7 类非法参数 fail-closed。PR #82 以 merge `8adb118` 合并，16/16 CI 全绿；CI 接线仍归 TASK-039。

**2026-09-28 —— TASK-032 策略 / 能力 / 成本面板**

新增 `apps/desktop-ui/src/features/policy/**`、`capability/**`、`cost/**`：出域策略支持 `local_only` / `redacted` / `full` 三档和逐应用覆盖，缺少本地模型时拒绝 `local_only`，升级到更宽级别必须逐次确认，状态栏常驻显示当前有效级别。Capability Matrix 视图严格解析运行时 probe、通道、能力风险/审批与降级，额外拒绝审批不一致和非法 UTC 时间；成本面板用整数 micro-USD 按本次任务 / 今日 / 本月与模型/应用聚合，拒绝超过 `Number.MAX_SAFE_INTEGER` 的值和非法 RFC 3339 时间，避免静默舍入或错位。26 个 Node 专项测试、UI typecheck/build、Rust workspace 与 xtask 门禁全绿；独立 review 的两轮测试覆盖见 PR 证据。

**2026-09-28 —— TASK-031 元素拾取器与目标绑定**

新增 `apps/desktop-ui/src/features/picker/**` 与 `binding/**`：拾取器对上屏快照做 fail-closed 校验，支持悬停/选中 bounds 高亮、稳定属性面板、候选链生成和 Adapter selector 草稿导出回调；候选首选必须非本地化且分数高于所有 fallback，`RoleAndParent` 在平台契约补齐 helper 语义前被显式省略。独立 review 找到的父候选误选 P0 与本地化越级、workspace 原点、最外层输入三条 P1 均已修复，19 个 Node 专项测试与 typecheck/build 通过；平台层未改动，剩余契约风险登记为 PL-094。

**2026-09-27 —— TASK-030 合并完成**

PR #74 以 **merge commit `4823cfd`** 合并到 `main`。新增 `apps/desktop-ui/src/features/approval/**` 与 `.../timeline/**`：审批卡完整覆盖 v2 §10.2，`app_content` 来源默认拒绝，高风险/不可逆范围收敛为 `once`，L3 增加二次确认输入；时间线覆盖 v2 §16.4，并在缺 Anchor、失败状态、不可逆或未接回调时禁用撤销/重放且显示原因。输入先经严格运行时校验，13 个 Node 内置测试覆盖正常、畸形、高风险、来源污染、撤销不可用与不安全证据链接路径；UI typecheck/build 与完整 Rust 门禁全绿，独立 review 修复后 16/16 远端 CI 全绿。

**2026-09-27 —— TASK-029 合并完成**

PR #72 以 **merge commit `21e35bc`** 合并到 `main`。新增 `apps/agent-core` 唯一 Host 装配点与可执行 `--self-check`，显式注入 storage / audit / model runtime / policy / tool-bus 组件，平台以 `WindowProvider + UiAutomationProvider` 约束的泛型注入并在非 Windows fail-closed；新增 `apps/desktop-ui` Tauri 2 + React/TS/Tailwind 壳、空 capabilities、无 `unsafe-inline` 的严格 CSP 与静态安全测试。独立 review 还闭环了审计链探针、App Map root escape、豁免解析 fail-closed 和跨平台测试/ lint 问题；push 与 pull_request 两个三平台 CI run 均全绿。下一张主线卡为 **TASK-030**。

**2026-09-27 —— TASK-208 合并完成**

PR #70 以 **merge commit `e8d95f2`** 合并到 `main`。新增 App Map v1 的路径/形状/版本校验、注入式文件读取与 FTS5 检索消费接口、按需 Memory projection 和显式 omission；来源引用由加载器生成，声明 token estimate 只能抬高保守字符数估算。独立 review 发现并闭环了预算绕过、来源伪造、omission 信息不足与测试文件超长问题，CI 全绿。下一张主线卡为 **TASK-029**。

**2026-09-27 —— TASK-206 合并完成**

PR #68 以 **merge commit `6da9007`** 合并到 `main`。新增 0004 迁移、contentless FTS5 候选索引、fail-closed 检索 API、来源引用与行缺失/孤儿/完整性自检；storage / audit / task-engine 的真实装配测试与全 workspace 验收全绿。独立 review 先后发现并闭环了未接装配、占位迁移假绿、热路径性能与 tokenizer 语义问题。下一张主线卡为 **TASK-208**。

**2026-09-27 —— TASK-207 独立 review：修复可修项，P1 契约缺口回退为 Review**

PR #66 的 8/8 CI 全绿后，独立 review 发现并暴露真实问题。已修复：Planner 反序列化后可重验 `StepId` / `StepTimeouts`；unknown tool 与模型非法 Plan 改用 `ModelInvalidOutput`；tool catalog 收紧 version/name/input/output/tag；ADR-0053 依赖扫描覆盖 renamed 与 target-specific dependency。仍未闭环的是 **DRIFT-207-1**：当前 `ToolSchema` 没有权威 `effect` / `reversibility`，Planner 无法校验模型自报的安全字段；完整修复必须扩展公共契约或引入独立工具元数据层，需 ADR。TASK-207 因此保持 **Review**，PR #66 不合并；TASK-206 暂缓。

**2026-09-27 —— ADR-0055：ToolSchema 权威 effect / reversibility**

人类确认「继续」后，ADR-0055 落地：`ToolSchema` 新增必填 `effect` / `reversibility`，`ToolDefinition::new` 必须显式声明，工具集指纹包含两者；Planner 的模型输出只接受意图字段，出现 `effect` / `reversibility` 即 `ModelInvalidOutput`，最终 PlanStep 的行为元数据只从可信工具目录注入。这样写步骤 postcondition 与 L3 `point_of_no_return` 校验不再信任模型自报值，`DRIFT-207-1` 闭环。TASK-207 仍为 Review，等待最终 CI、review 和合并。

**2026-09-27 —— TASK-207 合并完成**

PR #66 以 **merge commit `78acfed`** 合并到 `main`。最终 CI 8/8 全绿；两轮独立 review 的问题已由 `abbe1c7` 与 `43d33e8` 闭环，TASK-207 状态收口为 Done。下一张主线卡为 TASK-206。

**2026-09-26 —— TASK-207（`crates/core`：Planner）**

新增模型输出到 `assistant-task-engine` Plan / Step DAG 的严格转换：调用方持有 plan/task identity、goal、预算与 `ToolSchema` 目录，模型只能返回单一 `steps` JSON 数组。Planner 逐事件校验 Provider 流、传播取消并把单次 poll 限制在 500 ms 内；随后调用 `Plan::validate()`，再拒绝目录外工具。重复 id、缺依赖、依赖环、非法工具名、写步骤缺 postcondition、L3 未标 point-of-no-return，以及不可解析 / 空 / 类型错误 / tool-call / 无 finish 输出全部 fail-closed。新增 14 个 Planner 测试 + 3 个 ADR-0053 依赖白名单 arch 测试；`assistant-core` 行覆盖 88.60%，零第三方新增。

**2026-09-26 —— 治理：TASK-206 DRIFT-206-1 裁决（方案 A）+ ADR-0054「先落地，再自删」**

把 14:15 那次一次性自动化**搁浅在未 push 本地分支**（`d3ef5cb`）的证据回填进 `main`：TASK-206 的执行记录 9 节 + `DRIFT-206-1` + `TASK-206 Blocked` 的 `LEDGER.md` 行 + **PL-091** + pitfall 条目 + 轮次产物（按 ADR-0054 D5 加 `-run1415` 后缀，避开 16:15 轮已占用的 `round-1`）。同时落地 **DRIFT-206-1 方案 A**：TASK-206 write scope 增列 `docs/storage-design.md`（**仅 §3.4** 全局迁移登记表）→ 该卡**由 Blocked 回 Ready**。根因立 **ADR-0054**：① **先落地，再自删** —— 本轮产生留痕却没有可合并 PR 时，必须先开 docs-only PR 再自删（唯一例外 = 完全没产生留痕）；② **轮次编号只数 `main`**（`git ls-tree`，禁止数工作区；路径已占用则顺延、禁止覆盖）。章程 §11.3 + 新增 §11.12（v1.17）、手册 §4.1.b / §4.1.c（v1.14）同步。


**2026-09-26 —— TASK-028（`crates/core`：会话管理 + 上下文管理）**

新增 `assistant-core` 的会话生命周期、消息树、根到叶显式分支选择、Required / Summarizable / Droppable 三档裁剪、结构化摘要注入与 token 预算。必留内容超预算直接失败；被丢弃或摘要的历史都生成带原因与可恢复性的 `ContextOmission`，压缩失败 / 摘要来源不匹配绝不退化成静默丢历史。`SessionStore` / `SessionClock` / `HistoryCompressor` 全部注入，测试使用内存 store + 固定时钟，`assistant-core` 27 个测试、行覆盖 92.62%，零第三方新增。storage 当前没有 conversation/session 公开记录 API，已登记 DRIFT-028-6 + PL-092，生产 adapter 由 TASK-029 装配前解决。

**2026-09-26 —— 治理：DRIFT-028 五点落地（ADR-0053：`core` 编排层接口面 + 依赖白名单 + 拆卡 + 组装下沉 + spec）**

人类裁决「drift-028 的 5 点都按照你的建议做」→ ① **ADR-0053** 把 `crates/core` 定为**可装配的编排组件库**（**不是**装配层）：依赖白名单 = `protocol` / `storage` / `platform/api`（**仅 trait**）/ `task-engine`（Plan·Step DAG 类型）/ `model-gateway`（`ModelProvider` trait）；黑名单 = `platform/{windows,macos,linux}` / `tool-bus` / `policy` / `audit` / `hitl` / `verify` / `undo` / `lease` / `secrets` / `ipc` / `apps/*` / UI（理由：铁律 3 放行点唯一 + 装配单点在 binary + 依赖 DAG 无环）。② 新建 `docs/spec/core-orchestration.md`（四组组件接口面 + 10 条不变量 + 错误语义），**契约先行**（铁律 10）。③ **拆卡**：原 TASK-028（五合一）→ **TASK-028**（会话 + 上下文）/ **TASK-207**（Planner）/ **TASK-208**（Memory），「组装」下沉 **TASK-029**（binary = 唯一装配点）；FTS5 检索归 `crates/storage` 的前置卡 **TASK-206**（`libsqlite3-sys` 的 `bundled` 已带 `-DSQLITE_ENABLE_FTS5` → **零新增依赖**）。④ `crates/core/README.md` 与 `crates/core/src/lib.rs` 的职责段 / 不变量 3 同步改写为白名单口径 —— 消除「README 不变量 3 与『装配』职责互相矛盾、而 `arch_layering.rs` 拦不住」的**静默漂移**（DRIFT-028-4）。**未改任何产品代码**。

**2026-09-26 —— TASK-027（`crates/hitl`：ADR-0048 无损确认投影 + 审批 / 授权 / 接管 / diff）**

新增 `assistant-hitl`：ADR-0048 让 `PolicyDecision` 以可选 `scope_options` / `show_diff` 无损跨协议边界，并规定“非空 scope 的存在”是唯一 confirmation 判据。审批请求只从该 confirmation 构造；四维授权（subject / tool / target / effect）配正 TTL 与有限次数，五种 scope 各自显式匹配边界；高风险只允许 `once` 且必须一次。接管记录基线指纹，交还时无论指纹是否相同都要求重新解析目标与重同步；暂停/恢复直接委托 task-engine。diff 数据覆盖文本 LCS、字段、文件和 UI 步骤，超预算返回错误而不静默截断。新增 18 个契约测试，行覆盖 77.93%，零新增第三方依赖。

**2026-09-26 —— TASK-026（`crates/model-gateway`：Provider 流 / 路由 / fallback / 成本）**

新增 `assistant-model-gateway`：Provider 契约统一流式事件、取消、usage、能力与定价；路由器按 stage / context tokens / sensitivity / remaining budget / tool count 的优先级规则选择 primary，并按配置降级链切换。重试使用指数退避和注入 jitter，错误分类明确区分“可同模型重试”与“可降级”；一旦产生 text 或 tool-call delta，任何后续失败都返回 `PartialOutput` 且不重试。prompt-cache 稳定前缀只透传、不改写；成本按 fresh/cached/output 三类整数 micro-USD 分量向上取整，记录 model/tokens/cost/latency/cache_hit。新增 20 个契约测试 + 1 doctest，零新增第三方依赖。

**2026-09-26 —— TASK-025（`crates/lease`：目标租约 + TTL / 续租 / 用户抢占 + 零提交批量获取）**

新增 `assistant-lease`：`Shared` / `Intent` / `Exclusive` 三模式，跨 owner 只允许一个写租约；TTL 到期边界明确，支持续租、懒清理与 `reap_expired`；用户接管按 target key 强制释放全部 agent 租约并把自然过期项单列。`acquire_many` 在任何 ID / 状态写入前完成规范 key 排序、去重、冲突与溢出预检，失败零提交，消除部分持锁与锁顺序死锁窗口。新增 20 个测试，零新增第三方依赖。

**2026-09-26 —— TASK-024（`crates/undo`：可逆性四级 + 锚点 + 回滚剧本 + 冲突检测 + incident）**

新增 `assistant-undo`：L0~L3 可逆性、内容快照 / 影子副本 / undo 预算 / 补偿剧本锚点、L0→L1 自动 fallback、默认最保守的冲突检测和结构化 incident。回滚动作经 `RollbackExecutor` 注入执行，最终指纹必须等于锚点 pre 指纹；缺证据绝不猜测，用户改动只有显式 `RestoreOverall` 才可越过。新增 23 个测试，零新增第三方依赖。

**2026-09-25 —— TASK-023（`crates/verify`：后置断言引擎 + 状态指纹 + 幂等判定）**

新增 `assistant-verify`：架构 v2 §7.4 的 **11 种断言**（= §7.4 的 12 种减 `visual_assert` —— 截图 / 感知哈希 / 容差归 TASK-042；附录 A 的 `target_resolvable` / `capability` 是 §5.3 的 **precondition**，解析期拒绝并说明归属）全部 fail-closed 解析：未知 `kind`、多余字段（拼写错误）、缺字段、类型错误、坏指纹、空 selector、`min > max`、`within_ms = 0` 一律拒绝，绝不静默跳过。求值是**三值**的 —— `Satisfied` / `Falsified` / `NotEvaluable`；只有全部满足才是 `Verified`，任一 `Falsified` 即 `Violated`，无 `Falsified` 但有 `NotEvaluable` 即 `Inconclusive`，**`Violated` 与 `Inconclusive` 都带 `ErrorCode::VerifyFailed`**，因此「验证失败却返回 ok」在类型上不可表达（§7.4 第一红线）。未观测（未探测元素 / 未记录文件 / 缺 previous 指纹 / 缺 elapsed）一律 `NotEvaluable`，绝不当作「不存在 / 未变化 / 满足」；字符串断言对上数值观测是「问错了量」→ `NotEvaluable` 而非 `Falsified`。§7.3 状态指纹用长度前缀 canonical form + SHA-256（分隔符注入无法伪造碰撞），忽略字段由 per-adapter 显式给出、**无全局默认**（否则光标闪烁会让每个动作都「有变化」）；§8.5 幂等判定缺 before/after 一律 `Unknown`、应用自报（L1）优先于指纹差异；`on_violation` 的 `retry_once` 收窄到 `Transient` / `TargetNotFound`，其余升级给用户（`VerifyFailed` **不**在此重试：`ErrorCode::retryable()` 回答的是「任务能否重试」，不是「同一调用能否重发」）。新增 66 个测试 + 1 个 doctest；`serde` / `sha2` / `thiserror` 只新增使用方并同批登记。`assert` 自由字符串不实现（= 表达式语言 = 新抽象层），记 `DRIFT-023-4` / **PL-086**。

**2026-09-25 —— TASK-204（`crates/tool-bus`：draft-07 关键字判据硬化）**

把「除白名单外一律拒绝」这条**隐式**判据**显式化**：新增三张带理由的表 —— `REJECTED_FOREVER_KEYWORDS`（12 条**永久放弃**：`$ref`/`definitions`、`pattern`/`patternProperties`/`format`、`if`-`then`-`else`/`dependencies`、元组 `items`/`additionalItems`、`contentEncoding`/`contentMediaType`）、`REJECTED_FOR_NOW_KEYWORDS`（4 条**暂未实现**：`multipleOf`/`uniqueItems`/`contains`/`propertyNames`）、`NON_DRAFT07_KEYWORDS`（15 条**其它方言**：`$defs`/`$dynamicRef`/`prefixItems`/`unevaluatedProperties` …），加未知关键字兜底。报错分四类标签（`never-supported` / `not-yet-supported` / `other-dialect` / `unknown`）且每条带 JSON pointer。

`$schema` 由「注解」改为**方言校验**：缺省或 draft-07 = 放行，声明**别的方言 = 拒绝**（消息含实际声明的方言）—— 否则会拿 draft-07 的语义去校验一份 2020-12 文档，那正是铁律 1 禁止的「用自己的语义冒充别人的语义」。`format` / `pattern` 按人类 2026-09-25 裁决归入**永久放弃**并写明替代（`enum` / `const` 收窄取值，或**由 handler 校验值**）。`SUPPORTED_KEYWORDS` 的 21 条**一字未改**，零新增依赖、零 `unsafe`、零 `#[allow]`。新增 9 个测试（表两两不相交 / 表内不重复 / 四类标签与理由 / 嵌套 pointer / 方言 4 接受 3 拒绝 / 未知关键字 typo 提示 / 深度上限仍 64）；`cargo test --workspace` = **371 passed**，`hygiene` 0 error。同批把 automation 调度器的 `COUNT=1` **实测反转**为「**真一次性**」（`docs/nightly/codex-automations-operations.md` §4.1.a 更正块：**必须同时带 `BYHOUR` + `BYMINUTE`**，且**不要**用 `COUNT≠1` / `UNTIL`）。

**2026-09-25 —— TASK-022（`crates/task-engine`：任务状态机与恢复）**

新增 `assistant-task-engine`：架构 v2 §8.1 的 12 个 Task 状态与 Step 生命周期均有显式迁移表，非法 `(state, event)` 直接拒绝。Plan DAG 校验重复 id / 缺失依赖 / 环 / 非法工具名 / 写步骤缺 postcondition / L3 未标 point-of-no-return，并返回确定性 ready frontier。每次成功迁移先持久化完整快照再返回；`MemoryCheckpointStore` 供纯逻辑测试，`SqliteCheckpointStore` 只走 `assistant-storage` 公开记录 API，并在同一事务写入初始 task 与 checkpoint。崩溃恢复对 `Executing` / `Verifying` 只接受 `Completed` / `NotCompleted` / `Unknown` 外部证据：完成则提交、未完成则重做、`Unknown` 或缺证据则 `NeedsHuman`。预算覆盖步数 / 时长 / token / 成本，看门狗覆盖 resolve / execute / verify 三段超时。新增 43 个测试 + 1 个 doctest，`cargo llvm-cov -p assistant-task-engine --fail-under-lines 85` 实测 **87.03%**。storage 暂无 task 行更新 API，最新状态以 checkpoint 为准，遗留记为 **PL-083**。

**2026-09-25 —— TASK-021（`crates/policy`：唯一策略放行点）**

新增 `assistant-policy`：规则集用 JSON DSL v0 表达架构 v2 §12.2 的五条示例规则，求值全程无 IO、时钟、随机与全局可变状态。任一命中的 deny 压过 allow / confirmation；`L3 + unattended` 与污点上下文中的 L3 / High / Critical 由安全底线先行拒绝，自定义规则不可覆盖；空规则集或未命中一律 `default_deny`。参数护栏覆盖路径穿越 / 编码逃逸 / UNC / 设备名 / resolved-root containment、URL scheme / host / IP / port、文本长度 / 行数 / 控制字符、数值范围和正则 lookaround / 回引 / 嵌套量词；不支持即拒绝。新增 29 个测试 + 1 个 doctest，`cargo llvm-cov -p assistant-policy --fail-under-lines 85` 实测行覆盖 **93.60%**，判定中位数 **3.6~5.6 µs**。`PolicyDecision` 的 confirmation 投影缺口已记录为 `DRIFT-021-1` / PL-082。

**2026-09-25 —— TASK-020（`crates/tool-bus`：MCP 工具通道）**

新增 `assistant-tool-bus`：内置工具以**同进程 MCP server** 暴露，Agent Core 侧是 MCP client，传输用 `tokio::io::duplex` 的内存管道（无 socket / 无子进程 / 无网络），因此「内部工具也以 MCP 表达」不需要 stdio 或 http。`tools/list` 与 `tools/call` 全链路有测试证据（`test_mcp_round_trip_lists_and_calls_tool`）；调用身份经 MCP `_meta` 跨边界传递，服务端从 `RequestContext.meta` 读回 `task_id` / `step_id` 写进信封。
参数按 `ToolSchema.input` 走手写的 draft-07 **子集**校验（Q2 选 (c)；(a) `jsonschema` 会把传递依赖 `borrow-or-share`(MIT-0) 带进依赖图，而 `deny.toml` 白名单不含它 —— 放宽白名单是漂移触发器 ⑥），**不在支持清单里的关键字一律拒绝注册**（fail-closed）；非法参数返回 `ErrorCode::ToolInvalidArgs` 且**永不进入 handler**（负向用例断言计数器仍为 0）。
返回一律是 `assistant_protocol::ToolEnvelope`：`untrusted` 内容强制带 `source`，超 `max_bytes` 截断并显式写 `reason` / `original_bytes`，截不动（如键名开销就超预算）则 fail-closed 成 `Fatal` 失败信封。工具集指纹对「名字 + 版本 + 描述 + 风险级 + 规范化 schema」取 SHA-256（顺序无关），两个元工具 `toolset.list` / `toolset.search` 提供按需检索，单次挂载 > 40 个工具产出结构化 `ToolsetOversizeWarning` + 未串链 `AuditEvent`。
本卡引入 `rmcp` 3.4 / `tokio` 1 / `thiserror` 2 并逐条登记 `docs/DEPENDENCIES.md`（Q1 / Q4 / Q5）；`cargo deny check` 四项全 ok。新增 `crates/tool-bus/README.md`（职责 / 边界 / 不变量 / **已知限制**）与 8 个集成测试（新基线：`cargo test --workspace` = 43 target / 644 passed）。

**2026-09-25 —— TASK-019（`crates/ipc` + `apps/automation-host`：Host IPC 传输层）**

新增 `assistant-ipc` 与 `assistant-automation-host`：线上帧按 spec 固定为 `magic + envelope_size + payload + CRC32`，
超长 / magic / CRC / 版本 / token 五类错误均带 `ErrorCategory`；握手用 `ClientHello` / `ServerHello` / `Heartbeat`，
token 只作为传输层认证材料伴随发送，不改协议定义。Windows 端以 NamedPipe 实现传输，拒绝远端客户端，并以
`GetNamedPipeClientProcessId` + `QueryFullProcessImageNameW` 做默认拒绝的对端镜像白名单；双向心跳与看门狗在静默时明确报错。
真实验收由测试启动 host 子进程，完成握手、心跳，再 kill host 并确认客户端在 2 s 内检测断连；非 Windows 两个目标的 clippy 同样全绿。
新增 `handle_discipline` 契约测试，禁止平台实现依赖与元素句柄类型进入 IPC 载荷层。

**2026-09-25 —— TASK-018（`crates/platform/windows`：合成输入 + 坐标归一化 + IME）**

TASK-018 把铁律 5 的 **L4（合成输入）** 层补齐：`SendInput` 封装（VK 路径 + `KEYEVENTF_UNICODE` 文本路径 —— 后者把
UTF-16 码元直接投进输入队列，**绕过 IME 组字与键盘布局**，所以「IME 开着也能正确写入」）、**发送前 100% 校验前台窗口**
（不一致 → `SetForegroundWindow` + **回读**，仍不一致 → `TargetUnresponsive`，**绝不盲发**）、`SendInput` 返回值逐次校验
（UIPI `ERROR_ACCESS_DENIED` → `PlatformPermission` / 87 → `ToolInvalidArgs` / 队列被阻塞 → `TargetUnresponsive` / 未识别码 → `Fatal`）、
显示器枚举 + `GetDpiForMonitor` / `GetDpiForWindow` → **实际**显示器的 `CoordinateSpace`、`NormalizedPoint` → `PhysicalPoint`
（**只经** `CoordinateSpace::to_physical`）、虚拟屏幕 `0..=65535` 归一化、`is_ime_open` 两级查询。
真机验收 **2/2**：坐标精度 **0 px**（判据 ≤ 2 px）、真实记事本写入「中文abc」+ `Ctrl+S` **磁盘回读**通过。
纯逻辑（键名映射 / 组合键顺序 / UTF-16 代理对 / DPI → 缩放 / 显示器半开区间命中 / 归一化 / 失败分类）刻意留在
**三平台编译**的 `mod.rs` → **ADR-0045** 的两条非宿主 `--target` clippy 也覆盖得到（这是 PL-070 教训的第一次兑现）。
新提 **PL-074**（`pointer_action` 签名不带目标窗口 → 混合 DPI 多屏下逻辑点无法唯一归属显示器；跨显示器拖拽终点偏差）。

**2026-09-24 —— 阶段 1 地基层 + 平台抽象层 + Windows UIA provider + 治理池收口 + 护栏补齐 + TASK-016 / TASK-017 遗留裁决**

阶段 1 的地基层已经落地（含密钥层），治理池把 TASK-013 现场撞出的三个**结构性**缺陷一次性收口，
`xtask` 护栏同批补齐并把两条 CI 门禁由软转硬。
同批把 TASK-016 留下的 4 项治理遗留收口：**ADR-0042**（能力命名三分：`CapabilityCatalog` / `CapabilityMatrix` / `CapabilityEntry`）+
`MEMORY.md` §1 快照指针化（手抄派生进度改为指向 `PLAN.md`）+ `plans/*` 头部进度句纳入 `AGENTS.md` §11.1 的更新职责 +
4 份 schema 的悬空 `TASK-103` 前缀清除（PL-064 / 065 / 066 / 067 全部闭环）。

| 卡 | 内容 | 状态 |
|---|---|---|
| TASK-011 | `crates/protocol`：JSON Schema → Rust/TS 类型 codegen；`codegen --check` 变成**真门禁**（drift 会 exit 1） | ✅ Done |
| TASK-012 | `crates/storage`：SQLite WAL + 迁移框架 + blob 池（zstd + sha256 内容寻址） | ✅ Done |
| TASK-013 | `crates/audit`：append-only + SHA-256 hash chain + ring buffer 批量 flush（摊销 < 1 ms/条） | ✅ Done |
| TASK-200 | `docs/spec/*` 7 份契约草案的系统性结构缺陷（外来模板块 / 空节 / 断链引用）—— PL-038 闭环 | ✅ Done |
| TASK-201 | `crates/core` 骨架提前（PL-037 闭环） | ✅ Done |
| TASK-202 | 存储**迁移注册表**：storage 只提供机制、各 crate 自持迁移 + 唯一装配点（**ADR-0038**，PL-046 闭环） | ✅ Done |
| TASK-203 | `audit_logs` 列语义去重 + 显式链序：删与 `id` 同义的 `hash`、加 `sequence`（**ADR-0040**，PL-043 / PL-045 闭环） | ✅ Done |
| TASK-014 | `crates/secrets`：OS keychain 封装（`keyring` 4.2 / DPAPI·Keychain·Secret Service）+ `zeroize` + 访问审计注入点（fail-closed） | ✅ Done |
| TASK-016 | `crates/platform/api`：**铁律 7 的唯一平台入口** —— 4 个纯类型（`TargetDescriptor` / `NormalizedPoint` / `Fingerprint` / `CapabilityMatrix`）+ 3 个 trait 形状（RPITIT，**零 `async-trait` 依赖**）+ 能力矩阵 4 条不变量的机器校验；`ResolvedWindow` / `ResolvedElement` = 不透明句柄（铁律 8 有源码扫描断言，含负向样本） | ✅ Done |
| TASK-015 | `xtask` 护栏补齐：`docscan` 4 条结构规则（裸 NUL / 数字节号重复 / 整节为空 / 标题文字重复）+ `crates/core/tests/arch*` 分层断言（`arch::` 由 0 → 5 个测试）+ `check-ledger`（ADR-0039 D3）+ `check-migrations`（PL-047）；同批修 PL-048（flaky）与 PL-051（裸 NUL） | ✅ Done |
| TASK-017 | `crates/platform/windows`：**铁律 7 在 Windows 侧的落地** —— `UiAutomationProvider` 全 9 方法 + `WindowProvider` 全 5 方法（UIA 树快照 / selector 链解析 / `read_text` / `set_value` / `edit_text` / `invoke_action` / `bounds` / `fingerprint` / 窗口枚举与状态）；句柄纪律（铁律 8）有源码扫描断言 + 5 个负向样本；全 crate 零 `serde`。7 条 DRIFT 已全部裁决接受（含 crate 级 `unsafe_code` 放行、`crates/platform/api` 补 `SelectorChain` re-export），PL-068 / 069 / 070 闭环（**ADR-0043 / 0044 / 0045**）| ✅ Done |
| TASK-018 | `crates/platform/windows`：**铁律 5 的 L4（合成输入）落地** —— `SendInput` 封装（VK 路径 + `KEYEVENTF_UNICODE` 文本路径，后者绕过 IME 与键盘布局）、发送前 **100% 前台窗口校验**（不一致 → 置前 + 回读，仍不一致 → `TargetUnresponsive`，绝不盲发）、`SendInput` 返回值逐次校验（UIPI / 87 / 队列被阻塞 / 未识别码四分类）、显示器枚举 + `GetDpiForMonitor` / `GetDpiForWindow` → **实际**显示器的 `CoordinateSpace`、`NormalizedPoint` → `PhysicalPoint`（只经 `CoordinateSpace::to_physical`）、虚拟屏幕归一化、`is_ime_open` 两级查询；纯逻辑留在三平台编译的 `mod.rs`（ADR-0045 的两条非宿主 `--target` clippy 覆盖得到）；真机验收 2/2（坐标误差 0 px / 记事本 Unicode + Ctrl+S 磁盘回读）；新提 PL-074 | ✅ Done |

TASK-016 把**平台抽象层**立起来：`core` / `Host` 从此只能经 `assistant-platform-api` 的 trait 用平台能力
（铁律 7），且**该 crate 全 crate 零 `#[allow]`、零 `unsafe`、零第三方依赖**（除已登记的 `serde`）——
`f64 → i32` 这类"`as` 会饱和 + pedantic 会拦"的转换改用整数域逐位合成，越界一律报 `TargetNotFound`。

治理机制同步前进：**ADR-0039** 把「卡 Done = 同一 PR 内同步 `PLAN.md` + `README.md`」写成硬契约
（DRIFT-202-2 闭环），其机器判据 `check-ledger` 已由 **TASK-015** 落地并**由软门禁转硬**（CI 的 `[HARD #16]`）；
同批把 gov §5.1 **#5 arch test** 也转硬 —— 它此前是 `running 0 tests` 的假绿，现在是真的 5 个断言。

TASK-017 把**平台抽象层**在 Windows 侧兑现：`assistant-platform-windows` 提供 UIA 树快照、selector 链解析、
读写与动作、窗口枚举与指纹，错误一律映射成带 `ErrorCode` 的 `PlatformError`（铁律 1）；
**L4 合成输入（`SendInput`）与 IME 仍留空并显式返回 `CapabilityMissing`**，归 TASK-018（铁律 5 的层次不能跳）。
真机实测暴露一个**架构层缺口**（记 **PL-068**）：受 TASK-016 冻结的 trait 形状所限，`resolve_element` 只能
从**桌面根**发 `FindAll(TreeScope_Descendants)` → 中位数 **1.53 s**（窗口子树内同一操作只要 30~40 ms）。
本卡**没有**偷偷改搜索策略去掩盖它，而是把证据写进 `docs/PARKING_LOT.md` 等裁决。

TASK-017 的遗留裁决同批收口：**ADR-0043**（元素解析必须有 scope —— `resolve_element` / `wait_for` 的首参改为
`&ResolvedWindow`，搜索起点 = 该窗口的 UIA 根；桌面根搜索在元素解析里彻底消失，PL-068 / DRIFT-017-7 闭环）、
**ADR-0044**（歧义策略收敛为唯一 `ErrorAndAsk`，删掉在候选链模型里不可实现的 `HighestScore`；架构 v2 §6.6 其余
3 种策略写明归属层，PL-069 闭环）、**ADR-0045**（把「非宿主平台」`--target` clippy 写进 `AGENTS.md` §6 验收清单 ——
`clippy` 不链接故不需要 C 交叉编译器，但 `--workspace` 会撞 `cc-rs`，须按 crate 指定；PL-070 闭环）。
同批新提 PL-071 / PL-072 / PL-073（ADR-0035 allow 表缺口 / `*Regex` 命名与子串语义不一致 / 卡片状态行归属机制）。

### 历史小节（按时间倒序）

- **2026-09-19 TASK-059/060/061/062/063/064（xtask 护栏升级六连发）**：
  `xtask` 从 4 个子命令扩到 7 个（新增 `refscan` / `docscan` / `card-check`），
  原 `hygiene / memory-counts / adr-index / guard` 保持；全部在 CI 硬门禁 #12b（`doc-consistency`）下统一跑过。
  `docscan` 扫破表（PL-031）+ setext 风险 + UTF-8 BOM/CRLF/缺末行 LF；`card-check` 是 ADR-0031 D6 的机器化；
  同一批清掉了 `xtask/src` 生产代码里的全部 per-line `#[allow]`（**ADR-0034** / **ADR-0035**）。
### 历史小节（按时间倒序）

- **2026-09-19 TASK-064**（promote `extension_is` helper + 收 repowalk.rs:172 per-line allow）:
    - 新增 `pub fn extension_is(path: &Path, expected: &str) -> bool` 到 `xtask/src/repowalk.rs`（紧邻 `has_rust_extension`）；同步把 refscan.rs 的 local `ext_is` closure 删掉、import 此 helper
    - `collect_repo_files_recursively` 内 2 处 `lower.ends_with(...)` 改 `extension_is(&path, ...)`；删函数级 `#[allow(clippy::case_sensitive_file_extension_comparisons)]`
    - 加 4 条 `extension_is` 单元测试：positive match / case-insensitive / 非 UTF-8 返回 false / 无扩展名返回 false（cargo test 279 → 283 passed）
    - **副作用**（行为变化）：`.ends_with(&format!(".{ext}"))` 在 caller 传大写 ext（如 `"MD"`）时会漏匹配，`extension_is` 修复了此 bug —— **现状** caller 全部传小写（card_check `["md"]` / docscan `["md"]` / refscan `["md","rs","ps1"]`）故无回归；**测试覆盖** test_extension_is_is_case_insensitive 显式断言 4 种大小写组合
    - ADR-0035 §1 baseline 表补登 `repowalk.rs:172` 已清 + §决策 2 加一条「抽到 `pub fn` 替代 per-line allow 必须同步登记」（教训：TASK-063 提升到 helper 后才发现 repowalk.rs:172 还有同型 allow）
    - **真正的最终态**：全 xtask/src 生产代码 per-line allow = 0（refscan:0 + repowalk:0 + docscan:0 + card_check:0 + exemptions:0；保留 = test wrapper 4 处合法 + card_check 9 + exemptions 1 共 10 处 `#[allow(dead_code)]` 占位常量）
    - 详见 `tasks/TASK-064-promote-extension-is-helper.md` 执行记录 9 节 + LEDGER.md 2026-09-19 行
- **2026-09-19 TASK-063**（xtask refscan 清最后 3 处 per-line allow — 达到「生产代码 per-line allow = 0」）:
  - refscan.rs:94 `#[allow(clippy::single_char_pattern)]` → `replace("\r", "\n")` 改 `replace('\r', "\n")`（char 字面量，触发 Pattern impl 即可）
  - refscan.rs:101/103 `#[allow(clippy::case_sensitive_file_extension_comparisons)]` → `lower.ends_with(".md"/".rs"/".ps1")` 改 `Path::extension().and_then(to_str).is_some_and(eq_ignore_ascii_case)`（closure `ext_is` 复用）
  - 删除 `let lower = rel_path.to_ascii_lowercase();` 与配套注释（5 处出现 → 0）
  - 行为不变：refscan 仍报 151 errors（baseline 一致）；UTF-8 非合法扩展名按 `to_str` 失败语义 = 与原本 `ends_with`(`&str`) 在非 UTF-8 路径上失败同形
  - **复核 GAP**：repowalk.rs:336 `#[allow(clippy::case_sensitive_file_extension_comparisons)]` **未在本卡 scope 内**（卡面标题与 In scope 严格限制 refscan.rs），且 ADR-0035 baseline 表也漏列；建议下一卡 `TASK-064` 或新卡处理（统一改 `Path::extension` 模式）
  - 详见 `tasks/TASK-063-clear-last-3-per-line-allows.md` 执行记录 9 节 + LEDGER.md 2026-09-19 行
- **2026-09-19 TASK-062**（xtask render() 返回 Result — 消除 per-line allow）:
  - `pub fn render(findings: &[Finding]) -> String` → `pub fn render(findings: &[Finding]) -> Result<String, std::fmt::Error>`
  - 删除 `#[allow(clippy::unwrap_used, clippy::expect_used)]` per-line allow（TASK-060 遗留）
  - `writeln!(..).expect("...")` → `writeln!(..)?` + `.map_err(|e| e.to_string())?` 在 `run()` 中
  - 行为不变（writeln! to String 永不失败；Result 类型强制 caller 处理 = 编译期保证）
- **2026-09-19 TASK-061**（xtask lint cleanup pass 2 — TASK-060 遗留 5 处 str[Range] + 1 处 stale dead_code + 3 处 test f[0]）:
  - card_check.rs 5 处 str[Range] → 全替换为 `.get(range)` + `?` operator 重构
  - refscan.rs:290 stale `#[allow(dead_code)]` 删除（render 已被 run() 调用）
  - docscan/card_check 测试代码 `f[0]` → `f.first().expect("non-empty")`（3 处）
- **2026-09-19 TASK-059 / TASK-060**（xtask 护栏升级 + 纯重构）：
  - TASK-059 cherry-pick 悬空 commit `10f78db` 收回 → 4 子命令可执行 + ADR-0034 注册
  - TASK-060 人类裁决 B 路（漂移不可忍受）= 纯重构 = 4 模块顶部 `#![allow(...)` 块全清
    （**DoD 硬证据**：`grep '^#![allow' xtask/src/{refscan,docscan,card_check,exemptions}.rs` = 0 命中）
    + 32 处 `indexing_slicing` 用 `while let Some + .get(n).expect()` 全替换
    + `docs/PARKING_LOT.md` PL-NEW 关闭
  - 防御性 PITFALL：`docs/memory/pitfalls.md` 追加「禁止 sub-card 后缀；commit 标题必须对应 `tasks/TASK-NNN-*.md`」
  - 详见 `LEDGER.md` 2026-09-19 三行 + `tasks/TASK-059-...md` / `tasks/TASK-060-...md` 执行记录
- **2026-09-17 TASK-001**：仓库骨架 + CI 8 硬门禁 + ADR 编号登记表建立

## 许可证

`MIT OR Apache-2.0`（见 `LICENSE`；M5 已按双许可裁决落地）。

> 说明：内部项目，暂不公开。双许可 `MIT OR Apache-2.0` 已是现行许可证决定，不再处于待裁决状态。
> 开源前需要完成脱敏，清单待建（见 `docs/PARKING_LOT.md` PL-005）。
