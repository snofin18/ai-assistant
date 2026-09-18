# ADR-0019　硬门禁必须配「负向验证」（元门禁）

状态：Accepted　日期：2026-09-17　Supersedes：—　Superseded by：—
来源：人类裁决 **DRIFT-001-2-c**（2026-09-17）
关联：`docs/governance-ai-agent-execution.md` §5.1、`.github/workflows/ci.yml`、
`.github/workflows/gate-selftest.yml`、`docs/PARKING_LOT.md` PL-016 / PL-018、
`tasks/TASK-001-repo-skeleton.md` §5.2、`MEMORY.md` §5 的 2026-09-17 cargo-deny-action 条目

---

## 背景（为什么现在要决定）

`deny.toml` 里曾写成 `db-path = ["target/deny-db"]`（数组）。在 cargo-deny 0.20.2 下这会让
**配置文件解析直接失败**（`error[wanted]: expected a string`）。

关键点：这类失败的后果**不是"CI 变红"，而是"门禁根本没跑"**。而"没跑"与"跑过且通过"
在只看绿灯时无法区分。本次它没被抓住，有两个叠加原因：

1. CI 用 `EmbarkStudios/cargo-deny-action@v1` 且**未钉 cargo-deny 版本**，与本地 0.20.2
   不是同一个解析器（PL-016）；
2. **本仓库从未推送过** —— `origin` 于 2026-09-17 才建立，GitHub Actions **一次都没真正执行**。
   也就是说，TASK-001 交付时"CI 三平台全绿"这条 DoD 实际上**从未被机器验证过**。

这与铁律 1「无静默失败」是同一类问题，只是发生在**护栏自身**上：
护栏静默失效比没有护栏更危险，因为它提供虚假的安全感，而项目的所有信任都建立在护栏上。

## 决策（一句话）

**每一项硬门禁必须配一条「负向验证」—— 能证明该门禁在"应该失败时确实失败"的机制。**
只证明"这次是绿的"不算验收；必须同时证明"该红的时候会红"。

## 允许的三种形式

| 形式 | 说明 | 触发时机 | 现有实例 |
|---|---|---|---|
| **N1 单元测试负向用例** | 护栏自身的测试里含"喂坏输入必须报错"的用例 | 每次 `cargo test` | `xtask` 任务卡 §7.3：占位卡号被拒、仓库根指向文件必须失败、拼错命令必须是"用法错误"而非"未实现"、登记表数量不自洽必须测试失败 |
| **N2 CI 内建显式失败步骤** | 主 CI 里有一步专门断言某命令以**特定非零退出码**失败 | 每次 push | `ci.yml` 的 `deferred-inventory` job：断言未实现子命令必须 exit 3 |
| **N3 canary 工作流** | 独立 workflow，注入已知坏样本，断言门禁变红 | `workflow_dispatch` 手工触发 | `gate-selftest.yml` 的 deny 段（本 ADR 落地） |

### 为什么 canary 用 N3、且不配 `schedule`

- **不进主 CI**：注入坏样本需要额外安装 cargo-deny（`cargo install --locked` 约 5~8 分钟），
  放进每次 push 会让 CI 时长翻倍，而 canary 的价值在于"低频但确定"。
- **不配定时**：GitHub 会在仓库 **60 天无活动**后自动停用 scheduled workflow。那会造成
  "以为每月在自检、其实早就停了"的**静默失效** —— 正是本 ADR 要消灭的东西。
- 改为**规程约束**（可被 LEDGER 审计）：
  ① 改动任何硬门禁（软转硬、改命令、改版本钉法）后必须手工跑一次 `gate-selftest`；
  ② 每个阶段末评审（gov §7.2）必须跑一次并把运行编号记入 `LEDGER.md`。

## 硬门禁负向验证登记表（截至 2026-09-17）

| gov §5.1 # | 硬门禁 | 形式 | 负向验证内容 | 状态 |
|---|---|---|---|---|
| 1 | `cargo fmt --all --check` | — | 注入一个格式错误的 `.rs`，断言 exit ≠ 0 | ❌ **缺**（PL-018） |
| 2 / 3 | `cargo clippy -D warnings`（含 `[workspace.lints]` 禁用项） | — | 注入一个含 `unwrap()` / `dbg!` 的 `.rs`，断言 exit ≠ 0 | ❌ **缺**（PL-018） |
| 4 | `cargo test --workspace` | N1 | 99 个测试中大量为负向用例（任务卡 §7.3） | ✅ |
| 8 | `cargo deny check` | **N3** | 正：`cargo deny check licenses bans sources` → exit 0；负：`cargo deny --config <坏配置>`（`db-path` 写成数组）→ 断言 exit 1 **且** stderr 含 `expected a string` | ✅ 本 ADR 落地 |
| **8b** | `cargo deny check licenses sources`（逐个 `spikes/*/Cargo.toml`；ADR-0024 D2） | **N3** | 正：复刻 ci.yml 的枚举逻辑跑真实 `spikes/`（含 `windows =0.62.2`）→ `licenses ok, sources ok` exit 0；负：注入本地 path 依赖 + `license = "GPL-3.0-only"` 的 fixture → 断言 exit ≠ 0 **且**输出含 `license is not explicitly allowed` **且**含 `GPL-3.0-only`（本机 2026-09-18 实测 exit **4**） | ✅ 本行随 ADR-0024 落地 |
| 10 | `cargo build --release` | — | 注入一个编译不过的 `.rs`，断言 exit ≠ 0 | ❌ **缺**（PL-018） |
| 12 | `xtask hygiene` | N1 + N2 | 测试含三条规则各自的 通过 / 告警 / 失败 / 边界 四类用例；CI 另有 `deferred-inventory` 断言未实现子命令 exit 3 | ✅ |
| **12b** | `xtask memory-counts` + `xtask adr-index`（文档一致性；ADR-0030 D5，**2026-09-18 追加**） | **N1** | `memory_counts.rs`（8 条规则）与 `adr_index.rs`（11 条规则）的单测里，**每条规则都有至少一个「喂不一致输入 → 必须产生该规则 id 的 Error」的负向用例**；另含两条「表结构解析不到必须失败而不是静默通过」的用例（`memory/scale-table-unparsable`、`adr/registry-section-missing`，铁律 1），与「一致输入 → 零发现项」的正向用例、以及「同一输入两次结果逐字节相同」的确定性用例。ADR-0030 验证方式 2 / 4 另给了两处**真仓库**负向实证：把规模表里 `facts.md` 的行数改成 `1` → exit 1 且输出含 `memory/line-count-mismatch`；往登记表 §2 临时加一行 `0019` → exit 1 且输出含 `adr/number-collision`（即 0019 双重占用事故的机器判据） | ✅ 本行随 ADR-0030 落地 |

> **规则（自本 ADR 生效起适用于 TASK-015 及之后所有卡）**：
> 把软门禁转硬的那张卡，**必须同时提交该门禁的负向验证**（N1 / N2 / N3 任一）并在登记表补一行，
> 否则不得转硬。理由：软转硬的时机正是"这道防线第一次真正开始拦人"的时机，
> 此时若不证明它拦得住，等于把未验证的信心写进章程。

## 本 ADR 的落地动作

1. **`ci.yml`**：`EmbarkStudios/cargo-deny-action@v1` → **`@v2.1.1`**（钉到具体 release；
   该 release 标题即 "Release 2.1.1 - cargo-deny 0.20.2"，与本地开发机同版本）
   → 消除 PL-016 的"两边解析器不同"根因。
2. **新增 `.github/workflows/gate-selftest.yml`**：deny 门禁的 N3 canary（正向 + 负向两步）。
3. **`gov §5.1`** 表后追加一段，声明上述规则并指向本 ADR 的登记表。
4. 缺失的 3 项（fmt / clippy / build）登记为 **PL-018**，归 TASK-015 处置。
5. **本地验收清单**追加一条（PL-016 建议②）：`cargo deny check licenses bans sources`
   —— 它不需要联网 clone advisory-db，专门用来证明 `deny.toml` 能被当前版本的 cargo-deny 加载。

## 影响（需要改的 spec / 代码 / 文档 / 任务卡）

- **gov §5.1 属契约文件**：本 ADR 即铁律 10 要求的"先有 ADR 再改契约"。
- 每张"软转硬"的卡工作量 **+0.2 天**（写 canary 或负向测试）。
- `gate-selftest.yml` 是新增 workflow，**不改主 CI 的触发条件与时长**。
- `docs/dev-env-setup.md`（PL-017）需收录第 5 条本地验收命令。

## 考虑过的选项（含被否理由）

| # | 选项 | 结论 | 理由 |
|---|---|---|---|
| 1 | 只钉 action 版本，不做负向验证 | ❌ 否决 | 钉版本只解决"本地与 CI 解析器不同"，不解决"门禁没跑却显示绿"。下一次同类事故（例如某个 `xtask` 子命令被改成静默成功）依然抓不到 |
| 2 | 每次 push 都跑 canary | ❌ 否决 | 需要每次安装 cargo-deny（5~8 分钟），CI 时长翻倍；canary 检出的是**配置漂移**，不是代码回归，低频足够 |
| 3 | 配 `schedule` 每月自动跑 | ❌ 否决 | GitHub 60 天无活动自动停用 → 制造新的静默失效，与本 ADR 目的相反 |
| 4 | 在 `xtask` 里加 TOML 解析器检查 `deny.toml` | ❌ 否决 | `xtask` 的硬约束是**零第三方依赖**，手写 TOML 解析器的成本与风险都远高于收益；且它检查的是语法，而真实事故是**类型/schema** 错误 |
| 5 | **三种形式（N1/N2/N3）+ 登记表 + 规程约束** | ✅ **采纳** | 成本按价值分配：已有的负向测试直接登记为 N1；已在 CI 里的显式失败步骤登记为 N2；只有需要外部工具的门禁才建 N3 canary |

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| canary 靠人工触发，容易被忘 | 规程写进本 ADR 与 gov §5.1；每次运行必须在 `LEDGER.md` 留一行（可被 `check-ledger` 审计） |
| 登记表随门禁演进而过期 | TASK-015 的 `card-check` 落地后加机器检查："软转硬的 PR 必须同时改登记表"（PL-018） |
| canary 自身静默失败 | canary 断言用 `if [ "$code" -eq 0 ]; then exit 1; fi`，且**结尾必须打印一行显式 `OK:`** —— 与 `deferred-inventory` 同一写法 |
| 负向断言与真实故障模式脱钩（"断言了个不相干的错误"） | canary 不仅断言"非零退出"，还断言 **stderr 含 `expected a string`**，把断言钉在具体故障文本上 |

## 验证方式（怎么知道这个决策是对的；何时应重新评估）

**验收**：

1. 手工触发 `gate-selftest` → deny job 两步都打印 `OK:`，job 绿。
2. 把 `deny.toml` 的 `db-path` 临时改回数组 → 主 CI 的 deny job **必须变红**
   （这本身就是一次真实的负向验证，验证完改回）。
3. `cargo deny check licenses bans sources` 在本地 exit 0（已实测：`bans ok, licenses ok, sources ok`）。

**重新评估的触发条件**：

- TASK-015 引入 `card-check` / `check-ledger` 后，登记表可由机器维护
  → 把本 ADR 的"人工规程"升级为"机器门禁"，并考虑把 canary 并入主 CI 的独立 job。
- 若 cargo-deny 官方 action 提供"版本钉定 + 配置校验"内建能力 → 可删除 N3 canary 的对应部分。
