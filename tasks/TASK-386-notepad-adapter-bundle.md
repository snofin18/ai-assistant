# TASK-386 Notepad Adapter bundle convergence

- 状态：**Blocked**
- 阶段：2　子阶段：Adapter pack convergence A　批次：1
- 依赖：ADR-0104、ADR-0105、TASK-380、TASK-383、TASK-384

---

## write scope

- `tasks/TASK-386-notepad-adapter-bundle.md`
- `adapters/com.microsoft.notepad/adapter.toml`
- `adapters/com.microsoft.notepad/app_map.json`
- `adapters/com.microsoft.notepad/memory/app_map.v1.json`
- `adapters/com.microsoft.notepad/selectors/targets.json`
- `adapters/com.microsoft.notepad/tools/tools.json`
- `adapters/com.microsoft.notepad/rollback/recipes.json`
- `adapters/com.microsoft.notepad/interrupts/interrupts.json`
- `adapters/com.microsoft.notepad/tests/validate_adapter_pack.py`
- `adapters/com.microsoft.notepad/README.md`
- `plans/stage-2-excel.md`
- `PLAN.md`
- `README.md`
- `LEDGER.md`

## 目标

把现有 Notepad Adapter 六件套迁移到 ADR-0104/0105 冻结的 bundle v1 契约，
保持现有 selector、五工具、L1 rollback 与人工 interrupt 语义不变，并通过通用
`adapter-validate` 与 Pack 专属回归验证器。

## In scope

- 在 `adapter.toml` 增加唯一的 `[bundle]` envelope 和六路径。
- 为富 App Map 增加 `identity`、`projection_manifest` 与 omission coverage，
  生成 canonical core projection。
- 将 selector 文件迁移到 `uia_candidate_chain_v1`，显式声明 target/candidate
  probe、fallback 与 `on_not_found` policy。
- 将五个工具迁移为 Adapter tool envelope，补 canonical capability、postcondition、
  risk/approval/idempotent 字段与 ToolSchema 1.1 `none_readonly`。
- 将 rollback 迁移为每 write 工具恰好一条 canonical coverage；Save As 保持 L3
  `no_recovery` 人工边界。
- 将 interrupts 迁移为显式 priority、typed match/action、classification 与
  `requires_human/resume_after` 矩阵。
- 新增 Notepad pack 专属 validator，锁定既有 Notepad 关键语义和迁移字段。

## Out of scope

- 不修改 `crates/**`、`apps/**`、`protocol/**`、`docs/adr/**`、`docs/spec/**`、
  CI 或依赖。
- 不迁移 Paint、Edge 或 Chrome pack。
- 不实现 Excel Adapter、COM runner、真实 Excel/Notepad GUI 验收或运行时 loader。
- 不新增 Tool、Target、Capability、ErrorCode、Postcondition kind、rollback anchor
  或 interrupt action。
- 不放宽通用 validator，不用自动改写生成文件规避 finding。

## 必须遵守

1. selector 的主/fallback 顺序、可见文本 fallback 限制和 `error_and_ask` 语义不得退化。
2. write 工具只能声明已批准的 L1/L3 恢复策略；Save As 不允许伪装成可恢复。
3. interrupt 的 Save/Don't Save 永久人工，Save As 通过声明 tool delegate，不保留
   `delegate_to_task_*`。
4. projection 必须由 manifest 确定性生成，token estimate 与 canonical bytes 必须一致。
5. 任何公共 schema/接口或运行时缺口出现即停止并记录 DRIFT，不在本卡扩张。

## 验收命令

```powershell
cargo run -p xtask -- adapter-validate adapters/com.microsoft.notepad --report-only
python adapters/com.microsoft.notepad/tests/validate_adapter_pack.py
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check
cargo run -p xtask -- hygiene
cargo run -p xtask -- card-check
cargo run -p xtask -- docscan
cargo run -p xtask -- refscan
cargo run -p xtask -- memory-counts
cargo run -p xtask -- check-ledger
cargo deny check
git diff --check
```

预期：通用 validator 与 Pack 专属 validator 均 exit 0；所有仓库门禁通过；
Notepad 五工具保持原 name/effect/reversibility，runtime 与公共 schema 零改动。

## DoD

- [ ] `adapter.toml` 含唯一合法六路径 `[bundle]`，通用 validator 通过。
- [ ] App Map manifest 覆盖全部知识域，projection 逐字节可由 validator 重建。
- [ ] selector 使用 `uia_candidate_chain_v1`，八个既有 target 与 fallback policy 保留。
- [ ] 五个工具具有合法 capabilities、canonical postconditions 和 1.1 read-only marker。
- [ ] 四个 write 工具各有一条合法 rollback coverage；Save As 为显式 no_recovery。
- [ ] interrupts 使用 typed action/priority/human matrix，不含历史 delegate_to_task。
- [ ] Pack 专属 validator 锁定 Notepad selector、tool、rollback、interrupt 关键语义。
- [ ] 公共 schema、Rust/TS 产品代码、依赖和 CI 零改动。
- [ ] PR checks、merge 与 post-merge main CI 回填。

<!-- ─── 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ───
     以下由 Implementer 填写。改动分界线上方任何一行 = 漂移触发器 ⑤。
-->

## 执行记录

### 1. 约束回执

```text
【任务】TASK-386 Notepad Adapter bundle convergence
【目标】按 ADR-0104/0105 迁移 Notepad 六件套并保持既有 selector/tool/rollback/interrupt 语义
【write scope】仅：本卡、Notepad Adapter 数据/README/专属 validator、Stage 2/PLAN/README/LEDGER
【铁律】契约先行；不可信输入先校验；写操作有可执行 postcondition；资源/失败不静默；证据不得伪造
【禁止】不改公共 schema/ADR/spec/产品代码/CI/依赖；不迁移其他 pack；不启动真实 GUI
【验收】通用 adapter-validate + 专属 validator + Rust/工作区/文档/deny 门禁全绿
【依赖】ADR-0104/0105、TASK-380/383/384 Done；Stage 2 InProgress
【疑问】无；若通用 validator 暴露冻结契约冲突则停止并记 DRIFT
```

### 2. 实际改动文件（逐个核对是否在 In scope 内）

- `adapters/com.microsoft.notepad/adapter.toml`：新增六路径 `[bundle]`。
- `adapters/com.microsoft.notepad/app_map.json`：新增 identity、projection manifest 与
  knowledge omission coverage。
- `adapters/com.microsoft.notepad/memory/app_map.v1.json`：由 manifest 确定性生成。
- `adapters/com.microsoft.notepad/selectors/targets.json`：迁移到
  `uia_candidate_chain_v1`，补 typed policy、target/candidate probe 与 fallback 字段。
- `adapters/com.microsoft.notepad/tools/tools.json`：五个工具迁移到 1.1 envelope，
  补 capability 与 canonical postconditions；`notepad.tab.new` 改为 L2。
- `adapters/com.microsoft.notepad/rollback/recipes.json`：迁移为四个 write 工具的
  canonical coverage；Save As 保持 L3 `no_recovery`，tab.new 使用
  `close_created_target`。
- `adapters/com.microsoft.notepad/interrupts/interrupts.json`：迁移为 typed
  priority/action/human matrix；不再保留 `delegate_to_task_038` 或自由文本 action。
- Pack 专属 validator 与 README 仍未修改，原因是通用 validator 仍存在
  `DRIFT-386-2`。

### 3. 验收输出摘要（命令 → 结果，全绿 / 失败项）

- 迁移前 `adapter-validate adapters/com.microsoft.notepad --report-only`：
  `scanned_files=1`，唯一 finding 为 manifest 缺七个 bundle 字段。
- 迁移 manifest / projection / selector / tool envelope 后重跑：`scanned_files=7`，
  manifest、projection、selector、tool finding 均为 0；
  剩余 31 个 finding 全部集中在尚未迁移的 rollback/interrupt。
- 迁移 rollback/interrupt 后重跑：`scanned_files=7`，剩余 **6 个错误**，全部是
  interrupt match 的可选字段被 validator 误当成必填：每个规则都报告缺失未声明的
  `automation_id` / `class_name_contains` / `class_name`；rollback finding 已归零。
- 未运行全量 Rust/文档门禁，因为本卡仍被 `DRIFT-386-2` 阻塞；不把局部验证冒充全绿。

### 4. DoD 逐条核对

- [x] `[bundle]` 六路径与 projection model 已加入，validator 已越过 manifest。
- [x] App Map manifest、identity、omission coverage 与 canonical projection 已生成并通过。
- [x] selector model、policy、target/candidate 字段已迁移且通过 validator。
- [x] 五工具 envelope、capabilities、postconditions 与 1.1 `none_readonly` 已迁移并通过。
- [x] 四个 write 工具已各有一条 rollback coverage；Save As 为显式 `no_recovery`，
  tab.new 为 L2 `close_created_target`。
- [ ] interrupts 已写成 spec 形状，但通用 validator 的可选 match 字段缺陷阻断验收，
  见 `DRIFT-386-2`。
- [ ] Pack 专属 validator、README 与完整验收未完成。
- [ ] PR / CI / merge 未创建。

### 5. 偏差

**DRIFT-386-1**

- 现象：`notepad.tab.new` 当前声明为 `effect=write`、`reversibility=l1_snapshot`，
  但 bundle 中没有可声明的 close-tab 工具。D5 要求每个 write 工具恰有一条真实可执行的
  recovery coverage；L1 的 `restore_content_snapshot` 只能恢复内容快照，不能证明 tab 树
  被恢复，L2 的 `invoke_tool` 又找不到可引用的已声明工具。
- 影响：若强行补一条 L1 content-snapshot coverage，会伪造“可恢复 tab 结构”的证据；
  若新增 `notepad.tab.close` Tool，则改变模型可见 Tool surface，超出本卡 write scope；
  若按 L3 `no_recovery` 收口，又必须把普通新 tab 动作错误升为 high/critical + 人工批准。
- 建议：由人类在三种路径中裁决：①新增并审批 `notepad.tab.close` Tool 及其独立恢复语义；
  ②在 ADR/spec 中定义 opener-specific builtin close-created-tab rollback；③明确接受
  `notepad.tab.new` 暂不进入 bundle。裁决前不修改 D5 或 validator。
- 已停工作：未修改 rollback/interrupt；未新增 Tool；未放宽 validator；未创建 PR / merge。

`DRIFT-386-1` 已由 TASK-387 / ADR-0106 方案 A 关闭：新增 rollback-only
`close_created_target`，不新增模型可见 Tool；本卡已将 `notepad.tab.new` 改为 L2。

**DRIFT-386-2**

- 现象：`docs/spec/adapter-bundle.md` §8.2 将 `class_name`、`automation_id`、
  `automation_id_candidates`、`title_candidates`、`name_candidates`、
  `class_name_contains` 定义为可选 union 字段，只要求 match 非空；但
  `xtask/src/adapter_validate/interrupts.rs` 对所有规则无条件要求
  `class_name`、`automation_id`、`class_name_contains` 三个字符串同时非空。
  因而合法的 `class_name + title_candidates` 和
  `automation_id_candidates + class_name_contains` 都被误判为缺字段。
- 影响：Notepad 三条 interrupt 无法在不通伪造 selector 的前提下通过通用 validator；
  在 pack 中补不存在的 class/id 会制造与真实 UIA 不匹配的假 selector。
- 建议：新开前置卡修复 `interrupts.rs`，只校验实际出现的字符串字段，保持“match
  非空、文本候选不得单独成立”的原规则；补所有合法/非法子集测试后，再恢复 TASK-386。
- 已停工作：未修改 `xtask/**`、公共 schema、ADR/spec、CI 或依赖；未绕过 validator。

### 6. 更合理做法（实施中发现，非漂移）

- manifest 使用 omissions 显式声明 commands/ui_map/workflows/state_machine 不进入 core
  projection，避免模型摘要或静默丢失知识域。
- selector 的 Save As 三目标保持 `probe_status=required`，其余已由真实 UIA 探针观察的目标
  保持 `observed`，没有把未测控件冒充已测。

### 7. 遗留问题

- `DRIFT-386-1` 已关闭；`DRIFT-386-2` 等待前置 validator 修复卡。
- Pack 专属 validator、README 与完整门禁未完成。
- Paint / Edge / Chrome 的三套 pack 迁移仍未开始。

### 8. 新增长期记忆

- 无；新的阻塞是 validator 实现偏差，不是新 ADR 决策；待修复卡完成后再决定是否记
  PITFALL。

### 9. 给审阅者的关注点（风险最高的 1~3 处）

- 重点确认 `notepad.tab.new` 的恢复语义是否必须有新的 Tool，不能仅用 content snapshot
  假装恢复 tab 结构。
- rollback 已归零；剩余失败集中在 interrupt match 的 validator 实现偏差。
- 如果选择新增 Tool，应另拆 runtime handler / policy / approval 影响评估，不能在数据包卡
  内静默扩展。
