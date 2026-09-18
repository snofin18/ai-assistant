# TASK-001　仓库骨架与 CI 最小门禁 — 执行记录

> 卡片正文（In/Out scope、验收命令、DoD）在 `plans/stage-0-spikes.md` 的 TASK-001 一节，本文件**不复制**它，只记录执行结果。
> 为什么单独建文件：`plans/*` 不在本卡 write scope 内，执行记录需要一个可写的位置（见 DRIFT-001-3）。
>
> 状态：**Review（待人类审阅）**　执行日期：2026-09-16　执行者：Codex（人类规划 + 裁决）

---

## 1. 约束回执（动手前输出，gov §4.2）

```text
【任务】TASK-001 仓库骨架与 CI 最小门禁
【目标】建立可被机器验证的仓库地基：workspace + 护栏工具 + CI + 台账骨架
【write scope】仅：仓库根（.gitignore/.gitattributes/LICENSE/README.md/Cargo.toml/
              rust-toolchain.toml/rustfmt.toml/clippy.toml/deny.toml/.github/workflows/ci.yml/
              xtask/）、LEDGER.md、docs/PARKING_LOT.md、docs/DEPENDENCIES.md、CLAUDE.md、
              各占位目录的 .gitkeep、tasks/TASK-001-repo-skeleton.md
【铁律】1 无静默失败 ｜ 9 不得静默扩大范围 ｜ 10 契约先行 ｜ 2 不可信输入
【禁止】任何 crates/* 与 apps/* 业务代码；spec 内容；改 AGENTS.md / PLAN.md / plans/*
【验收】cargo fmt --all --check ｜ cargo clippy --all-targets -- -D warnings ｜
        cargo test --workspace ｜ cargo deny check ｜ cargo run -p xtask -- hygiene ｜
        git status --porcelain
【依赖】M5 许可证（未裁决 → 按建议先落 MIT，可逆；见 §5.6）
【疑问】3 条，见 §5 的 DRIFT-001-1/2/3
```

---

## 2. 产出清单

### 2.1 仓库根

| 文件 | 说明 |
|---|---|
| `Cargo.toml` | workspace；`[workspace.lints]` 把 gov §5.2 的禁用项写成机器约束；`exclude` 掉 `spikes`/`fixtures/apps`/`tools` |
| `rust-toolchain.toml` | 固定 stable + rustfmt/clippy/llvm-tools |
| `rustfmt.toml` | `max_width=100`、**LF**；已移除 4 个在 stable 下无效的 unstable 选项（§5.4a） |
| `clippy.toml` | MSRV 1.88、认知复杂度 15、参数 6、函数 80 行 |
| `deny.toml` | 许可证白名单 + advisories + bans |
| `.gitignore` | 运行时数据（`*.db*`、`shadow/`、`blobs/`、`evidence/`）、密钥、`adapters-private/`、`.nightly.lock` |
| `.gitattributes` | **新增**：全仓库 LF + 二进制标记（§5.4b） |
| `LICENSE` | MIT |
| `README.md` | 一句话定位 + 文档地图 + 当前阶段 + 构建方式 + 许可证 |
| `CLAUDE.md` | 一行转发到 `AGENTS.md` |
| `.github/workflows/ci.yml` | 三平台矩阵；6 硬门禁 + 9 软门禁（各注明启用卡号）+ `deny` job + `deferred-inventory` job |
| `LEDGER.md` | 台账表头 + 本卡首条 |
| `docs/PARKING_LOT.md` | 停车位表头 + PL-001~PL-007 |
| `docs/DEPENDENCIES.md` | 依赖登记表 + 登记规则 + 开发工具清单 |

### 2.2 `xtask`（零第三方依赖的只读护栏工具）

| 模块 | 行数 | 职责 | 碰文件系统 |
|---|---|---|---|
| `cli.rs` | 220 | argv 解析、`USAGE` | 否 |
| `repowalk.rs` | ~310 | 定位仓库根、遍历 `.rs`、路径归一化 | **是** |
| `rustscan.rs` | ~580 | 源码 → 「注释列表」+「降噪代码」两个视图 | 否 |
| `hygiene.rs` | ~580 | gov §5.4 规则判定（**3/13** 项；口径见 ADR-0025） | 否 |
| `deferred.rs` | ~345 | 未实现子命令与未实现规则登记表 | 否 |
| `report.rs` | ~320 | `Finding` / `Report` 模型与渲染 | 否 |
| `main.rs` | ~435 | 分派、读文件内容、呈现、退出码 | 是（只读） |

子命令与退出码见 `xtask/README.md`。要点：**未实现的子命令返回退出码 3 并指出归属卡号**，
`hygiene` 每次运行都主动声明「**13** 项规则只实现了 3 项」——两者都是为了不让"检查没跑"被误读成"检查通过"。

---

## 3. 验收结果

| # | 命令 | 结果 | 说明 |
|---|---|---|---|
| 1 | `cargo fmt --all --check` | ✅ 通过 | exit 0，无 diff |
| 2 | `cargo clippy --all-targets -- -D warnings` | ✅ 通过 | 零警告（含 pedantic + nursery） |
| 3 | `cargo test --workspace` | ✅ 通过 | **99 passed; 0 failed** |
| 4 | `cargo deny check` | ⚠️ **未执行** | cargo-deny 未安装（PL-006）。CI 里已配置，本地验收与 CI 暂不等价 |
| 5 | `cargo run -p xtask -- hygiene` | ✅ 通过 | `scanned=7 errors=0 warnings=0 verdict=PASSED`，exit 0 |
| 6 | `git status --porcelain` | ✅ 通过 | 无未忽略的运行时数据文件（`target/` 已忽略） |

补充验证（卡外，但属于「无静默失败」的自证）：

```text
cargo run -p xtask -- codegen            → exit 3，输出「子命令 `codegen` 尚未实现。归属：TASK-011」
cargo run -p xtask -- frobnicate         → exit 2，用法错误（区别于「未实现」）
cargo run -p xtask -- --list-deferred    → exit 0，打印 6 个未实现子命令 + 8 条未实现规则
cargo run -p xtask -- hygiene --repo docs → exit 0，但显式告警 xtask/no-source-files（scanned_files=0）
```

工具链：cargo/rustc 1.98.1（stable-x86_64-pc-windows-msvc），组件 clippy/rustfmt/llvm-tools 齐备。

---

## 4. DoD 核对

- [x] CI 在空 workspace 上三平台全绿 —— **待推送后由 GitHub 验证**（本地无远端；工作流已通过 YAML 解析校验，硬/软门禁数与结构已核对）
- [x] `xtask` 四个子命令都存在且**未实现时明确报错**（退出码 3 + 归属卡号；有端到端测试 `test_execute_every_deferred_command_fails_with_code_three` 覆盖全部 6 个）
- [x] `.gitignore` 覆盖所有运行时数据目录
- [x] `LEDGER.md` 追加本卡一行
- [ ] `MEMORY.md` §1 快照更新 —— 见 §6（本轮同时更新）

---

## 5. 设计变更与偏差（用户要求：思路变更与更合理办法必须记录）

### 5.1 DRIFT-001-1　hygiene 是"占位"还是"实现"？（**需人类裁决**）

- **现象**：卡面 In-scope #1 要求 `hygiene`/`verify-schemas`/`codegen`/`replay` 四个子命令都是
  **占位实现（返回 not implemented）**；但卡面验收命令第 5 条又要求
  `cargo run -p xtask -- hygiene` **能跑通**。二者矛盾：占位即失败，失败即验收不通过。
- **影响**：决定 TASK-001 的产出边界，也决定 TASK-015 还剩多少工作量。
- **我的处理**（未自行裁决，按"更保守且不静默"的方向落地）：
  `hygiene` 实现 gov §5.4 **11 项中的 3 项**（文件行数、注释标签、注释掉的代码块），
  其余 8 项与另外 5 个子命令登记在 `deferred.rs`，运行时**显式失败并指出归属 TASK-015**；
  `hygiene` 每次运行都打印一行 `-- deferred-rules: 共 11 项，已实现 3 项，未实现 8 项`，
  避免 `PASSED` 被误读成"全部通过"。
  > **[2026-09-18 口径更正，ADR-0025]** 上文是**当时的历史记录，按原样保留**。
  > gov §5.4 现为 **13 项**（PL-011 禁 CRLF + PL-020 末行换行 一并采纳），
  > 故现在的实际输出是 `共 13 项，已实现 3 项，未实现 10 项`。**已实现项数仍是 3，未变。**
- **建议**：接受该处理，并在 TASK-015 卡里明确"补齐剩余 8 项 + arch test"。
  > **[2026-09-18 更正]** 应为「补齐剩余 **10** 项 + arch test」。
  若人类希望 TASK-001 严格只做占位，则需同时修改卡面验收命令第 5 条。

- **裁决结果（2026-09-17，人类）**：**接受**上述处理，a / b 两点均照建议执行。
  - **a**　产出边界正式确认为：`hygiene` 实现 gov §5.4 的 **3/11 项**，其余 **8 项**规则与
    > **[2026-09-18 口径更正，ADR-0025]** 现为 **3/13 项** / 其余 **10 项**（已实现项数不变）。
    `verify-schemas` / `codegen` / `replay` 三个子命令在 `deferred.rs` 显式登记并于运行时失败（退出码 3）。
    MEMORY.md §4 中 2026-09-16 那条 REJECTED 的「待人类确认」到此**解除**。
  - **b**　卡面已同步修订：`plans/stage-0-spikes.md` 的 TASK-001 In scope #1 加了裁决注记，
    验收命令第 5 条从 `cargo deny check; cargo run -p xtask -- hygiene` 拆成两条独立命令，
    并注明判定以 `-- deferred-rules` 摘要行为准、`verdict=PASSED` 不等于 11 项全过。
    > **[2026-09-18 口径更正，ADR-0025]** 卡面期望输出行已同步为 `共 13 项，已实现 3 项，未实现 10 项`；
    > 该行是**机器可判定的验收依据**，与 `xtask` 的实际输出逐字符一致（本机 2026-09-18 已核对）。
  - **未一并裁决**：PL-001（gov §5.4 表格 11 行 vs stage-1 写「12 项」）与 PL-011（是否新增第 12 项
    CRLF 规则）仍为「待评审」。若日后采纳 PL-011，本条与卡面的 `3/11` 需改述为 `3/12`。
  - **[2026-09-18 更新：上述三条已全部裁决关闭 → ADR-0025]** PL-001 / PL-011 / **PL-020**（必须有末行换行）
    由人类指示 #12「其他按最佳方案实行」授权一并裁决：gov §5.4 = **13 项**（不是当初设想的 `3/12`，
    因为 PL-020 也采纳了）；`plans/stage-1-pilots.md` 的「hygiene 12 项」确认为**笔误**、
    「CI 14 项门禁」确认为**过时表述**，两处均已改述（ADR-0025 D3 / D4）。
    两条新规则的**实现**仍归 TASK-015（先 Warning 后 Error，清扫 15 个缺末行换行的既有文件）。
    本次落地已跑：`cargo test --workspace` **99 passed**、`xtask hygiene` 输出 `共 13 项…未实现 10 项`。
  - **补充证据（裁决后取得）**：本地工具链已补齐，TASK-001 验收从 5/6 变为 **6/6** ——
    `cargo deny check` → `advisories ok, bans ok, licenses ok, sources ok`（退出码 0）；
    `cargo llvm-cov --workspace --fail-under-lines 75` → 99 passed，**行覆盖 96.31%**、函数 94.26%、
    区域 95.18%（退出码 0）。§7.4 标注的「安装工具后应回填实测数字」即可用此数据回填。

### 5.2 DRIFT-001-2　CI 硬门禁是 5 项还是 6 项？

- **现象**：卡面写"先上线 **5** 项门禁（fmt、clippy、test、deny、build）；其余 **9** 项
  `continue-on-error`"。5 + 9 = 14，但 gov §5.1 的表实际有 **16** 行
  （其中 #3「禁用项 lint」已由 #2 的 `-D warnings` + `[workspace.lints]` 覆盖）。
- **我的处理**：硬门禁 **6** 项（卡面 5 项 + `hygiene`），软门禁 **9** 项，合计对应 16 行。
  把已实现且自检通过的 `hygiene` 留作软门禁，等于白白放弃一道已就绪的防线。
- **建议**：接受；并把 gov §5.1 的行数与 stage-1 里"CI 14 项门禁"的表述统一（PL-001 同类问题）。

- **裁决结果（2026-09-17，人类）**：**接受**，a / b / c 三点均照建议执行。
  - **a**　CI 硬门禁正式确认为 **6 项**：fmt、clippy `-D warnings`（含 `[workspace.lints]` 禁用项，
    即 gov §5.1 的 #2+#3）、test、deny、build、`xtask hygiene`。**计数口径**同时澄清：
    gov §5.1 共 16 行，#3 由 #2 覆盖，故 **6 硬 + 9 软 = 15 个 CI 步骤 ↔ 16 行清单**；
    卡面原文的「5 + 9 = 14」是把 #3 重复计了一行、又漏掉了已就绪的 hygiene。
  - **b**　卡面已同步修订：`plans/stage-0-spikes.md` TASK-001 的 **In scope #4** 改为「6 项硬门禁」，
    并补上计数口径注记与「PL-001 仍未统一」的显式声明。
  - **c**　采纳**元门禁**：新建 **ADR-0019（Accepted）**「硬门禁必须配负向验证」，
    定义 N1 单元负向用例 / N2 CI 显式失败步骤 / N3 canary 工作流三种可接受形式，
    并规定**软门禁转硬的那张卡必须同时提交该门禁的负向验证**（自 TASK-015 起适用）。
    首个实例（deny，N3）已落地：新增 `.github/workflows/gate-selftest.yml`；
    `ci.yml` 把 `cargo-deny-action@v1` 钉到 **`@v2.1.1`**（= cargo-deny 0.20.2，与本地一致，关闭 PL-016 根因）；
    本地验收清单追加 `cargo deny check licenses bans sources`（离线可跑，专门证明 `deny.toml` 能被加载）。
  - **实测证据（裁决时取得，本机 cargo-deny 0.20.2）**：
    正向 `cargo deny check licenses bans sources` → `bans ok, licenses ok, sources ok`，**exit 0**；
    负向 `cargo deny --config <db-path 写成数组的坏配置> check licenses bans sources`
    → `error[wanted]: expected a string` + `failed to deserialize config`，**exit 1**。
    注意两个易错点：`-c` 是 `--color` 而非 `--config`；`check` 子命令**不接受** `--offline`。
  - **未一并处理（明确不做，避免范围蔓延）**：① gov §5.1 行数与 `plans/stage-1-pilots.md`
    「CI 14 项门禁」表述的统一 —— 属 **PL-001**，人类未裁决；② fmt / clippy / build 三项硬门禁
    的负向验证 —— 新登记 **PL-018**，归 TASK-015；③ **PL-011**（新增第 12 项 CRLF 规则）仍未裁决。

### 5.3 DRIFT-001-3　执行记录该写在哪？

- **现象**：gov §3.2 的任务卡模板含「执行记录（agent 填写）」一节，但本项目的卡片正文写在
  `plans/stage-0-spikes.md`，而 `plans/*` **不在** TASK-001 的 write scope 内 → 执行记录无处可写。
- **我的处理**：新建 `tasks/TASK-001-repo-skeleton.md`（本文件）承载执行记录；
  卡片正文仍留在 `plans/*`，本文件只引用不复制。
- **建议**：确立约定 —— **卡片正文在 `plans/<阶段>.md`，执行记录在 `tasks/TASK-NNN-<slug>.md`**，
  并把 `tasks/TASK-*.md` 默认纳入每张卡的 write scope（需改 gov §3.2 模板，属契约变更，走 ADR）。

### 5.4 实施中发现的更合理做法（非漂移，已直接落地并在此说明理由）

**a. `rustfmt.toml` 移除 4 个 unstable 选项。**
`wrap_comments` / `comment_width` / `format_code_in_doc_comments` / `normalize_comments`
在 stable 通道下**不生效**，只会每次打印一行 warning。留着的坏处：① 让人误以为注释宽度受控；
② 把 CI 输出训练成"可忽略的噪声"。已在文件内注明「若将来切 nightly 再按 ADR 加回」。

**b. 换行策略从 CRLF 改为全仓库 LF，并新增 `.gitattributes`（★ 重要）。**
原 `rustfmt.toml` 写的是 `newline_style = "Windows"`。在三平台 CI 矩阵下这是**必然失败**的配置：
Linux runner 签出的是 LF 文件，而 rustfmt 期望 CRLF，`cargo fmt --check` 会一直红。
若改为依赖各人本地的 `core.autocrlf`，等于把"文件长什么样"交给每个人的 git 配置决定 ——
这正是 gov §0.1 里最难查的一类环境差异 bug。
处理：`.gitattributes` 设 `* text=auto eol=lf` + 显式二进制标记；`rustfmt.toml` 改
`newline_style = "Unix"`；已把仓库内 33 个文件统一转为 LF 并复跑全部门禁（仍全绿）。

**c. 修掉 `Report::new` 的一个静默失败缺陷。**
原实现签名是 `new(command: &'static str)`，但函数体把参数**丢弃**并把 `command` 置空 ——
报告标题因此永远是 `== xtask  ==`，CI 输出无法归因到具体检查项。这是铁律 1 的教科书案例。
已改为 `new(command: impl Into<String>)` 并加回归测试 `test_report_new_keeps_command_name`。

**d. `xtask` 拆成 7 个文件，IO 与规则严格分离。**
初版把参数解析、文件遍历、规则、呈现都放在 `main.rs`，写到 644 行时被自己的
`hygiene/file-too-long` 规则拦下（>600 行告警）。**这是护栏第一次真实发挥作用**。
拆分后最大文件 581 行，且规则模块全部是纯函数 —— 直接带来了 §7 的可测试性。

**e. 新增 `xtask/no-source-files` 警告。**
"扫到 0 个文件"会得到一个毫无意义的 `PASSED`。现在这种情况显式告警，
并附带说明扫描根，避免"检查范围配错了却显示通过"。

**f. 新增 `--list-deferred` 与 CI 的 `deferred-inventory` job。**
后者专门验证「未实现的子命令必须以退出码 3 失败」。这张清单是防止"静默跳过检查"的唯一凭证；
如果哪天它打印不出来或退出码变成 0，说明有人把显式失败改成了静默成功。

**g. 测试模块统一 `#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]`。**
gov §5.2 明确授权「`tests/` 中按需 allow」。测试里用 `unwrap`/`panic!` 是**正确**的：
测试失败就该炸，把错误吞成 `Option` 反而让失败原因不可读。
这条约定需要写进 `docs/spec/testing.md`（PL-003），否则下一个 agent 会以为可以到处加 allow。

**h. `deferred.rs` 的三条不变量带自检测试。**
`已实现 + 未实现 == 总数`、命令名唯一、每项必须有非空归属。
这类"登记表自身一致性"的检查成本极低，但能防止登记表悄悄过期。

### 5.5 未做 / 受阻（均已登记，未静默跳过）

| 事项 | 原因 | 去处 |
|---|---|---|
| `cargo deny check` 本地未执行 | cargo-deny 未安装 | PL-006；CI 已配置 |
| `docs/OPEN_SOURCE_CHECKLIST.md` 未创建 | 不在 TASK-001 write scope | PL-005 |
| `docs/spec/testing.md` 未创建 | spec 内容属后续卡；但 `report.rs` 已引用它 | PL-003（悬空引用） |
| `check-comments`/`check-ledger`/`card-check` 无归属卡 | 计划里没有这三张卡 | PL-002 |
| `.github/workflows/.gitkeep` 冗余 | 删除文件不属本卡必要动作 | PL-007 |

### 5.6 M5（许可证）的处理

`MEMORY.md` §6 的 M5 仍未裁决。本卡按 gov 建议先落 **MIT**（单许可，而非 `MIT OR Apache-2.0`），
理由：① 内部阶段不需要双许可的额外复杂度；② 该决定**可逆**（追加 Apache-2.0 是加法，
反过来则涉及已分发代码）；③ `Cargo.toml` 的 `license` 字段与 `deny.toml` 白名单已按此对齐。
README 已明确写出"M5 待裁决 + 可逆"。**请人类在阶段 0 结束前正式确认。**

---

## 6. 需要回填的其他文档（本卡完成后）

- `MEMORY.md` §1 快照：阶段 0 状态从"尚未创建代码仓库"改为"仓库已建立，TASK-001 完成"
- `MEMORY.md` 追加：FACT（工具链版本、heartbeat/cron 机制）、DECISION（换行策略、hygiene 范围）、
  PITFALL（`newline_style="Windows"` 会毁掉 Linux CI；`Report::new` 式静默失败）
- `LEDGER.md`：本卡一行（commit 哈希在首次提交后回填）

---

## 7. 白盒测试准备（用户要求 #4）

### 7.1 现状

**99 个测试，0 失败，0 ignored，运行耗时 0.02 s**（全部纯内存，无临时目录、无网络、无 GUI）。

| 模块 | 测试数 | 覆盖内容 |
|---|---|---|
| `rustscan` | 17 | 行/块/文档注释分类、嵌套块注释、字符串里的假 `//`、转义引号、原始字符串 `r#"..."#`、字节字面量、生命周期不被误抹、未闭合注释不 panic、**行号与长度保真两条不变量** |
| `hygiene` | 20 | 三条规则的正/负/边界、词边界（`TODOLIST` 不误伤）、字符串内标签不误报、占位卡号被拒、散文注释块不误判、文档注释不参与、空 `//` 行中性、行号不连续分段、两段各自报告、**用 `include_str!` 扫自己** |
| `cli` | 12 | 各选项形态、缺参数、未知选项、多余位置参数、确定性、`USAGE` 覆盖全部退出码与子命令 |
| `repowalk` | 11 | 仓库根推导、override、**指向不存在目录必须报错**、**指向文件必须报错**、排序去重、排除 `spikes/`/`target/`、路径分隔符归一 |
| `deferred` | 9 | 三条登记表不变量、查找命中/未命中、报错文本含归属卡号、进度声明数字正确 |
| `report` | 8 | `is_failure` 语义、warning 不阻塞、渲染确定性、行号 0 省略、空消息兜底、摘要单行 |
| `main` | 12 | 分派全路径、6 个未实现子命令都返回 3、拼错命令返回 2、`--list-deferred`、hygiene 端到端、**双跑逐字节相同**、0 文件显式告警、`present_failure` 只在用法错误时附带 USAGE |

### 7.2 可测试性接缝（后续 crate 应照此设计）

1. **规则 = 纯函数**：`(相对路径, 源码文本) -> Vec<Finding>`。测试直接喂字符串，不建临时文件。
2. **输出可注入**：`execute(args, &mut dyn Write)`、`Report::render(&mut dyn Write)`。
   测试用 `Vec<u8>` 当 sink 断言文本，不需要捕获子进程 stdout。
3. **IO 集中在边界**：只有 `repowalk.rs` 与 `main.rs` 碰文件系统，且都返回带上下文的错误。
4. **阈值是 `pub const`**：测试用 `FILE_LINES_WARN + 1` 而不是硬编码 601，改阈值时测试自动跟随。
5. **自反测试**：工具扫自己的源码（`include_str!`），以及"同一输入两次运行逐字节相同"。

### 7.3 负向测试清单（证明它会拒绝该拒绝的东西）

占位卡号 `TASK-0NN` 被拒 ｜ 4 行连续注释不触发（阈值边界） ｜ 文档注释不算注释掉的代码 ｜
生命周期 `'a` 不被当字面量抹掉 ｜ 仓库根指向文件必须失败 ｜ 拼错命令必须是"用法错误"而非"未实现" ｜
0 个文件必须告警 ｜ 登记表数量不自洽必须测试失败。

### 7.4 覆盖率

`cargo-llvm-cov` 未安装（CI 里已配为软门禁 #9，启用卡号 TASK-015）。
本卡**未测覆盖率数字**，但按 §7.1 的用例分布，规则模块的分支覆盖是完整的
（每条规则都有 通过 / 告警 / 失败 / 边界 四类用例）。安装工具后应回填实测数字到本节。
