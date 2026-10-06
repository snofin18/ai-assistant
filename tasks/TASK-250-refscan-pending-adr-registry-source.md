# TASK-250　`refscan` 待建 ADR 集合对齐登记表

- 状态：**Done（2026-10-07；PL-099 / DRIFT-W4-1 闭环；PR #262 / merge `7be80b6`）**
- 阶段：1　子阶段：1c　批次：治理池　依赖：无
- 预估：S　难度：S
- 本文件 = 卡片正文 ＋ 执行记录（ADR-0031）。分界线以上为正文（Orchestrator 所有，Implementer 只读）。
- 关联：`xtask/src/refscan.rs`、`docs/adr/README.md` §2、`docs/PARKING_LOT.md` PL-099、`docs/adr/0032-doc-rule-exemption-registry.md`

---

## 目标（一句话）

让 `xtask refscan` 的待建 ADR 集合与 `docs/adr/README.md` §2 的单一事实源保持一致，消除已转 Draft / 已建文件的编号继续被当作裸待建引用的假阳性。

## 背景（为什么现在做）

W4 已把 0016 / 0017 / 0020 / 0027 从 `[ADR:待建]` 落成 Draft，0007 也已正式建文件，
但 `ADR_BARE_PENDING` 仍保留这些旧号。当前仓库没有裸引用命中，所以门禁是“偶然绿”：
一旦后续文档按正常写法引用这些已存在的 ADR，`refscan` 就会误报，或者迫使作者使用不自然的
`[ADR:待建]` 措辞。PL-099 / DRIFT-W4-1 已确认该缺口，需要一张小的 xtask 治理卡闭环。

## write scope

- `xtask/src/refscan.rs`
- `tasks/TASK-250-refscan-pending-adr-registry-source.md`（本卡）
- `docs/PARKING_LOT.md`
- `LEDGER.md`
- `PLAN.md`、`README.md`、`plans/stage-1-pilots.md`（仅状态同步）

## In scope

- 将 `ADR_BARE_PENDING` 收敛为当前登记表 §2 的真实待建号集合。
- 复用既有 `adr_registry::parse_registry`，新增单测机器核对常量与登记表 §2 完全一致。
- 补正负测试：登记表待建号必须命中；已转 Draft / 已建文件的编号必须不命中。
- 闭环 `PL-099` / `DRIFT-W4-1`，并在轮次报告中给出实测证据。

## Out of scope（做了算漂移）

- 不改 `refscan` 命令行接口、规则标识符、退出码或输出格式。
- 不改 ADR 编号分配规则、登记表结构、ADR-0032 豁免机制或既有豁免行。
- 不新增依赖、crate、顶层目录，不操作真实 GUI。
- 不顺手重构 `find_adr_ranges` 或拆 `refscan.rs`。

## 必须遵守

- **铁律 1 / 2**：待建集合由登记表解析结果核对，缺锚点或解析异常不得静默通过。
- **ADR-0026 D2 / ADR-0030 D3**：登记表 §2 是待建号集合的权威落点。
- **ADR-0032**：既有豁免行只追加不改写；本轮不新增豁免。
- **ADR-0028**：写热点文件前先 `guard acquire`，写完立即 release。

## 验收命令

```powershell
cargo test -p xtask refscan::
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo run -p xtask -- hygiene / memory-counts / adr-index / refscan / docscan / card-check / check-ledger / check-comments / verify-schemas / codegen --check / check-migrations
```

## 完成定义（DoD）

- [ ] `ADR_BARE_PENDING` 与 `docs/adr/README.md` §2 的待建集合完全一致。
- [ ] 已转 Draft 的 0016 / 0017 / 0020 / 0027 与已建文件的 0007 均不再被判为裸待建引用。
- [ ] 登记表待建号（如 0001）仍会被 `scan_file` 正确报为 `adr/bare-pending-reference`。
- [ ] `PL-099` / `DRIFT-W4-1` 在停车位追加闭环记录。
- [ ] 全门禁绿；PR CI 11/11 SUCCESS + `MERGEABLE` + `CLEAN` + base=main 后合并并回填 merge hash。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤，
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写）

### 1. 约束回执

```text
【任务】TASK-250 refscan 待建 ADR 集合对齐登记表
【目标】用 docs/adr/README.md §2 校正 ADR_BARE_PENDING，并补正负测试闭环 PL-099 / DRIFT-W4-1
【write scope】xtask/src/refscan.rs、本卡、PARKING_LOT、LEDGER、PLAN、README、plans/stage-1-pilots.md
【铁律】1 无静默失败；2 输入先校验；3 单一事实源；10 契约先行；12 资源有界
【禁止】不改 schema / IPC / ErrorCode / 公共 trait；不新增依赖/crate/顶层目录；不操作真实 GUI
【验收】refscan 专项测试 + fmt / clippy / workspace tests / xtask 十一项门禁
【依赖】PL-099 / DRIFT-W4-1 已确认开放；无阻塞前置卡
【疑问】无
```

### 2. 实际改动文件

- `xtask/src/refscan.rs`：`ADR_BARE_PENDING` 收敛为 `docs/adr/README.md` §2 的真实待建集合；
  新增登记表一致性单测与 3 条正负测试（真实待建号命中、Draft / 已建号不命中）。
- `tasks/TASK-250-refscan-pending-adr-registry-source.md`：本卡正文、执行记录与最终状态。
- `docs/PARKING_LOT.md`：追加 `PL-099` / `DRIFT-W4-1` 闭环行。
- `LEDGER.md`：追加 TASK-250 InProgress 与 merge-hash 回填 Done 两行。
- `PLAN.md`、`README.md`、`plans/stage-1-pilots.md`：仅完成状态 / 当前状态块同步。
- `docs/automations/2026-10-07-round-3.md`（新增）：本轮产物与偏差记录。

### 3. 验收输出摘要

- `cargo test -p xtask refscan::` → **11 passed / 0 failed**。
- `cargo run -p xtask -- refscan` → **scanned_files=737，0 error(s), 0 warning(s)，PASSED**。
- `cargo fmt --all --check` → EXIT 0。
- `cargo clippy --all-targets -- -D warnings` → EXIT 0（仅仓库既有 `clippy::assert_is_empty` unknown-lint 提示）。
- `cargo test --workspace` → EXIT 0。
- xtask 十一项门禁全 EXIT 0：`hygiene` / `memory-counts` / `adr-index` / `refscan` / `docscan` /
  `card-check` / `check-ledger` / `check-comments` / `verify-schemas` / `codegen --check` / `check-migrations`。
- PR #262 `pull_request` CI：**11/11 SUCCESS**，pending=0、failed=0；合并前 `baseRefName=main`、
  `mergeable=MERGEABLE`、`mergeStateStatus=CLEAN`、`state=MERGED`。

### 4. DoD 逐条核对

- [x] `ADR_BARE_PENDING` 与 `docs/adr/README.md` §2 的待建集合完全一致。
- [x] 已转 Draft 的 0016 / 0017 / 0020 / 0027 与已建文件的 0007 均不再被判为裸待建引用。
- [x] 登记表待建号（0001）仍会被 `scan_file` 正确报为 `adr/bare-pending-reference`。
- [x] `PL-099` / `DRIFT-W4-1` 在停车位追加闭环记录。
- [x] 全门禁绿；PR CI 11/11 SUCCESS + `MERGEABLE` + `CLEAN` + base=main 后合并，merge hash 已回填。

### 5. 偏差

两项过程偏差，均未影响受控文件：

1. 七次 `exec_command` 的 `shell` 参数误设为 `bash`，造成 `cargo` / `Get-Content` / `rg`
   不适配；其中一次由 bash 命令替换生成了 20 个根目录 0 字节未跟踪乱码文件。已逐一解析绝对路径、
   确认全部位于 `D:\csart\ai-assistant` 且未被 git 跟踪后，用 .NET 文件 API 删除；`git status -uall`
   确认这些文件归零。实现与验收命令均以 PowerShell 重跑，未伪造证据。
2. `PLAN.md` / `README.md` / `plans/stage-1-pilots.md` / `docs/PARKING_LOT.md` / `LEDGER.md`
   均在 `guard acquire` 后写入并将在本 PR 完成后统一释放。

### 6. 更合理做法

沿用最小修复：登记表 §2 继续作为唯一事实源，`refscan` 常量由单测机器核对，不改扫描流程。
这样既消除假阳性，又保持 `scan_file` / 命令行接口和既有豁免行号不变。

### 7. 遗留问题

无。若未来再次把待建号转成 Draft 或正式 ADR，只需同时更新登记表 §2；`refscan` 单测会拦住残留。

### 8. 新增长期记忆

无新增 FACT / PITFALL / REJECTED。`PL-099` / `DRIFT-W4-1` 已在 `docs/PARKING_LOT.md` 标为闭环。

### 9. 给审阅者的关注点

1. 重点确认 `ADR_BARE_PENDING` 与登记表 §2 的集合相等，且测试不是通过放宽断言实现。
2. 确认 0007 / 0016 / 0017 / 0020 / 0027 不再命中，而真实待建号 0001 仍命中。
3. 确认 `docs/adr/0032-doc-rule-exemption-registry.md` 的既有行号未漂移，未新增豁免。
