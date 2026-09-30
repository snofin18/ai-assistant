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
【依赖】TASK-015、TASK-039、TASK-212（均 Done/已完成，TASK-212 已清零 9 处真实违规）
【疑问】DRIFT-087-1 已由人类派单 TASK-212 修复；gate-selftest 的成功 run number 待推送后回填。
```

### 2. 实际改动文件

| 文件 | 改动 |
|---|---|
| `xtask/src/comments.rs`（新增，600 行） | naming §10 的 8 条规则实现：①② 复用 hygiene 判据（同一规则 id），③ 公共 API 文档、④ 模块头、⑤ unsafe SAFETY、⑥ PITFALL 格式、⑦ 受控词汇、⑧ 缩写；`rule_coverage()` 自证 8 条全部有实现位置 |
| `xtask/src/comments_tests.rs`（新增，272 行） | `comments` 的私有单测（用 crate 既有的 `#[path]` 外置约定），27 条用例 |
| `xtask/src/main.rs` | 声明 `mod comments`；分派 `check-comments` → 新增 `run_check_comments`（遍历 + 排序 + `-- rule-coverage` 行）；两处 deferred 测试改用 `replay-skeleton` |
| `xtask/src/deferred.rs` | 从 `DEFERRED_COMMANDS` 移除 `check-comments`；删除随之失去引用的 `UNASSIGNED_CARD` |
| `xtask/src/cli.rs` | 用法文本把 `check-comments` 从「未实现」移到已实现列表 |
| `.github/workflows/gate-selftest.yml` | 新增 fmt / clippy / build 三个 N3 canary job：各含正向基线 + 注入坏样本 + 具体退出码/故障文本断言 + 还原 |
| `.github/workflows/ci.yml` | 新增 `[HARD #15] xtask check-comments`，并把头部硬门禁说明同步到 #15 |
| `docs/adr/0019-hard-gate-negative-verification.md` | 登记表 #1/#2/#3/#10 转 N3 ✅；新增 #15 check-comments 的 N1+N2 登记 |
| `xtask/README.md` | 把 `check-comments` 从未实现列表移到已实现清单，并登记 `comments.rs` 模块 |

### 3. 验收输出摘要

```text
cargo fmt --all --check                → PASS（0 diff）
cargo clippy --all-targets -- -D warnings → PASS（exit 0）
cargo test -p xtask                    → PASS（411 passed / 0 failed，其中 comments 模块 27 条用例）
cargo test --workspace                 → PASS（1041 passed / 0 failed）
cargo run -p xtask -- hygiene          → PASS（scanned=292，0 error，4 warning —— 与基线一致）
cargo run -p xtask -- check-comments   → PASS：扫描 292 个文件，0 error / 67 warning
                                          rule-coverage 行打印 8 条规则各自的实现位置
cargo run -p xtask -- --list-deferred  → 只剩 replay-skeleton（check-comments 已移出）
cargo run -p xtask -- refscan          → PASS（scanned=536，0 error / 0 warning）
cargo run -p xtask -- docscan          → PASS（0 error / 397 warning，既有基线）
cargo run -p xtask -- card-check       → PASS（0 error / 27 warning，既有基线）
cargo run -p xtask -- check-ledger     → PASS（0 error / 0 warning）
cargo run -p xtask -- memory-counts    → PASS（scanned=8，0 error / 0 warning）
cargo run -p xtask -- adr-index       → PASS（scanned=40，0 error / 0 warning）
gate-selftest                         → 首跑 36598318298 FAILURE（clippy 门禁真实 exit 101，
                                          但断言钉错为下划线 lint 名）；修正渲染口径为
                                          `-D clippy::unwrap-used` / `-D clippy::dbg-macro`
                                          并提交 d4d38ac 后，run 36598959358 SUCCESS：
                                          fmt / clippy / build / deny / spike-deny 五个 job
                                          的正向与负向步全部 success
```

### 4. DoD 逐条核对

- [x] **fmt / clippy / build 三类 canary**：实现已完成（正向 + 负向 + 具体退出码/故障文本断言）；gate-selftest run `36598959358` 五个 job 全部 success。
- [x] **`check-comments` 不再是 exit 3 stub，负向/正向测试齐全**：实现 8 条规则；27 条单测含每条的通过/违规/边界用例，以及「`pub const fn` 不得把 `fn` 当名字」「多行 SAFETY 说明」「空行切断 SAFETY 组」「`#[doc]` 属性算文档」四条实跑发现的回归用例。
- [x] **CI 将 `check-comments` 作为真实门禁**：`ci.yml` 的 `[HARD #15]` 已接入；TASK-212 已把 9 处 Error 清零，不会形成永久红灯。
- [x] **ADR-0019 登记表同步 / PL-018 可关闭**：登记表已同步；PL-018 以 gate-selftest run `36598959358` 为成功证据关闭。
- [x] **gate-selftest 至少一次成功 run 记录在 LEDGER**：run `36598959358` 于 2026-09-30 记录。
- [x] **未修改 Out of scope 文件**：TASK-212 按人类派单单独修复 9 处真实违规；本卡未改 core/policy/task-engine 行为。

### 5. 偏差

- **DRIFT-087-1（已由 TASK-212 裁决/修复）**：首次真跑的 9 条 Error 已按推荐方案 ① 立卡修复，只加注释、不改可执行语句；`check-comments` 当前 0 error。无新增偏差。

**gate-selftest 首跑失败（非 DRIFT，卡内修复）**：run `36598318298` 中 clippy 负向步证明门禁真实返回 exit 101，但 canary 断言写成了 Clippy 源码配置名的下划线形式，而 Clippy 诊断输出渲染为连字符形式。修复仅改断言与登记口径为 `-D clippy::unwrap-used` / `-D clippy::dbg-macro`，仍要求 exit 101 且两个 lint 都被指名；未放宽门禁。

### 6. 更合理做法

**规则判据用「实跑反例」逼出来，而不是一次写死**：本卡实现过程中，第一次真跑报 502 warnings / 97 errors，其中绝大部分是检测器自己的 bug —— `pub const fn` 被当成名字叫 `fn`（53 条噪声）、多行 `// SAFETY:` 只看紧邻上一行（49 条误报）、`#[doc = concat!(...)]` 宏文档不被认作文档、测试辅助模块与生成代码被当成"公共 API"。逐条修完后降到 67 warnings / 9 errors，剩下的 9 条才是**仓库真实存在的**违规。这也说明：**新门禁必须先跑真实仓库，再谈接阻断** —— 否则接上去的是一堆误报。

**⑧ 的判据从「短词」改成「可判定」**：原计划是「长度 ≤4 且不在 23 个白名单里」，实跑后为了压误报只能不断往豁免表里加 `read` / `list` / `name` 这类普通英文词 —— 一张靠"看着顺眼"增长的豁免表不可审计。改为两条可判定判据：**无元音的 2~4 字母词干**（`tgt` / `cfg` / `ptr`）+ **显式缩写黑名单**（`buf` / `val` / `idx`）。新增黑名单词需要理由。

### 7. 遗留问题

- **`deferred.rs` 的 `replay-skeleton` 条目疑似过期**：它的 reason 写「真实 fixture + diff 留待 TASK-034 完整版」，而 TASK-034 已 Done。它现在是 `--list-deferred` 与 `deferred-inventory` CI 步骤的唯一对象；是否删除需治理裁决（属 PL-059 同型：登记表指向已完成的卡）。

### 8. 新增长期记忆

无新增 `docs/memory/*` 条目（实现经验写在本卡 §6；DRIFT-087-1 的处置已记录在 TASK-212 与本卡 §5）。

### 9. 给审阅者的关注点

1. **三类 canary 的退出码断言**：fmt=1、clippy=101、build=101，且分别钉住 `Diff in` / `-D clippy::unwrap-used`+`-D clippy::dbg-macro` / `error[E0425]`；请确认没有把断言弱化为“非零即可”。
2. **坏样本还原路径**：每个负向步都先备份 `xtask/src/main.rs`，捕获退出码后立即还原；请核对 CI 输出中正向步与负向步的断言都执行。
3. **check-comments 接入时机**：本 PR 同时包含 TASK-212 的 9 处注释修复；若拆分提交，必须保持 TASK-212 先于 #15 硬门禁生效，否则主 CI 会按设计变红。
