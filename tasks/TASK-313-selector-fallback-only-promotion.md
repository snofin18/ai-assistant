# TASK-313　`fallback_only` 升格为运行时字段（ADR-0095 落地）

- 状态：**InProgress**
- 阶段：1　子阶段：**1b**　批次：Paint 真机链路 / 治理　依赖：TASK-309、TASK-312
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md`。

---

- **write scope**：
  - `docs/adr/0095-selector-fallback-only-runtime-semantics.md`
  - `docs/adr/README.md`（登记表追加一行）
  - `crates/platform/api/src/target.rs`
  - `crates/replay/src/provider.rs`（解析顺序）
  - `adapters/com.microsoft.paint/selectors/targets.json`
  - `adapters/com.microsoft.paint/tests/validate_adapter_pack.py`
  - `LEDGER.md`、`PLAN.md` 当前状态块、`plans/stage-1-pilots.md`、`README.md` 三处、`docs/PARKING_LOT.md`（PL-115 关联）
  - 本卡
- **关联**：ADR-0095（本卡即其落地）、ADR-0091、ADR-0094、PL-115、
  `docs/memory/pitfalls.md` 2026-10-17 的 `fallback_only` 条目

**目标**

把 `fallback_only` 从「只存在于适配包 JSON 与 Python 校验器的作者期标注」升格为
`SelectorCandidate` 的真实字段，并让解析器按 ADR-0095 的链内语义执行。

**现象**

`crates/platform/api/src/target.rs` 的 `SelectorCandidate` 只有
`id / kind / value / score / locale_dependent / ttl_ms` 六个字段，**没有 `fallback_only`**。
因此「此候选只作兜底」在运行时完全不生效；真降权只有 `locale_dependent`。
同时它挡住了「分层选择」的表达：同一个控件作为「打开容器的一步」应是主通道、
作为「最终目标」才是兜底，而现在无法区分。

**本卡修法**

按 ADR-0095 D1–D6 落地：加字段（缺省 false，旧 pack 兼容）→ 解析器按 primary/fallback
**两组顺序**尝试（不做分数混排）→ 只有 fallback 候选的链须显式 `allow_fallback_entry`，
否则既有 `TargetNotFound` → 组内歧义不降级 → 与 `locale_dependent` 正交 →
校验器与 paint pack 对齐。

**Out of scope（做了算漂移）**

- 新增 ErrorCode 或改 `envelope` / `tool-schema`。
- 重写 `SelectorKind` 词表（ADR-0091 / 0094 已定形状）。
- Paint handler 扩展、真机端到端（TASK-309 剩余项）。
- 采样器与 shell 规则的后续（已由 2026-10-18 的治理补强落地）。

**验收命令**

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test -p assistant-platform-api
cargo test -p assistant-replay
cargo test --workspace
python adapters/com.microsoft.paint/tests/validate_adapter_pack.py
cargo run -p xtask -- adr-index
```

**DoD**

- [ ] ADR-0095 落档 Accepted 并登记进 `docs/adr/README.md`
- [ ] `SelectorCandidate` 带 `fallback_only`，缺省 `false`，旧 pack 反序列化语义不变
- [ ] 解析器按 primary / fallback 两组顺序尝试，**不做分数混排**
- [ ] 只有 fallback 候选且未声明 `allow_fallback_entry` → `TargetNotFound`（不新增 ErrorCode）
- [ ] 组内歧义不降级（正反测试固定）
- [ ] `validate_adapter_pack.py` 与 paint pack 对齐且 PASSED
- [ ] 静态验收全绿；9 节记录填写；`LEDGER.md` 追加一行
- [ ] 无任何 Out of scope 文件被修改

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

- 任务：TASK-313（fallback_only 升格为运行时字段）。
- 铁律：1（未命中与歧义分开处理，不猜）、9（不静默扩大范围）、10（契约先行，ADR-0095 已 Accepted）。
- 禁止：新增 ErrorCode；重写 SelectorKind；Paint handler 扩展；真机端到端。
- 疑问：D3（allow_fallback_entry）与 D6（校验器 / paint pack）本段未做。本段**不改变"只有 fallback 候选的链"的既有行为**，因此无回归。

### 2. 实际改动文件

- crates/platform/api/src/target.rs：SelectorCandidate 新增 fallback_only 字段（serde default）+ 构造参数 + is_fallback_only() 访问器。
- crates/platform/windows/src/selector.rs：rank_candidates 按 ADR-0095 D2 改为**两级顺序**（先分组、组内按有效分降序）+ 两条正反单测。
- 11 处调用点补参数（**编译期强制**，不用隐式默认掩盖遗漏）。
- LEDGER.md 追加一行；本卡记录区。

### 3. 验收输出摘要

- cargo check --workspace --all-targets → **errors=0**
- cargo test -p assistant-platform-windows --lib selector → **12 passed / 0 failed**（含新增 test_rank_candidates_puts_primary_group_before_fallback_group）
- cargo clippy --all-targets -- -D warnings → **exit 0**
- cargo test --workspace → **exit 0**（153 组 test result: ok）
- cargo fmt --all --check → 首跑 exit 1（新测试格式不符），cargo fmt --all 后复跑绿

### 4. DoD 逐条核对

（待填）

### 5. 偏差

（待填）

### 6. 更合理做法

（待填）

### 7. 遗留问题

（待填）

### 8. 新增长期记忆

（待填）

### 9. 给审阅者的关注点

（待填）

### 附：2026-10-18 贯通段（PR 2/3）

- 已加 TargetDescriptor.allow_fallback_entry + SelectorChain.allow_fallback_entry（builder / 访问器）+ 装载器贯通 + paint pack 两处显式声明。
- **闸门未开**：rank_candidates 行为与上一段完全一致，本段是纯贯通，零行为变更。
- 下一段（PR 3/3）才开 D3 闸门 + 补 D4 显式测试 + validate_adapter_pack.py 规则。

### 附：2026-10-18 收口段（PR 3/3）

- D3 闸门已启用：无 primary 且未声明 → 判空 → 既有 TargetNotFound；两条正反单测固定。
- D4 在解析循环层加显式测试（不是纯函数层），确认歧义不降级。
- validate_adapter_pack.py 新增机器门禁：全 fallback 的 target 必须显式声明。
- ADR-0095 D1–D6 至此全部落地。
