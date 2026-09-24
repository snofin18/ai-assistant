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

### 2. 实际改动文件

### 3. 验收输出摘要

### 4. DoD 逐条核对

### 5. 偏差

### 6. 更合理做法

### 7. 遗留问题

### 8. 新增长期记忆

### 9. 给审阅者的关注点
