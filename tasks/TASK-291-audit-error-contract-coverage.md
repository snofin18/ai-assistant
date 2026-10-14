# TASK-291　audit 错误公开契约覆盖率补强（章程 §2 P2）

- 状态：**Done**
- 阶段：1　子阶段：**治理/质量**（跨阶段）　批次：**治理池**　依赖：无（`assistant-audit` 与 `AuditError` 已就位）　预估：S　难度：S
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031）；分界线以上正文只读，`- 状态：` 行按 ADR-0083 例外。

---

- **write scope**：`crates/audit/tests/audit_unit.rs`、`tasks/TASK-291-audit-error-contract-coverage.md`、`LEDGER.md`、`PLAN.md`、`README.md`、`plans/stage-1-pilots.md`、`docs/automations/2026-10-14-round-6.md`
- **关联**：`docs/automation-charter.md` §2 P2 / §5、`crates/audit/src/error.rs`、`crates/audit/README.md`

**目标**

仅用 `crates/audit/tests/audit_unit.rs` 的公开 API 契约测试补强 `AuditError` 覆盖率：
覆盖 JSON 变体、五种 `Display` 文案、`Error::source`、`From<rusqlite::Error>` 与
`From<serde_json::Error>` 转换，使 `error.rs` 行覆盖率从 52.27% 提升到 ≥85%；
生产源码零改动，不新增依赖。

**背景**

运行日 2026-10-14 的 P2 现场扫描显示 `crates/audit/src/error.rs` 为 23/44 行
（52.27%）。`cargo llvm-cov -p assistant-audit --tests --show-missing-lines` 证明
未覆盖行全部是公开可达的错误展示 / 原因错误链 / 转换路径，适合用 `tests/**`
公开契约测试收口；`AuditError` 的稳定 `reason_code` 与 `ErrorCategory` 映射是
审计链排障契约，不应依赖私有白盒状态。

**步骤**

1. 在本卡 write scope 内扩展 `crates/audit/tests/audit_unit.rs`，只调用公开 API。
2. 覆盖 `AuditError::Json`、`Display`、`source` 与两个 `From` 转换。
3. 用真实 `cargo llvm-cov` 对比改动前后行覆盖率，记录完整门禁输出。
4. 同步卡片状态与 `LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md`。

**DoD**

- [ ] 生产逻辑零改动；新增测试只调用公开 API、不新增依赖、测试文件 ≤600 行
- [ ] `crates/audit/src/error.rs` 行覆盖率从 52.27% 提升到 ≥85%；crate 行覆盖率保持 ≥85%
- [ ] `cargo llvm-cov -p assistant-audit --tests --fail-under-lines 85` 与 `cargo test -p assistant-audit` 全绿
- [ ] `fmt` / `clippy -D warnings` / `test --workspace` / xtask 硬门禁 / `cargo deny check` 全绿
- [ ] `LEDGER.md` / `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` 同批同步

**验收命令**

```powershell
cargo llvm-cov -p assistant-audit --tests --fail-under-lines 85
cargo test -p assistant-audit
cargo fmt --all --check; cargo clippy --all-targets -- -D warnings; cargo test --workspace
cargo run -p xtask -- hygiene; cargo run -p xtask -- check-comments; cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index; cargo run -p xtask -- refscan; cargo run -p xtask -- docscan
cargo run -p xtask -- card-check; cargo run -p xtask -- check-ledger; cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check; cargo run -p xtask -- check-migrations; cargo deny check
```

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══ -->

## 执行记录

### 1. 约束回执

【任务】TASK-291 audit 错误公开契约覆盖率补强　【目标】只加公开 API 契约测试，把
`AuditError` 行覆盖率提升到 ≥85%　【write scope】仅本卡列出的测试与状态 / 轮次文件
【铁律】无静默失败；测试不改生产行为；不扩 scope；热点文件先 guard
【禁止】改公共接口、依赖、lint/CI、schema/DB/IPC；真实 GUI / 网络 / 凭据
【验收】见上；【依赖】无；【疑问】无

### 2. 实际改动文件

- `crates/audit/tests/audit_unit.rs`（新增 1 个公开 API 契约测试，并扩展既有错误映射测试）
- `tasks/TASK-291-audit-error-contract-coverage.md`
- `LEDGER.md`（Review 行）
- `plans/stage-1-pilots.md`（新增 TASK-291 条目）
- `docs/automations/2026-10-14-round-6.md`
- `PLAN.md` / `README.md`（merge-hash 回填）

### 3. 验收输出摘要

- 改动前：`crates/audit/src/error.rs` 23/44 行（52.27%），crate 350/387 行（90.44%）。
- 改动后：`crates/audit/src/error.rs` 44/44 行（**100.00%**），crate 371/387 行
  （**95.87%**）；workspace 34374/42718 行（**80.47%**，改动前 80.43%）。
- `cargo test -p assistant-audit --test audit_unit`：9 passed，0 failed。
- `cargo llvm-cov -p assistant-audit --tests --summary-only --json
  --output-path target\audit-after.json --fail-under-lines 85`：EXIT 0。
- `cargo fmt --all --check`：EXIT 0。
- `cargo clippy --all-targets -- -D warnings`：EXIT 0（仅既有 `clippy::assert_is_empty`
  unknown-lint 警告）。
- `cargo test --workspace`：EXIT 0；xtask 汇总 469 passed，0 failed。
- `cargo test -p assistant-core arch::`：10 passed，0 failed。
- `cargo llvm-cov --workspace --summary-only --json --output-path
  target\workspace-after.json --fail-under-lines 75`：EXIT 0；34374/42718 行（80.47%）。
- `cargo run -p xtask -- replay --suite core`：2 steps / 5 changes / 0 mismatch，PASSED。
- `cargo deny check`：advisories / bans / licenses / sources 全 ok，EXIT 0。
- `apps/desktop-ui`：`pnpm lint` / `pnpm typecheck` / `pnpm test` 全 EXIT 0
  （node 78 passed + vitest 3 passed）。
- xtask 硬门禁：`hygiene` scanned=417 0E/118W、`check-comments` scanned=417
  0E/67W、`memory-counts` 0E/0W、`adr-index` 0E/0W、`refscan` scanned=885
  0E/0W、`docscan` scanned=447 0E/264W、`card-check` scanned=186 0E/34W、
  `check-ledger` 0E/0W、`verify-schemas` 0E、`codegen --check` 0 drift / 0E、
  `check-migrations` 0E/0W，全部 PASSED。

### 4. DoD 逐条核对

- 生产逻辑零改动、测试只调用公开 API、无新依赖、测试文件使用现有 229 行文件：满足。
- `error.rs` 52.27% → 100.00%（≥85%）；crate 95.87%（≥85%）：满足。
- crate 覆盖率命令、crate 测试、全 workspace 门禁、deny、UI、回放：满足。
- `LEDGER.md` / `plans/stage-1-pilots.md` 已写入 Review 留痕；merge `60947ff`
  （PR #392）后由独立 docs-only 提交同批更新 Done、`PLAN.md` 与 `README.md`。

### 5. 偏差

- `clippy --all-targets -- -D warnings` 首跑指出新增测试中的手写 match 可改为
  `let...else`；已按 lint 修复后复跑全绿，未放宽 lint。
- `pnpm` 首次在仓库根执行，因根目录无 `package.json` 得到
  `ERR_PNPM_NO_IMPORTER_MANIFEST_FOUND`；随后在正确的 `apps/desktop-ui` 目录重跑，
  lint / typecheck / test 全绿。
- 至少 14 次 `exec_command` 误用默认 bash，均为读取 / 解析 / git 命令：
  `Get-Content` / `cargo` / PowerShell cmdlet 不存在或语法被 bash 误解析；均已立即以
  `shell: "powershell"` 重跑，没有仓库垃圾文件或未跑即报通过。

### 6. 更合理做法

- 下一次 P2 选靶先同时读取 `workspace-coverage.json`、`--show-missing-lines` 和
  crate 依赖图，再决定是否需要公开契约测试；这样可避免把平台 / FFI 低覆盖文件
  误列为候选。

### 7. 遗留问题

- 无产品遗留；主 PR #392 / merge `60947ff` 与回填 PR #393 / merge `6b36cb5`
  均已落地；CI run `38600729795` / `38602644835` 记录在回填提交。

### 8. 新增长期记忆

- 无新增 FACT / PITFALL / REJECTED；本轮结论均记录在本卡与 round 文件。

### 9. 给审阅者的关注点

1. 新增断言是否覆盖真实公开错误表面，而不是依赖私有实现细节。
2. 覆盖率提升是否来自可解释的 `Display` / `source` / `From` 路径，且生产源码零改动。
