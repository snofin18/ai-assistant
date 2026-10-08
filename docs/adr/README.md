# docs/adr/ — ADR 编号登记表（Numbering Registry）

> **用途**：回答「ADR-NNNN 到底存不存在、内容是什么、这个号能不能占用」。
> **为什么需要**：本项目的决策有**两个落点**，共用一套编号 ——
> ① `docs/memory/decisions.md` 的 `[ADR:待建 NNNN]` 条目（决定已做，正式 ADR 文件还没写）；
> ② `docs/adr/NNNN-*.md` 文件（gov §9.3 模板的完整 ADR）。
> **只有本表说明哪些号已被文件占用**。没有它，agent 很容易把「待建 0019」和
> 「已存在的 0019 文件」当成同一件事 —— 这个坑已真实发生过，见 §3 与 ADR-0026。
>
> **规矩**：新建 ADR 前**必须先查本表**取一个「可用」号；新建后**必须回填本表**。
> 本表只描述事实、不做决策；改「号的分配」属于决策 → 走 ADR。
>
> **本表由机器校验**（ADR-0030 D3，CI 硬门禁 **#12b**）：`cargo run -p xtask -- adr-index` 会三方交叉
> 比对本表 ↔ `docs/adr/NNNN-*.md` ↔ `docs/memory/decisions.md`，共 **11** 条规则（含 `adr/number-collision`
> —— 就是 §3 那次事故的机器判据）。工具**只读**，不一致时打印可直接粘贴的正确值让人改；
> 「靠记得回填」的护栏已经证明会失效，所以这一版把它换成了红灯。

---

## 1. 已存在的 ADR 文件（编号已被占用，不得复用）

| 编号 | 文件 | 状态 | 决策一句话 |
|---|---|---|---|
| **0007** | `0007-egress-policy-tiers-and-resolution.md` | **Accepted** | 应用覆盖替换默认、内容类型覆盖只收紧；`local_only` 无本地模型显式失败；非本地出域必须命中 egress-destination 白名单；DLP 返回变更记录但不自行写审计 |
| **0016** | `0016-repository-lf-line-endings.draft.md` | **Draft** | 仓库文本统一 LF；`.gitattributes` + `rustfmt.toml` 双重声明，不依赖个人 `core.autocrlf` |
| **0017** | `0017-deferred-implementations-must-fail-explicitly.draft.md` | **Draft** | 未实现项必须显式登记并显式失败；禁止占位成功或隐藏命令 |
| 0018 | `0018-nightly-automation-delivery-mechanism.md` | Accepted → **Superseded**（by ADR-0029） | 夜间自动化 = Windows 任务计划程序 + `codex exec`（否决 heartbeat）。**已被 ADR-0029 取代**：机制改回 Codex 官方 scheduled tasks；本 ADR 的纪律性内容由章程 §11 v1.4 原样保留 |
| 0019 | `0019-hard-gate-negative-verification.md` | Accepted | 硬门禁必须配「负向验证」（元门禁） |
| **0020** | `0020-repository-license-mit-or-apache-2.0.draft.md` | **Draft** | 仓库采用 `MIT OR Apache-2.0` 双许可证；`LICENSE` / `NOTICE` / Cargo / deny 白名单保持一致 |
| 0021 | `0021-memory-layering-and-app-profiles.md` | Accepted | `MEMORY.md` 只做 L0 索引，细节入 `docs/memory/` |
| 0022 | `0022-windows-target-identity-and-uia-selector-stability.md` | Accepted | Windows 目标身份与 UIA selector 稳定性契约 |
| 0023 | `0023-text-eol-normalization-contract.md` | Accepted | 文本进出口 EOL 归一化契约（规范形 = LF）+ 每应用习惯表 |
| 0024 | `0024-spike-toolchain-dependencies-and-gate-coverage.md` | Accepted | Spike 工具链 / 依赖 / 门禁覆盖（`windows` crate UIA、`spike-deny`、`.ps1` 纯 ASCII） |
| 0025 | `0025-hygiene-rule-count-unification.md` | Accepted → **Superseded**（by ADR-0030） | 仓库卫生规则总数 = 13（gov §5 表格行数为唯一事实源）— CI 门禁计数口径 = 17 行清单 ↔ 16 个步骤（7 硬 + 9 软）已被 ADR-0030 §决策 1 改为 18 行 ↔ 17 步骤 = 8 硬 + 9 软；supersede 关系 = 扩 #12b 子编号而非推翻（hygiene 13 项 SSOT 仍生效）（gov §5.4 表格行数为唯一事实源） |
| 0026 | `0026-adr-number-registry-and-0019-collision.md` | Accepted | 建立本登记表 + 修正 0019 号双重占用（待建号 0019 → 0027；章程 W4 修正） |
| **0027** | `0027-test-only-lint-allowlist.md` | **Accepted** | 三项 lint（`unwrap_used` / `expect_used` / `panic`）只允许在 `#[cfg(test)] mod tests` 豁免；产品代码不得按本决定放宽（TASK-251；2026-10-07 按预授权代为裁决；`PL-098` / `DRIFT-W3-1` 闭环） |
| 0028 | `0028-file-rewrite-mutex-protocol.md` | Accepted | 文件改写互斥锁协议（`xtask guard`：按文件加锁、等待、超时放弃、放弃必须通报） |
| 0029 | `0029-nightly-automation-back-to-codex-scheduled-tasks.md` | Accepted | 夜间自动化的投递机制改回 **Codex 官方 scheduled tasks**（取代 ADR-0018 的任务计划程序方案） |
| 0030 | `0030-machine-verified-memory-counts-and-adr-index.md` | Accepted | 「记忆规模计数」与「本登记表」由手工维护改为机器校验（`xtask memory-counts` / `adr-index`） |
| 0031 | `0031-task-card-one-file-per-card.md` | Accepted | 任务卡改为**一卡一文件**：正文 + 执行记录同在 `tasks/TASK-NNN-<slug>.md`，`plans/*` 退回阶段索引（裁决 DRIFT-001-3） |
| **0032** | `0032-doc-rule-exemption-mechanism.md` | **Accepted** | 文档护栏规则的豁免清单机制（覆盖 PL-032 / PL-034）；配套 `0032-doc-rule-exemption-registry.md` |
| **0033** | `0033-line-count-norm-vs-ci-gate.md` | **Accepted** | 单文件行数「写作规范 400/600」与「CI 门禁 600/900」的语义划分（PL-033） |
| **0034** | `0034-xtask-card-check-implementation.md` | **Accepted** | TASK-051 xtask 护栏升级 = refscan/docscan/card-check/exemptions 内化（ADR-0031 D6 机器化；本次 DRIFT-20260919-2 实现，悬空 commit 10f78db 收编） |
| **0035** | `0035-workspace-lint-policy-no-exceptions.md` | **Accepted** | workspace \[lints.clippy]\ 维持严格 deny 政策（B 路结论，2026-09-19）：不增加 exceptions 字段，per-line allow 仍允许但需注释 + 任务卡 §5 登记。人类在 TASK-051 报告后裁决 B 路 = 漂移 ⑥ 不可忍受；本 ADR 显式化 + 清点当前 4 类 per-line allow |
| **0036** | `0036-card-number-collision-resolution.md` | **Accepted** | xtask 卡 051~055+055b 重编号为 059~064；撤回 sub-suffix 命名（ADR-0031 D7 强化；撞号根因 = PL-022 现场复现）|
| **0037** | `0037-task-card-number-allocation-strategy.md` | **Accepted** | 任务卡号段分配策略（XTASK 池 072~099 + 业务池 100~199 + 治理池 200~299）；sub-suffix 永久禁用；防止未来 xtask ↔ stage-1 撞号 |
| **0038** | `0038-storage-migration-registry.md` | **Accepted** | 存储迁移注册表：各 crate 声明自己的迁移、storage 只提供机制（PL-046 闭环；删 `SCHEMA_VERSION` 常量，`Database::open` 加必填迁移集参数） |
| **0039** | `0039-task-end-state-sync-contract.md` | **Accepted** | 任务结束时的状态同步契约：每张卡 Done 时在同一 PR 内同步 `PLAN.md`「当前状态」块 + `README.md` 状态行/当前阶段/最近进展；无变化也要显式写；可机器校验部分（新鲜度）归 TASK-015 `check-ledger`（DRIFT-202-2 闭环） |
| **0040** | `0040-audit-log-column-semantics.md` | **Accepted** | `audit_logs` 列语义去重 + 显式链序：删与 `id` 同义的 `hash` 列、加 `sequence INTEGER PRIMARY KEY AUTOINCREMENT`；迁移 0003 重建表；0002 一字不改（PL-045 + PL-043 闭环） |
| **0041** | `0041-plan-file-write-scope-and-progress-sync.md` | **Accepted** | 计划文件的「可写面」：**只允许改完成状态与「当前进度」块**，**禁止改排期与条目正文**；打勾不得重写正文；授权修订 `AGENTS.md` §8/§11 + 章程 §3（人类 chat 2026-09-24 澄清） |
| **0042** | `0042-capability-catalog-vs-capability-matrix.md` | **Accepted** | 能力命名三分：**`CapabilityCatalog`** = 稳定能力标识目录（schema `title` 由 `CapabilityMatrix` 改名）/ **`CapabilityMatrix`** = 运行时探测结果（保留原义，架构 v2 §13.1.2）/ **`CapabilityEntry`** = 单条能力的风险·审批元数据；目录名 `protocol/capability-matrix/` 不改（PL-064 闭环，人类 chat 2026-09-24 授权） |
| **0043** | `0043-uia-element-resolution-scope.md` | **Accepted** | 元素解析必须有 scope（父窗口）：`resolve_element` / `wait_for` 的首参改为 `&ResolvedWindow`，搜索起点 = 该窗口的 UIA 根元素；**禁止**从桌面根搜元素（PL-068 + DRIFT-017-7 裁决；架构 v2 §13.1.1 / §6.2 已同步） |
| **0044** | `0044-ambiguity-policy-alignment.md` | **Accepted** | 歧义策略与架构 v2 §6.6 对齐：平台层只保留 fail-closed 的 `error_and_ask`，**删除** `OnAmbiguous::HighestScore`（候选链模型里不可实现）；`require_unique` / `first_by_order` / `disambiguate_by` 的归属层写清（PL-069 闭环） |
| **0045** | `0045-non-host-target-lint-gate.md` | **Accepted** | `#[cfg]` 分叉代码的「非宿主平台」编译门禁：Windows 开发机上用 `cargo clippy --target <非 Windows 目标> -p <纯 Rust crate>` 覆盖 `#[cfg(not(windows))]`；适用范围 = 无 C 依赖的 crate（PL-070 闭环） |
| **0046** | `0046-automation-one-shot-instruction-driven.md` | **Accepted** | 自主自动化的形态 = **人类指令驱动的「一次性」任务**：建立需人类指令；不同自动化间隔 **≥ 2.5 h（默认 3 h）**；任务内容**按实际进度**现场决定（不写死卡号）；任何时间可提；默认授权按最优解自决 + 授权 git 合并；无法决定时「可跳过则跳过 / 不可跳过则停止返回失败」（PL-077 闭环；授权修订章程 §11.1/§11.3/§11.4 + 新增 §11.11 与手册 §4.1） |
| **0047** | `0047-assertion-no-free-string-assert.md` | **Accepted** | 后置断言**没有**自由字符串字段 `assert`（永久不实现）：支持它 = 引入表达式语言（lexer / parser / evaluator / scope）且**模型无法枚举可写形式**；改为结构化 `field`+`op`+`value` 与各 kind 专用字段，**未知字段解析期拒绝**（PL-086 闭环；同步改附录 A `#/$defs/assertion` 与 §5.2 / §7.4 / §11 示例，顺带删掉附录 A 从未声明的 `optional_if_absent`） |
| **0048** | `0048-policy-decision-confirmation-projection.md` | **Accepted** | `PolicyDecision` 增加可选 `scope_options` / `show_diff`，使 confirmation 决策跨审计/进程边界无损；非空 scope 的存在是“需要确认”的唯一判据，高风险仍由 `hitl` 收敛为 `once`（PL-082 / `DRIFT-021-1` 闭环；schema 1.0 向后兼容） |
| **0049** | `0049-automation-minimum-interval-floor.md` | **Accepted** | 自动化**最小间隔**由 **2.5 h 下调为 2 h**（默认 3 h 不变）：依据 = 7 个一次性 automation 的一手运行时长（最长 64.1 分钟 = 1.87 × 下限，且高于 n=7 的单侧 95%/95% 上容限 ≈ 92 分钟）；只修订 ADR-0046 D3 的数值，锁与其余保护不变；授权同步章程 §11.1/§11.9/§11.11 与手册 §4.1/§6，并新增手册 §4.1.c「prompt 固定块」（`.nightly.lock` + 轮次产物；PL-087） |
| **0050** | `0050-automation-time-neutral-2h-mutex.md` | **Accepted** | 自动化规则**时间中性化**（「当夜」= 运行日、「晨间报告」= 阶段报告；路径/目录/锁名本次不改）+ **默认间隔 3 h → 2 h**（下限仍 2 h）+ **开始时刻互斥**（同时存在 ≥ 2 个 automation 时严禁同一时刻开始 / 同时运行）；只修订 ADR-0046 D3 的默认值与 ADR-0049 D2，锁与 D1/D2/D4 不变（PL-088 / PL-089 另案） |
| **0051** | `0051-de-nightly-rename.md` | **Accepted** | 去夜间化**改名**：`docs/overnight-automation-charter.md` → `docs/automation-charter.md`、`docs/nightly/` → `docs/automations/`、`.nightly.lock` → `.automation.lock`、`nightly/<date>` → `automation/<date>`；**只改现行文档 + 运行期标识，不回溯改写只追加 / 只读的历史记录**（ADR 只增不改 / LEDGER 只追加 / 任务卡正文只读）；授权例外 = README 工程元层 + 路由两行、MEMORY.md 路由 / 快照两行（只改路径）（PL-089 闭环） |
| **0052** | `0052-time-neutral-wording-sweep.md` | **Accepted** | 全量措辞**去夜间化**：现行正文里的 `夜间 / 每夜 / 当夜 / 晨间 / 整夜` ＋ **同族时间框架词** `早晨 / 早上 / 醒来 / 白天 / 深夜` 全量换成时间中性词；**只读 / 只追加的历史记录不动**（ADR 只增不改 / LEDGER 只追加 / 任务卡正文只读）；附带把章程 §12 的**孤儿行** `1.10`~`1.16` 归位（行内容逐字不变）（PL-089 措辞部分闭环） |
| **0053** | `0053-core-orchestration-layer-interface.md` | **Accepted** | `core` 编排层的接口面与**依赖白名单**（D2 白名单 = `protocol` / `storage` / `platform/api`（仅 trait）/ `task-engine` / `model-gateway`；D3 黑名单含 `tool-bus` / `policy` / `audit` / `hitl` / `verify` / `undo` / `lease` …）；**「组装」下沉到 binary（TASK-029）**；FTS5 检索归 `crates/storage`（前置卡 TASK-206）；原 TASK-028 拆为 TASK-028（会话 + 上下文）/ **207**（Planner）/ **208**（Memory）—— DRIFT-028-1~5 全部闭环 |
| **0054** | `0054-automation-run-evidence-landing.md` | **Accepted** | 自动化 run **先落地，再自删**（本轮产生留痕却没可合并 PR → 必须先开 docs-only PR 再自删；唯一例外 = 完全没产生留痕）+ **轮次编号只数 `main`**（`git ls-tree`，禁止数工作区；`round-<N>` 已占用则顺延、禁止覆盖）；动机 = 2026-09-26 的 14:15 一次性自动化把 DRIFT-206-1 证据留在未 push 的本地分支后自删 + 同日两轮撞 `round-1` |
| **0055** | `0055-tool-schema-authoritative-effect-reversibility.md` | **Accepted** | ToolSchema 新增必填 `effect` / `reversibility`；Planner 只从可信工具目录注入，拒绝模型自报，关闭 DRIFT-207-1（TASK-207） |
| **0056** | `0056-runtime-execution-contract.md` | **Accepted** | 运行执行链路由 binary 装配层 `RuntimeExecutor` 独占编排；verify 产生不透明 `VerificationReceipt`，task-engine 只有消费 receipt 才能成功提交（TASK-102，2026-09-29 人类确认） |
| **0057** | `0057-ui-core-ipc-transport-contract.md` | **Accepted** | UI↔Core 传输契约：Core 与 UI 分进程；新增 UI 专属 wire 信封（不复用工具形状的 `RequestMessage`/`ResponseMessage`）；传输复用 `crates/ipc` 的 NamedPipe 帧与「一次性 token + 对端镜像白名单」；命令带 correlation、事件单向推送（PL-095，2026-09-30 人类确认接受 → TASK-213 解锁） |
| **0058** | `0058-production-composition-root-and-plan-source.md` | **Accepted** | 生产装配根由 `apps/agent-core` binary 层独占；阶段 1a 的 Plan 来源 = 确定性「任务包 → Plan」`ModelProvider`（不是 LLM，真实 LLM 归 1c 前 + M3）；Notepad Host handler 在 binary 层实现并注册进 `ToolBus`；**2026-09-30 人类确认接受 → TASK-214 解锁** |
| **0059** | `0059-runtime-executes-hitl-and-rollback-steps.md` | **Accepted** | 1a 运行时**真的执行** `hitl` / `host_service` / `verify` 三类步骤（`request_approval` → HITL、`prepare_rollback_anchors` → Undo 锚点、任务包 `set` 级断言），`point_of_no_return` 按声明生效；未实现的步骤种类一律 fail-closed，**禁止**继续"静默跳过非 tool 步骤"；**2026-10-01 人类确认接受 → TASK-216 B 片解锁**（DRIFT-105-3） |
| **0060** | `0060-runtime-step-kinds-as-reserved-tools.md` | **Accepted** | 三类步骤以「**保留运行时工具**」落地：`assistant.runtime.request_approval` / `prepare_anchors` / `verify_postconditions`，**只进 Planner 目录、不进模型可见挂载集**（模型不得直接调审批）；`render_plan()` 按 kind 映射；handler 走 `crates/hitl` / `crates/undo` / receipt 断言；**不改 `PlanStep` 与 `task-engine` 公共形状**；`verify` 的 `set` = "此前所有 tool 步骤已验证提交"，不引入断言 DSL；**2026-10-01 人类确认接受 → B 片第 3b 步解锁** |
| **0061** | `0061-runtime-task-dataflow-and-resumable-approval.md` | **Accepted** | 运行时任务包数据流：整值 `$name` 上下文、提交后发布 step outputs、闭集 `pure` operation、按 `(kind, operation)` 分派 host_service、封闭谓词 `when`、审批暂停/恢复；**不改 PlanStep/task-engine 形状、不引入任意表达式语言**（`DRIFT-216-4`；D8 已按现成 T1.1/T1.3 声明修订） |
| **0062** | `0062-rollback-physical-snapshot-and-executor.md` | **Accepted** | 物理快照捕获 + `crates/undo` 回滚执行器接线：`prepare_anchors` 真实捕获编辑区文本与目标文件字节并记录 SHA-256；`UndoStack` → Adapter `Ctrl+Z`、`RestoreContentSnapshot`/`RestoreShadowCopy` → 文件字节恢复；回滚后同时复核内存文本与磁盘 digest；冲突/缺失证据一律 incident（TASK-218；不改 `crates/**` 公共形状） |
| **0063** | `0063-resource-lifecycle-discipline.md` | **Accepted** | 资源生命周期纪律：长期状态必须有硬上限与淘汰/拒绝；子进程必须终止进程树并 reap（含失败路径）；线程本地资源要能释放；新卡必须附泄露证据；不改公共契约（TASK-220/221；2026-10-02 人类要求记入操作规则） |
| **0064** | `0064-l1-file-channel-as-reserved-host-service.md` | **Accepted** | L1 文件通道以 binary 层保留 `host_service` 表达：`l1_file` 语义不变；新增 `assistant.runtime.host_read_utf8_prefix`，有界读取 UTF-8 前缀、显式 `truncated`、不改公共形状、不新增依赖（TASK-223；2026-10-03 自动化按授权接受） |
| **0065** | `0065-initial-fingerprint-reserved-host-step.md` | **Accepted** | 前置保留步骤产出初始指纹：新增 `assistant.runtime.host_capture_initial_fingerprint`，恒执行、指纹只来自**注入平台**（fake 与 `WindowsPlatform` 同一条代码路径）、严禁构建配置分叉与伪造、不改 `runtime_binding` 的 fail-closed 判据（TASK-223；`DRIFT-223-1` / `PL-100` 闭环；2026-10-03 人类派单确认） |
| **0066** | `0066-cli-single-write-channel-and-file-occupancy-probe.md` | **Accepted** | 命令行唯一写通道 = `xtask write` 复用 ADR-0028 协作锁 + Windows `share_mode(0)` 占用探测 + 有界退避重试；不做真 FIFO，`apply_patch` 不强制纳入，探测盲区与非 Windows fail-closed 明写（TASK-224；2026-10-04 按用户 2026-10-03 预授权代为裁决；`PL-107` 闭环） |
| **0067** | `0067-pointer-action-explicit-coordinate-space.md` | **Accepted** | pointer 动作显式携带起始点 `CoordinateSpace`；`DragTo` 释放点携带自己的 `CoordinateSpace`；删除混合 DPI 收敛启发式，改为设备名 / DPI / 物理点落屏的显式校验（TASK-231；2026-10-04 按用户 2026-10-03 预授权代为裁决；`PL-074` 闭环） |
| **0068** | `0068-cross-file-duplicate-code-hygiene.md` | **Accepted** | 跨文件重复代码 = 规范化 token shingle + 包含度阈值；本版 Warning，忽略测试 / 生成 / fixture，扫描有硬上限（TASK-234；2026-10-04 按用户 2026-10-04 预授权代为裁决；`PL-060` 闭环） |
| **0069** | `0069-top-level-directory-adr-whitelist.md` | **Accepted** | 顶层目录白名单 = `docs/adr/top-level-directories.md`；未登记目录 Error（TASK-234；2026-10-04 按用户 2026-10-04 预授权代为裁决；`PL-060` 闭环）。**`scripts/` 的处置后续由 ADR-0078 裁定为「不设立」**；本行早期的待裁决指向已失效，读契约以 ADR-0078 为准 |
| **0070** | `0070-role-and-parent-resolution-semantics.md` | **Accepted** | `RoleAndParent` 的父候选只作为作用域：顶层解析过滤被 `parent_id` 引用的候选，父单独命中时不得返回父元素（TASK-235；2026-10-04 按用户 2026-10-04 预授权代为裁决；`PL-094` 闭环） |
| **0071** | `0071-capture-and-dlp-crate-boundaries.md` | **Accepted** | 截图管线与出域脱敏的 crate 边界：新增 `crates/capture`（平台无关的窗口截图管线，唯一截图原语 = `platform/api` 的 `WindowProvider::capture`）与 `crates/dlp`（出域策略 + 脱敏 + 截图遮挡），零第三方依赖起步、铁律 7 不破、依赖方向单向（TASK-238；2026-10-04 按用户 2026-10-04「按你的推荐方案做」裁决；`PL-102` 闭环） |
| **0072** | `0072-derived-values-point-to-ssot.md` | **Accepted** | 会随仓库演变的当前派生值不得手抄进文档；只写唯一事实源指针或可重跑命令，历史观测值须带日期与测量来源；本轮不加 hygiene 规则（TASK-241；2026-10-05 按用户预授权代为裁决；`PL-022` / `PL-035` / `PL-065` / `PL-066` 同族收口） |
| **0073** | `0073-capture-privacy-and-redaction-decisions.md` | **Accepted** | 截图隐私保持与脱敏决策边界：像素遮挡归平台层，`NeverPersist` 不返回或保留 `ImageRef`，滚动清理只产生有界计划，文本命中只接受已判定区间 / 显式词表，不引入正则引擎（TASK-041 拆分 A；2026-10-06 按用户预授权代为裁决） |
| **0075** | `0075-machine-derived-governance-counts.md` | **Accepted** | `gov §5.4` 表格行数是 hygiene 规则总数唯一事实源；`gov §5.1` 编号集合必须与 `ci.yml` 的 `# gov-gate` 标记集合一致，解析失败 / 缺失 / 重复 / 额外均为 Error（TASK-244；2026-10-06 按用户预授权代为裁决；避让在飞分支已使用的 0074） |
| **0074** | `0074-visual-assertion-shape-and-perceptual-hash.md` | **Accepted** | `visual_assert` 只有结构化形状（`field` + `op` + 具名容差/阈值 + `confidence_min`），不引入表达式语言；pHash = 32×32 下采样 + raw 8×8 DCT-II 中位阈值、dHash = 9×8 下采样 + 相邻差分，均 64-bit；`max_hamming_distance` 硬上限 24（随机图对 `P(≤24) = 2.997% < 5%`）；低置信 `NeedsHuman` → `NotEvaluable`，不得单独判成功（TASK-042；2026-10-06 按用户预授权代为裁决） |
| **0076** | `0076-windows-gdi-capture-channel-and-redaction.md` | **Accepted** | Windows 单窗口截图 = GDI `PrintWindow(PW_RENDERFULLCONTENT)`（`BitBlt` 仅在未遮挡时回退）+ `redact=true` 时按 UIA `IsPassword` 矩形做不透明黑遮挡；平台层算 SHA-256 内容地址并把字节交给**注入的** `ImageBlobSink`（trait 在 `crates/platform/api`，binary 用 `crates/storage` 实现并在装配后 `attach` 句柄），未注入 → 显式 `Fatal`（TASK-041 拆分 B；2026-10-06 按用户裁决「选项 ①」；`DRIFT-041-2` 闭环） |
| **0077** | `0077-visual-assert-postcondition-wiring.md` | **Accepted** | `visual_assert` 以**加法式**接进后置断言引擎：新增 `Postcondition::VisualAssert` 与 `*_with_visual` 入口（旧签名保留并委托 `None`）；参考图 / 实测图作为**并列参数**传入，**不进**可序列化的 `Observation`（像素永不入 JSON，ADR-0074）；无图 → `NotEvaluable`，低置信 → `NeedsHuman` → `NotEvaluable`（TASK-247；2026-10-06 按用户预授权代为裁决；`PL-110` / `DRIFT-042-1` 闭环） |
| **0078** | `0078-no-scripts-top-level-directory.md` | **Accepted** | **不设立 `scripts/` 顶层目录**：脚本类内容一律归已获授权的 `tools/`（TASK-228）；`docs/nightly/logs/` 亦不设立（自动化留痕走 ADR-0054 的先落 PR 再自删）；ADR-0069 的「不预授权」结论继续有效，本 ADR 只取代其「由 PL-023 决定」的指向；`PL-023` 作废（TASK-248；2026-10-06 按用户 2026-10-06 裁决） |
| **0079** | `0079-visual-observation-source-wiring.md` | **Accepted** | `visual_assert` 的图像来源由宿主层接通：`ObservationCollector::observe_visual` 默认返回 `None`，生产 `StorageVisualObservationCollector` 只从成功信封的 blob 元数据读取参考图 / 实测图，经装配拥有的 storage 读取 BGRA、按固定 Rec.709 整数口径转灰度，再走 `verify_postconditions_with_receipt_and_visual`；像素不进 JSON / IPC / `Observation`（TASK-249；2026-10-06 按用户「继续 TASK-247」授权） |
| **0080** | `0080-taint-tracking-and-permission-decay.md` | **Accepted** | 污点追踪是 `SessionManager` 的运行时状态：Tool 消息置污、User 消息或 `clear_taint` 清除；恢复时从消息角色序列保守重算。policy 在 `tainted=true` 时把所有确认范围降级为 `Once`，高风险 / L3 强制拒绝不变；不改快照 / DB schema / protocol（TASK-051；2026-10-07 按用户「可以继续进行下一步」授权） |
| **0081** | `0081-clean-context-review-component.md` | **Accepted** | `core` 新增 `CleanContextReview` 公开组件：只接收 `SessionSnapshot::goal()` 的用户原始请求与已校验的高风险 Step 摘要，用注入的独立 `ModelProvider` 做严格 JSON 复核；不一致返回 `PolicyDenied`。复核请求不含 Tool / 外部内容、不写入会话、不改变 taint；组件不替代 policy / HITL，不改 protocol / IPC / DB schema（TASK-054；2026-10-07 按用户「授权你按你说的做」授权；`DRIFT-054-1` 闭环） |
| **0082** | `0082-instruction-origin-attribution.md` | **Accepted** | `core` 新增纯模型 `InstructionOrigin` / `InstructionAttribution`：固定 `user_request` / `plan_derived` / `app_content` / `tool_suggestion` 四类 token，按来源校验父目标 / 来源引用，`app_content` 只作为高风险信号；UI approval 继续默认拒绝 `app_content`。**不**改 policy / `PlanStep` / `PolicyDecision` / `ApprovalRequest` / audit schema / IPC / DB（TASK-052；2026-10-07 按用户「合并 PR #270，并授权你先立 ADR-0082 + 最小扩权」授权） |
| **0083** | `0083-task-card-status-line-writable-exception.md` | **Accepted** | 任务卡 `- 状态：` 行是分界线以上正文区的唯一可写例外；Implementer 只能改这一行的真实状态，且必须与产生该状态的提交同批，其他正文行仍只读；`card-check` 的未来正文 diff 判据只排除这一行（TASK-253；2026-10-07 按预授权自动化代为裁决；`PL-073` 闭环） |
| **0084** | `0084-paint-runtime-assembly-and-plan-source-extension.md` | **Accepted** | 生产装配根保持 binary 层独占并**参数化适配器**：target 目录 / handler 集 / 任务包路径由装配输入给出（Notepad 与 Paint 各一套），`TaskPackageProvider` 与 `ToolRegistry` 不改公共形状；Paint handler 在 binary 层新增模块实现，仍走 platform trait、仍 fail-closed；T3.1 做 **7 个**工具的垂直切片（**勘误 2026-10-08**：D4 原文写"6 个"漏列 `paint.layer.select`，任务包 `select_layer` 步骤依赖它，经人类裁决更正为 7 个，见 TASK-106 `DRIFT-106-1`；读契约以本行为准），真机十次验收另卡；不改 `crates/**` 公共接口、不新增 crate / 依赖（TASK-106；2026-10-08 按用户「接着按你建议的跑」授权；扩展 ADR-0058 范围，不推翻其决策） |
| **0085** | `0085-element-bounds-read-path.md` | **Accepted** | 新增 `UiAutomationProvider::element_bounds` 与 `ElementBounds`，返回全局虚拟屏物理像素矩形；不扩 `TreeSnapshot`、不新增树节点 / 不透明句柄暴露；Windows 走 `CurrentBoundingRectangle`，replay 读录制 bounds，unsupported fail-closed；补充 ADR-0084 D5/D8 为 TASK-106 的 Paint 坐标换算提供唯一读取路径（TASK-106；2026-10-08 按用户「选择后者」授权） |

**下一个可用编号：0086**（= §1 与 §2 已用最大号 **0085** + 1；由 `cargo run -p xtask -- adr-index`
的 `adr/next-number-wrong` 规则机器校验，写错即红灯）。

**0027 不是新的可用号** —— 它已被“`#[allow]` 的唯一合法位置”这条决策占用；该号现已由
`0027-test-only-lint-allowlist.md` 落成 **Accepted**（TASK-251，2026-10-07）。

> 编号**不连续是正常的**：本表只列已登记项；0016 / 0017 / 0020 已由 W4 落成 Draft，0027 已由 TASK-251 转 Accepted。

## 2. 已决定、但尚未写成 ADR 文件的编号（`[ADR:待建 NNNN]`）

这些号**已被预留**：决定本身已生效并记在 `docs/memory/decisions.md`，只是还没有
gov §9.3 格式的完整 ADR 文件。引用它们时**必须写 `[ADR:待建 NNNN]`**，
不能裸写 `ADR-NNNN`（裸写法意味着「文件存在」，会让下个 agent 去 `cat` 一个不存在的文件）。

| 编号 | 决定（全文见 `docs/memory/decisions.md`） |
|---|---|
| 0001 | 核心语言 Rust；UI = Tauri 2 + React/TS；工具协议 MCP-first（`rmcp`） |
| 0002 | API 优先：L1 应用接口 > L2 命令 > L3 无障碍 > L4 合成输入 > L5 视觉 |
| 0003 | Linux Wayland-first，X11 仅作 XWayland 兼容通道 |
| 0004 | element / 句柄不跨进程，Host 边界切在「定位之后」 |
| 0005 | 可逆性四级模型 L0~L3；撤销快捷键由 Adapter 显式声明 |
| 0006 | 无人值守暂不支持，但类型 / 契约 / 能力三处预留 |
| 0008 | 股票类软件只读 + 解读 + 图形展示；`TradingGate` 恒拒绝并预留 |
| 0009 | 平台基线 Windows 11 24H2+；Win10 仅 C 级尽力支持 |
| 0010 | Office 2019+；Photoshop 最低 2021(v22)，2020 列尽力而为 |
| 0011 | 试点顺序 Notepad → Paint → Edge/Chrome → Excel → Photoshop |
| 0012 | 存储四层：内存热缓存 / SQLite(WAL) / 内容寻址 blob(zstd) / 冷归档 |
| 0013 | 内部使用但按开源规范建设；`adapters/` 与 `adapters-private/` 第一天就分开 |
| 0014 | 执行模式 = AI agent 实现 + 人类裁决 |
| 0015 | 命名「一眼可懂」+ 受控词汇表；注释密度偏高，公共 API 100% 文档注释 |

**本表刻意不写「共 N 个待建号」** —— 那是一个会随追加漂移的派生计数（ADR-0030 的整条动机）。
一致性由 `cargo run -p xtask -- adr-index` 保证：§2 的每个号都必须能在 `docs/memory/decisions.md`
找到 `[ADR:待建 NNNN]` 条目（`adr/pending-not-in-decisions`），反过来 decisions.md 里未转正、
未退役的待建号也都必须出现在 §2（`adr/decisions-not-in-pending`），且 §1 ∩ §2 必须为空
（`adr/number-collision`）。

章程 §13 的自动化工作单 **W4** 原本要用范围写法把 0016 至 0020 落成草稿，但该范围已过期
（0018 / 0019 已有同号 Accepted 文件）→ **PL-029 已于 2026-09-18 关闭**：W4 按 ADR-0026 D4
改为 **0016 / 0017 / 0020 / 0027** 四个号（章程 v1.4），并新增「不得写编号范围形式」的要求。

## 3. ⚠ 已知编号事故：0019 曾被双重占用（2026-09-18 发现）

| 占用方 | 决策内容 | 时间 |
|---|---|---|
| `docs/memory/decisions.md` 的 `[ADR:待建 0019]` | `#[allow]` 的唯一合法位置是 `#[cfg(test)] mod tests` | 2026-09-16 预留 |
| `docs/adr/0019-hard-gate-negative-verification.md` | 硬门禁必须配「负向验证」（元门禁） | 2026-09-17 建文件 |

两者是**完全不同的决策**。对照 0018 就看得出差别：0018 的预留主题（夜间自动化用 heartbeat）
与文件主题（夜间自动化的投递机制）**是同一件事**，且文件头显式写了
`Supersedes：MEMORY.md §3 [ADR:待建 0018]` —— 取代链完整；0019 建文件时**没查预留表**，
文件头写的是 `Supersedes：—`，取代链断裂，两条决策静默共号。

**处置（ADR-0026，人类已于 2026-09-18 确认 → Accepted）**：文件保留 0019（已被十余个文件引用，
改号代价大）；`#[allow]` 那条的**待建号**改为 **0027** —— `docs/memory/decisions.md` 已追加
`[ADR:待建 0027][supersedes:2026-09-16 的 [ADR:待建 0019] 条目]`，原条目按「只追加不改写」保留原样。

已退役编号：0019（**专指**「硬门禁负向验证」这一条决策。原「`#[allow]` 的唯一合法位置」条目已改号为
0027，0019 **永不再**分配给它，也永不再分配给任何新决策。本行是 `xtask adr-index` 的机器可读锚点：
缺失 → `adr/registry-section-missing`；退役号被重新分配 → `adr/retired-number-reallocated`。
工具**只读本行标记与第一个括号之间**的编号，括号内是给人看的说明，不参与解析。）

> 为什么需要退役清单：`decisions.md` 是**只追加**的，0019 那条原始待建条目会永远留在文件里。
> 若不排除退役号，`adr/decisions-not-in-pending` 会永久报错；而永久红灯会让人学会忽略 CI。

## 4. 引用规范

| 想引用什么 | 正确写法 | 错误写法 |
|---|---|---|
| 已有文件的 ADR | `ADR-0023` | — |
| 只有 `decisions.md` 条目的决策 | `[ADR:待建 0016]` | `ADR-0016`（暗示文件存在） |
| ADR 文件取代某个待建条目 | 文件头 `Supersedes：decisions.md [ADR:待建 NNNN]` | `Supersedes：—`（丢取代链，0019 就是这样断的） |

**现存 2 处违规（裸引用）**：`0021-*.md:4` 写 `ADR-0017（显式登记）`、`0023-*.md:179` 写 `（ADR-0016）`。
ADR 文件「只增不改」，故**不直接改写**，留待阶段末评审，或等这两份 ADR 下次被 supersede 时顺带修正。

**机器检查尚未实现**：ADR-0030 D4 刻意**没有**在本轮实现裸引用规则 —— 上述 2 处违规都在「只增不改」的
ADR 正文里，先实现规则就等于造一条**永久红灯**，而永久红灯会让人学会忽略 CI。正确顺序是「先定豁免机制
（类似 §3 的退役清单），再实现规则」。→ **PL-028**（记录违规本身）＋ **PL-032**（记录「规则待实现」，归 TASK-015）。
