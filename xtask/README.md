# xtask — 仓库护栏与开发任务工具

## 职责

把「靠自觉」的规范变成**机器可执行的检查**，并提供一把开发期的文件互斥锁。当前提供：

- `hygiene`：仓库卫生检查（gov §5.4 的 **13** 项中已实现 3 项，口径见 ADR-0025；工具会在输出里主动声明覆盖范围）
- `memory-counts`：`MEMORY.md`「各文件当前规模」表 ↔ `docs/memory/` 实测计数是否一致（ADR-0030 D1/D2，**8** 条规则）
- `adr-index`：`docs/adr/README.md` 编号登记表 ↔ `docs/adr/NNNN-*.md` ↔ `docs/memory/decisions.md` 是否一致（ADR-0030 D3，**11** 条规则）
- `guard`：文件改写互斥锁（ADR-0028），操作 = `acquire` / `release` / `status` / `reap`
- `--list-deferred`：打印**未实现**的子命令与规则，含归属任务卡号
- 其余子命令（`verify-schemas` / `codegen` / `replay` / `check-comments` / `check-ledger` /
  `card-check`）已登记但**未实现**，运行会以退出码 3 显式失败

## 边界（不做什么）

- **只读仓库内容**：不创建、修改、删除任何**受版本控制**的文件，不访问网络。
  唯一例外是 `guard` 在 `target/locks/` 下管理的临时锁文件与放弃日志（ADR-0028 D8）——
  它们不入库、生命周期由 `guard` 自己负责。
- **不参与产品运行时**：不被任何 `crates/*` 或 `apps/*` 依赖。
- **零第三方依赖**：只用 `std`。护栏工具自身必须无供应链风险，且编译要快到
  「每次提交都能跑」（当前全量编译 < 1 s）。
- **不自动改写文档**：`memory-counts` / `adr-index` 发现不一致时**打印可直接粘贴的正确值**，
  由人来改（ADR-0030 选项 1 已否决「工具自动写回」）。
- 不做规则以外的判断：例如「这个文件该不该拆」由人决定，工具只报告「它超过了阈值」。

## 不变量（改动前必读）

1. **规则判定必须是纯函数**：输入文本 → 输出 `Finding`。IO 只在 `repowalk.rs`、
   `doccheck.rs`、`guard_store.rs` 与 `main.rs`。这是白盒测试能覆盖每条规则而不需要造临时目录的前提。
2. **无静默失败**：未实现的子命令必须显式失败并指出归属卡号；扫到 0 个文件必须告警；
   表结构解析不到必须报错（`memory/scale-table-unparsable`、`adr/registry-section-missing`）；
   IO 错误必须带上具体路径向上抛。
3. **输出确定性**：同一仓库状态两次运行输出逐字节相同（目录遍历排序 + 发现项排序 +
   相对路径统一用 `/`）。否则 CI 输出无法 diff、无法缓存。
4. **阈值与规则标识符是契约**：`hygiene/*.`、`memory/*`、`adr/*` 的规则名与 `FILE_LINES_*`
   等阈值被 CI、ADR 与任务卡引用，改动需 ADR。
5. **不放宽护栏来让自己通过**：xtask 自己也受 hygiene 检查（`cargo run -p xtask -- hygiene`
   会扫 `xtask/src`），且自身受 `memory-counts` / `adr-index` 的文档一致性门禁约束。
6. **锁的判定必须可测且不依赖真实多进程时序**：`decide_acquire` 是纯决策函数，
   测试用 `guard_testkit` 的内存存储替身，不碰磁盘。

## 模块分层

| 模块 | 职责 | 碰文件系统 |
|---|---|---|
| `cli.rs` | 解析 argv、用法文本、guard 专用选项 | 否 |
| `repowalk.rs` | 定位仓库根、遍历源文件、路径归一化 | **是** |
| `rustscan.rs` | 把源码拆成「注释列表」与「降噪代码」两个视图 | 否 |
| `hygiene.rs` | gov §5.4 卫生规则判定 | 否 |
| `deferred.rs` | 未实现项登记表 | 否 |
| `report.rs` | `Finding` / `Report` 模型与渲染 | 否 |
| `memory_table.rs` | `MEMORY.md` 规模表解析 + 行数/条目数判据（ADR-0030） | 否 |
| `memory_counts.rs` | 记忆规模一致性的 8 条规则 | 否 |
| `adr_registry.rs` | ADR 登记表 / ADR 文件状态行 / 待建号解析（ADR-0030） | 否 |
| `adr_index.rs` | ADR 编号一致性的 11 条规则 | 否 |
| `doccheck.rs` | `memory-counts` 与 `adr-index` 的 IO 边界 | **是**（只读） |
| `guard.rs` | 锁记录编解码、slug/摘要派生、`decide_acquire` 判定（ADR-0028） | 否 |
| `guard_model.rs` | guard 的请求 / 失败 / 三态读锁模型 | 否 |
| `guard_store.rs` | `LockStore` trait + 文件系统实现 + ISO-8601 时钟 | **是**（`target/locks/`） |
| `guard_runner.rs` | guard 分派 + `acquire`（等待 / 放弃 / 接管 / 回滚） | 经 `LockStore` |
| `guard_release.rs` | guard 的 `release` / `status` / `reap` | 经 `LockStore` |
| `guard_testkit.rs` | 内存锁存储替身（仅 `cfg(test)`） | 否 |
| `adr_index_tests.rs` / `guard_tests.rs` / `guard_runner_tests.rs` | 对应模块的私有单测，用 `#[path]` 外置（gov §5.4 的 600 行硬上限） | 否 |
| `main.rs` | 分派、读文件内容、呈现、退出码 | 是（只读） |

`rustscan` 存在的理由：直接对原始文本做子串匹配会误判 —— `let url = "http://x";`
里的 `//` 不是注释，字符串里的 `TODO` 字样也不是待办。抹平字面量与注释之后，
剩下的才是「真正的代码」。

**测试外置**：`adr_index` / `guard` / `guard_runner` 的 `#[cfg(test)] mod tests` 用 `#[path]`
指向同名 `*_tests.rs`。外置**不改变可见性语义** —— 那些文件里的 `mod tests` 仍是父模块的
私有子模块，因此 `use super::*;` 照常能访问私有项。

## 退出码

| 码 | 含义 |
|---|---|
| 0 | 通过（可能有 Warning） |
| 1 | 存在 Error 级发现项 → 阻塞合并 |
| 2 | 用法错误 |
| 3 | 子命令已登记但未实现（**故意不是 0**） |
| 4 | IO 或内部错误 |
| 5 | 锁获取超时（放弃；ADR-0028 D5，放弃必须通报） |

## 已实现的规则

### `hygiene`（仓库卫生）

| 规则标识符 | 级别 | 依据 |
|---|---|---|
| `hygiene/file-too-long` | > 600 行 Warning；> 900 行 Error | gov §5.4 |
| `hygiene/banned-comment-tag` | Error | naming §8（禁用标签） |
| `hygiene/missing-card-reference` | Error | naming §8（待办/占位必须带卡号） |
| `hygiene/commented-out-code` | Error（连续 ≥5 行形似代码的 `//`） | gov §5.4 / §6.1.2 |
| `xtask/no-source-files` | Warning | 本工具自证「扫到 0 个文件」不是通过 |

### `memory-counts`（记忆规模一致性，ADR-0030 D1/D2）

扫描集 = `docs/memory/` 下的 `facts.md` / `pitfalls.md` / `rejected.md` / `decisions.md` /
`open.md` ＋ `docs/memory/apps/*.md`（**自动发现**，递归；排除 `README.md` 与 `archive/`）。
新增应用档案不需要改代码；漏登记会被 `memory/file-unlisted` 抓到，报错消息直接给出应粘贴的表格行。

| 规则标识符 | 级别 | 判据 |
|---|---|---|
| `memory/file-missing` | Error | 表里登记的路径在磁盘上不存在 |
| `memory/file-unlisted` | Error | 扫描集里的文件没有出现在表中 |
| `memory/line-count-mismatch` | Error | 表中行数 ≠ 实测行数 |
| `memory/entry-count-mismatch` | Error | 表中条目数 ≠ 实测条目数 |
| `memory/index-too-long` | Error | `MEMORY.md` 自身 > **150** 行（ADR-0021 硬上限） |
| `memory/derived-total-in-prose` | Error | 正文出现「现合计」这类会随追加漂移的派生合计（ADR-0030 D2） |
| `memory/scale-table-unparsable` | Error | 规模表缺失或表头/列数不合规（铁律 1：必须失败，不得静默通过） |
| `memory/archive-threshold` | Warning | 任一被扫描文件 > **400** 行 → 触发 `docs/memory/archive/` 归档规则 |
| `memory/no-files-scanned` | Warning | 一个记忆文件都没扫到（PASSED 是假信号） |

**计数判据**（写在 `memory_table.rs`，是唯一的口径落点）：
- **行数** = 文件内容按 `\n` 切分后的行元素个数；文件以恰好一个 `\n` 结尾时即等于常识行数；空文件 = 0。
- **条目数** = 行首（**不允许缩进**）匹配 `- [YYYY-MM-DD]` 形态的行数。与 `MEMORY.md`「条目格式」一节一致。

**如何修正**：跑 `cargo run -p xtask -- memory-counts`，把输出里给出的正确行数/条目数粘进
`MEMORY.md` 的规模表。工具**不会**替你改文件（ADR-0030 选项 1）。

### `adr-index`（ADR 编号一致性，ADR-0030 D3）

三方交叉校验：`docs/adr/README.md`（登记表）↔ `docs/adr/NNNN-*.md`（文件集）↔
`docs/memory/decisions.md`（`[ADR:待建 NNNN]` 条目）。

| 规则标识符 | 级别 | 判据 |
|---|---|---|
| `adr/file-not-in-registry` | Error | 存在 `NNNN-*.md` 文件但登记表 §1 无此行 |
| `adr/registry-row-without-file` | Error | §1 表登记了某号但磁盘无对应文件 |
| `adr/pending-not-in-decisions` | Error | §2 待建表的号在 `decisions.md` 找不到待建条目 |
| `adr/decisions-not-in-pending` | Error | `decisions.md` 的待建号（减去退役清单）不在 §2 表 |
| `adr/number-collision` | Error | §1 表 ∩ §2 表 ≠ ∅ —— **这就是 0019 号双重占用事故的机器判据** |
| `adr/retired-number-reallocated` | Error | 退役清单里的号又出现在 §1 或 §2 表（退役号永久不复用） |
| `adr/next-number-wrong` | Error | 「下一个可用编号」≠ max(全部已用号) + 1 |
| `adr/missing-status-line` | Error | ADR 文件找不到 `状态：` 行 |
| `adr/status-mismatch` | Error | §1 表状态列的首个关键词 ≠ 文件状态行的状态关键词 |
| `adr/superseded-not-marked` | Error | 文件状态行 `Superseded by：` 非 `—`，但 §1 表未标 Superseded |
| `adr/registry-section-missing` | Error | 登记表缺四节锚点之一（§1 / §2 / §3 / `已退役编号：`） |

**退役清单约定**：`docs/adr/README.md` §3 必须含一行以 `已退役编号：` 开头、后接逗号分隔的
4 位编号（无退役号时写 `已退役编号：（无）`）。存在理由：`decisions.md` 是**只追加**的，
0019 那条原始待建条目会永远留在文件里；若不排除退役号，`adr/decisions-not-in-pending` 会永久报错。

**如何修正**：跑 `cargo run -p xtask -- adr-index`，按每条发现项消息里的「处置」照做
（补登记表行 / 改「下一个可用编号」/ 在 `decisions.md` 补待建条目）。

## guard — 文件改写互斥锁（ADR-0028）

解决的真实问题：多个 agent 会话 + subagent 并行 + 无人值守自动化是**默认工况**，
而「读文件 → 想 → 写回」之间没有任何原子性，两个进程各自基于旧内容写回 = 经典 lost update，
且 **git 不会报冲突**（两次写都「成功」）。风险最高的是三个只追加的公共热点文件：
`LEDGER.md`、`docs/memory/*.md`、`docs/PARKING_LOT.md`。

**语义要点**：
- **粒度** = 单个仓库相对路径。锁文件 = `target/locks/<slug>.lock`（不入库，`/target/` 已覆盖）。
- **slug** = 相对路径先按小写折叠（Windows/macOS 文件系统大小写不敏感），再把 `/` 换成 `__`、
  非字母数字换成 `-`、截断到 64 字符，最后拼 8 位 FNV-1a 十六进制摘要。纯函数、跨平台一致。
- **原子原语** = `OpenOptions::create_new(true)`（POSIX `O_CREAT|O_EXCL` / Windows `CREATE_NEW`），
  两平台都保证「不存在才创建」，不需要第三方锁库。
- **锁记录** = 行式 `key=value` 文本（不是 JSON），含 `target` / `owner` / `pid` / `task` /
  `acquired_at_unix` / `acquired_at_iso` / `intent`。必须带人类可读字段：超时放弃时，
  等待方要能说出「谁、什么时候、为了什么」拿着这把锁。
- **等待 / 超时 / 放弃 / 通报**：默认 `--timeout 30` 秒、退避轮询 100 ms。超时即放弃，并且
  ① 退出码 **5**；② stdout 打印 `-- guard-result: ABANDONED target=… held_by=… held_age_secs=… waited_ms=…`；
  ③ 往 `target/locks/abandonments.log` 追加一行；④ **协议义务**：调用方必须在 `LEDGER.md` 追加一行
  说明放弃了什么、为什么。①②③ 是工具做的，④ 是 `AGENTS.md` 规定的 —— 缺 ④ 就只是「日志」不是「通报」。
- **陈旧锁** = 纯时间判据（`--stale-after`，默认 900 秒），到期即可接管（`TAKEOVER`），
  但必须打印被接管者的完整锁记录并写进 `abandonments.log`。**刻意不做 PID 存活探测**：
  那需要第三方依赖，且 PID 复用会造成误判（旧 PID 被无关进程占用 → 判「还活着」→ 永久死锁）。
- **多文件按序获取**：`acquire` 接受多个路径时先按仓库相对路径**排序**再逐个获取；
  任何一个失败（含超时）→ **回滚已获取的全部锁**再放弃。这样两个 agent 以不同顺序请求同一组文件时不会死锁。
- **时钟回拨**：`acquired_at_unix` 在未来时判为「等待」而非「陈旧」，并在输出里标注 clock anomaly。

**协作式锁的固有弱点**：无法技术强制（agent 可以绕过它直接写文件）。缓解三层 ——
① `AGENTS.md` 把它写成义务；② review agent 的 PR 检查项；
③ 事后由 `memory-counts` / `adr-index` 发现「条目数变少」这类覆盖症状。

## 已知限制 / 技术债

- 8 项 gov §5.4 规则未实现（函数行数、参数个数、圈复杂度、重复代码、顶层目录白名单、
  依赖登记比对、空实现 stub、被跳过的测试）→ **TASK-015**
- `check-comments` / `check-ledger` / `card-check` **无任务卡认领** → `docs/PARKING_LOT.md` PL-002
- 「文档注释里用反引号引用的标签字样」是否豁免，尚未裁决 → PL-004
- 裸引用检查（`ADR-NNNN` 未写成 `[ADR:待建 NNNN]`）**本轮未实现** → PL-032（ADR-0030 D4：
  现存 2 处违规都在「只增不改」的 ADR 正文里，先实现就是一条永久红灯；正确顺序是先定豁免机制）
- `guard` 的锁文件在 `target/` 下，`cargo clean` 会全部删掉（fail-open）。已明确接受：
  guard 是**降低概率**的第二道防线，write scope + review 仍是主防线。
- `rustscan` 不是完整的 Rust 解析器：它只需要区分 代码 / 注释 / 字符串 / 字符字面量 /
  生命周期。宏内部的复杂 token 序列可能被误判（当前规则不依赖宏内部结构，故可接受）。

## 运行

```powershell
# 三个只读检查（CI 硬门禁）
cargo run -p xtask -- hygiene
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- hygiene --list-deferred
cargo run -p xtask -- hygiene --repo <路径>     # 指定仓库根（默认由编译期常量推导）

# guard：改写公共热点文件前先取锁，改完立刻释放
cargo run -p xtask -- guard acquire MEMORY.md --owner codex-1a2b3c4d --task TASK-030 --intent "追加 FACT 条目"
cargo run -p xtask -- guard status
cargo run -p xtask -- guard release MEMORY.md --owner codex-1a2b3c4d
cargo run -p xtask -- guard reap --stale-after 900    # 清理陈旧锁（会打印被清理者的记录）

cargo run -p xtask -- --help
```

相关：`docs/governance-ai-agent-execution.md` §5.1/§5.4、`docs/spec/naming.md` §10、
`docs/adr/0025-*.md`（hygiene 口径 SSOT）、`docs/adr/0028-*.md`（guard）、
`docs/adr/0030-*.md`（memory-counts / adr-index）。
