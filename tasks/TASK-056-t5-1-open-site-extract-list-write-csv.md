# TASK-056　T5.1：打开指定站点 → 提取列表页结构化数据 → 写入本地 CSV

- 状态：**Blocked**
- 阶段：1　子阶段：**1c**　批次：**1c**　依赖：055　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-1-pilots.md`。

---

- **依赖**：055　**预估**：M　**难度**：M
- **write scope**：`adapters/browser.edge/tasks/**`、`eval/tasks/edge/**`
- **关联**：`plans/stage-1-pilots.md` 批次表 1c（1c）、`docs/wbs-overview.md` §6（DoD）

**目标**

T5.1：打开指定站点 → 提取列表页结构化数据 → 写入本地 CSV。

**write scope**（本卡独有部分，完整列表见 plan 批次表）

`adapters/browser.edge/tasks/**`、`eval/tasks/edge/**`

**步骤**（占位 —— 派单前由 Orchestrator 按 gov §3.2 模板与实际调研补充）

1. 环境记录（OS / 依赖版本 / 输入 fixture）
2. 实现 card 标题声明的能力，附最小自检命令
3. 跑 `cargo test --workspace` + 本卡专项测试；不合格 → DRIFT
4. 更新 `docs/memory/apps/<app>.md` 或 `facts/pitfalls.md`（应用专属去 apps，跨应用去 pitfalls）

**DoD**

- [ ] card 标题声明的能力可被测试用例覆盖
- [ ] `cargo fmt --all --check` 0 diff
- [ ] `cargo clippy --all-targets -- -D warnings` 退出码 0
- [ ] `cargo test --workspace` 全绿
- [ ] `xtask hygiene / memory-counts / adr-index / refscan / docscan / card-check` 全部 PASSED
- [ ] LEDGER.md 追加一行；如新增事实/坑则追加 `docs/memory/{facts,pitfalls}.md`

**验收命令**

```powershell
cargo fmt --all --check; cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo run -p xtask -- hygiene / memory-counts / adr-index / refscan / docscan / card-check
```

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-056 T5.1 打开指定站点、提取列表并写 CSV
【目标】验证 Edge/Chrome 列表页读取、结构化提取与本地 CSV 输出
【write scope】卡片声明仅：adapters/browser.edge/tasks/**、eval/tasks/edge/**
【铁律】1 无静默失败 / 2 不可信输入先校验 / 6 不可无人值守处理真实登录 / 9 不扩大范围
【禁止】不启动默认用户 profile；不复制 Cookie；不猜测目标站点或 CSV schema
【验收】卡片正文命令 → 当前因输入契约缺失无法执行
【依赖】TASK-055 Done（已核对 LEDGER）
【疑问】目标 URL、列表字段、CSV 列/编码/空值规则、fixture 与人工登录流程均未给出；按规则记 DRIFT
```

### 2. 实际改动文件

本轮仅记录 DRIFT，不创建 adapter task 或 eval fixture：

- `tasks/TASK-056-t5-1-open-site-extract-list-write-csv.md`
- `LEDGER.md`

### 3. 验收输出摘要

未执行产品/浏览器验收。原因是缺少可执行输入契约且目标要求真实浏览器、网络和人工登录状态；进入实现会命中 gov §4.3 漂移触发器。

### 4. DoD 逐条核对

- 未勾选任何 DoD；卡片所声明的能力没有可验证输入，不能伪造完成。

### 5. 偏差

```text
DRIFT-056-1
触发器：#8（卡片仍是占位稿，正文与可执行验收之间矛盾）+ #11（需要真实网络/浏览器/人工登录）
现象：卡片“步骤”明确标注为占位，未给出目标站点、列表字段、CSV schema、输入 fixture、专用 profile 路径或人工登录流程
影响：无法确定 adapter task 参数、DOM 选择器、输出列顺序/编码/空值规则，也无法安全复现真实站点读取与 CSV 写入
我的建议：由 Orchestrator 在派单前补齐目标 URL、测试 fixture、CSV 契约、专用 profile 与人工登录步骤；完成后改由人类在场会话执行 T5.1
已停止的工作：未启动 Edge/Chrome，未连接 CDP，未读取真实站点或 profile，未创建 adapter task / eval 文件
需要人类裁决：是
```

- 本次启动、文档门禁、PR 状态查询及最终复核共 **12** 条只读命令误用默认 bash：前 11 条因 `cargo` / `gh` / PowerShell cmdlet 不可用而失败（exit 127 或语法错误），第 12 条在最终检索 LEDGER 时因 `rg` / `Select-Object` 不存在而失败；均未写入仓库，随后全部以显式 `shell: "powershell"` 重跑。

### 6. 更合理做法

不猜测站点或数据形状；先用明确的离线 fixture 固化字段与 CSV 契约，再进行真实站点验收。

### 7. 遗留问题

Stage 1c 的 T5.1 仍未完成，并依赖人类提供目标站点、专用 profile、登录状态与输出契约。

### 8. 新增长期记忆

无；本次仅记录阻塞，没有产生已验证的新事实。

### 9. 给审阅者的关注点

- 这是按规则停止，不是实现失败；没有任何产品代码或测试被修改。
- 请先补齐 TASK-056 正文中的目标站点、CSV schema、fixture 与人工登录流程，再派发给人在场的执行会话。
