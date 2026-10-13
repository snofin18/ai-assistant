# TASK-282　质量告警清理：PITFALL / 模块头 / bare-stub（章程 §2 P3）

- 状态：**Review**
- 阶段：1　子阶段：**治理/质量**（跨阶段）　批次：**治理池**（ADR-0037 号段 200~299）　依赖：无　预估：S　难度：S
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**；`- 状态：` 行按 ADR-0083 为唯一例外）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md`。

---

- **依赖**：无　**预估**：S　**难度**：S
- **write scope**：`apps/agent-core/src/runtime_tools.rs`、`apps/agent-core/src/ui_server_tests.rs`、`apps/automation-host/src/lib.rs`、`apps/desktop-ui/src-tauri/build.rs`、`apps/desktop-ui/src-tauri/src/main.rs`、`crates/ipc/src/handshake.rs`、`crates/model-gateway/tests/common/mod.rs`、`crates/platform/windows/src/window/mod.rs`、`crates/platform/windows/src/window/capture/tests.rs`、`crates/secrets/tests/secrets_unit.rs`、`xtask/src/comments.rs`、`tasks/TASK-282-quality-warning-burndown.md`、`LEDGER.md`、`PLAN.md`、`README.md`、`plans/stage-1-pilots.md`、`docs/automations/2026-10-13-round-8.md`、`docs/automations/2026-10-13-report.md`
- **关联**：`docs/automation-charter.md` §2 P3、`docs/spec/naming.md` §8 / §10、`AGENTS.md` §5.2

**目标**

只清 `check-comments` / `hygiene` 中可安全修复的非设计告警：`comments/pitfall-format` 2 条、
`comments/module-header-missing` 4 条、`hygiene/bare-stub` 14 条（共 20 条）。
不修改生产行为、公共接口、依赖、lint / CI 配置，也不处理缩写、受控词汇、复杂度或文件长度告警。

**背景**

2026-10-13 round 8 基线：`hygiene` 0E/131W、`check-comments` 0E/73W。
其中 `clippy::assert_is_empty` unknown-lint 来自 `Cargo.toml` 的 workspace lint 配置，属契约面；
缩写 / 受控词汇 / 复杂度 / 文件长度需要公共 API 改名或结构性重构，均不属本卡 write scope。

**In scope**

- 把 `runtime_tools.rs` 的非规范 PITFALL 注释改为普通说明，保留 `DRIFT-214-2` 溯源。
- 把 `comments.rs` 中“规则名”注释改为避免被扫描器误判为 PITFALL 标签的措辞。
- 为 4 个缺模块头的文件补 `//!` 职责 / 边界 / 不变量说明。
- 为 14 个测试替身 no-op 方法补 `// STUB(TASK-282): ...`，明确其故意不获取 OS 资源。

**Out of scope（做了算漂移）**

- 改 `Cargo.toml` / `clippy.toml` / `rustfmt.toml` / `.github/workflows/**`。
- 公共 API / trait / schema / ErrorCode / DB / IPC 变更。
- 新增依赖、crate、顶层目录。
- 为清 `comments/abbreviation` / `comments/controlled-vocabulary` 而重命名公共 API。
- 为清 complexity / file-too-long 告警而拆文件或重构生产控制流。
- 任何真实 GUI、网络、凭据或真机验收操作。

**步骤**

1. 修 2 条 `comments/pitfall-format`。
2. 补 4 条 `comments/module-header-missing`。
3. 补 14 条 `hygiene/bare-stub` 的 STUB 卡号说明。
4. 重跑 `hygiene` / `check-comments`，确认目标 rule id 为 0；再跑全部硬门禁。
5. 按 ADR-0039 / ADR-0041 同步 `LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md`。

**DoD**

- [ ] 目标 20 条告警清零：`comments/pitfall-format`、`comments/module-header-missing`、`hygiene/bare-stub`
- [ ] 未新增任何告警类别或错误；`hygiene` / `check-comments` verdict 保持 PASSED
- [ ] 生产逻辑零行为改动；改动仅限注释、STUB 标记和文档
- [ ] `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --workspace` 全绿
- [ ] 全部现行 xtask 硬门禁 PASSED
- [ ] `LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` 与最终状态同批同步

**验收命令**

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo run -p xtask -- hygiene
cargo run -p xtask -- check-comments
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- refscan
cargo run -p xtask -- docscan
cargo run -p xtask -- card-check
cargo run -p xtask -- check-ledger
cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check
cargo run -p xtask -- check-migrations
```

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

【任务】TASK-282 质量告警清理：PITFALL / 模块头 / bare-stub
【目标】清零 20 条非设计告警，不改变产品行为
【write scope】仅：卡内列出的 11 个告警文件与本轮状态 / 留痕文件
【铁律】无静默失败；不扩 scope；契约面只读；热点文件先 guard
【禁止】改公共接口 / 依赖 / lint / CI；重命名公共 API；重构复杂度；真实 GUI / 网络 / 凭据
【验收】见「验收命令」→ 目标 rule id 为 0 且全部硬门禁 PASSED
【依赖】无；章程 §2 P3；运行日 2026-10-13 round 8（机制 = automation）
【疑问】无

### 2. 实际改动文件

- `apps/agent-core/src/runtime_tools.rs`
- `apps/agent-core/src/ui_server_tests.rs`
- `apps/automation-host/src/lib.rs`
- `apps/desktop-ui/src-tauri/build.rs`
- `apps/desktop-ui/src-tauri/src/main.rs`
- `crates/ipc/src/handshake.rs`
- `crates/model-gateway/tests/common/mod.rs`
- `crates/platform/windows/src/window/mod.rs`
- `crates/platform/windows/src/window/capture/tests.rs`
- `crates/secrets/tests/secrets_unit.rs`
- `xtask/src/comments.rs`
- `tasks/TASK-282-quality-warning-burndown.md`
- `LEDGER.md`
- `docs/automations/2026-10-13-round-8.md`
- `docs/automations/2026-10-13-report.md`

### 3. 验收输出摘要

- 基线：`hygiene` 0E/131W、`check-comments` 0E/73W。
- 修复后：`hygiene` 0E/117W、`check-comments` 0E/67W；目标 20 条告警清零
  （`comments/pitfall-format` 0、`comments/module-header-missing` 0、`hygiene/bare-stub` 0）。
- `cargo fmt --all --check`：EXIT 0。
- `cargo clippy --all-targets -- -D warnings`：EXIT 0（仅既有 `clippy::assert_is_empty` unknown-lint）。
- `cargo test --workspace`：EXIT 0。
- `cargo llvm-cov --fail-under-lines 75`：EXIT 0；workspace 行覆盖率 78.77%。
- `cargo deny check`：advisories / bans / licenses / sources 全 ok。
- `pnpm lint` / `pnpm typecheck` / `pnpm test`（`apps/desktop-ui`）：全部 EXIT 0。
- xtask：`hygiene` / `check-comments` / `memory-counts` / `adr-index` / `refscan` / `docscan` /
  `card-check` / `check-ledger` / `verify-schemas` / `codegen --check` / `check-migrations` /
  `replay --suite core` 全部 PASSED。
- 非宿主 clippy：`x86_64-unknown-linux-gnu` 与 `aarch64-apple-darwin` 的 `assistant-ipc` /
  `assistant-model-gateway` 均 EXIT 0。

### 4. DoD 逐条核对

- 目标 20 条告警清零：满足；三个目标 rule id 均为 0。
- 未新增告警类别或错误：满足；两工具均 0E/PASSED。
- 生产行为零改动：满足；仅注释与文档，未改任何 Rust 语句 / 公共 API。
- fmt / clippy / workspace test：满足。
- 全部 xtask 硬门禁：满足。
- `LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` 同批同步：
  Review 提交先落 LEDGER Review 行；Done / merge hash 回填与 PLAN / README / plans 同步待合并后执行。

### 5. 偏差

none。P3 子集按卡内 write scope 完成；缩写 / 受控词汇 / 复杂度 / 文件长度与 lint 配置不在本卡范围。

### 6. 更合理做法

把测试替身的 no-op 方法标成 `STUB(TASK-282)`，而不是扩大 `#[allow]`：既消除 `bare-stub` 告警，
又保留“这是测试替身、不是产品实现”的机器可读证据。

### 7. 遗留问题

- 章程 §2 P6 的 `MEMORY.md` §5 路径与 ADR-0021 后的分层记忆不匹配；当前代码 PITFALL 的目标应为
  `docs/memory/pitfalls.md` 或 `docs/memory/apps/<app>.md`。本轮未执行 P6，建议另立治理卡同步章程口径。
- 剩余 `comments/abbreviation` / `comments/controlled-vocabulary` 需公共 API 改名，属契约动作，不应由 P3 顺手处理。
- complexity / file-too-long 告警需要结构性重构，另行拆卡。

### 8. 新增长期记忆

无。

### 9. 给审阅者的关注点

1. 确认所有改动都只是注释 / 模块头 / STUB 标记，没有语义差异。
2. 确认 `STUB(TASK-282)` 只用于测试替身和编译期断言函数。
3. 确认目标 rule id 清零而 `hygiene` / `check-comments` 没有新增其他类别。
