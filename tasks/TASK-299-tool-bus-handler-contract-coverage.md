# TASK-299　tool-bus handler 公开契约覆盖率补强（章程 §2 P2）

- 状态：**Review**
- 阶段：1　子阶段：**治理/质量**（跨阶段）　批次：**治理池**　依赖：无（`CallContext` / `ToolHandler` / `ToolOutput` 已在 main）　预估：S　难度：S
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031）；分界线以上正文只读，`- 状态：` 行按 ADR-0083 例外。

---

- **write scope**：`crates/tool-bus/tests/handler_contract.rs`、`tasks/TASK-299-tool-bus-handler-contract-coverage.md`、`LEDGER.md`、`PLAN.md`、`README.md`、`plans/stage-1-pilots.md`、`docs/automations/2026-10-15-round-2.md`
- **关联**：`docs/automation-charter.md` §2 P2 / §5、`crates/tool-bus/src/handler.rs`、`crates/tool-bus/README.md`

**目标**

仅用公开 API 契约测试补强 `crates/tool-bus/src/handler.rs` 的行覆盖率：覆盖
`CallContext` 身份访问、闭包 handler 的 blanket impl、`ToolOutput::json` /
`untrusted` / `with_evidence` / `with_max_bytes` 与来源、证据、预算访问器。
使 `handler.rs` 行覆盖率从 75.38% 提升到接近 100%，`assistant-tool-bus` crate
行覆盖率保持在 90% 以上；生产源码零改动，不新增依赖。

**背景**

运行日 2026-10-15 的 P2 现场扫描显示 `crates/tool-bus/src/handler.rs` 为
49/65 行（75.38%），未覆盖行集中在适配器作者可见的
`with_evidence` / `with_max_bytes` / `is_untrusted` / `source` / `evidence` /
`max_bytes` 访问器。此前 TASK-281 / TASK-264 已收口 registry、声明与 schema
公开契约，但 handler 侧这些 builder / accessor 仍没有外部契约测试；
`cargo llvm-cov -p assistant-tool-bus --show-missing-lines` 证明它们均为公开可达路径。

**步骤**

1. 在本卡 write scope 内新增 `crates/tool-bus/tests/handler_contract.rs`，只调用公开 API。
2. 覆盖 `CallContext` 构造与身份访问，以及闭包 `Fn` handler 的实现路径。
3. 覆盖 `ToolOutput` 三条构造路径与证据 / 预算 builder、全部访问器。
4. 用真实 `cargo llvm-cov` 对比改动前后行覆盖率，记录完整门禁输出。
5. 同步卡片状态与 `LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md`。

**DoD**

- [ ] 生产逻辑零改动；新增测试只调用公开 API、不新增依赖、测试文件 ≤600 行
- [ ] `crates/tool-bus/src/handler.rs` 行覆盖率从 75.38% 提升到 ≥95%；crate 行覆盖率保持 ≥90%
- [ ] `cargo llvm-cov -p assistant-tool-bus --tests --fail-under-lines 85` 与 `cargo test -p assistant-tool-bus` 全绿
- [ ] `fmt` / `clippy -D warnings` / `test --workspace` / xtask 硬门禁 / `cargo deny check` 全绿
- [ ] `LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` 同批同步

**验收命令**

```powershell
cargo llvm-cov -p assistant-tool-bus --tests --fail-under-lines 85
cargo test -p assistant-tool-bus
cargo fmt --all --check; cargo clippy --all-targets -- -D warnings; cargo test --workspace
cargo run -p xtask -- hygiene; cargo run -p xtask -- check-comments; cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index; cargo run -p xtask -- refscan; cargo run -p xtask -- docscan
cargo run -p xtask -- card-check; cargo run -p xtask -- check-ledger; cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check; cargo run -p xtask -- check-migrations; cargo deny check
```

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤ -->

## 执行记录

### 1. 约束回执

【任务】TASK-299 tool-bus handler 公开契约覆盖率补强　【目标】只加公开 API
契约测试，把 `crates/tool-bus/src/handler.rs` 从 75.38% 提升到接近 100%　
【write scope】仅本卡列出的测试与状态 / 轮次文件　【铁律】无静默失败；生产源码
零改动；不扩 scope；热点文件先 guard　【禁止】改公共接口、依赖、lint/CI、
schema/DB/IPC；真实 GUI / 网络 / 凭据　【验收】见上；【依赖】无；【疑问】无

### 2. 实际改动文件

- `crates/tool-bus/tests/handler_contract.rs`（新增 7 个公开 API 契约测试）
- `tasks/TASK-299-tool-bus-handler-contract-coverage.md`

### 3. 验收输出摘要

改动前后均用 `cargo llvm-cov` 实测；改动前证据来自新增测试文件加入前的
`cargo llvm-cov -p assistant-tool-bus --show-missing-lines`：

```text
# 改动前
handler.rs   54  20  62.96%  13   5  61.54%   65  16  75.38%
TOTAL      2583 235  90.90% 199  24  87.94% 1864 188  89.91%

# 改动后
handler.rs   54   0 100.00%  13   0 100.00%   65   0 100.00%
TOTAL      2583 211  91.83% 199  18  90.95% 1864 165  91.15%

$ cargo llvm-cov -p assistant-tool-bus --tests --fail-under-lines 85
handler_contract.rs: 7 passed; 0 failed; 0 ignored
TOTAL 91.15%
EXIT 0

$ cargo test -p assistant-tool-bus
coverage_contract.rs:  9 passed
definition_contract.rs: 5 passed
handler_contract.rs:  7 passed
schema_keywords.rs:    9 passed
schema_shape_contract.rs: 6 passed
tool_bus.rs:           5 passed
toolset_governance.rs: 7 passed
doc-tests:             1 passed
EXIT 0

$ cargo fmt --all --check                    -> EXIT 0
$ cargo clippy --all-targets -- -D warnings  -> EXIT 0（仅既有 clippy::assert_is_empty unknown-lint 警告）
$ cargo test --workspace                     -> EXIT 0；xtask 汇总 469 passed / 0 failed
$ cargo test -p assistant-core arch::        -> EXIT 0；arch 10 passed
$ cargo clippy --target x86_64-unknown-linux-gnu -p assistant-tool-bus --all-targets -- -D warnings -> EXIT 0
$ cargo clippy --target aarch64-apple-darwin -p assistant-tool-bus --all-targets -- -D warnings -> EXIT 0
$ cargo run -p xtask -- replay --suite core  -> 2 step(s) / 5 change(s) / 0 mismatch / PASSED
$ cargo deny check                           -> advisories / bans / licenses / sources 全 ok；EXIT 0

xtask 硬门禁（全部 verdict=PASSED）：
hygiene 422 files 0E/114W；check-comments 422 files 0E/68W；memory-counts 0E/0W；
adr-index 83 files 0E/0W；refscan 0E/0W；docscan 461 files 0E/264W；
card-check 193 files 0E/34W；check-ledger 0E/0W；verify-schemas 0E；
codegen --check 0 drift / 0E；check-migrations 0E/0W
```

### 4. DoD 逐条核对

- 生产逻辑零改动、测试只调用公开 API、无新依赖、测试文件 96 行（≤600）：**满足**。
- `handler.rs` 75.38% → 100.00%（≥95%）；crate 89.91% → 91.15%（≥90%）：**满足**。
- `cargo llvm-cov ... --fail-under-lines 85` 与 `cargo test -p assistant-tool-bus` 全绿：**满足**。
- `fmt` / `clippy -D warnings` / `test --workspace` / xtask 硬门禁 / `cargo deny check`：**满足**。
- `LEDGER.md` / `plans/stage-1-pilots.md` 已写入 Review 留痕；`PLAN.md` / `README.md` 与
  Done 状态、merge hash 由合并后的 docs-only 回填 PR 同批更新（ADR-0039 / ADR-0041）。

### 5. 偏差

- 选靶阶段先核查 `crates/ipc/src/frame.rs`（84.43%），`--show-missing-lines` 显示其未覆盖行
  全部是长度校验后不可达的转换 / 切片防御分支，故主动改靶到 `handler.rs`（75.38%）。
- 环境纪律偏差 18 次：17 次 `exec_command` 误用了默认 bash（均为只读探查或单条门禁命令，
  报错后立即改用 PowerShell 重跑，没有任何失败命令被写成通过），另有 1 次 PowerShell
  文件长度探查因 `foreach` 后直接接管道触发 `An empty pipe element is not allowed`，
  修正表达式后重跑成功。所有最终门禁结果均来自 PowerShell 下的真实执行。

### 6. 更合理做法

- 选靶时必须先跑 `--show-missing-lines` 再判断公开可达性；本轮 `frame.rs` 的放弃证明了
  仅按百分比排序会把不可达防御分支误当 P2 目标。
- 本轮再次证明默认 bash 误用会在长会话中反复出现；后续轮次必须在每次工具调用前显式
  核对 `shell: "powershell"`，而不是依赖惯性。

### 7. 遗留问题

- 无产品遗留。主 PR 见 round 文件与 `LEDGER.md`；merge hash 由 docs-only 回填 PR 落地。
- 非产品遗留：`bus.rs`（75.82%）与 `server.rs`（83.58%）仍有低覆盖行，但逐行核查后
  主要是私有解析 / 序列化防御分支；若后续要收口，应先证明其可通过公开 API 或注入替身到达。

### 8. 新增长期记忆

- 无新增 FACT / PITFALL / REJECTED；选靶方法与环境偏差已写入本卡和 round 文件。

### 9. 给审阅者的关注点

1. 新增用例是否只经公开 API 构造并读取 handler 返回元数据。
2. `handler.rs` 是否生产源码零改动、crate 覆盖率是否保持 ≥90%。
