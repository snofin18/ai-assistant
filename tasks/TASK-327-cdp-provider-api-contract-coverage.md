# TASK-327　CDP provider API 契约覆盖率补强

- 状态：**Done**
- 阶段：1　子阶段：跨阶段治理　批次：治理池　依赖：TASK-048 已 Done
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md`。

---

- **write scope**：
  - `crates/platform/api/tests/cdp_contract.rs`（新增公开契约测试）
  - `crates/platform/api/README.md`（仅测试目录表新增一行）
  - `tasks/TASK-327-cdp-provider-api-contract-coverage.md`
  - `docs/automations/2026-10-21-round-1.md`
  - `plans/stage-1-pilots.md`（仅新增任务行、完成标记与当前进度）
  - `PLAN.md`（仅当前状态块）
  - `README.md`（仅状态行、`## 当前阶段`、`## 最近进展`）
  - `LEDGER.md`（只追加）
- **关联**：TASK-048、`docs/spec/testing.md` §4.4、ADR-0019、ADR-0027、ADR-0039、ADR-0041、ADR-0088、ADR-0089。

**目标**

把 `crates/platform/api/src/cdp.rs` 的纯值对象、访问器、坐标边界与连接参数校验纳入公开契约证据；只新增测试，不触发浏览器、网络、真实 GUI 或 OS 凭据。

**现象**

2026-10-21 基线：

- `cargo llvm-cov -p assistant-platform-api --tests --summary-only` 显示 crate 行覆盖 **900 / 1130 = 79.65%**。
- `crates/platform/api/src/cdp.rs` 在 package-only 覆盖中为 **0 / 175 = 0.00%**。
- `cdp.rs` 的 `CdpPoint::check_in_bounds`、CDP 不透明句柄访问器、身份/snapshot 访问器、load 状态、截图元数据与 `CdpConnectOptions` 校验尚无本 crate 直接契约测试。

**本卡修法**

1. 新增 `crates/platform/api/tests/cdp_contract.rs`，按公开 API 组织正负用例。
2. 覆盖：
   - `CdpPoint::check_in_bounds` 的闭开区间边界、非正矩形与越界点。
   - `ResolvedCdpSession` / `ResolvedCdpTarget` / `CdpTargetIdentity` / `CdpSessionSnapshot` 的构造与访问器。
   - `CdpDomSnapshot`、`CdpLoadCondition`、`CdpScreenshot` 的纯数据契约。
   - `CdpConnectOptions` 的正常参数、空 host / 空 URL 拒绝及全部访问器。
   - 可序列化值对象具备 `Serialize + DeserializeOwned` 约束。
3. 目标：`cdp.rs` 行覆盖从 **0.00%** 提升到 **≥ 85%**；crate 行覆盖不低于 85%。

**Out of scope（做了算漂移）**

- 不改 `cdp.rs` 生产逻辑、公共 API、trait 签名、`ErrorCode`、schema、依赖、lint 或 CI。
- 不启动 Edge / Chrome，不连接 CDP，不访问网络，不读写真实 profile 或凭据。
- 不测试 `CdpProvider` 的平台实现职责；那属于 Windows provider 与后续真实传输卡。
- 不重构 `assistant-platform-api` 其他模块，不追其他文件覆盖率。

**必须遵守**

- 测试不调用真实浏览器、网络、文件系统或 OS 密钥库。
- 公开 API 正负用例都要有显式断言，不得只增加行数。
- 本 crate 测试不使用 `expect` / `unwrap` / `panic!`，也不新增 `#[allow]`。
- 公共热点文件写前取 guard，写完立即释放；ADR-0041 允许面之外不改计划条目正文。
- 每条 `exec_command` 必须显式声明 `shell: "powershell"`。

**验收命令**

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test -p assistant-platform-api
cargo llvm-cov -p assistant-platform-api --tests --summary-only --fail-under-lines 85
cargo llvm-cov -p assistant-platform-api --tests --show-missing-lines
cargo test --workspace
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

**完成定义（DoD）**

- [ ] `cdp.rs` 行覆盖率从 0.00% 提升到 ≥ 85%，crate 行覆盖 ≥ 85%。
- [ ] 新增公开契约测试覆盖上述纯值对象、访问器、边界与负向路径。
- [ ] 生产逻辑零改动；新增源码仅测试文件，README 只在测试目录表加一行。
- [ ] 验收命令全绿。
- [ ] 9 节记录填写；`LEDGER.md` 追加一行。
- [ ] 无 Out of scope 文件被修改。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-327 CDP provider API 契约覆盖率补强
【目标】用离线公开契约测试把 cdp.rs 从 0.00% 提升到 ≥85%
【write scope】仅：cdp_contract.rs、platform-api README 测试表一行、本卡、本轮留痕、计划/PLAN/README 允许区域、LEDGER
【铁律】1 无静默失败 / 2 不可信输入校验 / 9 不扩大范围 / 10 契约先行
【禁止】改生产逻辑 / 公共 API / schema / 依赖 / lint / CI；不启动浏览器或连接 CDP
【验收】见正文命令 → 全部 exit 0，cdp.rs 行覆盖 ≥85%
【依赖】TASK-048 Done；已核对 LEDGER 与 origin/main
【疑问】无；按预授权 automation 自决推进
```

### 2. 实际改动文件

- `crates/platform/api/tests/cdp_contract.rs`（新增 8 个离线公开契约测试）
- `crates/platform/api/README.md`（测试目录表新增一行）
- `tasks/TASK-327-cdp-provider-api-contract-coverage.md`
- `plans/stage-1-pilots.md`（任务行与当前进度）
- `PLAN.md`（当前状态块）
- `README.md`（状态行、当前阶段、最近进展）
- `LEDGER.md`（只追加）

### 3. 验收输出摘要

- `cargo test -p assistant-platform-api` → 5 + 14 + 8 + 14 + 6 + 11 + 3 + 10 + 1 doctest 全通过，0 failed。
- `cargo llvm-cov -p assistant-platform-api --tests --summary-only --fail-under-lines 85`
  → exit 0；`cdp.rs` 行覆盖 **175 / 175 = 100.00%**，crate 行覆盖 **1075 / 1130 = 95.13%**。
- `cargo llvm-cov -p assistant-platform-api --tests --show-missing-lines`
  → `cdp.rs` 未覆盖行为空；未覆盖行仅存在于本卡范围外的既有模块。
- `cargo fmt --all --check` → exit 0。
- `cargo clippy --all-targets -- -D warnings` → exit 0；仅有既有 `clippy::assert_is_empty` unknown-lint warning。
- `cargo test --workspace` → exit 0，workspace 测试全绿；预期 ignored 真机用例保持 ignored。
- `cargo run -p xtask -- hygiene` → PASSED，447 files，0 error，136 warning（既有基线）。
- `cargo run -p xtask -- card-check` → PASSED，221 files，0 error，35 warning（既有基线）。
- `cargo run -p xtask -- check-ledger` → PASSED，0 error，0 warning。
- `cargo run -p xtask -- memory-counts` → PASSED，9 files，0 error，1 warning（既有 `paint.md` 归档阈值）。
- `cargo run -p xtask -- adr-index` → PASSED，87 files，0 error，0 warning。
- `cargo run -p xtask -- check-comments` → PASSED，447 files，0 error，70 warning（既有基线）。
- `cargo run -p xtask -- refscan` → PASSED，1013 files，0 error，0 warning。
- `cargo run -p xtask -- docscan` → PASSED，537 files，0 error，202 warning（既有基线）。
- `cargo run -p xtask -- check-migrations` → PASSED，5 files / 5 entries，0 error，0 warning。
- `cargo run -p xtask -- verify-schemas` → PASSED，5 schemas，0 error。
- `cargo run -p xtask -- codegen --check` → PASSED，0 drift，0 error。
- `cargo deny check` → advisories / bans / licenses / sources 全部 ok；仅有既有 duplicate 与未命中 license allowlist warning。

### 4. DoD 逐条核对

- `cdp.rs` 行覆盖率从 **0.00%** 提升到 **100.00%**，crate 行覆盖 **95.13%**（≥85%）。
- 新增 8 个公开契约测试，覆盖坐标边界、不透明句柄访问器周边纯值、身份 / snapshot、DOM / load / screenshot、连接参数正负路径与 serde bound。
- 生产逻辑零改动；新增源码仅测试文件，`crates/platform/api/README.md` 只增加测试目录表一行。
- 正文列出的验收命令全部 exit 0。
- 9 节执行记录已填写；`LEDGER.md` 已追加卡级事件。
- 未修改 Out of scope 文件；未启动浏览器、未连接 CDP、未读取 profile 或凭据。

### 5. 偏差

- 原 17:00 automation 在 2026-10-21 18:14 因 `429 Too Many Requests` 失败，留下未提交 WIP、4 把陈旧 guard 锁和 ACTIVE automation 目录；本次人工接管按最新 `origin/main` 保留 WIP 继续完成。
- 本次恢复过程中，启动读取和验收阶段共有 10 条命令误用默认 bash，均在 `cargo` / PowerShell cmdlet 不可用时失败（exit 127 或语法错误），未写入仓库；随后全部以显式 `shell: "powershell"` 重跑。先前独立审计阶段另有 5 条同类误用。
- 第一次修复 `assert_serde_contract` 时将其从空的 `const fn` 改为带 `TypeId` 断言的运行函数，消除了新增 `hygiene/bare-stub` warning；没有放宽 lint。

### 6. 更合理做法

- 保留原 automation 已完成的离线测试 WIP，不重新设计；仅补齐会触发 hygiene 的空辅助函数，并把卡面验证证据与恢复过程落入既有 9 节记录。
- 用 `xtask guard reap --stale-after 900` 清理已超过 15 小时的旧锁，再用最新 `origin/main` 创建正式任务分支，避免直接污染 main。

### 7. 遗留问题

- TASK-327 本身无遗留。
- Stage 1c 的真实浏览器传输、专用 profile、外部登录与 T5.1/T5.2/T5.3 安全验收仍需要真实 GUI / 人工操作，继续由后续卡承接。

### 8. 新增长期记忆

- 无；PowerShell 7.6.x 更新横幅问题及其 `POWERSHELL_UPDATECHECK=Off` 修复属于宿主环境事实，不在本卡 write scope 内，未写入项目 memory。

### 9. 给审阅者的关注点

- 测试只覆盖 `assistant-platform-api` 的离线公开契约，不证明 Windows CDP provider 的真实传输或浏览器行为。
- `assert_serde_contract` 只做编译期 trait 约束和类型非 unit 的运行检查；生产 `cdp.rs` 未改动。
- 既有 opaque handle 不可序列化约束由原有 `handles_not_serializable.rs` 继续守护。
