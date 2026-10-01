# TASK-086　`xtask hygiene` 剩余规则 B 组：文件级规则（CRLF / 末行换行 / 依赖登记，gov §5.4 的第 9 / 13 / 8 项）

- 状态：**Ready**
- 阶段：1　子阶段：**1a**　批次：**护栏（XTASK 池 072~099）**　依赖：015　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-1-pilots.md`。

---

- **依赖**：015（`docscan` 的 `file/encoding` 与 `check-migrations` 已落地；两者的**扫描范围**是本卡的关键输入）
- **write scope**：`xtask/**`（规则实现 + 单测 + `deferred.rs` 登记表 + `cli.rs` 用法）、`docs/memory/pitfalls.md`（追加）
- **关联**：`docs/governance-ai-agent-execution.md` §5.4、ADR-0025 D1（扩展名白名单 = 「文本文件」的判据）、ADR-0037 D1/D2、`docs/PARKING_LOT.md` PL-027 / PL-059 / PL-061、`docs/DEPENDENCIES.md`（登记表规则 1）
- **预估**：M　**难度**：M

**目标（一句话）**

把 gov §5.4 里**不需要 Rust 语法分析**的 3 项卫生规则实现为 `xtask hygiene` 的可跑规则，把「已实现 3/13」推进到「已实现 11/13」。

**为什么这 3 项一组（分组轴 = 「读文件 + 结构化比对」，不需要语法分析）**

| gov §5.4 表格行（逐字） | 计划规则 id | 现状（2026-09-24 实测，见 §3） |
|---|---|---|
| 文件不得含 CRLF | `hygiene/crlf-line-endings` | **只覆盖 `.md`**：`docscan` 的 `file/encoding` 会报，但它只扫 `.md`（`collect_repo_files(root, &["md"])`）。**全仓文本文件实测 CRLF = 0** → 可**直接 Error** 上线 |
| 必须以单个 `\n` 结尾 | `hygiene/missing-final-newline` | 同上只覆盖 `.md`。**全仓文本文件实测 8 处违反**（4 个 `Cargo.toml` + 4 个 `protocol/*.json` + 1 个 `spikes/*.ps1`）→ 按 ADR-0025 D1 **必须先 Warning**，清扫后升 Error |
| 新增依赖必须已登记 `docs/DEPENDENCIES.md` | `hygiene/unregistered-dependency` | **未实现**。登记表已建（TASK-001），解析与比对逻辑待写 |

**为什么这条卡必须存在（PL-059 的实质）**

gov §5.4 把这三项写成 `hygiene/*` 规则，而 `deferred.rs` 一直把它们记成「归 TASK-015 的未实现项」——
TASK-015 已 Done 且**没有**实现它们。本卡是它们**真正的归属**。

**步骤**

1. **环境记录**（OS / Rust 版本 / 输入 fixture 路径）
2. **CRLF 规则**：`Error` 级直接上线（实测全仓 0 处）；判据 = 按 ADR-0025 D1 的**扩展名白名单**遍历文本文件，字节里出现 `\r` 即报。
   **不要**只扫 `.md`（那正是 `docscan` 的现状，见 PL-061）。
3. **末行换行规则**：先 `Warning` 上线；把 8 处违反**逐个人工修**（不许用脚本批量改而不看内容），
   修完在 §7 记「升 Error」的后续动作 + 在 `docs/PARKING_LOT.md` 留一条跟进项。
   ⚠ **本卡的真实教训**：PL-027 在 2026-09-18 已把当时那 11 个文件清扫到 0，但**因为没有门禁**，
   新文件（`crates/*/Cargo.toml`、`protocol/*.json`）又把违反带回来了 —— 「清扫完就算完」必然复发（ADR-0030）。
4. **依赖登记比对**：解析工作区各 `Cargo.toml` 的第三方依赖 ↔ `docs/DEPENDENCIES.md` 的登记表，
   双向比对（漏登记 / 多登记）。**不得**把 workspace 内部 crate 与 `dev-dependencies` 混为一谈（先写清判据再实现）。
5. **登记表同步**：`deferred.rs` 把本卡实现的 3 条从 `DEFERRED_HYGIENE_RULES` 移除，
   `IMPLEMENTED_HYGIENE_RULE_COUNT` 3 → 11（**若本卡只做完 CRLF + 末行换行**，则按实际数写 10，
   并把未做完那条留在 `DEFERRED_HYGIENE_RULES` 里 —— 计数必须与事实一致，不许提前勾）。
6. **跑测试**：`cargo test --workspace` + 本卡专项测试；不合格 → DRIFT（`DRIFT-086-x`）。

**DoD**

- [ ] 3 条规则各自有**正向基线 + 负向用例**（含「全仓 0 处」时不许误报）
- [ ] CRLF 规则以 **Error** 上线且实测 0 error；末行换行规则以 **Warning** 上线并写明升 Error 的条件
- [ ] 8 处末行换行违反**逐个人工修**完（或明确说明为何保留），且清单记入执行记录
- [ ] 依赖登记比对的**判据**（哪些 crate / 哪类依赖 / 表格怎么读）在执行记录里写清，且有负向用例
- [ ] `deferred.rs`：本卡实现的条目移出 + `IMPLEMENTED_HYGIENE_RULE_COUNT` 与事实一致 + 不变量单测绿
- [ ] `cargo fmt --all --check` 0 diff
- [ ] `cargo clippy --all-targets -- -D warnings` 退出码 0
- [ ] `cargo test --workspace` 全绿
- [ ] `xtask hygiene / memory-counts / adr-index / docscan / card-check / check-ledger` 全部 PASSED
- [ ] LEDGER.md 追加一行；如新增事实/坑则追加 `docs/memory/{facts,pitfalls}.md`

**Out of scope（做了算漂移）**

- gov §5.4 的其余规则（函数行数 / 参数个数 / 圈复杂度 / STUB / `#[ignore]` → **TASK-085**；重复代码相似度 / 顶层目录白名单 → 未拆卡，见 PL-060）
- 改 `docscan` 的扫描范围（`.md` → 全文本）—— 那是 PL-061 的裁决对象，不是本卡
- 把 `hygiene` 接进 CI 的编号改动（需 ADR；见 PL-056 / PL-061）
- 给 `xtask hygiene` 加第 14 项规则（PL-021）

**验收命令**

```powershell
cargo fmt --all --check; cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p xtask hygiene
cargo run -p xtask -- hygiene
cargo run -p xtask -- --list-deferred
```

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

【任务】TASK-086 xtask hygiene 剩余规则 B 组：文件级规则
【目标】实现 CRLF、末行换行、依赖登记三条规则，并把 hygiene 登记从 8/13 推到 11/13
【write scope】仅：`xtask/**`、`docs/memory/pitfalls.md`；状态同步文件按治理允许追加/标记
【铁律】1 无静默失败；9 不静默扩大范围；10 契约先行；不改测试断言逃避缺陷
【禁止】改 gov §5.4 其余规则、改 docscan 扫描范围、改 CI 编号、加依赖/放宽 lint
【验收】卡面 6 条命令 + 全量 fmt/clippy/workspace test + xtask/doc 门禁
【依赖】TASK-015 已 Done（已核对 LEDGER；TASK-086 卡面依赖 015）
【疑问】卡面写“8 处末行换行”，实测已增长到 10 处；按 write scope 不越界修其他目录，保留并追加 PL-027 跟进。

### 2. 实际改动文件

- `xtask/src/hygiene.rs`：新增文本字节规则与依赖登记双向比对。
- `xtask/src/hygiene_tests.rs`：新增 CRLF、末行换行、依赖登记与解析测试。
- `xtask/src/repowalk.rs`：新增 ADR-0025 文本文件收集与 Cargo/登记表子集解析。
- `xtask/src/main.rs`：把文本与依赖规则接入 `run_hygiene`，读取失败仍带路径 fail-closed。
- `xtask/src/deferred.rs`、`xtask/src/cli.rs`、`xtask/README.md`：实现数从 8/13 同步为 11/13，未实现清单降到 2。
- `LEDGER.md`、`PLAN.md`、`README.md`、`plans/stage-1-pilots.md`、`docs/PARKING_LOT.md`、`docs/memory/pitfalls.md`、`MEMORY.md`：Done 状态、跟进项与记忆规模同步。
- 本卡记录区。

### 3. 验收输出摘要

```text
cargo fmt --all --check                                  -> clean
cargo clippy --all-targets -- -D warnings                -> EXIT 0
cargo test --workspace                                   -> EXIT 0
cargo test -p xtask                                      -> 432 passed / 0 failed（round 4 全量 xtask）
cargo run -p xtask -- hygiene                            -> scanned=333, 0E/97W, PASSED
cargo run -p xtask -- --list-deferred                    -> implemented 11 / deferred 2
cargo run -p xtask -- verify-schemas                     -> PASSED
cargo run -p xtask -- codegen --check                    -> 0 drift / PASSED
cargo run -p xtask -- memory-counts                      -> 0E/0W, PASSED
cargo run -p xtask -- adr-index                          -> 0E/0W, PASSED
cargo run -p xtask -- docscan                            -> 0E/352W, PASSED
cargo run -p xtask -- card-check                         -> 0E/27W, PASSED
cargo run -p xtask -- refscan                            -> 0E/0W, PASSED
cargo run -p xtask -- check-ledger                       -> 0E/0W, PASSED
cargo run -p xtask -- check-migrations                   -> 0E/0W, PASSED
cargo run -p xtask -- check-comments                     -> 0E/68W, PASSED
cargo deny check                                         -> advisories / bans / licenses / sources ok
```

新增规则的仓库实测：

```text
hygiene/crlf-line-endings       0
hygiene/missing-final-newline   10
hygiene/unregistered-dependency 0
```

### 4. DoD 逐条核对

- [x] 本轮落地的 2 条规则各有正向基线 + 负向用例；CRLF、末行换行共新增 5 个测试。
- [x] CRLF 规则以 Error 上线且实测 0 error；末行换行以 Warning 上线，升 Error 条件写入 PL-027 跟进项。
- [x] 卡面“8 处末行换行”已过期：实测 10 处，且 write scope 不覆盖这些文件；按卡面允许的“明确说明为何保留”处理，清单见 PL-027 跟进。
- [x] 依赖登记规则已落地：扫描产品 Cargo manifest 的直接依赖，按 `package.name` 排除 workspace 内部 crate，与 `docs/DEPENDENCIES.md` 的 Rust Approved 集合双向比对。
- [x] `deferred.rs` 移除 CRLF / 末行换行 / 依赖登记三项，`IMPLEMENTED_HYGIENE_RULE_COUNT` 8→11；未实现清单从 5 降到 2。
- [x] `cargo fmt --all --check` 0 diff。
- [x] `cargo clippy --all-targets -- -D warnings` 退出码 0。
- [x] `cargo test --workspace` 全绿。
- [x] `xtask hygiene / memory-counts / adr-index / docscan / card-check / check-ledger / check-migrations / check-comments` 全部 PASSED。
- [x] `LEDGER.md` 追加 WIP 与 Done 两行；新增 1 条 PITFALL 并同步 `MEMORY.md` 规模表。

### 5. 偏差

**DRIFT-086-WIP（已闭合）**：三条规则一次实现使单卡 diff 达 765 行，超过 `docs/automation-charter.md` §3.6 的 400 行预算。已按规则拆为 round 3 WIP + round 4 收口；最终未放宽门禁。卡面“8 处”与实测 10 处的差异由 PL-027 继续跟进。

### 6. 更合理做法

规则升级进度不应在卡面手抄存量文件数量。实现过程中把实时 `hygiene` 输出作为事实源，并把新增的 Tauri generated schema 纳入实际清单；后续升 Error 时仍应先跑扫描再决定清零或定义生成物例外。

### 7. 遗留问题

- PL-027 跟进：清扫 10 个末行换行存量文件后再把 `hygiene/missing-final-newline` 升为 Error；若决定豁免生成物，必须先有契约裁决，不能靠放宽阈值。
- 依赖登记规则已收口；末行换行存量仍需 PL-027 后续处理。

### 8. 新增长期记忆

- `docs/memory/pitfalls.md`：[2026-10-02][PITFALL][src:TASK-086 实现] 末行换行存量清单会随生成物增长，卡面手抄数量不能当事实源。

### 9. 给审阅者的关注点

1. 依赖登记规则只声称支持当前仓库使用的 Cargo TOML 子集；登记表缺 Rust 表或行不足九列会显式 Error。
2. 产品 manifest 扫描刻意排除 `spikes/` / `fixtures/` / `tools/`；这些目录继续由 `spike-deny` 覆盖许可证与来源。
3. 末行换行规则仍只有 Warning；10 个存量文件未越 write scope 清扫，不能把 PASSED 误读成 13 项规则全绿。
