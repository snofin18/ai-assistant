# TASK-087　CI 硬门禁负向验证与 `check-comments` 落地

- 状态：**Ready**
- 阶段：1　子阶段：1a 补救　批次：A5-REMEDIATION　依赖：015、039
- 预估：L　难度：L
- 本文件 = 卡片正文 ＋ 执行记录（ADR-0031）。
- 关联：ADR-0019、gov §5.1/§5.4、`xtask/src/deferred.rs`、`.github/workflows/**`

## 目标

关闭 PL-018 并实现 gov #15：为 fmt / clippy / build 增加可执行的负向验证，
实现 `check-comments` 的真实检查与 CI 接线，删除“未实现 stub”状态。

## In scope

- `.github/workflows/gate-selftest.yml`、`.github/workflows/ci.yml`。
- `xtask/src/**`、`xtask/README.md` 中与 `check-comments` 相关的实现。
- `docs/adr/0019-hard-gate-negative-verification.md` 登记表。
- `docs/PARKING_LOT.md`、测试与本卡记录。

## Out of scope

- UI Prettier/ESLint/Vitest。
- commitlint。
- 放宽现有 lint 或删除任何门禁。
- 修改 core/policy/task-engine 的产品行为。

## 必须遵守

- 每个新硬门禁必须有 N1/N2/N3 负向验证。
- canary 断言必须绑定具体失败模式，不能只断言“非零退出”。
- `check-comments` 必须按 `docs/spec/naming.md` 实现，未知规则不得静默忽略。
- 先证明门禁会红，再接入阻断。
- ADR-0019 登记表与 workflow 同步，否则不得宣称关闭 PL-018。

## 验收命令

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p xtask
cargo run -p xtask -- check-comments
cargo run -p xtask -- hygiene
cargo run -p xtask -- refscan
cargo run -p xtask -- docscan
```

另须手工触发 `gate-selftest` 并记录成功 run。

## 完成定义（DoD）

- [ ] fmt / clippy / build 三类 canary 全部可执行且绑定具体失败模式。
- [ ] `check-comments` 不再是 exit 3 stub，负向/正向测试齐全。
- [ ] CI 将 `check-comments` 作为真实门禁。
- [ ] ADR-0019 登记表同步，PL-018 可关闭。
- [ ] gate-selftest 至少一次成功 run 记录在 LEDGER。
- [ ] 未修改 Out of scope 文件。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写）

### 1. 约束回执

```text
【任务】TASK-087 CI 硬门禁负向验证与 check-comments 落地        【目标】关闭 PL-018：为 fmt/clippy/build 增加负向验证；把 check-comments 从 exit 3 stub 变成真实现并接入 CI
【write scope】仅：.github/workflows/**、xtask/src/**、xtask/README.md（check-comments 相关）、docs/adr/0019-*.md、docs/PARKING_LOT.md、测试与本卡记录
【铁律】1 无静默失败（未实现必须可见）；9 不得静默扩大范围（超出 write scope 必须停下升级）；10 契约先行（规则集来自 naming §10）
【禁止】UI Prettier/ESLint/Vitest；commitlint；放宽现有 lint 或删除门禁；修改 core/policy/task-engine 的产品行为
【验收】cargo fmt/clippy/test --workspace；cargo test -p xtask；xtask check-comments / hygiene / refscan / docscan；手工触发 gate-selftest 并记录 run
【依赖】TASK-015、TASK-039（均 Done，已核对 LEDGER）
【疑问】**已在执行中命中**：check-comments 首次真跑发现 9 处真实违规位于 `crates/ipc` / `crates/policy` / `crates/platform/windows` —— 不在本卡 write scope 内 → 见 §5 DRIFT-087-1，已停止接入阻断。
```

### 2. 实际改动文件

| 文件 | 改动 |
|---|---|
| `xtask/src/comments.rs`（新增，600 行） | naming §10 的 8 条规则实现：①② 复用 hygiene 判据（同一规则 id），③ 公共 API 文档、④ 模块头、⑤ unsafe SAFETY、⑥ PITFALL 格式、⑦ 受控词汇、⑧ 缩写；`rule_coverage()` 自证 8 条全部有实现位置 |
| `xtask/src/comments_tests.rs`（新增，272 行） | `comments` 的私有单测（用 crate 既有的 `#[path]` 外置约定），27 条用例 |
| `xtask/src/main.rs` | 声明 `mod comments`；分派 `check-comments` → 新增 `run_check_comments`（遍历 + 排序 + `-- rule-coverage` 行）；两处 deferred 测试改用 `replay-skeleton` |
| `xtask/src/deferred.rs` | 从 `DEFERRED_COMMANDS` 移除 `check-comments`；删除随之失去引用的 `UNASSIGNED_CARD` |
| `xtask/src/cli.rs` | 用法文本把 `check-comments` 从「未实现」移到已实现列表 |

### 3. 验收输出摘要

```text
cargo fmt --all --check                → PASS（0 diff）
cargo clippy --all-targets -- -D warnings → PASS（exit 0）
cargo test -p xtask                    → PASS（411 passed / 0 failed，其中 comments 模块 27 条用例）
cargo test --workspace                 → PASS（1041 passed / 0 failed）
cargo run -p xtask -- hygiene          → PASS（scanned=292，0 error，4 warning —— 与基线一致）
cargo run -p xtask -- check-comments   → 扫描 291 个文件：9 error / 67 warning
                                          rule-coverage 行打印 8 条规则各自的实现位置
cargo run -p xtask -- --list-deferred  → 只剩 replay-skeleton（check-comments 已移出）
```

### 4. DoD 逐条核对

- [ ] **fmt / clippy / build 三类 canary**：**未做**（被 §5 的 DRIFT 阻断，见 §7）。
- [x] **`check-comments` 不再是 exit 3 stub，负向/正向测试齐全**：实现 8 条规则；27 条单测含每条的通过/违规/边界用例，以及「`pub const fn` 不得把 `fn` 当名字」「多行 SAFETY 说明」「空行切断 SAFETY 组」「`#[doc]` 属性算文档」四条实跑发现的回归用例。
- [ ] **CI 将 `check-comments` 作为真实门禁**：**未接**。它当前在真实仓库上返回 exit 1（9 处真实违规），接成阻断会得到一条**永久红灯** —— 正是 ADR-0019 明确要消灭的东西（"永久红灯会让人学会忽略 CI"）。需先裁决 §5。
- [ ] **ADR-0019 登记表同步 / PL-018 可关闭**：**未做**（同上，等裁决）。
- [ ] **gate-selftest 成功 run 记录在 LEDGER**：**未做**。
- [x] **未修改 Out of scope 文件**：未改任何产品代码；`crates/ipc` 等 9 处违规**未擅自修复**（见 §5）。

### 5. 偏差

- **DRIFT-087-1（超出 write scope，已停止）**：`check-comments` 首次真跑（扫描 291 个文件）报出 **9 条 Error**，全部位于本卡 write scope 之外：
  - `crates/policy/src/dsl.rs:13`：`pub fn parse_rule_set` 缺文档注释（规则 ③）
  - `crates/ipc/src/frame.rs:19`：`pub struct FramePrefix` 缺文档注释（规则 ③）
  - `crates/platform/windows/src/uia/actions.rs:71/92/124/157/197/323` 与 `tree.rs:189`：`unsafe` 块缺少 `// SAFETY:`（规则 ⑤）
  **影响**：把 `check-comments` 接成 CI 硬门禁会立刻让主 CI 永久变红，违反本卡「先证明门禁会红，再接入阻断」与 ADR-0019「不许永久红灯」。
  **建议（三选一，需人类裁决）**：① 立一张小卡（或把 `crates/ipc` / `crates/policy` / `crates/platform/windows` 加进本卡 write scope）修掉这 9 处，再接入阻断；② 把规则 ③⑤ 的判据放宽到能覆盖现状（**不推荐** —— 那是"为让门禁变绿而放宽规则"，正是本卡禁止项）；③ 先只接入当前为绿的部分规则（同样不推荐，等于把 8 条规则拆成两套口径）。
  **已停止的工作**：未修改上述产品文件；未把 `check-comments` 写进 `ci.yml`；未改 ADR-0019 登记表（避免登记"已完成"）。

### 6. 更合理做法

**规则判据用「实跑反例」逼出来，而不是一次写死**：本卡实现过程中，第一次真跑报 502 warnings / 97 errors，其中绝大部分是检测器自己的 bug —— `pub const fn` 被当成名字叫 `fn`（53 条噪声）、多行 `// SAFETY:` 只看紧邻上一行（49 条误报）、`#[doc = concat!(...)]` 宏文档不被认作文档、测试辅助模块与生成代码被当成"公共 API"。逐条修完后降到 67 warnings / 9 errors，剩下的 9 条才是**仓库真实存在的**违规。这也说明：**新门禁必须先跑真实仓库，再谈接阻断** —— 否则接上去的是一堆误报。

**⑧ 的判据从「短词」改成「可判定」**：原计划是「长度 ≤4 且不在 23 个白名单里」，实跑后为了压误报只能不断往豁免表里加 `read` / `list` / `name` 这类普通英文词 —— 一张靠"看着顺眼"增长的豁免表不可审计。改为两条可判定判据：**无元音的 2~4 字母词干**（`tgt` / `cfg` / `ptr`）+ **显式缩写黑名单**（`buf` / `val` / `idx`）。新增黑名单词需要理由。

### 7. 遗留问题

- **fmt / clippy / build 三类 canary 未写**：它们的形状与既有 deny canary 相同（注入坏样本 → 断言**具体退出码** → 还原），但因为本卡停在 DRIFT-087-1，未继续。
- **`deferred.rs` 的 `replay-skeleton` 条目疑似过期**：它的 reason 写「真实 fixture + diff 留待 TASK-034 完整版」，而 TASK-034 已 Done。它现在是 `--list-deferred` 与 `deferred-inventory` CI 步骤的唯一对象；是否删除需治理裁决（属 PL-059 同型：登记表指向已完成的卡）。

### 8. 新增长期记忆

无新增 `docs/memory/*` 条目（本卡停在 DRIFT，未形成可复用的长期结论；实现经验写在本卡 §6）。

### 9. 给审阅者的关注点

1. **本卡未完成，请勿按"Done"合并**：`check-comments` 已可用且自测齐全，但**未接入 CI**，DoD 的 4 项未达成。
2. **DRIFT-087-1 需要裁决**：9 处真实违规在 write scope 之外。我的建议是**方案 ①**（立小卡或扩 scope 修掉），因为方案 ②③ 都是"为让门禁变绿而放宽规则"。
3. **规则 ③ 的判据范围**：我只对「`crates/*/src/**` 且非生成物」判文档缺失。测试辅助模块（`tests/common/mod.rs`，28 条）与 `xtask` 二进制、`protocol` 生成代码都排除了 —— 理由是它们没有"外部读者"。若你认为测试辅助函数也该有文档，这会新增 28 条 Error，需要一并纳入 §5 的裁决。
