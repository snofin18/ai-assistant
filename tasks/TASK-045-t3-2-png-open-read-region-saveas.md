# TASK-045　T3.2：打开 PNG → 读尺寸与缩放 → 区域标记 → 另存为新文件

- 状态：**Blocked**
- 阶段：1　子阶段：**1b**　批次：**1b**　依赖：044　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-1-pilots.md`。

---

- **依赖**：044　**预估**：M　**难度**：M
- **write scope**：`adapters/com.microsoft.paint/selectors/**`、`adapters/com.microsoft.paint/tasks/**`、`apps/agent-core/src/paint_handler_actions.rs`、`apps/agent-core/src/paint_handler_map.rs`、`apps/agent-core/src/paint_handler_support.rs`、`apps/agent-core/src/paint_handlers.rs`、`apps/agent-core/src/paint_registry.rs`、`apps/agent-core/tests/production_paint.rs`、`apps/agent-core/tests/support/paint_real_run.rs`、`eval/tasks/paint/t3.2/**`；收尾时按 AGENTS.md 更新本卡记录区、`LEDGER.md`、状态文件和 `docs/memory/apps/paint.md`
- **关联**：`plans/stage-1-pilots.md` 批次表 1b、`docs/wbs-overview.md` §6（DoD）、ADR-0067（显式坐标空间）、ADR-0084 / ADR-0085（Paint 运行时装配与元素几何）、ADR-0092（窗口几何 / DPI）、ADR-0096（Paint ribbon 分组寻址）、TASK-106（`paint.file.open` / `paint.file.save_as` 留待后续）、TASK-307（T3.2 续画基线）

**目标**

让 Paint 的真实生产路径完成一次可验证的 T3.2：

1. 打开一个确定性的本地 PNG fixture；
2. 从 Paint 当前文档读回真实画布尺寸与缩放；
3. 在当前缩放 / 滚动坐标变换下，用 3 px 红色、无填充矩形标记指定区域；
4. 另存为新的 PNG 路径，绝不静默覆盖已有文件；
5. 保存后从磁盘读回新文件并验证 PNG 尺寸、来源文档和像素变化。

**write scope**（本卡独有部分，完整列表见 plan 批次表）

仅上面列出的文件和 AGENTS.md 允许的收尾公共文件。`adapters/com.microsoft.paint/tools/tools.json`
已经声明 `paint.file.open` / `paint.file.save_as`，本卡**不修改公共 Tool schema**；如果实现时
发现必须改 schema、新增工具、加依赖或改平台 trait，停止并按漂移触发器记 DRIFT。

**In scope**

- 把已声明的 `paint.file.open` 与 `paint.file.save_as` 接入 Paint 生产 registry / handler map。
- 为 Windows common file dialog 增加 Paint 专用 selector target 和实测证据；对话框按
  `#32770` + 跨进程身份约束，不以本地化标题作主 selector。
- 新增 `t3.2.open-png-mark-region-save-as.json` 任务包；步骤只引用已注册工具，复用
  `paint.shape.select`、`paint.outline.select`、`paint.fill.select`、`paint.size.select`、
  `paint.color.select_foreground`、`paint.layer.select`、`paint.canvas.resolve_point`、
  `paint.canvas.draw_rectangle`、`paint.canvas.capture_pixels`。
- 新增 `eval/tasks/paint/t3.2/**`：fixture 生成、静态校验、10 次运行、独立坐标误差测量与
  raw evidence。
- fake platform 单测覆盖打开、读取尺寸 / 缩放、矩形标记、另存成功、目标已存在、
  对话框取消 / 缺失、保存后文件不存在等失败边界。
- 真机 ignored 用例使用生产 host / registry / task package；人类负责把 Paint 置前台，
  harness 只在前台复核通过后发送输入。

**Out of scope**

- 不修改 `adapters/com.microsoft.paint/tools/tools.json`、任何公共 Tool schema、ErrorCode、
  `crates/platform/api` trait 或 ADR 已决事项。
- 不实现 `paint.canvas.rollback_last_write`；撤销与像素回滚属 TASK-046。
- 不支持覆盖已有文件、备份后覆盖、自动确认 Paint 的未保存提示或无人值守 GUI 运行。
- 不新增第三方依赖、不引入新抽象层、不做 Drive-by refactor。

**契约细节**

- fixture：`eval/tasks/paint/t3.2/prepare_fixture.py` 使用 Python 标准库确定性生成
  `2048 x 1536` 白底 PNG，带蓝色边框和两条黑色对角线；`expected.json` 固定 SHA-256、
  宽高和字节数。fixture 生成必须可重复，CI 静态检查不得依赖 Windows GUI。
- 目标路径：全部使用绝对路径；运行目录位于 `%TEMP%\assistant-paint-t3-2\<run-id>`。
  成功输出为 `marked-output.png`，保护目标为 `protected-output.png`。
- 打开：`paint.file.open` 校验路径为绝对、存在的普通 `.png` 文件，且
  `discard_unsaved_confirmed=false`。`Ctrl+O` 后必须在有界时间内找到并确认
  `file_open_dialog`；填入路径并触发 Open。若出现未保存提示、用户取消、对话框缺失或
  身份不符，分别 fail-closed 为 `UserInteraction`、`TargetNotFound` 或
  `TargetUnresponsive`，不得继续发送绘图输入。
- 读尺寸与缩放：打开完成后从 `CanvasSizeTextBlock` 读文档宽高，从 canvas render bounds
  与文档宽度计算 `zoom_ratio`；输出 `opened_path`、`canvas_width_px`、
  `canvas_height_px`、`zoom_ratio`。真实用例必须记录实际 `zoom_ratio < 1.0`，不得硬编码
  1.0；若设备始终只能以 1.0 打开，停止记 `DRIFT-045-ZOOM`，不得把未缩放状态写成通过。
- 区域标记：固定为“3 px 红色、无填充矩形”，画布坐标 `(64,48) -> (1984,1488)`，
  前景色 `(237,28,36)`。任务包先用 `paint.file.open` 的真实 `zoom_ratio` 解析两个点，
  再绘制；`paint.canvas.draw_rectangle` 必须继续使用 pre/post capture 的 content address
  下界，发现 no-op 或尺寸变化即 `VerifyFailed`。
- 另存：`paint.file.save_as` 只接受绝对 `.png` 路径、父目录存在且可写、
  `overwrite_existing=false`。任务包在调用工具前使用 `inspect_target_path` host service +
  policy 前置步骤：目标静态已存在时返回 `PolicyDenied`，且 `paint.file.save_as` handler
  不被调用。handler 仍做防御性预检，并在填写对话框后、点击 Save 前再次检查；若目标在
  竞态窗口出现，取消 / 关闭对话框、不得点击 Save，并以现有可表达的 `UserInteraction`
  fail-closed，断言目标字节、大小和 mtime 不变。此处不改 `crates/tool-bus` 的公共错误
  映射。成功时等待新文件出现，校验 PNG 签名、尺寸
  `2048 x 1536`、文件非空且可由 fixture 解码器读回；否则 `VerifyFailed`。
- 坐标验收：除 10 次成功运行外，另用 `coordinate-error.ps1` 读取拖拽起终点并记录期望 /
  实际物理像素，误差判据 `<= 2 px`。不得由成功次数反推像素误差。
- 真实 GUI：Paint 版本记录在 raw evidence；测试启动前等待人类把 Paint 置前台，等待窗口
  静置后再次复核 foreground。Paint 不在前台、弹出未保存提示或用户中途取消时必须失败。

**步骤**

1. 记录环境：Windows 版本、Paint 版本、显示器 / DPI、fixture SHA-256 与目标临时目录。
2. 增加 file dialog selector targets，并用真实 Paint / Shell dialog 取证后更新
   `probe_status`；不得只凭猜测把本地化标题升级为主 selector。
3. 在现有 Paint handler 文件中实现 `paint.file.open` / `paint.file.save_as`，复用现有
   key / target lease / fingerprint / bounded polling 原语；注册到
   `PAINT_EXPECTED_TOOL_NAMES`，`paint.canvas.rollback_last_write` 仍保持 unmounted。
4. 新增 T3.2 任务包与 eval fixture / cases / expected / validate；覆盖成功路径与所有负例。
5. 扩展 `production_paint.rs` 的 fake platform 测试，确保 handler 调用、输出 postcondition、
   目标不覆盖和失败不落盘都有断言。
6. 增加真机 ignored 用例与 `run-ten.ps1` / `coordinate-error.ps1`；先由人类打开 Paint，
   再执行并保存 raw 输出。
7. 跑本卡专项验收、全量 Rust / Python / xtask 门禁；失败时按真实错误修实现，不改断言
   或放宽门禁。
8. 更新本卡记录区、LEDGER、应用记忆与状态文件，提交 PR。

**DoD**

- [ ] `paint.file.open` / `paint.file.save_as` 已在生产 registry 注册，工具 schema 零改动。
- [ ] T3.2 任务包只引用已注册工具；打开、缩放读取、矩形标记、另存四段均有 postcondition。
- [ ] fixture 生成可重复，静态校验固定 SHA-256、宽高和字节数。
- [ ] fake platform 正向 + 负向测试全绿；目标静态存在时由 policy 返回 `PolicyDenied`，
      目标不变且 `paint.file.save_as` 未被调用；对话框阶段竞态返回 `UserInteraction`，
      未点击 Save 且目标不变。
- [ ] 真机 ignored 用例在人类前台守护下通过；实际 `zoom_ratio < 1.0`，矩形坐标误差
      `<= 2 px`，输出 PNG 可从磁盘读回且尺寸正确。
- [ ] 10 次运行证据落 `eval/tasks/paint/t3.2/ten-run-summary.json`；成功率与坐标误差来自
      独立 raw 输出，不伪造、不由成功次数反推。
- [ ] `cargo fmt --all --check` 0 diff；`cargo clippy --all-targets -- -D warnings` exit 0；
      `cargo test --workspace` 全绿。
- [ ] `python adapters/com.microsoft.paint/tests/validate_adapter_pack.py` 与
      `python eval/tasks/paint/t3.2/validate.py` 均 PASSED。
- [ ] `xtask hygiene / memory-counts / adr-index / verify-schemas / codegen --check /
      refscan / docscan / card-check / check-ledger` 全部 PASSED。
- [ ] `LEDGER.md` 追加一行；新增应用事实进入 `docs/memory/apps/paint.md`，并同步
      `MEMORY.md` 规模表（如行数变化）。

**验收命令**

```powershell
python eval/tasks/paint/t3.2/prepare_fixture.py --check
python eval/tasks/paint/t3.2/validate.py
python adapters/com.microsoft.paint/tests/validate_adapter_pack.py
cargo test -p assistant-agent-core --test production_paint
cargo test -p assistant-agent-core --test production_paint test_paint_t3_2_open_png_mark_region_save_as -- --ignored --nocapture
./eval/tasks/paint/t3.2/run-ten.ps1 -AllowClosePaint
./eval/tasks/paint/t3.2/coordinate-error.ps1 -AllowClosePaint
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check
cargo run -p xtask -- hygiene
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- refscan
cargo run -p xtask -- docscan
cargo run -p xtask -- card-check
cargo run -p xtask -- check-ledger
```

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-045 T3.2：打开 PNG -> 读尺寸与缩放 -> 区域标记 -> 另存为新文件
【目标】接入 Paint 的 file.open / file.save_as，完成任务包、静态评测、fake 正负例和真机证据
【write scope】adapters/com.microsoft.paint/selectors/**、adapters/com.microsoft.paint/tasks/**、
              apps/agent-core/src/paint_handler_actions.rs、paint_handler_map.rs、
              paint_handler_support.rs、paint_handlers.rs、paint_registry.rs、
              apps/agent-core/tests/production_paint.rs、
              apps/agent-core/tests/support/paint_real_run.rs、eval/tasks/paint/t3.2/**；
              收尾时按 AGENTS.md 更新本卡记录区、LEDGER、状态文件和 paint 应用记忆
【铁律】无静默失败；不可信输入先校验；写操作必须有 postcondition；
        不得静默扩大范围；公共接口/schema 契约先行
【禁止】修改 tools/tools.json、公共 Tool schema、ErrorCode、平台 trait；
        实现 paint.canvas.rollback_last_write；覆盖已有文件；新增依赖；
        无人值守操作真实 GUI
【验收】fixture/validate/adapter validate；production_paint fake 测试；
        ignored 真机单次、十次、独立坐标误差；fmt/clippy/workspace tests；指定 xtask 门禁
【依赖】TASK-044 Done（LEDGER af005c9）；TASK-106 Done（f5bdfa6）；
        TASK-307 Done（a075f1f）；ADR-0067/0084/0085/0092/0096 Accepted
【疑问】保存竞态要求 handler 返回 PolicyDenied，但当前 ToolBusError 无法表达；
        已停止并记录 DRIFT-045-1，等待裁决
```

### 2. 实际改动文件

无产品代码改动。仅更新本卡状态与执行记录，并在 `LEDGER.md` 追加阻塞事件。

### 3. 验收输出摘要

未运行实现或真机验收。契约阻断发生在设计复核阶段：

- 卡面要求目标已存在或竞态出现时返回 `PolicyDenied`。
- 目标静态已存在可用任务包前置 `policy` 步骤拒绝，handler 不会被调用。
- 竞态检查发生在 `paint.file.save_as` handler 打开对话框后、点击 Save 前，
  因此拒绝发生在 handler 内部。
- 现有 `ToolBusError::error_code()` 没有 `PolicyDenied` 映射；
  `Mcp` 仅特判 `-32601`（`CapabilityMissing`）与 `-32010`（`UserInteraction`），
  其余 MCP code 映射为 `Transient`。

### 4. DoD 逐条核对

- [ ] 全部未满足；实现尚未开始，等待 `DRIFT-045-1` 裁决。

### 5. 偏差

`DRIFT-045-1`

- **触发器**：#3（修改公共接口 / 错误映射）、#5（超出 write scope）、#8（卡片契约与代码错误语义冲突）。
- **现象**：卡片要求 `paint.file.save_as` 在目标已存在或竞态创建时返回
  `PolicyDenied`，且目标不变、save handler 未被调用。静态已存在可由任务包的
  `inspect_target_path` + `policy` 步骤覆盖；但竞态检查必须在 handler 已打开并填写
  Save As 对话框后、点击 Save 前执行，此时只能通过 `ToolBusError` 返回失败。
  当前 `ToolBusError::error_code()` 无法产生 `PolicyDenied`。
- **影响**：若按现状实现竞态负例，只能伪造为 `Transient` 或其它错误码，无法满足 DoD；
  若要产生 `PolicyDenied`，必须修改 `crates/tool-bus` 的公共错误类型或错误码映射行为，
  该路径不在本卡 write scope 内，并触发公共契约变更门槛。
- **建议**：由人类/Orchestrator 裁决三选一：
  1. 批准 ADR + spec 变更，扩权本卡修改 `ToolBusError` 与其 `ErrorCode` 映射；
  2. 增加一个能在保存对话框阶段执行 policy 拒绝的既有运行时能力，不扩公共错误面；
  3. 显式修改本卡契约，将竞态错误码改为现有可表达的错误，但不以放宽测试断言实现。
- **Orchestrator 裁决（2026-10-19）**：采用方案 3。静态目标存在由任务包
  `inspect_target_path` + policy 返回 `PolicyDenied`，handler 不进入；保存对话框打开后的
  竞态由 handler 取消对话框、不点击 Save，并以既有 `UserInteraction` fail-closed，
  同时保持目标字节 / 大小 / mtime 不变。该裁决不修改 `crates/tool-bus`、公共 `ErrorCode`
  或 ADR/spec；恢复实现，状态改为 `InProgress`。

`DRIFT-045-2`

- **触发器**：#3（公共错误码映射）、#5（超出 write scope）、#8（卡片契约与代码错误语义冲突）。
- **现象**：卡片要求 `paint.file.open` 在对话框“身份不符”时 fail-closed 为
  `TargetUnresponsive`。但 `ToolBusError::error_code()` 的可达集合中没有
  `TargetUnresponsive`：`InvalidArguments -> ToolInvalidArgs`、`UnknownTool /
  ToolNotMounted -> TargetNotFound`、`Transport` 与未知 MCP code -> `Transient`，
  `Mcp` 只特判 `CapabilityMissing` 与 `UserInteraction`。
- **影响**：在对话框已解析、但跨进程身份与 Paint 不一致时，handler 若返回
  `TargetUnresponsive`，必须修改 `crates/tool-bus` 的公共错误类型或映射行为；
  这不在本卡 write scope，也会留下未配套 ADR/spec 的公共契约漂移。若改用
  `TargetNotFound`，则与卡面明确指定的错误码不同，不能静默替换。
- **建议**：由人类/Orchestrator 裁决：
  1. 批准 ADR/spec 变更，扩权本卡修改 `ToolBusError` 与 `ErrorCode` 映射；
  2. 明确把“对话框身份不符”改为既有 `TargetNotFound` 语义（窗口匹配失败）；
  3. 增加一个能在对话框身份校验阶段表达 `TargetUnresponsive` 的既有运行时路径。
- **已停止的工作**：本轮仅完成只读复核；未写文件对话框 selector、handler、task package、
  fixture、eval 或 fake 测试。未调用真实 GUI。
- **需要人类裁决**：是。

### 6. 更合理做法

无。当前 blocker 属于公共错误契约缺口，不能在卡内用局部变通替代。

### 7. 遗留问题

`DRIFT-045-1` 已裁决，按“静态 `PolicyDenied` + 对话框竞态 `UserInteraction`”实现。
不得把竞态负例改写成 `Transient`、点击 Save 或跳过目标不变断言。

`DRIFT-045-2` 未裁决：文件对话框身份不符要求 `TargetUnresponsive`，但该错误码
当前不可由 handler 产生。实现暂停，等待明确的错误码映射或卡面修正。

### 8. 新增长期记忆

无。契约尚未裁决，暂不把未决方案写入长期事实。

### 9. 给审阅者的关注点

1. 确认静态目标存在与保存竞态是否需要不同错误码。
2. 若选择 `PolicyDenied`，需明确修改的是 `ToolBusError` 公共枚举还是仅 MCP code 映射，
   并补齐 ADR/spec。
3. 保持“目标字节/大小/mtime 不变”与“竞态时不得点击 Save”两条安全断言不变。
4. 裁决 `DRIFT-045-2`：文件对话框身份不符是新增公共错误映射，还是按
   `TargetNotFound` 的既有语义处理。
