# TASK-316　tool-bus `bus.rs` 私有失败边界契约覆盖率补强

- 状态：Review
- 阶段：1　子阶段：1b　批次：跨阶段治理池　依赖：TASK-020 / TASK-299 / TASK-301 已 Done
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md`。

---

- **write scope**：
  - `crates/tool-bus/src/bus.rs`（仅允许新增 `#[cfg(test)]` 外置测试模块声明）
  - `crates/tool-bus/src/bus_tests.rs`（新增私有白盒测试）
  - `MEMORY.md`、`LEDGER.md`、`PLAN.md` 当前状态块、`plans/stage-1-pilots.md`、`README.md` 三处
  - `docs/automations/2026-10-18-round-2.md`
  - 本卡
- **关联**：`docs/spec/testing.md` §4.4、ADR-0027、ADR-0089、PL-033（`#[path]` 外置测试模块惯例）、TASK-311（同型私有 IO 边界补强先例）。

**目标**

把 `crates/tool-bus/src/bus.rs` 的私有失败边界纳入契约证据：MCP 结果信封提取、服务错误映射与传输失败映射。只加测试，不改生产行为、公共 API、依赖、schema、lint 或 CI。

**现象**

`cargo llvm-cov -p assistant-tool-bus --tests --show-missing-lines` 在 2026-10-18 的基线上显示：
`bus.rs` 仅 **116 / 153 = 75.82%** 行覆盖，未覆盖集中在 `extract_envelope`、`map_service_error`、`transport_failure` 等私有边界；公开测试只覆盖了正常挂载、调用、文本回退的间接路径。

**本卡修法**

1. 在 `bus.rs` 末尾加 `#[cfg(test)] #[path = "bus_tests.rs"] mod tests;`。
2. 在 `bus_tests.rs` 中用 `use super::*` 对私有函数做白盒契约测试：
   - `extract_envelope`：合法 `structured_content`、畸形 `structured_content`、文本块回退、缺少信封载荷 fail-closed。
   - `map_service_error`：保留 MCP error code / message，非 MCP 变体归入 `Transport`。
   - `transport_failure`：保留底层 Display 文本。
3. 目标：`bus.rs` 行覆盖 ≥ 90%，`assistant-tool-bus` crate 行覆盖 ≥ 85%。

**Out of scope（做了算漂移）**

- 不改 `bus.rs` 的生产逻辑、公共 API、错误类型、依赖、schema、lint 或 CI。
- 不追 `close_service` 的关停超时 / join 失败分支；那需要构造受控故障的 `RunningService`，本轮不为此增加生产测试钩子。
- 不重构 `fingerprint.rs` 或其他 tool-bus 模块。

**验收命令**

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test -p assistant-tool-bus
cargo test --workspace
cargo llvm-cov -p assistant-tool-bus --tests --summary-only --fail-under-lines 85
cargo run -p xtask -- hygiene
cargo run -p xtask -- card-check
cargo run -p xtask -- check-ledger
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- check-comments
cargo run -p xtask -- refscan
cargo run -p xtask -- docscan
cargo run -p xtask -- check-migrations
cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check
cargo deny check
```

**DoD**

- [ ] `bus.rs` 行覆盖率从 75.82% 提升到 ≥ 90%。
- [ ] 新增 7 个私有白盒测试，覆盖上述四类信封边界与两类服务错误映射。
- [ ] 生产逻辑零改动；新增源码仅测试模块声明与测试文件。
- [ ] 验收命令全绿。
- [ ] 9 节记录填写；`LEDGER.md` 追加一行。
- [ ] 无任何 Out of scope 文件被修改。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

- **任务**：TASK-316（tool-bus `bus.rs` 私有失败边界契约覆盖率补强）。
- **write scope**：`bus.rs` 测试模块声明、`bus_tests.rs`、台账状态同步、本卡、本轮轮次文件。
- **铁律**：1（无静默失败）、2（不可信输入先校验）、9（不得静默扩大范围）、10（契约先行）。
- **禁止**：改生产逻辑 / 公共 API / 依赖 / schema / lint / CI；不追关停超时分支。
- **验收**：见正文命令。
- **依赖**：TASK-020 / TASK-299 / TASK-301 已 Done（`LEDGER.md` 已核）。
- **疑问**：无；按预授权自动化自决推进。

### 2. 实际改动文件

- `crates/tool-bus/src/bus.rs`：末尾新增 3 个 `#[cfg(test)]` + `#[path]` 声明行；生产逻辑零改动。
- `crates/tool-bus/src/bus_tests.rs`：新增 7 个私有白盒测试，覆盖 `extract_envelope` 四条路径、
  `map_service_error` 两个分支与 `transport_failure` 文本保留。
- `tasks/TASK-316-tool-bus-bus-failure-boundary-coverage.md`：本卡。
- `plans/stage-1-pilots.md` / `PLAN.md` / `README.md` / `LEDGER.md`：按 ADR-0039 / ADR-0041 同步状态。
- `docs/automations/2026-10-18-round-2.md`：本轮留痕。

### 3. 验收输出摘要

- 基线（`origin/main` = `e1d1bef`，改动前）：
  `cargo llvm-cov -p assistant-tool-bus --tests --show-missing-lines`
  → `bus.rs 116 / 153 = 75.82%`；crate `1739 / 1866 = 93.19%` 是补测后的结果，基线 crate 为
  `assistant-tool-bus` 91.59%。
- `cargo test -p assistant-tool-bus`：
  7 个新白盒用例全过；现有 51 个集成 / schema / round-trip 用例全过；1 个 doctest 全过。
- `cargo llvm-cov -p assistant-tool-bus --tests --summary-only --fail-under-lines 85`：
  exit 0；`bus.rs` **146 / 153 = 95.42%**、crate **1739 / 1866 = 93.19%**。
- `cargo llvm-cov -p assistant-tool-bus --tests --show-missing-lines`：
  `bus.rs` 残余 `162, 314-319`；其中 `162` 是缓存配置条件的行映射伪影，`314-319` 是
  `close_with_timeout` 的 `Ok(None)` / `Err` 关停失败分支，需注入关停故障，本轮不改生产代码强追。
- `cargo fmt --all --check`：EXIT 0。
- `cargo clippy --all-targets -- -D warnings`：EXIT 0（仅既有 `unknown lint: clippy::assert_is_empty` 警告）。
- `cargo test --workspace`：EXIT 0；151 个 `test result: ok`，0 个 FAILED。
- `cargo test -p assistant-core arch::`：EXIT 0；`arch_dependencies` 5 passed + `arch_layering` 5 passed。
- `cargo run -p xtask -- hygiene`：0E/117W，PASSED；`card-check` 0E/34W，PASSED；
  `check-ledger` 0E/0W，PASSED；`memory-counts` 0E/0W，PASSED；`adr-index` 0E/0W，PASSED；
  `check-comments` 0E/68W，PASSED；`refscan` 0E/0W，scanned_files=946，PASSED；
  `docscan` 0E/264W，PASSED；`check-migrations` scanned=5 registry=5，PASSED；
  `verify-schemas` PASSED；`codegen --check` 0 drift / 0 error，PASSED；`cargo deny check` EXIT 0。
- guard：`LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` acquire / release 成功；
  结束 `guard status` = NONE。

### 4. DoD 逐条核对

- [x] `bus.rs` 行覆盖率 75.82% → **95.42%**，达到 ≥ 90%。
- [x] 新增 7 个私有白盒测试：4 条 `extract_envelope` 路径 + 2 条 `map_service_error` + 1 条 `transport_failure`。
- [x] 生产逻辑零改动：仅新增 `#[cfg(test)]` 模块声明与测试文件；未改函数体、公共 API、依赖、schema、lint、CI。
- [x] `fmt` / `clippy` / tool-bus 测试 / workspace 测试 / 覆盖率专项 / 全部 xtask 门禁 / deny 均通过。
- [x] 9 节记录填写；`LEDGER.md` 已追加一行。
- [ ] PR / CI / merge hash：待本 PR 推送与 CI 完成后回填。
- [x] 无 Out of scope 文件被修改。

### 5. 偏差

- 环境偏差：按 prompt 要求首探 `shell: "powershell"`，系统 PowerShell 与 `pwsh` 在本机均出现
  zero-output 挂起；已按 `ai-assistant-automation-round` skill 的逃生路径改用 WSL2 bash +
  绝对 Windows exe 路径执行 `cargo.exe` / `gh.exe` / `git.exe`。该偏差写入本轮轮次文件。
- 测试策略偏差：`FingerprintEntry` 字段为 `pub(crate)`，外部集成测试无法构造；本轮目标因而是
  `bus.rs` 而非 `fingerprint.rs`，并使用仓库既有的 `#[path]` 私有白盒测试先例（TASK-311 / PL-033）。
- 其他：无。

### 6. 更合理做法

- 对私有失败边界的覆盖率补强，优先选公开入口背后仍可显式构造的纯转换函数；若需要故障注入才能覆盖，
  先证明该分支的公共可达性，再决定是否值得为测试增加生产钩子。

### 7. 遗留问题

- `close_service` 的关停超时 / join 失败分支尚未覆盖；本轮明确不追。
- `fingerprint.rs` 仍有 20 / 100 行未覆盖，且主入口类型字段为 crate-private，外部集成测试不可达；
  未来若要补强，应先讨论是否增加公开构造契约或沿用私有白盒测试。
- 300+ 号段池缺口仍由 PL-118 跟踪，本卡未裁决。

### 8. 新增长期记忆

- 无新增 `FACT` / `PITFALL` / `REJECTED`；本卡仅新增私有测试证据。

### 9. 给审阅者的关注点

- 核对 `bus.rs` 只多了测试模块声明，没有生产逻辑改动。
- 核对 `bus_tests.rs` 的 7 条断言确实覆盖失败语义，而不是只增加行数。
- 核对残余 7 行未被诚实声明为完成，且没有为覆盖率放宽门禁。
