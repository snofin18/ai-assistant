# TASK-335　CDP DOM expression 实际求值与结果校验

- 状态：**Done**
- 阶段：1　子阶段：1c　批次：基础设施修复
- 依赖：TASK-109、TASK-332、TASK-334、`DRIFT-056-7`
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031）。分界线以上是正文；以下由 Implementer 填写。

---

- **write scope**：`crates/platform/windows/src/cdp/runtime.rs`、
  `crates/platform/windows/tests/cdp_provider_contract.rs`、`crates/platform/windows/README.md`、
  `eval/tasks/edge/t5.1/evidence/2026-10-27-task-335-cdp-evaluate-expression/**`

## 目标

修复 `RuntimeCdpProvider::evaluate_dom` 把调用方 expression 先 JSON 字符串化再放入
可执行位置的问题，使受控 DOM expression 真正执行，并保持异常、缺失/非字符串结果与
快照形状全部 fail-closed。

## 必须遵守

1. 出站 `Runtime.evaluate` 必须执行调用方 expression，不能把 expression 源码当作字符串值返回。
2. `exceptionDetails`、缺失 `result.result.value`、非字符串 value 与非法 snapshot JSON
   继续返回明确 `VerifyFailed`，不得回退为空 DOM 或猜测成功。
3. 既有 64 KiB expression、1 MiB DOM、WebSocket message 与 timeout 上限保持不变。
4. 不改公共 trait、Tool schema、ErrorCode、依赖、Policy、CSV 契约或 TASK-056 断言。

## Out of scope

- 不执行 TASK-056 的完整 10/10 或外部 `books.toscrape.com` 3/3 人工验收。
- 不实现新的 browser tool、DOM serializer、CSV normalizer 或 click/download 能力。
- 不为绕过失败而放宽 DOM JSON 解析器。

## 验收命令

```powershell
cargo test -p assistant-platform-windows --test cdp_provider_contract
cargo test -p assistant-platform-windows
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p assistant-core arch::
cargo run -p xtask -- verify-schemas
cargo run -p xtask -- codegen --check
cargo run -p xtask -- hygiene
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- refscan
cargo run -p xtask -- docscan
cargo run -p xtask -- card-check
cargo run -p xtask -- check-ledger
cargo run -p xtask -- check-comments
cargo run -p xtask -- check-migrations
cargo deny check
```

## DoD

- [ ] TCP 契约验证 expression 以代码执行，而不是作为 JSON 字符串返回。
- [ ] evaluation exception、缺失/非字符串 value 与非法 snapshot JSON 均显式失败。
- [ ] 真实 Edge 至少将 `read_page_1` 从 expression 类型错误推进到下一个独立边界。
- [ ] 上述验收命令全绿；TASK-056 仍保持 Blocked，除非其独立 10/10 证据真实完成。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-335 CDP DOM expression 实际求值与结果校验
【目标】修复 evaluate_dom 把 expression 当字符串返回的根因，解除 DRIFT-056-7
【write scope】仅：platform CDP runtime/contract/README、TASK-335/TASK-056 记录、状态同步与证据
【铁律】1 无静默失败 / 2 CDP 输入与返回值先校验 / 4 DOM 读取有类型 postcondition / 9 不扩范围 / 12 有界
【禁止】不放宽 JSON 解析；不改公共 trait/schema/ErrorCode/依赖；不把 navigate 成功冒充 TASK-056 完成
【验收】expression 形态、异常/非字符串契约、platform/workspace/xtask/deny → 全绿
【依赖】TASK-109 / TASK-332 / TASK-334 Done，DRIFT-056-7 已核对
【疑问】无
```

### 2. 实际改动文件

- `crates/platform/windows/src/cdp/runtime.rs`
- `crates/platform/windows/tests/cdp_provider_contract.rs`
- `crates/platform/windows/README.md`
- `eval/tasks/edge/t5.1/evidence/2026-10-27-task-335-cdp-evaluate-expression/**`
- `tasks/TASK-335-cdp-dom-expression-execution.md`
- `tasks/TASK-056-t5-1-open-site-extract-list-write-csv.md`
- `PLAN.md` / `plans/stage-1-pilots.md` / `README.md` / `LEDGER.md`
- `docs/memory/facts.md` / `MEMORY.md`

### 3. 验收输出摘要

- `cargo test -p assistant-platform-windows --test cdp_provider_contract`
  → **12 passed / 0 failed**。fixture 直接断言出站 expression 为
  `const value = (document.querySelector('main').outerHTML);`，并覆盖 exception、
  缺失 value、非字符串 value 与非法 snapshot JSON。
- `cargo test -p assistant-platform-windows`
  → **136 passed / 5 ignored / 0 failed**；另有 cdp contract 12/12、
  handle-discipline 7/7、synthetic-input 4/4、doctest 1/1。
- `cargo fmt --all --check` → exit 0。
- `cargo clippy --all-targets -- -D warnings` → exit 0；仅既有
  `clippy::assert_is_empty` unknown-lint warning。
- `cargo test --workspace` → exit 0，所有测试套件 failed=0。
- `cargo test -p assistant-core arch::` → **10/10 passed**。
- `verify-schemas` / `codegen --check` → 0 error / 0 drift。
- 真实 Edge `156.0.4314.42` 本地运行 **9/10**：第 1~9 轮六步全部 `committed`，
  每轮 3 个 DOM 输出、60 行 CSV，CSV SHA-256 均为
  `4365c928631f5a07b81c08ceac4e204326e495013ae891d807bbf09602015ab3`；
  `read_page_1` 已越过原 expression/JSON 阻塞。
- 第 10 轮在 task execution 前暴露独立 `DevToolsActivePort` Windows sharing race
  （`os error 32`）；已记 `DRIFT-056-8`，不属于本卡 expression scope。

### 4. DoD 逐条核对

- [x] TCP 契约验证 expression 以代码执行，而不是作为 JSON 字符串返回。
- [x] evaluation exception、缺失/非字符串 value 与非法 snapshot JSON 均显式失败。
- [x] 真实 Edge 已执行 `read_page_1` 到 `read_page_3`，并推进到下一独立 launcher 边界。
- [x] 上述验收命令全绿；TASK-056 仍保持 Blocked，未宣称完整 10/10。

### 5. 偏差

- `DRIFT-056-7` 已解除：expression 不再是字符串字面量，真实 Edge DOM 读取可提交。
- 新发现 `DRIFT-056-8`：连续第 10 次启动时 `DevToolsActivePort` 被浏览器占用，
  launcher 的一次 `File::open` 遇到 `os error 32` 即 Fatal；本卡未扩 write scope 修改
  `browser_launcher.rs`。
- 当前 rollout 机器审计为 **96** 次 bash 误填；其中 exit 0 有 6 次、exit 2 有 10 次、
  exit 127 有 80 次。新增误用均未放宽断言或写产品文件内容；LEDGER 追加最终更正行。

### 6. 更合理做法

- expression 是 handler 在 selector JSON literal 与 max_nodes 数值校验后构造的受控
  JavaScript code，应直接求值；不能为了“防注入”把它 JSON 字符串化，因为那会改变语义。
  真正的输入边界已由 handler 与 64 KiB 上限负责，CDP 返回值仍按异常、非字符串与
  snapshot JSON 形状 fail-closed。

### 7. 遗留问题

- TASK-056 本地为 **9/10**，外部 smoke 未开始。下一步需另开 TASK-336，仅修
  `DevToolsActivePort` 的 Windows sharing/lock violation 有界重试，然后重新执行完整 10/10
  与外部 3/3。

### 8. 新增长期记忆

- FACT：CDP DOM expression 必须执行而非先 JSON 字符串化；真实 Edge 修复后
  `read_page_1`~`read_page_3` 可提交，9 轮 CSV 逐字节一致。

### 9. 给审阅者的关注点

- 重点检查 wrapper 是否只移除 expression 的字符串化，而没有放宽 expression 长度、
  WebSocket 帧、DOM 大小、exception 或 snapshot 形状校验。
- 真实 Edge 证据只证明 DOM expression 修复与 9/10 轮 CSV；第 10 轮 launcher race
  明确保留为 TASK-336，不能把本卡误报为 TASK-056 Done。
