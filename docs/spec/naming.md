# spec: 命名与注释规范（naming）

> 状态：Draft（待 ADR 批准）　版本：0.1　日期：2026-09-16
> 上位：`AGENTS.md` §5、`docs/governance-ai-agent-execution.md` §6.1.1/§6.1.2
> 强制性：**本文档是契约**。违反即 CI 失败（`xtask check-comments`）或 Reviewer 拒绝合并。
> 变更门槛：新增受控词或修改白名单 → 需 ADR。

---

## 1. 目标

1. **一眼可懂**：人类通过阅读逐步熟悉 Rust，因此命名的自解释性优先于简洁性。
2. **全项目一致**：同一概念只用一个词（受控词汇表），消除"看起来是两个东西其实是一个"。
3. **可机器校验**：规则尽量落到 lint 与 `xtask check-comments`，而不是靠自觉。

---

## 2. 通用规则

| # | 规则 | 反例 | 正例 |
|---|---|---|---|
| N1 | 禁止缩写，除 §3 白名单 | `resolve_tgt_desc()` | `resolve_target_descriptor()` |
| N2 | 禁止无信息名 | `data` `info` `temp` `helper` `util2` `manager` | `resolved_window` `audit_event` `shadow_copy_path` |
| N3 | 禁止拼音、禁止中英混杂 | `getChuangKou()` `readWenJian()` | `list_windows()` `read_document()` |
| N4 | 函数名 = 动词 + 宾语 | `window()` `policy()` `fingerprint()` | `list_windows()` `evaluate_policy()` `compute_state_fingerprint()` |
| N5 | 布尔量带助动词前缀 | `visible` `dirty` `enabled` | `is_visible` `has_unsaved_changes` `is_enabled` |
| N6 | 枚举变体表达完整语义 | `TaskStatus::Wait` `Error::E1` `Result::Bad` | `TaskStatus::AwaitingApproval` `TargetNotFound` `VerifyFailed` |
| N7 | 集合复数、单个单数 | `task: Vec<Task>` | `tasks: Vec<Task>` |
| N8 | trait 名 = 能力（-er/-or 或名词短语），struct 名 = 名词 | `struct WindowProvider` | `trait WindowProvider` + `struct ResolvedWindow` |
| N9 | 避免双重否定 | `is_not_disabled` `should_not_skip` | `is_enabled` `should_run` |
| N10 | 长度服从清晰度：宁可长，不可含糊 | `fp()` `chk()` | `state_fingerprint()` `verify_postconditions()` |
| N11 | 名称不得编码实现细节（除非该细节是契约） | `read_text_via_uia()`（在 core 层） | `read_text()`（实现细节属于 platform 层） |
| N12 | 同一层的同类事物用同一构词模式 | `list_windows` / `getWindows` / `window_all` | 统一 `list_*` / `resolve_*` / `compute_*` |

**动词约定**（全项目统一）：

| 动词 | 语义 | 例 |
|---|---|---|
| `list_` | 返回集合，不做筛选保证 | `list_windows()` |
| `resolve_` | 由描述/身份解析出实例（可能失败、可能歧义） | `resolve_target_descriptor()` |
| `find_` | 按查询条件搜索（返回 Option/Vec） | `find_element()` |
| `compute_` | 纯计算，无副作用 | `compute_state_fingerprint()` |
| `evaluate_` | 依规则做判定（纯函数） | `evaluate_policy()` |
| `acquire_` / `release_` | 获取/释放资源（成对出现） | `acquire_target_lease()` |
| `capture_` | 采集证据（截图、快照） | `capture_window_screenshot()` |
| `verify_` | 后置校验 | `verify_postconditions()` |
| `persist_` / `load_` | 落盘 / 载入 | `persist_checkpoint()` |

---

## 3. 缩写白名单（仅此 23 个）

```text
id  url  uri  ui  os  db  ipc  mcp  uia  ax  atspi  cdp  dpi
ocr  ttl  http  json  sql  fs  vm  px  ms  us  ns
```

规则：白名单内的缩写**只能整体使用**，不得二次截断（`cfg` `mgr` `impl2` 一律禁止）；白名单外的缩写需要 ADR 才能加入。

---

## 4. 受控词汇表（controlled vocabulary）★

同一概念全项目只用左列的词。**新增受控词需 ADR**。

| 用这个词 | 禁止的同义词 | 定义 | 详见 |
|---|---|---|---|
| `Target` | Element / Object / Item / Control / Widget | 一次操作的对象（应用+窗口+文档+控件），由 `TargetDescriptor` 描述身份 | v2 §6 |
| `TargetDescriptor` | Selector / Locator / Address | 目标的稳定身份 + selector 候选链 | v2 §6.2 |
| `Selector` | Path / Query / Matcher | 候选链中的一条定位策略 | v2 §6.3 |
| `Step` | Action / Operation / Stage / Phase | 任务计划中的一个执行单元 | v2 §8.3 |
| `Task` | Job / Run / Session | 用户请求对应的完整工作单元 | v2 §8 |
| `Session` | Conversation / Context | 一次对话上下文（可跨多个 Task） | v2 §5.1 |
| `Tool` | Command / Function / API / Endpoint | 模型可见的单个能力 | v2 §5.1 |
| `Skill` | Plugin / Extension / Module / Package | 可分发单元（tools + prompts + 权限 + 签名） | v2 §5.1 |
| `Adapter` | Driver / Connector / Integration / Provider | 某个被控应用的适配包 | v2 §6.8 |
| `AppMap` | KnowledgeBase / AppProfile | 应用的静态知识库（命令表/UI 地图/已知坑/undo 能力） | v2 §6.7 |
| `Lease` | Lock / Mutex / Reservation / Claim | 目标级排他租约 | v2 §8.8 |
| `Anchor` | Snapshot / Backup / Checkpoint / Mark | 撤销锚点（内容快照 / 影子副本 / undo 预算 / historyState 的统称） | v2 §9.3 |
| `Fingerprint` | Hash / Digest / Signature / State | 状态指纹 | v2 §7.3 |
| `Evidence` | Artifact / Attachment / Proof / Capture | 树快照、截图等取证物 | v2 §7.5 |
| `Capability` | Feature / Support / Flag / Permission | 运行时探测出的能力（**与"权限 Permission"严格区分**） | v2 §13.1.2 |
| `Permission` | Right / Privilege / Grant | 授权（谁 × 工具 × 目标 × 范围 × TTL） | v2 §10.3 |
| `Egress` | Upload / Send / Network / Outbound | 数据出域 | v2 §12.6 |
| `Taint` | Dirty / Unsafe / Flagged / Suspect | 污点（不可信内容标记） | v2 §12.4 |
| `Channel` | Backend / Transport / Route | L1~L5 的能力获取通道 | v2 §2.1 |
| `Reversibility` | Undoability / Safety | 可逆性级别 L0~L3 | v2 §9.1 |
| `Host` | Worker / Runner / Agent | 执行进程（`automation-host`） | v2 §3.3 |
| `Postcondition` | Assertion / Check / Verify | 后置条件断言 | v2 §7.4 |
| `Drift` | Deviation / Detour | 偏离规划的记录 | gov §4.3 |

**易混词特别提示**：
- `Capability`（能做什么）≠ `Permission`（允许做什么）：前者是探测结果，后者是策略结果。
- `Anchor`（撤销用）≠ `Checkpoint`（恢复用）：前者面向"回到操作前状态"，后者面向"任务续跑"。
- `Adapter`（应用适配包）≠ `Provider`（平台能力实现的 trait 实现体，如 `WindowsUiAutomationProvider`）。
- `Tool`（模型可见）≠ `Action`（Adapter 内部的原子操作，可能对应多个平台调用）。

---

## 5. Rust 命名

| 对象 | 规则 | 例 |
|---|---|---|
| crate | `assistant-<domain>`，kebab-case；代码中为 `assistant_<domain>` | `assistant-policy`、`assistant-platform-windows` |
| 模块文件 | 小写单数名词或名词短语，snake_case | `target_lease.rs`、`tree_snapshot.rs` |
| 类型 | `PascalCase`，名词 | `ResolvedWindow`、`LeaseGuard` |
| trait | `PascalCase`，能力名（常以 `-er/-or` 或 `Provider` 结尾） | `UiAutomationProvider`、`TradingGate` |
| 函数/方法 | `snake_case`，动词开头 | `acquire_target_lease()` |
| 常量 | `SCREAMING_SNAKE_CASE` | `DEFAULT_RESOLVE_TIMEOUT_MS` |
| 静态/线程局部 | `SCREAMING_SNAKE_CASE` | `POLICY_CACHE` |
| 错误枚举 | `<Domain>Error`，变体说原因 | `TargetError::NotFound`、`PolicyError::DeniedByRule` |
| newtype id | `<Thing>Id`，禁止裸 `String`/`u64` 传递 | `TaskId`、`StepId`、`AppId`、`BlobId` |
| 泛型参数 | 单大写字母或短描述名 | `T`、`Provider` |
| 测试函数 | `test_<unit>_<condition>_<expected>` | `test_resolve_target_with_ambiguous_matches_returns_ambiguous_error` |
| feature flag | 小写连字符 | `wayland-portal`、`sqlcipher` |

**Rust 特别要求**：
- **id 必须用 newtype**（`struct TaskId(Uuid)`），禁止把 `String` 当 id 传参 —— 这是防止 agent 把 `task_id` 和 `step_id` 传错的最有效手段；
- 时间/超时用 newtype（`Timeout`、`DurationMs`），禁止裸 `u64`；
- 错误一律 `thiserror` 枚举（库）/ `anyhow`（二进制顶层），禁止 `Box<dyn Error>` 作为公共接口。

---

## 6. TypeScript / React 命名

| 对象 | 规则 | 例 |
|---|---|---|
| 组件文件与组件名 | `PascalCase.tsx`，与组件同名 | `ApprovalCard.tsx` |
| hooks | `use<CamelCase>.ts` | `useTaskTimeline.ts` |
| store / context | `<domain>Store.ts` | `policyStore.ts` |
| 类型/接口 | `PascalCase`，**由 schema 生成，不手写** | `ToolSpec`、`AuditEvent` |
| 常量 | `SCREAMING_SNAKE_CASE` | `MAX_DIFF_PREVIEW_LINES` |
| 事件处理函数 | `handle<Event>` | `handleApproveClick` |
| IPC 封装 | `ipc/<domain>.ts`，函数名 = 动词 + 宾语 | `ipc/task.ts` → `cancelTask()` |
| CSS 类 | Tailwind 优先；自定义类用 kebab-case | `approval-card--danger` |
| i18n key | `<feature>.<element>.<purpose>` | `approval.card.approve_once` |

---

## 7. 协议、Schema 与数据命名

| 对象 | 规则 | 例 |
|---|---|---|
| Tool 名 | `<app>.<domain>.<action>`，全小写下划线 | `notepad.replace_text`、`excel.write_named_range` |
| 权限标识 | `<domain>.<action>` | `document.write`、`window.read` |
| 能力标识 | `<layer>.<capability>` | `a11y.editable_text`、`input.takeover_detection` |
| ErrorCode | `PascalCase`，`<Domain>.<Reason>` 或单段 | `Target.NotFound`、`PolicyDenied`、`VerifyFailed` |
| 审计事件类型 | `<noun>.<past_verb>` | `tool.executed`、`approval.granted`、`lease.acquired` |
| DB 表 | 复数 snake_case | `task_steps`、`audit_logs` |
| DB 列 | snake_case；布尔用 `is_`/`has_`；时间用 `_at`；外键用 `_id` | `is_reversible`、`created_at`、`task_id` |
| DB 索引 | `idx_<table>_<cols>` | `idx_tasks_status_started` |
| 配置键（TOML） | kebab-case 或 snake_case（**同一文件内统一**） | `capability-level`、`version_range` |
| 文件/目录 | kebab-case（文档与目录）、snake_case（代码文件） | `docs/storage-design.md`、`tree_snapshot.rs` |
| 任务卡 | `TASK-<NNN>-<slug>.md` | `TASK-035-notepad-adapter.md` |
| ADR | `NNNN-<slug>.md` | `0005-reversibility-four-levels.md` |
| 分支 | `task/TASK-<NNN>-<slug>` | `task/TASK-035-notepad-adapter` |
| Spike 报告 | `SPIKE-<X>.md` | `SPIKE-A.md` |

---

## 8. 注释标签注册表（只允许这些）

| 标签 | 用途 | 格式 | CI 校验 |
|---|---|---|---|
| `// SAFETY:` | unsafe/FFI 的安全性说明 | 紧跟 unsafe 块上一行 | unsafe 块必须有，否则失败 |
| `// PITFALL(app=<id>):` 或 `(platform=<os>):` | 应用/平台的坑 | 一行说明 + 应对 | 被 `xtask` 汇总进 `MEMORY.md` §5 与 App Map |
| `// PERF:` | 性能预算与实测依据 | 含预算数值 | 涉及热路径的函数必须有 |
| `// TODO(TASK-NNN):` | 待办 | **必须带卡号** | 无卡号 → 失败 |
| `// STUB(TASK-NNN):` | 占位实现 | **必须带卡号** | 无卡号 → 失败 |
| `// INVARIANT:` | 本模块保证的性质 | 与 README 的不变量一致 | 公共模块建议有 |
| `// NOTE(spec=<path>#<section>):` | 指向契约 | — | — |

**禁止的标签**：`FIXME`（用 `TODO(TASK-NNN)`）、`HACK`（不允许进主干）、`XXX`、裸 `TODO`。

---

## 9. Reviewer 命名审查清单

```text
1. 是否有白名单外的缩写？
2. 是否有 data/info/temp/helper 这类无信息名？
3. 是否违反受控词汇表（用了同义词）？
4. 函数名是否动词开头且动词语义符合 §2 的约定？
5. 布尔量是否有 is_/has_/can_/should_ 前缀？
6. id 是否用了 newtype（而不是裸 String/u64）？
7. 错误变体名是否说明了原因？
8. 测试名是否符合 test_<unit>_<condition>_<expected>？
9. 公共 API 是否有文档注释（含错误语义、超时与取消、幂等性）？
10. 模块头是否写了职责 + 边界 + 不变量？
11. 注释与代码是否一致（改过函数是否改了注释）？
12. 是否有被禁止的标签（FIXME/HACK/裸 TODO）？
```

---

## 10. `xtask check-comments` 的规则（机器护栏）

```text
① 扫描禁用标签：FIXME / HACK / XXX / 裸 TODO（无卡号）→ 失败
② 扫描被注释掉的代码块（连续 ≥5 行以 // 开头且形似代码）→ 失败
③ 公共 API（pub fn/struct/trait/enum）缺少文档注释 → 失败
④ 模块文件缺少头部 //! 注释 → 警告（core/policy/task-engine/verify/undo → 失败）
⑤ unsafe 块缺少 // SAFETY: → 失败
⑥ PITFALL 标签格式不合法（缺 app= 或 platform=）→ 警告
⑦ 受控词汇表违规：对标识符做词干匹配，命中禁用同义词（如 target_element、step_action、lease_lock）→ 警告并列出位置
⑧ 缩写检测：标识符中出现白名单外的 ≤4 字母全小写词干（启发式）→ 警告
```

⑦⑧ 采用**警告而非失败**，避免启发式误报阻塞开发；但同一位置连续两次被警告仍未处理 → Reviewer 必须给出理由。
