# TASK-286　core planner 模型输出解析契约覆盖率补齐（章程 §2 P2）

- 状态：**Done**
- 阶段：1　子阶段：**治理/质量**（跨阶段）　批次：**治理池**　依赖：无（`assistant-core` / `assistant-model-gateway` 已就位）　预估：S　难度：S
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031）；分界线以上正文只读，`- 状态：` 行按 ADR-0083 例外。

---

- **write scope**：`crates/core/tests/planner_output_contract.rs`、`tasks/TASK-286-core-planner-output-contract-coverage.md`、`LEDGER.md`、`PLAN.md`、`README.md`、`plans/stage-1-pilots.md`、`docs/automations/2026-10-13-round-12.md`、`docs/automations/2026-10-13-report.md`
- **关联**：`docs/automation-charter.md` §2 P2 / §5、`crates/core/src/planner.rs`、`crates/core/README.md`、`docs/spec/naming.md`

**目标**

仅为 `crates/core/src/planner.rs` 的公开解析路径增加跨平台公共契约测试，覆盖请求校验的空目录 /
畸形 `output` / 非法标签、`Planner::with_event_timeout` 的上下界、预取消令牌、超长输出、
非 Stop 结束原因、空输出、非对象信封、非对象 step、缺少 `tool`、违反 `PlanStep` 契约、
控制字符 goal、空名段，以及 L2 补偿可逆性投影；把 `planner.rs` 行覆盖率提升到 ≥90%，
不修改生产逻辑、不新增依赖。

**背景**

planner 是铁律 2「模型输出 = 不可信输入」在 core 的落点。实测 `planner.rs` 385 行仅覆盖
308 行（80.00%），失败分支缺少跨平台回归；core 属 §5 的 ≥85% 门槛 crate。

**步骤**

1. 新建 `crates/core/tests/planner_output_contract.rs`，只用公开 API + 内存假 provider。
2. 覆盖请求校验与事件流的全部 fail-closed 分支（取消 / 超长 / 非 Stop / 空 / 畸形形状）。
3. 覆盖 L2 补偿工具元数据投影与成功路径。
4. 复测 crate / workspace 覆盖率并跑全部全局硬门禁。

**DoD**

- [ ] 生产逻辑零改动；新增测试只调用公开 API、不新增依赖
- [ ] `crates/core/src/planner.rs` ≥90%，`assistant-core` ≥85%
- [ ] `cargo llvm-cov -p assistant-core --fail-under-lines 85` 与 `cargo test -p assistant-core` 全绿
- [ ] `fmt` / `clippy -D warnings` / `test --workspace` / xtask 硬门禁 / `cargo deny check` 全绿
- [ ] `LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` 同批同步

**验收命令**

```powershell
cargo llvm-cov -p assistant-core --fail-under-lines 85
cargo test -p assistant-core
cargo fmt --all --check; cargo clippy --all-targets -- -D warnings; cargo test --workspace
cargo run -p xtask -- hygiene; cargo run -p xtask -- check-comments; cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index; cargo run -p xtask -- refscan; cargo run -p xtask -- docscan
cargo run -p xtask -- card-check; cargo run -p xtask -- check-ledger; cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check; cargo run -p xtask -- check-migrations; cargo deny check
```

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══ -->

## 执行记录

### 1. 约束回执

【任务】TASK-286 core planner 模型输出解析契约覆盖率补齐　【目标】仅追加测试，把
`crates/core/src/planner.rs` 行覆盖率提升到 ≥90%　【write scope】仅本卡列出的测试与状态 /
轮次文件　【铁律】无静默失败；测试不改生产行为；不扩 scope；热点文件先 guard
【禁止】改公共接口、依赖、lint/CI、schema/DB/IPC；真实 GUI / 网络 / 凭据
【验收】见上；【依赖】无（crate 已就位）；【疑问】无

### 2. 实际改动文件

- `crates/core/tests/planner_output_contract.rs`（新增 17 个跨平台公共契约测试）
- `tasks/TASK-286-core-planner-output-contract-coverage.md`、`LEDGER.md`、`PLAN.md`、`README.md`、`plans/stage-1-pilots.md`
- `docs/automations/2026-10-13-round-12.md`、`docs/automations/2026-10-13-report.md`

### 3. 验收输出摘要

- 改动前：`crates/core/src/planner.rs` 308/385 行（80.00%），`assistant-core` crate 90.04%。
- `cargo test -p assistant-core --test planner_output_contract`：17 passed。
- `cargo llvm-cov -p assistant-core --show-missing-lines`：`planner.rs` **359/385 = 93.25%**；
  crate 行覆盖率 90.04% → **92.09%**；剩余 26 行全部为不可达分支（见 §7）。
- `cargo llvm-cov --workspace --fail-under-lines 75`：EXIT 0；workspace 80.30% → **80.41%**。
- `fmt` / `clippy --all-targets -D warnings` / `test --workspace` / `cargo deny check` /
  非宿主 `x86_64-unknown-linux-gnu` 与 `aarch64-apple-darwin` clippy / `codegen --check` /
  `replay --suite core` / UI `lint` / `typecheck` / `test` 全部 EXIT 0。
- xtask：`hygiene` 0E/117W、`check-comments` 0E/67W，其余硬门禁 PASSED。

### 4. DoD 逐条核对

- 生产逻辑零改动、测试只用公开 API、无新依赖：满足。
- `planner.rs` ≥90%、crate ≥85%、覆盖率命令与全门禁：满足（93.25% / 92.09%）。
- `LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md`：同批同步完成。

### 5. 偏差

- 本轮首选 P3 / P6 均不可执行：`hygiene` 117W 与 `check-comments` 67W 的剩余项全部是
  复杂度 / 文件长度重构或 `as_str` / `element` 等需改**公共 API** 的受控词汇改名（契约动作，
  出无人值守范围）；P6 的 `MEMORY.md` §5 路径与 ADR-0021 分层记忆冲突。按章程回退 P2。

### 6. 更合理做法

- 用单个 core-local integration test 直接驱动公开 `Planner` / `PlannerRequest`，以内存假
  provider 逐事件构造失败流，避免依赖真实模型或 IO。

### 7. 遗留问题

- `planner.rs` 剩余 26 行不可达：序列化失败（`239-242`）、`usize` 溢出（`292-295`）、
  `contains_key` 之后的 `get` 兜底（`345-346`）、`effect`/`reversibility` 二次检查
  （`411-415`，被更早的未知字段检查拦截）、`build_tool_catalog` 重复分支（`450-453`，
  被 `validate` 拦截）、两个 `#[non_exhaustive]` 枚举的 `_ => "unknown"` 兜底
  （`526` / `536`）。要继续提升需改生产逻辑，本卡不做。
- P6 旧路径仍需一张治理卡裁决；不属于本卡范围。

### 8. 新增长期记忆

- 无新增 FACT / PITFALL / REJECTED。

### 9. 给审阅者的关注点

1. 新增测试是否只调用公开 API，并真实覆盖取消 / 超长 / 非 Stop / 畸形形状等失败语义。
2. 93.25% 是否来自可解释的 17 条契约用例，而非用低强度断言堆砌成功路径。
