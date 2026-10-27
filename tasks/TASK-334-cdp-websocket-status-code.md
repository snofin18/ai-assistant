# TASK-334　CDP WebSocket 101 状态解析不绑定 reason phrase

- 状态：**Done**
- 阶段：1　子阶段：1c　批次：基础设施修复
- 依赖：TASK-109、TASK-332、`DRIFT-056-6`
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031）。分界线以上是正文；以下由 Implementer 填写。

---

- **write scope**：`crates/platform/windows/src/cdp/websocket.rs`、`crates/platform/windows/src/cdp/websocket_tests.rs`、
  `crates/platform/windows/tests/cdp_provider_contract.rs`、`crates/platform/windows/README.md`、
  `eval/tasks/edge/t5.1/evidence/2026-10-27-task-334-websocket-reason-phrase/**`

## 目标

修复 CDP WebSocket upgrade 对 HTTP reason phrase 的精确字符串依赖，使 Edge 返回
`HTTP/1.1 101 WebSocket Protocol Handshake` 时仍能成功升级。

## 必须遵守

1. 状态行只接受受限长度的 HTTP version 与三位 status-code；101 后 reason phrase 可为空或任意文本。
2. 非 101、畸形 version/code、缺少 `Upgrade` / `Connection`、错误
   `Sec-WebSocket-Accept` 继续 fail-closed。
3. 16 KiB handshake 上限、loopback 限制与既有 ErrorCode 不变。
4. 不改公共 API、schema、依赖、浏览器启动参数或 TASK-056 断言。

## 验收命令

```powershell
cargo test -p assistant-platform-windows cdp::http::tests
cargo test -p assistant-platform-windows
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo run -p xtask -- hygiene
cargo run -p xtask -- memory-counts
cargo run -p xtask -- adr-index
cargo run -p xtask -- refscan
cargo run -p xtask -- docscan
cargo run -p xtask -- card-check
cargo run -p xtask -- check-ledger
cargo run -p xtask -- check-comments
cargo deny check
```

## DoD

- [ ] 真实 Edge 风格 `HTTP/1.1 101 WebSocket Protocol Handshake` 的本地 TCP 契约通过。
- [ ] 非 101、错误 accept、缺失升级 header 与超限 handshake 继续 fail-closed。
- [ ] 真实 Edge 对本地 fixture 至少执行一次 navigate，确认 transport 不再拒绝合法 101。
- [ ] 上述验收命令全绿，真实 Edge 10/10 与外部 3/3 由 TASK-056 继续记录。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

```text
【任务】TASK-334 CDP WebSocket 101 状态解析不绑定 reason phrase
【目标】接受 Edge 合法 101 reason phrase，修复 DRIFT-056-6
【write scope】仅：crates/platform/windows/src/cdp/websocket.rs、tests/cdp_provider_contract.rs、platform/windows/README.md
【铁律】1 无静默失败 / 2 网络输入先校验 / 4 升级有 header 与 digest postcondition / 9 不扩范围 / 12 header 有界
【禁止】不改公共 API/schema/ErrorCode/依赖；不放宽 accept/header/超限校验
【验收】真实 TCP reason phrase 回归 + platform/windows/workspace/xtask/deny → 全绿
【依赖】TASK-109 / TASK-332 Done，DRIFT-056-6 已核对
【疑问】无
```

### 2. 实际改动文件

- `crates/platform/windows/src/cdp/websocket.rs`
- `crates/platform/windows/src/cdp/websocket_tests.rs`
- `crates/platform/windows/tests/cdp_provider_contract.rs`
- `crates/platform/windows/README.md`
- `eval/tasks/edge/t5.1/evidence/2026-10-27-task-334-websocket-reason-phrase/**`
- `tasks/TASK-334-cdp-websocket-status-code.md`
- `PLAN.md` / `plans/stage-1-pilots.md` / `README.md` / `LEDGER.md`
- `docs/memory/facts.md` / `MEMORY.md`

### 3. 验收输出摘要

- `cargo test -p assistant-platform-windows cdp::websocket::tests` → **4/4 passed**。
- `cargo test -p assistant-platform-windows --test cdp_provider_contract` → **8/8 passed**：
  包含 Edge 风格 101 reason phrase 成功与 200 状态拒绝。
- `cargo test -p assistant-platform-windows` → **136 passed / 5 ignored / 0 failed**；
  `cdp_provider_contract` 8/8、handle-discipline 7/7、synthetic-input 4/4。
- 真实 Edge `156.0.4314.42`：WebSocket 101 upgrade 成功，`navigate_page_1` 为
  `Committed`，post fingerprint
  `sha256:64af0b490d30828a0507c8c581637f5dc33ceaf304fe77e6ffacf06af3ef8dbf`。
- 同一真实运行随后在 `read_page_1 / edge.cdp.dom_read` 报
  `CDP DOM payload is not the required JSON object`；这是 transport 之外的下一个独立问题，
  本卡未越权修改，失败与清理证据已留盘。
- `cargo test --workspace`、`cargo fmt --all --check`、
  `cargo clippy --all-targets -- -D warnings` → exit 0。
- `verify-schemas` / `codegen --check` / `hygiene` / `memory-counts` /
  `adr-index` / `refscan` / `docscan` / `card-check` / `check-ledger` /
  `check-comments` / `check-migrations` → PASSED。
- `cargo deny check` → advisories / bans / licenses / sources ok。

### 4. DoD 逐条核对

- [x] 真实 Edge 风格 `HTTP/1.1 101 WebSocket Protocol Handshake` 的本地 TCP 契约通过。
- [x] 非 101、错误 accept、缺失升级 header 与超限 handshake 继续 fail-closed。
- [x] 真实 Edge 对本地 fixture 执行 navigate，`navigate_page_1` 提交成功，transport 不再拒绝合法 101。
- [x] 上述验收命令全绿；真实 Edge 10/10 与外部 3/3 仍由 TASK-056 记录。

### 5. 偏差

- 无新 DRIFT。`DRIFT-056-6` 已解除。
- 真实 Edge 在 transport 修复后暴露下一层 `edge.cdp.dom_read` JSON 解析问题；本卡停止在
  WebSocket scope，不把它混入 transport 修复。
- 本会话新增 8 次 bash 误填（只读状态/文件检索、一次 guard release、一次成功 staging 与一次失败 release），
  均未执行验收目标或写产品文件内容；rollout 累计 68，
  LEDGER 追加更正。

### 6. 更合理做法

- 状态行只验证 HTTP version、三位 status-code 与 1 KiB 行上限；101 后 reason phrase
  不作为协议断言。Upgrade、Connection 与 accept digest 仍逐项强校验。

### 7. 遗留问题

- TASK-056 的真实 Edge 下一步阻塞：`edge.cdp.dom_read` 将 CDP evaluate 返回值误当作
  JSON object。需在 TASK-056 后续记录/另开最小卡，不能把本卡 navigate 成功冒充完整 10/10。

### 8. 新增长期记忆

- FACT：CDP WebSocket 101 的 reason phrase 不能作为固定契约；Edge 156 使用
  `WebSocket Protocol Handshake`，按 status-code 101 解析后可成功 navigate。

### 9. 给审阅者的关注点

- 重点审查 `validate_upgrade_status` 是否只放宽 reason phrase，而没有放宽 HTTP version、
  status-code、Upgrade/Connection/accept digest 或 16 KiB handshake 上限。
- 本卡真实 Edge 证据只证明 WebSocket upgrade + first navigate；不得据此宣称 TASK-056 完成。
