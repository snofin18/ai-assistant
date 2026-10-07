# TASK-043　Paint Adapter：工具选择/颜色/图层（UIA）+ 画布坐标动作 + 像素快照回滚 + 缩放与滚动的坐标换算

- 状态：**Review**
- 阶段：1　子阶段：**1b**　批次：**1b**　依赖：040,042　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-1-pilots.md`。

---

- **依赖**：040,042　**预估**：M　**难度**：M
- **write scope**：`adapters/com.microsoft.paint/**`
- **关联**：`plans/stage-1-pilots.md` 批次表 1b（1b）、`docs/wbs-overview.md` §6（DoD）

**目标**

Paint Adapter：工具选择/颜色/图层（UIA）+ 画布坐标动作 + 像素快照回滚 + 缩放与滚动的坐标换算。

**write scope**（本卡独有部分，完整列表见 plan 批次表）

`adapters/com.microsoft.paint/**`

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
【任务】TASK-043 Paint Adapter：工具选择/颜色/图层 + 画布坐标动作 + 像素快照回滚 + 缩放/滚动坐标换算
【目标】在 declarative Adapter 包内落地 Paint 的 UIA 目标链、工具契约、回滚剧本、App Map 与中断规则
【write scope】仅 adapters/com.microsoft.paint/**；按 AGENTS §11 同步任务卡、LEDGER、PLAN、README、plans/stage-1-pilots.md
【铁律】无静默失败；不可信输入先校验；写操作必须有 postcondition；不静默扩大范围；契约先行；资源生命周期有界
【禁止】新增依赖/crate/top-level 目录；改公共 trait/schema/spec；伪造真机 UIA 证据；未取 guard 就写 hot file
【验收】adapter 专项校验；fmt / clippy / workspace tests；xtask 全套门禁
【依赖】TASK-040、TASK-042 Done；TASK-041 / TASK-249 的截图与视觉验证能力已核对
【疑问】卡面仍是占位；本轮按数据包 + 专项契约校验实现，真机坐标精度不能在本轮冒充通过
```

### 2. 实际改动文件

新增 `adapters/com.microsoft.paint/`：

- `adapter.toml`
- `app_map.json`
- `memory/app_map.v1.json`
- `selectors/targets.json`
- `tools/tools.json`
- `rollback/recipes.json`
- `interrupts/interrupts.json`
- `README.md`
- `tests/validate_adapter_pack.py`

### 3. 验收输出摘要

- `python adapters/com.microsoft.paint/tests/validate_adapter_pack.py` ->
  `PASSED: app_id=com.microsoft.paint targets=16 tools=10 recipes=6 probe_status=required`
- `python -m json.tool` 解析 `tools.json` / `selectors/targets.json` -> 退出码 0
- `cargo fmt --all --check` -> EXIT 0
- `cargo clippy --all-targets -- -D warnings` -> EXIT 0（仅既有
  `unknown lint: clippy::assert_is_empty` 提示）
- `cargo test --workspace` -> EXIT 0
- xtask：`hygiene` 0E/109W；`memory-counts` 0E/0W；`adr-index` 0E/0W；
  `refscan` 0E/0W；`docscan` 0E/282W；`card-check` 0E/34W；
  `check-ledger` 0E/0W；`check-comments` 0E/71W；`verify-schemas` 0E；
  `codegen --check` 0 drift；`check-migrations` 0E/0W -> 全部 PASSED
- `cargo deny check` -> advisories / bans / licenses / sources 全 ok
- PR #273 已创建，分支 `task/TASK-043-paint-adapter-tools-canvas-coords`；
  卡面在本轮保持 `Review`，等待 CI 与人类合并。

### 4. DoD 逐条核对

- [x] card 标题声明的 Adapter 数据能力可被专项校验脚本覆盖。
- [x] `cargo fmt --all --check` 0 diff。
- [x] `cargo clippy --all-targets -- -D warnings` 退出码 0。
- [x] `cargo test --workspace` 全绿。
- [x] `xtask hygiene / memory-counts / adr-index / refscan / docscan / card-check` 全部 PASSED。
- [x] 追加 LEDGER；新增事实/坑落 `docs/memory/apps/paint.md`（如本轮最终落地）。

### 5. 偏差

`DRIFT-043-1`：本轮无法取得 Windows 11 Paint 的真实 UIA 树。PowerShell/UIA
通道在本机处在已知挂起状态，启动 AUMID 也未得到可检查窗口；因此所有 selector
候选都标 `probe_status=required`，工具包是可审查的 declarative WIP，不是真机
可执行验收。建议 TASK-044 第一件事就是跑真实 Paint 校准，替换本包的 provisional
selector 证据、坐标误差与像素容差数据。

**Scope note**：本卡新增量高于 400 行软预算。超出部分是声明式 Adapter
数据包本身（16 个目标、10 个工具、6 条回滚剧本、App Map、记忆投影和专项校验），
拆成多个半成品目录会让 TASK-044 无法得到完整契约；本轮没有新增运行时代码、公共
接口或依赖。

### 6. 更合理做法

- 不在数据包里硬编码未经验证的 AutomationId；用低分、locale-dependent 的
  文本兜底显式暴露不确定性。
- 把坐标换算、工具/颜色/图层回读、L0+L1 回滚、视觉容差都写成结构化字段，
  让 TASK-044~046 可以直接消费，而不是在代码里重新猜。
- 用 `tests/validate_adapter_pack.py` 将“probe required”“无自由字符串断言”
  “高 risk 必须审批”“回滚工具引用必须存在”等边界机器化。

### 7. 遗留问题

- **PL-113 / DRIFT-043-1**：Paint 真机 UIA selector 校准与坐标误差测量仍需
  TASK-044；本轮未伪造该项证据。
- **PR #273** 处于 open / Review；合并后再补 merge hash 回填。
- `latest protocol ToolSchema` 尚未容纳本包的 `postconditions` / `risk_level` /
  `requires_approval` 等 adapter-level 字段；按现有 Notepad 约定先显式保留。

### 8. 新增长期记忆

新增 `docs/memory/apps/paint.md`：记录 Paint 包身份
`Microsoft.Paint_8wekyb3d8bbwe!App`、自绘画布、坐标换算、L0+L1 回滚、
Probe status 与网络 AI 功能边界。

### 9. 给审阅者的关注点

1. 高风险的 `paint.document.new` / Save As 是否应要求更强的用户确认与
   文件影子副本，而不是仅依赖 `requires_approval=true`。
2. `paint.canvas.draw_rectangle` 的视觉断言目前是未实测的容差契约；
   TASK-044 必须以真机截图替换 `max_changed_ratio` / `pixel_delta_threshold`。
3. `selectors/targets.json` 的候选链目前是 provisional；所有 selector id
   在真实 Paint 树校准前不得宣称已命中。
