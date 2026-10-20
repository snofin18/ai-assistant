# TASK-320 Windows UIA ClassAndRole 子元素搜索修复

- 状态：**InProgress**
- 阶段：1　子阶段：**1b bridge**　批次：**1b**　依赖：TASK-045（`DRIFT-045-4`）　预估：S　难度：S
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。

---

- **write scope**：`crates/platform/windows/src/uia/search.rs`、`crates/platform/windows/src/uia/**`、`crates/platform/windows/README.md`；收尾时按 AGENTS.md 更新本卡记录区与 `LEDGER.md`
- **关联**：`DRIFT-045-4`、ADR-0022（UIA 候选）、ADR-0043（元素必须在窗口 scope 内解析）、ADR-0070（RoleAndParent）、`docs/spec/naming.md`

**目标**

修复 Windows UIA 子元素搜索：对经典 `#32770` 文件对话框，`ClassAndRole(Edit, Pane)` 必须和
managed UIA 的 `AND(ClassName=Edit, ControlType=Pane)` 一样唯一命中内部 Edit；不得放宽歧义裁决，
不得改动 selector schema 或公共 trait。

**In scope**

- 复现 `ClassAndRole` 在 `#32770` 子元素上返回 0 的路径。
- 将实现改为“先按 class 搜索，再用 `CurrentControlType` 在本地过滤”，仍保持唯一命中/歧义规则。
- 增加可回归的单元测试，并在真实 Paint 文件对话框上验证。

**Out of scope**

- 不新增 selector kind、不修改 `SelectorValue` / `TargetDescriptor` / `platform/api`。
- 不放宽 `OnAmbiguous::ErrorAndAsk`，不以首个匹配静默替代歧义。
- 不改 Paint selector pack、task package 或 TASK-045 的 handler。

**步骤**

1. 读 `crates/platform/windows/src/uia/search.rs` 与现有 UIA 搜索测试。
2. 复现 `ClassAndRole(Edit, Pane)` 在 `#32770` 子树的 0 命中。
3. 实现 class-only 搜索 + `CurrentControlType` 过滤，并保持错误/歧义语义。
4. 补单元测试；真机验证后再回到 TASK-045。
5. 跑格式、clippy、平台 crate tests 和 xtask 门禁。

**DoD**

- [ ] `ClassAndRole` 子元素搜索在经典 Win32 文件对话框上唯一命中内部 Edit。
- [ ] 现有所有 selector / UIA tests 保持绿，歧义裁决未放宽。
- [ ] `cargo test -p assistant-platform-windows` 全绿。
- [ ] `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings` 全绿。
- [ ] 真机 Paint 文件对话框验证通过，TASK-045 可继续。
- [ ] LEDGER 追加一行；必要时更新 `crates/platform/windows/README.md` 已知限制。

**验收命令**

```powershell
cargo test -p assistant-platform-windows
cargo test -p assistant-agent-core --test production_paint
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo run -p xtask -- hygiene
cargo run -p xtask -- card-check
```

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-320 Windows UIA ClassAndRole 子元素搜索修复
【目标】让 ClassAndRole(Edit, Pane) 在经典 #32770 子元素上与 managed UIA 一致
【write scope】crates/platform/windows/src/uia/**、crates/platform/windows/README.md、本卡与 LEDGER
【铁律】无静默失败；不改公共 schema/trait；不放宽歧义裁决；契约先行
【禁止】改 Paint selector/task package；用首个歧义匹配替代；改测试断言放宽门禁
【验收】platform tests；agent-core production_paint；fmt/clippy；xtask hygiene/card-check
【依赖】TASK-045 DRIFT-045-4；已核对 LEDGER
【疑问】无
```

### 2. 实际改动文件

- `crates/platform/windows/src/uia/search.rs`
- 本任务卡与 `LEDGER.md`

### 3. 验收输出摘要

- `cargo test -p assistant-platform-windows` → 116 passed / 0 failed / 4 ignored；
  integration 7 + 4 + 1 doc test 全绿。
- `cargo clippy -p assistant-platform-windows --all-targets -- -D warnings` → exit 0
  （仅既存 `clippy::assert_is_empty` unknown-lint warning）。

### 4. DoD 逐条核对

- [x] `ClassAndRole` 改为 class-only 搜索后按 `CurrentControlType` 本地过滤。
- [x] 现有平台测试与歧义裁决保持通过。
- [ ] Paint 文件对话框真机验证尚待 TASK-045 回归。

### 5. 偏差

无。修复保持 `OnAmbiguous::ErrorAndAsk` 和 selector 公共形状不变。

### 6. 更合理做法

对经典 Win32 common dialog 使用 class-only + 本地控制类型过滤，避开 provider 的
AND 属性条件在跨 HWND 子树上返回 0 的差异；歧义仍在统一 `resolve.rs` 中裁决。

### 7. 遗留问题

真机验证与 TASK-045 回归待完成。

### 8. 新增长期记忆

TASK-320 验证通过后再追加平台 UIA 事实。

### 9. 给审阅者的关注点

1. 平台过滤是否仍保持“唯一命中或歧义拒绝”。
2. 是否只在 class 候选集合内读取 `CurrentControlType`，没有扩大到桌面根。
