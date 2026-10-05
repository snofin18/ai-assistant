# TASK-041　截图管线：窗口截图 + 脱敏（密码框/正则命中区域遮挡）+ 滚动清理 + 隐私模式（不保存截图）

- 状态：**InProgress（拆分 B WIP：DRIFT-041-2 待裁决）**
- 阶段：1　子阶段：**1b**　批次：**1b**　依赖：017　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-1-pilots.md`。

---

- **依赖**：017　**预估**：M　**难度**：M
- **write scope**：`crates/capture/**`、`crates/dlp/src/redact*`
- **关联**：`plans/stage-1-pilots.md` 批次表 1b（1b）、`docs/wbs-overview.md` §6（DoD）

**目标**

截图管线：窗口截图 + 脱敏（密码框/正则命中区域遮挡）+ 滚动清理 + 隐私模式（不保存截图）。

**write scope**（本卡独有部分，完整列表见 plan 批次表）

`crates/capture/**`、`crates/dlp/src/redact*`

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
【任务】TASK-041 截图管线
【目标】窗口截图 + 脱敏 + 滚动清理 + 隐私模式
【write scope】crates/capture/**、crates/dlp/src/redact*
【铁律】1 无静默失败；9 不静默扩大范围；ADR-0063 有界资源
【禁止】新增 crate / 顶层目录、操作真实 GUI、放宽 lint、改公共契约
【验收】fmt / clippy / workspace tests / xtask 门禁
【依赖】TASK-017 Done
【疑问】目标目录当前不存在；新增 crate 属自动化黑名单，本轮无法开工
```

### 2. 实际改动文件

**2026-10-06 拆分 A（纯逻辑落地）**：

- `crates/dlp/src/redact.rs`（新增）：已判定区域规则模型、遮挡矩形计算、空规则 / 负坐标 /
  零宽高 / 越界 / 溢出 / 超上限的显式失败与单测。
- `crates/dlp/src/lib.rs`（最小接线）：导出 `redact` 模块；除此之外未改 DLP 既有行为。
- `crates/capture/Cargo.toml`：新增两个 workspace 路径依赖
  `assistant-platform-api` / `assistant-dlp`，两者都不是新的第三方包。
- `crates/capture/src/{error,pipeline,privacy,scroll}.rs`（新增）：隐私模式、脱敏策略、
  注入式 `WindowProvider::capture` 编排、遮挡决策与有界滚动计划。
- `crates/capture/src/lib.rs`、`crates/capture/README.md`：导出与文档更新。
- `crates/capture/tests/capture_pipeline.rs`（新增）：7 个合同测试。
- `docs/adr/0073-capture-privacy-and-redaction-decisions.md`（新增）+
  `docs/adr/README.md` + `docs/memory/decisions.md` + `MEMORY.md` 规模表。

### 3. 验收输出摘要

**2026-10-06 按退出码核**：

- `cargo fmt --all --check` EXIT 0。
- `cargo clippy --all-targets -- -D warnings` EXIT 0（仅仓库既有
  `unknown lint: clippy::assert_is_empty` 提示）。
- `cargo test --workspace` EXIT 0；`cargo test -p assistant-capture -p assistant-dlp`
  EXIT 0（capture 7 passed / dlp 8 passed / 0 failed）。
- xtask：`hygiene` scanned=367 0E/105W；`memory-counts` 8 0E/0W；
  `adr-index` scanned=61 0E/0W；`refscan` scanned=704 0E/0W；
  `docscan` scanned=317 0E/336W；`card-check` scanned=137 0E/34W；
  `check-comments` scanned=367 0E/69W；`verify-schemas` 5/5；
  `codegen --check` 0 drift；`check-migrations` 5 files / 5 entries。
- `cargo deny check`：advisories / bans / licenses / sources 全 ok。
- 首轮 pull_request CI run `37343345239`：**11/11 SUCCESS**、0 failed；
  `baseRefName=main`、`mergeable=MERGEABLE`、`mergeStateStatus=CLEAN`。
- 最终 pull_request CI run `37344725563`：**11/11 SUCCESS**、0 failed；PR #239 已合并，
  merge hash = `9c30ba2`，feature 分支已删除。

### 4. DoD 逐条核对

**2026-10-06 拆分 A**：

- [x] 拆分 A 的纯逻辑能力可被单测覆盖：规则解析、隐私保留、provider 错误透传、
  滚动上界与全部规定负向样本均有测试。
- [x] `cargo fmt --all --check` 0 diff。
- [x] `cargo clippy --all-targets -- -D warnings` EXIT 0。
- [x] `cargo test --workspace` 全绿。
- [x] xtask 十项门禁本地 PASSED；首轮 CI 11/11 SUCCESS。
- [x] LEDGER / ADR / decisions / MEMORY 规模表已同步。
- [x] CI 11/11、PR 合并与 merge hash 回填完成。
- [ ] 真机截图与像素级遮挡：明确不在本拆分范围，归后续平台实现。

### 5. 偏差

**DRIFT-041-1（write scope 指向尚不存在的 crate，自动化无法合法创建）**

- 现象：`Test-Path crates/capture` = False、`Test-Path crates/dlp` = False。
- 影响：TASK-041 的第一项工作会变成新增两个 crate；自动化章程 §3.2 禁止新增 crate
  或顶层目录，且根 `Cargo.toml` 不在本卡 write scope。
- 建议：由 Orchestrator/人类先批准并落一张“capture + dlp 骨架”前置卡，或把 TASK-041
  的 write scope 扩展到根 `Cargo.toml` 并明确授权新增 crate。
- 已停工作：未创建 `crates/capture`、未创建 `crates/dlp`、未改根 workspace。

**DRIFT-041-1 已闭环（2026-10-06 复核）**：前置 TASK-238 已按 ADR-0071 创建两个骨架并闭环
`PL-102`；当前 main 上不再是阻塞。本拆分另发现卡面 / ADR-0071 D2 的“正则命中区域”与
`docs/memory/rejected.md` 2026-09-25 的“不得引入正则引擎”冲突，按预授权新写 ADR-0073
将该措辞替换为「调用方已判定区间 / 显式词表」，未改卡面正文。

**Scope note**：本轮 prompt 明确要求一次完成“拆分 A”的 ADR、纯规则模型、编排骨架、
负向单测与文档同步，因此单 PR diff 为 **1126 insertions / 17 deletions**，高于 400 行软预算；
超出部分已按 prompt 的整片授权记录，未把纯逻辑拆成两个不可独立验收的半成品。

### 6. 更合理做法

**2026-10-06**：不扩展 `CaptureOptions` 传遮挡矩形（会改公共 trait / schema），也不在
`capture` 内解码像素；先以 ADR-0073 冻结纯决策边界，把像素遮挡留给平台实现卡。

### 7. 遗留问题

- **PL-102**：TASK-041 的前置 crate 尚不存在；新增 crate 需人类/Orchestrator 授权。

**2026-10-06 复核**：`PL-102` 已由 TASK-238 闭环；本拆分无新增停车位项。TASK-041 的
剩余部分是平台层真实截图 / 像素遮挡实现，不能由本轮零依赖纯逻辑伪造。

### 8. 新增长期记忆

- **DECISION**：ADR-0073 已登记到 `docs/memory/decisions.md`；未新增 FACT / PITFALL /
  REJECTED 条目，因为本轮的“禁止正则”来自既有 REJECTED 条目而不是新事实。

### 9. 给审阅者的关注点

- `CaptureOutcome::retained_image` 在 `NeverPersist` 下为 `None`，但平台 provider 是否
  自行创建 blob 仍由后续平台实现负责；本拆分只保证纯管线不返回 / 不保留引用。
- `CaptureOptions.redact=true` 只表达“要求平台脱敏”；纯逻辑层产出的遮挡矩形尚未穿过现有
  trait 传给平台，这是公共接口未改的已知边界。
- 滚动清理当前只有有界计划，不含真实滚动或拼接。

---

## 拆分 B（2026-10-06 05:30 自动化轮次，Windows `capture()` 平台实现）

### B1. 约束回执

```text
【任务】TASK-041 拆分 B Windows capture() 真实实现
【目标】GDI 单窗口截图 + CaptureOptions.redact 像素遮挡，trait 形状不变
【write scope】crates/platform/windows/**、docs/DEPENDENCIES.md、ADR / memory / LEDGER / 运维记录
【铁律】1 无静默失败；7 platform 层独占 Win32；9 不静默扩大范围；10 契约先行；12 资源有界
【禁止】新增第三方 crate；image codec；改公共 trait / schema；放宽 lint；真机截图冒充 CI
【验收】fmt / clippy / workspace tests / assistant-platform-windows / 非宿主 clippy / xtask 十一项
【依赖】TASK-017 Done；TASK-041 拆分 A 已由 PR #239 / merge 9c30ba2 落地；ADR-0073 Accepted
【疑问】ImageRef 只有 blob id + 宽高，provider 无 storage 注入点；若不能闭合则记 DRIFT-041-2 留 WIP
```

### B2. 实际改动文件

- `docs/adr/0074-windows-gdi-capture-channel-and-redaction.md`（新增，Accepted）+
  `docs/adr/README.md` + `docs/memory/decisions.md`。
- `crates/platform/windows/Cargo.toml`：新增同一已登记 `windows` crate 的
  `Win32_Storage_Xps` feature（`PrintWindow` 在 0.62.2 的投影位置）。
- `crates/platform/windows/src/window/capture.rs`（新增）：GDI 单窗口截图、尺寸上界、
  UIA 密码框矩形、BGRA 遮挡、状态错误映射；纯逻辑测试在 `window/capture/tests.rs`
  （7 个）。
- `crates/platform/windows/src/window/mod.rs`：`capture()` 从显式未实现改为调用平台实现。
- `crates/platform/windows/src/digest.rs`：新增无前缀 SHA-256 hex 入口，供内容地址使用。
- `crates/platform/windows/src/error.rs`：删除不再使用的 `stub_not_implemented`。
- `crates/platform/windows/README.md` / `src/lib.rs` / `src/uia/mod.rs`：
  同步职责、边界与已知限制。
- `docs/DEPENDENCIES.md`：记录 `windows` crate 的 feature 增量。
- `docs/memory/{facts,pitfalls}.md`：新增本轮实测事实与坑。

### B3. 验收输出摘要

**截至本轮 WIP 提交前已按退出码核**：

- `cargo fmt --all --check` EXIT 0。
- `cargo test -p assistant-platform-windows` EXIT 0：
  lib **104 passed / 4 ignored**、handle_discipline **7 passed**、
  synthetic_input_contract **4 passed**、doctest **1 passed**。
- `cargo clippy -p assistant-platform-windows --all-targets -- -D warnings` EXIT 0
 （仅仓库既有 `unknown lint: clippy::assert_is_empty` 提示）。
- `capture` 的 7 个纯逻辑测试覆盖尺寸上限、区域裁剪、BGRA 遮挡、缓冲长度负向与错误码映射。

**未完成证据**：workspace 全量门禁、非宿主 clippy、xtask 十一项、CI、真机截图均尚未由本轮
标记完成；WIP 状态下不得合并。

### B4. 偏差

**DRIFT-041-2（storage sink 未接入，持久化 blob 无法闭合）**

- 现象：`WindowProvider` 的 `capture()` 只接收 `ResolvedWindow` + `CaptureOptions`，
  返回 `ImageRef`；provider 本身没有 `crates/storage` 的 `Database` / `Connection` 注入点。
- 影响：平台层能截取并遮挡 BGRA、计算 SHA-256 内容地址，但无法在不改公共 trait、
  不在平台层私开 SQLite 写连接的前提下把字节写入 `crates/storage` 的 blob 池。
- 建议：由人类选择 storage sink 形状（例如装配层注入 writer，或把 `ImageRef` 语义改成
  内容地址 token 并由上层立即落盘）；在该决定前不合并本拆分。
- 已做工作：真实 GDI 捕获、像素遮挡、错误映射、资源回收与纯逻辑测试均落地；
  未伪造“已持久化 blob”，PR 保持 WIP。

### B5. 遗留问题

- **DRIFT-041-2**：等待人类裁决 storage sink。
- **真机截图验收待人工**：本轮仅编译和单测，不运行真实 GUI；已知部分硬件合成窗口可能返回黑帧。
- **调用方任意遮挡区间未实现**：现有 `CaptureOptions` 只有 `redact: bool`，传入任意区间会改公共
  形状；当前只消费 UIA `IsPassword` 已判定矩形。

### B6. 给审阅者的关注点

- 重点审查遮挡场景拒绝 `BitBlt` 回退的理由是否足够保守，以及 `PrintWindow` 黑帧的残余风险。
- 重点审查 `GdiCaptureResources` 在成功 / 错误 / 提前返回三路的 DC、bitmap、selected object 回收。
- 重点审查 `DRIFT-041-2` 是否确实必须由人类决定；未裁决前不要把本 PR 合并。
