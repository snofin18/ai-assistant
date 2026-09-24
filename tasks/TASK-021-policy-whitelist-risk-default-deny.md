# TASK-021　`policy`：白名单 + 风险分级 + 参数校验（路径穿越/URL/长度/正则复杂度）+ 规则 DSL v0 + **默认拒绝**

- 状态：**Ready**
- 阶段：1　子阶段：**1a**　批次：**A2**　依赖：011　预估：M　难度：M
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息（阶段 In/Out scope、阶段 DoD、批次表与并行建议）见 `plans/stage-1-pilots.md`。

---

- **依赖**：011（`crates/protocol` 的 `ErrorCode` / `RiskLevel` / `PolicyDecision` 已生成；本卡**复用**）
- **预估**：M　**难度**：M
- **write scope**：`crates/policy/**`（新 crate `assistant-policy`）、`docs/DEPENDENCIES.md`（**仅追加**本卡实际引入的依赖行）
- **关联**：`plans/stage-1-pilots.md` 批次表 A2（1a）；架构 v2 **§12.2（工具白名单与策略引擎，含五条示例规则）** / §12.3（参数校验清单）/ §12.4（反注入四层机制）/ §12.5（密钥红线）/ §8.7（错误分类）；`docs/spec/error-codes.md`；`docs/spec/tool-schema.md` §4 不变量 4（`risk_class` 单调）；**铁律 1 / 2 / 3 / 5 / 6**

**目标**

新建 `crates/policy`，把架构 v2 §12.2 的「策略引擎是唯一放行点」落成**纯函数**：

- **默认拒绝**：规则集没有显式放行 → `Deny`。
- **规则 DSL v0**：架构 v2 §12.2 的**五条示例规则**必须**全部可表达**（这是本卡的硬验收点）。
- **风险分级**：`RiskLevel`（Low / Medium / High / Critical，复用 `crates/protocol`）+ 四级可逆性模型（架构 v2 §9.1）。
- **参数校验**：路径穿越（`..` / 符号链接逃逸 / UNC / 设备路径）、URL（协议白名单 / 禁 `file://` / 禁内网 IP 直连）、长度、正则复杂度（防 ReDoS）。
- **每个 deny 带 `rule_id` 与可读 reason** → 直接能填 `crates/protocol::PolicyDecision`。
- **判定 < 50 µs**（批次表验收要点）。

**In scope（本卡交付物）**

| # | 交付物 | 依据 |
|---|---|---|
| 1 | `crates/policy/Cargo.toml`（`assistant-policy`）+ `src/lib.rs` + `README.md`（职责 / 边界 / **不变量** / 已知限制） | gov §5.4 + 阶段 1 DoD |
| 2 | **决策类型**：`Allow` / `AllowWithConfirmation{ scope_options, show_diff }` / `Deny{ rule_id, reason }`；可**无损**映射到 `crates/protocol::PolicyDecision` | 架构 v2 §12.2 + `docs/spec/audit-event.md` |
| 3 | **求值上下文**：调用所需字段（`effect` / `risk_level` / `reversibility` / `unattended` / `tainted` / `target_app` / `egress` / …），**只放结构体、不做 IO** | 架构 v2 §12.2 五条规则用到的字段全集 |
| 4 | **规则集模型 + 五条示例规则**：`allow_read_low_risk` / `confirm_medium_write` / `block_irreversible_unattended` / `block_after_untrusted_content` / `deny_sensitive_egress`；**default = deny** | 架构 v2 §12.2（逐条） |
| 5 | **纯函数求值**：`evaluate(rule_set, context) -> Decision`；**无 IO / 无时钟 / 无随机 / 无全局可变状态** | 批次表验收要点「决策是纯函数」 |
| 6 | **参数校验器（纯函数集）**：路径规范化 + 防穿越；URL 协议与目标校验；文本长度 / 行数；数值边界；**正则复杂度**静态判据（见 Q2） | 架构 v2 §12.3（逐条） |
| 7 | **性能测试**：单次判定 < 50 µs（用内置基准或 `#[test]` 里的计时断言，须**稳定**，见 Q4） | 批次表验收要点 |
| 8 | `docs/DEPENDENCIES.md` 追加本卡引入的第三方依赖行（**默认零新增**，见 Q1 / Q2） | 登记表「登记规则」1 |
| 9 | 单测（正向 + **负向**，ADR-0019 N1）：五条规则各 ≥1 条命中/不命中；默认拒绝；参数校验的**拒绝**用例 ≥8 条 | gov §5.5 |

**Out of scope（做了算漂移）**

- `crates/tool-bus/**`（MCP / 信封 / 挂载）→ **TASK-020**（本卡**不做**工具注册，只做判定）
- `crates/hitl/**`（审批请求 / 授权范围 / 接管）→ TASK-027（本卡只**产出**「需要确认」的决策，不实现审批流程）
- `crates/lease/**`（目标租约）→ TASK-025
- 污点状态的**存储与传播**（架构 v2 §12.4 第 2 层）→ TASK-051（本卡只**读取** `tainted: bool`）
- DLP 出域脱敏链 → TASK-050（本卡只按 `egress` 字段判定）
- 规则集的**持久化 / 热加载 / UI 编辑** → 阶段 2 起（本卡用内存规则集 + 测试 fixture）
- 改 `crates/protocol/**`（schema 与生成物）；确需改 → DRIFT（触发器 ③）
- 改 `crates/platform/**`、`crates/core/**`
- 任何 `#[allow]` 放宽（触发器 ⑥）

**必须遵守**

1. **铁律 3（唯一的放行点）**：`policy` 是**唯一**产出放行结论的地方；`tool-bus` / Host / UI **不得**自行判断。反向也成立：`policy` **不得**执行动作、不得访问文件或网络。
2. **默认拒绝**：规则集为空 / 无规则命中 / 规则集加载失败 → `Deny`（**绝不**默认放行）。
3. **铁律 1（无静默失败）**：每个 `Deny` 必须带 `rule_id` + 可读 `reason`；判定过程的错误（如规则集自相矛盾）也必须返回错误而**不是**「当作放行」。**禁止** `let _ =` / `unwrap()` / `expect()` / `panic!()`（workspace lint 已 deny）。
4. **铁律 6（不可逆动作 + 无人值守）**：`reversibility = L3` **且** `unattended = true` → **必须** `Deny`（架构 v2 §12.2 第 3 条规则）。本卡**不得**实现「无声放行」的变体。
5. **纯函数优先**：`evaluate` 与全部校验器必须是纯函数（可单测、可重放）；**禁止**在任何判定路径里读系统时间 / 环境变量 / 文件系统。
6. **复用 `crates/protocol` 生成物**：`RiskLevel` / `ErrorCode` / `PolicyDecision` **一律复用**；`Reversibility` 若生成物里没有 → 在 `crates/policy` 内定义**唯一一份**并在 README 标注「与架构 v2 §9.1 四级模型对齐，待 TASK-024 收敛」；**禁止**同名类型出现两份。
7. **受控词**（ADR-0021 + `docs/spec/naming.md`）：用 `Target` / `Step` / `Tool` / `Lease` / `Anchor` / `Fingerprint`，禁缩写，禁 `data` / `info` 这类无信息名。
8. **公共热点文件写前取锁**（ADR-0028）：`LEDGER.md` / `PLAN.md` / `README.md` / `docs/memory/*` / `docs/PARKING_LOT.md` / `plans/*` 写前 `guard acquire`，写完**立刻** release；超时（exit 5）必须在 LEDGER 追一行，**不得** `--force`。
9. **规模**：单文件 ≤ 400 行（软）/ 600（硬）；函数 ≤ 80 行；参数 ≤ 6 个。
10. **不得**为让门禁变绿而改测试断言或放宽 lint（触发器 ⑥ / ⑦）。

**待裁决（命中即记 `DRIFT-021-x`；夜间运行时按人类 2026-09-25 授权**自决**，裁决必须落盘 §5 + LEDGER）**

- **Q1（规则集序列化格式）**：架构 v2 §12.2 的示例是 **TOML**，但解析 TOML 需要新依赖（`toml`）。**默认取向 = DSL v0 用 `serde_json`（已登记且已批准）表达同一结构**，TOML 加载推迟并记 `docs/PARKING_LOT.md`（理由：本卡验收点是「五条规则**可表达**」，不是「TOML 可加载」；先零新增依赖）。若判断必须 TOML → 登记 `toml` + 记 DRIFT。
- **Q2（正则复杂度判据）**：**默认取向 = 不引入 regex 引擎**，用**静态启发式**（模式长度上限、禁 `.*` 连续出现、禁嵌套量词如 `(a+)+`、禁回引、禁 lookaround 嵌套）并**对不支持的语法一律拒绝**；判据必须写进 README「已知限制」，且**不得**出现「不认识就放行」。
- **Q3（`Reversibility` / `Effect` 的归属）**：先查 `crates/protocol` 生成物（`protocol/**/*.json`）里是否已有对应枚举。已有 → **复用**；没有 → 在 `crates/policy` 内定义并在 README 标注「待收敛」。**禁止**擅自把新类型加进 `protocol/`（超 write scope + 改契约）。
- **Q4（< 50 µs 的测法）**：单次计时断言在 CI 机器上容易 flaky（触发「改断言才能过」的诱惑 = 触发器 ⑦）。**默认取向 = 断言「单次判定 < 50 µs」用多轮取中位数 + 宽松上限（如 10× 余量）**，并在 §3 记录实测中位数；若 CI 仍 flaky → 记 DRIFT + 在 §6 说明改法，**不得**直接删断言。
- **Q5（`unattended` 的取值来源）**：本卡只**读取**上下文里的布尔字段；无人值守模式的判定归属 core / hitl。若 §3 发现没有上游字段可用 → 用测试 fixture 提供，并在 §7 记「需在 TASK-027 / TASK-028 接线」。

**验收命令**

```powershell
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo test -p assistant-policy
cargo run -p xtask -- hygiene / memory-counts / adr-index / docscan / card-check / check-ledger
```

**完成定义（DoD）**

- [ ] 上列命令全部通过（`refscan` 只需**不新增** Error —— 既有 151 项 baseline 见 PL-058）
- [ ] **架构 v2 §12.2 五条示例规则全部可表达**，且每条各 ≥1 条命中用例 + ≥1 条不命中用例
- [ ] **默认拒绝**：空规则集 / 无命中规则 → `Deny`
- [ ] **决策是纯函数**：无 IO / 无时钟 / 无随机（用测试证明同一输入两次调用结果相同）
- [ ] **每个 deny 带 `rule_id` 与可读 reason**（断言字段非空）
- [ ] **策略判定 < 50 µs**：§3 记录实测中位数
- [ ] 参数校验：路径穿越 / URL / 长度 / 数值边界 / 正则复杂度各有**拒绝**用例（≥8 条负向断言）
- [ ] `crates/policy/README.md` 含职责 / 边界 / 不变量 / 已知限制四节
- [ ] `docs/DEPENDENCIES.md` 已登记本卡引入的**全部**第三方依赖（零新增则显式写明「本卡零新增」）
- [ ] `LEDGER.md` 追加一行；如新增事实/坑 → `docs/memory/{facts,pitfalls}.md`
- [ ] 无任何 Out of scope 文件被修改

**夜间自动化补充约束（仅当本卡由夜间 automation 执行时适用；依据 = 人类 chat 2026-09-25 授权）**

1. 每次运行**只做一张卡**（AGENTS.md §3）。
2. 分支 = `task/TASK-021-policy-whitelist-risk-default-deny`，从**最新 `origin/main`** 切出；**PR base 必须是 `main`**（PL-075）。
3. 需要裁决的项**自决**，但必须**落盘**（卡 §5 全文 + `LEDGER.md` 一行）。
4. 合并自己的 PR 需**同时**满足：CI 全部 success **且** `mergeable_state == clean` **且** base = `main`；否则只开 PR、**不合并**。
5. 结束后按 ADR-0039 / ADR-0041 同步 `PLAN.md` 当前状态块（4 行）、`README.md` 三处、`plans/stage-1-pilots.md` 头部进度句与本卡完成标记、`LEDGER.md` 一行。
6. **本卡是安全底座卡** → 审查关注点必须包含「有没有任何一条路径默认放行」（铁律 3 + 架构 v2 §12.2「策略引擎是唯一放行点」）。
<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->


## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）

### 1. 约束回执

### 2. 实际改动文件

### 3. 验收输出摘要

### 4. DoD 逐条核对

### 5. 偏差

### 6. 更合理做法

### 7. 遗留问题

### 8. 新增长期记忆

### 9. 给审阅者的关注点
