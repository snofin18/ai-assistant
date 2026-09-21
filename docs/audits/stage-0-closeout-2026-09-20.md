# Stage-0 Closeout Audit — 2026-09-20

> **第一份正式审计文档**。模板与本审计同时落地；后续 closeout（stage-1/2/...）照此格式。
> 本审计为"阶段末对齐审计"（gov §7.2）的首次实施。
> **审计 agent**：Codex（TASK-073，2026-09-20）　**审批人**：待人类　**下次审计**：stage-1 closeout（预计 2026-12 ~ 2027-01）

## 摘要

| 维度 | 数值 |
|---|---|
| 周期 | 2026-09-16 ~ 2026-09-20（**4 天**；原估 2~3 周） |
| 产出类型 | 文档 + xtask 护栏 + spike 探针（**零产品代码**，符合 AGENTS.md §6） |
| 任务卡数 | 73 张（TASK-001 ~ TASK-073） |
| DoD 达成 | **3 ✅ + 1 ❌ OBSOLETE + 4 ⚠️**（详 §2） |
| 已关闭 PARKING_LOT | 12 条（PL-001/009/011-015/019/020/022/026/027/029/033/036） |
| 新增 ADR | 14 份（0018~0037，跳号由 ADR-0026 解释） |
| L1 记忆 | facts 72 / pitfalls 62 / rejected 31 / decisions 51 / open 26 条 |
| **结论** | **stage-0 治理基础完成；spike 实测 carry-over 至 stage-1；stage-1 开工** |

## 1. 范围与周期

- **范围**：阶段 0（per `plans/stage-0-spikes.md` In/Out scope）
- **Out of scope**（per plans）：任何 `crates/*` 产品代码、Paint/Excel/Photoshop Adapter、macOS/Linux 实现、无人值守、UI 完整实现
- **周期**：2026-09-16 ~ 2026-09-20
- **参与 agent**：Codex / opencode / Claude Code（含人类会话 5+）

## 2. DoD 达成盘点（详 `plans/stage-0-spikes.md:25-34`）

| # | DoD 项 | 状态 | 证据 / carry-over 卡片 |
|---|---|---|---|
| 1 | 8 个 Spike 报告齐全 | ⚠️ **1/8**（SPIKE-A PARTIAL） | `docs/spike-reports/SPIKE-A.md`；其余 7 份 = **stage-1 carry-over**（A2/B/C/E/F/G/H，详 `tasks/TASK-003~010-*.md`） |
| 2 | 每个 no-go 判据明确 | ⚠️ **N/A** | 无 spike 报告 → 无 no-go；触发条件未满足 |
| 3 | 结论回填 L1 记忆 | ⚠️ **未达成** | spike 缺 → 缺结论；回填目标 = `docs/memory/{facts,rejected,pitfalls,open}.md`（ADR-0021 L1 分层后） |
| 4 | 首批 ADR 0001~0015 | ❌ **OBSOLETE** | ADR-0026 W4 改号；现有 0018~0037 共 20 份 |
| 5 | docs/spec/ 7 份契约草案 | ✅ | **TASK-072** |
| 6 | 阶段 1 任务卡就位 | ✅ | **TASK-035 / TASK-036~058**（PL-036 落地） |
| 7 | CI 最小门禁 | ✅ | **TASK-001**（fmt + clippy + test 三件套） |
| 8 | spike no-go → 升级机制 | ⏸️ **未触发** | 机制在 ADR + plans；spike 实测开工后自动生效 |

**最终判定**：3 项完整 ✅ + 1 项 OBSOLETE ❌ + 4 项 ⚠️ 延期。本审计签字后，stage-0 视为 closeout；spike 实测归 stage-1。

## 3. 治理闭环盘点

### 3.1 已关闭 PARKING_LOT 项（12 条）

| ID | 主题 | 关闭方式 |
|---|---|---|
| PL-001 | hygiene 12 vs 11 | ADR-0025 D4 统一为 13 |
| PL-009 | arch v1/v2 共存 | 风险下降（本卡 §6 列迁移建议） |
| PL-011 | CRLF 规则 | hygiene 末行换行升级 |
| PL-012~015 | 夜间自动化机制层 | rejected/facts/open 登记 |
| PL-019 | 补 dependency 登记流程 | ADR 机制 |
| PL-020 | hygiene 14 项 | ADR-0025 → 13 项 |
| PL-022 | 手抄派生值 | ADR-0030 机器校验 |
| PL-026 | probe-01/02 非 ASCII | 全部翻译 + E-021/022/023 豁免 |
| PL-027 | 末行换行 | docscan 11→0 |
| PL-029 | main table 超限 | 同步 |
| PL-033 | 行数口径 | ADR-0033 双层护栏 |
| PL-036 | stage-1 46 张卡片 | 全部生成 |

### 3.2 本审计关闭

- **PL-NEW**（PARKING_LOT line 80/81 PL-NEW 重复）：line 80 加 `[supersedes:2026-09-20]` 标注，line 81 删

### 3.3 仍待评审（转 stage-1）

| ID | 主题 | 建议归属 |
|---|---|---|
| PL-002 | check-comments/check-ledger/card-check 未认领 | TASK-015 |
| PL-003 | xtask/report.rs 悬空引用 testing.md | **已落地**（TASK-072） |
| PL-004 | 反引号豁免 | TASK-015 |
| PL-005 | OPEN_SOURCE_CHECKLIST.md 缺失 | 开源准备阶段 |
| PL-006 | cargo-deny 本地未装 | **已落地**（LEDGER 2026-09-17） |
| PL-007 | .gitkeep 冗余 | TASK-015 |
| PL-008 | git 提交身份占位 | 开源准备阶段 |
| PL-010 | LEDGER commit 占位 | pending 合法 |
| PL-016/017 | 环境配置 | **已落地** |
| PL-018/021 | context window + 模型 catalog | **已修复** |
| PL-023 | 夜间自动化两个未定项 | 仍 open |
| PL-024 | create_thread 上游跟踪 | 跟踪中 |
| PL-025 | gov §10 落地清单脱节 | **已更新** |
| PL-028 | 裸 ADR 引用 | ADR-0030 D4 推迟 |
| PL-030 | adr-index 子命令建议 | **已实现**（ADR-0030 D3） |
| PL-031 | 破表裸竖线 | TASK-015 |
| PL-032 | PL-030 另一半推迟 | TASK-015 |
| PL-034 | 范围写法 | TASK-015 |
| PL-035 | arch-v2 其它手抄值 | TASK-070 已修（"改对数字"路线），残留归 TASK-015（改用"删数字"路线 + 加 lint 规则禁止手抄行数） |

## 4. 文档/工具产出清单

### 4.1 文档骨架

- **L0 索引**：`MEMORY.md`（116 行 ≤ 150 上限）
- **L1 记忆**：`docs/memory/{facts,rejected,pitfalls,decisions,open}.md` + `apps/notepad.md`
- **Spec 契约**：`docs/spec/{tool-schema,envelope,error-codes,capability-matrix,audit-event,ipc-protocol,testing,naming}.md`（8 份）
- **ADR 决策**：`docs/adr/` 14 份（0018, 0019, 0021~0037；跳号由 ADR-0026 / ADR-0026 D4 解释）
- **任务卡**：`tasks/TASK-001~073-*.md`（73 张）
- **治理文件**：`AGENTS.md`（185 行）/ `docs/governance-ai-agent-execution.md` / `docs/subagent-orchestration.md`
- **本审计**：`docs/audits/stage-0-closeout-2026-09-20.md`（首份）

### 4.2 xtask 护栏（7 子命令 + 1 helper module）

| 子命令 | 用途 | 状态 |
|---|---|---|
| `hygiene` | 仓库卫生（gov §5.4；13 项中 3 项已实现） | 部分 |
| `memory-counts` | MEMORY.md 规模表 ↔ docs/memory/ 实测 | ✅ |
| `adr-index` | ADR 编号登记表 ↔ docs/adr/*.md ↔ decisions.md | ✅ |
| `refscan` | ADR-0032 + ADR-0026 范围写法 / 裸引用 / .ps1 非 ASCII | ✅ |
| `docscan` | 破表 / setext / 编码形状 | ✅ |
| `card-check` | 任务卡格式完整性（ADR-0031 D6，部分实现） | 部分 |
| `guard` | 文件改写互斥锁（ADR-0028） | ✅ |
| `exemptions` | helper module（不暴露） | ✅ |

### 4.3 工具链

- rustc/cargo 1.98.1 stable-msvc ✅
- cargo-deny 0.20.2 ✅
- cargo-llvm-cov 0.9.1（行覆盖 96.31%）✅
- inspect.exe（Windows Kits 10.0.26100.0）✅
- Accessibility Insights ❌ **不装**（ADR-0024 D3 已否决）

### 4.4 Spike 探针

- `spikes/spike-a-notepad/probe-01~04.ps1`（纯 ASCII 化完成，仅 3 条 STR-LIT 豁免 E-021/022/023）
- `spikes/spike-a-notepad/src/bin/uia_dep_proof.rs`（E1~E6 全 PASS）
- `docs/spike-reports/SPIKE-A.md`（**PARTIAL**）

## 5. 残留 finding（本审计不修）

| # | finding | 来源 | 建议归属 |
|---|---|---|---|
| F-1 | 仓库根 17 个 0 字节乱码文件名（前期脚本残留，git status 可见） | git status | **TASK-074** |
| F-2 | `xtask/src/card_check.rs` 664 行 + `main.rs` 649 行（超 600 软上限） | xtask hygiene | TASK-015（PL-033 残余） |
| F-3 | root `cross-platform-ai-assistant-architecture.md` (v1) 应迁 `docs/history/` | PL-009 | 开源准备阶段 |
| F-4 | TASK-070 修 PL-035 用 "185 行" 硬编码（PL-035 原建议是删数字而非修数字） | PL-035 | TASK-015（加 lint rule 禁止手抄行数） |
| F-5 | stage-0 DoD 4 项 ⚠️ carry-over（spike 报告 / no-go / 结论回填 / 升级机制触发） | 本审计 §2 | stage-1 各项 spike 卡承接 |

## 6. 后续行动（stage-1 kickoff）

1. **批次 A1**（地基层，**必须串行**，任一阻塞/失败 → 停整批）：
   - **TASK-011** protocol schema + codegen → 012 存储 → 013 audit → 014 secrets → 015 xtask 护栏
2. **批次 A2**（平台层，2 路并行）：016 platform/api + 017/018 windows provider + 019 automation-host
3. **批次 A3**（内核层，3 路并行）：020~028
4. **批次 A4**（应用 + UI，2 路并行）：029~033
5. 子阶段 1a DoD = TASK-039 stage-1a-integration-audit
6. 详 `plans/stage-1-pilots.md`

## 7. 风险与遗留

- **R-1**：TASK-002 (SPIKE-A 续做) 仍 Blocked on `create_thread` 上游 #36315/#36250 → 人类手工建会话（rejected.md 2026-09-18 末条）
- **R-2**：夜间自动化 GATE-0 未执行（`open.md N9`）→ **禁止创建真实 automation**
- **R-3**：5/8 DoD ⚠️ 项依赖 spike 实测 = stage-1 内化（详 §5 F-5）
- **R-4**：ADR 中 14 份多数仍 Proposed 状态（详 `docs/adr/README.md`），stage-1 开工后批量 Approved
- **R-5**：本审计为 **Implementer-as-Orchestrator** 操作（plans/* + MEMORY.md §1 属 Orchestrator 维护），建议下次类似工作显式标 `ORCH-...` 角色

## 8. 本审计与下游一致性

| 信号 | 位置 | 与本审计一致性 |
|---|---|---|
| stage-0 closeout 信号 | `MEMORY.md §1「下一步」①` | ✅ 已同步 |
| stage-1 kickoff 信号 | `MEMORY.md §1「下一步」②` | ✅ 已同步 |
| DoD 状态 | `plans/stage-0-spikes.md:25-34` | ✅ 已勾选 |
| 残留 finding | 本审计 §5 + `tasks/TASK-073-stage-0-closeout.md` §7 | ✅ 双向交叉引用 |

## 9. 审计签字

- **审计 agent**：Codex（TASK-073, 2026-09-20）
- **审批人**：待人类（详 gov §7.2）
- **下次审计**：stage-1 closeout（预计 2026-12 ~ 2027-01）
