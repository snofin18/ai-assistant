# 文档护栏规则豁免清单（机器可读）

状态：**Accepted**（2026-09-18，ADR-0032 D3 配套生效）　Supersedes：—　Superseded by：—


> 与 **ADR-0032** 配套。机器规则（`xtask card-check` / `xtask adr-index` 的扩展）
> 跑时必须读这张表：登记的违规 = 跳过（豁免），未登记的违规 = 报错。
>
> **写入规则**（ADR-0032 D3）：
> ① 只**追加行**，不改不删旧行（与 `docs/memory/*` 的「只追加」规矩一致）；
> ② 每次添加必须在 `docs/memory/decisions.md` 留 `[DECISION][src:ADR-0032]` 理由；
> ③ 「移除」用行尾 `[supersedes:YYYY-MM-DD]` 标记，**不删原行**（ADR-0032 D4）。
>
> **机器解析**：
> - 表头第 1 列 = 人类读；第 2 列 = 规则短名（与 `xtask` 规则标识符一一对应）；
>   第 3 列 = `path:line`，路径相对仓库根、1-based 行号；
>   第 4 列 = 人类审计用；第 5 列 = 移除触发，"**永不**" = 只追加文件 = 永远留着。
> - 「同一行有多个违规」共用同一 ID（如 `LEDGER.md:36` 同时有 0018~0025 与 0021~0026）。

## 基线豁免清单（2026-09-18 由 ADR-0032 一次性登记）

> **来源**：2026-09-18 refscan.py 全仓扫描命中数；登记数与命中数之差为 0 即 PASSED。
> **当前 refscan 命中**：10 处裸 ADR 待建引用 + 10 处 ADR 编号范围写法 = **20 处**。
> 本表登记 **20 条**（E-001 至 E-020）。

### 规则 `adr/bare-pending-reference`（PL-032）

| ID | 规则 | 位置 | 理由 | 移除触发 |
|---|---|---|---|---|
| E-001 | adr/bare-pending-reference | `docs/PARKING_LOT.md:37` | 解析 PL-028 的违规范围时引用 ADR-0017 / ADR-0016 | PL-028 关闭行被 supersede |
| E-002 | adr/bare-pending-reference | `docs/PARKING_LOT.md:41` | 同上行的下一处 | 同上 |
| E-003 | adr/bare-pending-reference | `docs/governance-ai-agent-execution.md:171` | ADR 草稿模板的标题示例（ADR-0007 占位） | 模板移除 |
| E-004 | adr/bare-pending-reference | `docs/governance-ai-agent-execution.md:487` | 变更历史表「依据」列的 ADR-0011 引用 | 表项被 supersede |
| E-005 | adr/bare-pending-reference | `docs/governance-ai-agent-execution.md:790` | ADR 草稿模板示例（同 E-003） | 同 E-003 |
| E-006 | adr/bare-pending-reference | `docs/governance-ai-agent-execution.md:805` | commit message 示例（ADR-0011） | 同 E-003 |
| E-007 | adr/bare-pending-reference | `xtask/src/adr_registry_tests.rs:216` | 测**「裸引用 = 报错」**的负向用例，故意写 `ADR-0016` 触发 | **永不**（测试夹具就是这条违规） |
| E-008 | adr/bare-pending-reference | `xtask/src/adr_registry_tests.rs:217` | 同上（同一测的第二行） | **永不** |
| E-009 | adr/bare-pending-reference | `xtask/src/hygiene.rs:368` | 测**「裸引用 = 报错」**的另一个负向用例（ADR-0007） | **永不** |
| E-010 | adr/bare-pending-reference | `xtask/src/hygiene.rs:549` | 测 ADR 编号负向用例（ADR-0012） | **永不** |

### 规则 `adr/number-range-notation`（PL-034）

| ID | 规则 | 位置 | 理由 | 移除触发 |
|---|---|---|---|---|
| E-011 | adr/number-range-notation | `LEDGER.md:33` | 2026-09-17 的人类会话行只追加（0042：ADR-0021~0025） | **永不**（只追加台账） |
| E-012 | adr/number-range-notation | `LEDGER.md:35` | 2026-09-17 同上（ADR-0021~0026） | **永不** |
| E-013 | adr/number-range-notation | `LEDGER.md:36` | 同会话同行的两个范围（0018~0025 + 0021~0026） | **永不** |
| E-014 | adr/number-range-notation | `docs/PARKING_LOT.md:64` | 章程 §13 W4 的 write scope 描述（0016~0020） | **永不** |
| E-015 | adr/number-range-notation | `docs/adr/0026-adr-number-registry-and-0019-collision.md:19` | ADR-0026 解释 0001~0020 的「待建号清单改 0027」上下文 | ADR-0026 被 supersede |
| E-016 | adr/number-range-notation | `docs/adr/0026-adr-number-registry-and-0019-collision.md:112` | ADR-0026 解释 0021~0026 的迁移历史 | 同 E-015 |
| E-017 | adr/number-range-notation | `docs/memory/decisions.md:33` | 解释 ADR-0026 D2 的 `[ADR:待建 0016~0020]` 上下文 | 该条目被 supersede |
| E-018 | adr/number-range-notation | `docs/overnight-automation-charter.md:494` | 同上（章程里同步解释） | 同 E-017 |
| E-019 | adr/number-range-notation | `docs/wbs-overview.md:147` | 最小就绪清单里 ADR 范围 0001~0015 | 清单项被勾选后下次重构时移除 |
| E-020 | adr/number-range-notation | `plans/stage-0-spikes.md:30` | 阶段 DoD 列表里的 ADR 范围 0001~0015 | 列表重构时移除 |

## 字段格式（机器约束）

每行必须恰好 5 列（不算首尾管道符），顺序：

1. `ID` — `E-NNN`（三位数字，前缀大写 `E-`），同一表内不可重号
2. `规则` — 形如 `<scope>/<short-name>`，与 `xtask` 规则标识符一一对应
3. `位置` — `<relative-path>:<line>`，路径相对仓库根、1-based 行号
4. `理由` — 一句话，可含 `**粗体**` 与行内代码；不解析
5. `移除触发` — 一句话；「**永不**」用 `**永不**（<原因>）` 形态以供 xtask 标红

分隔行（5 个 `|---|`，列数 = 5）必须与表头列数一致。

### 规则 `file/pure-ascii-ps1`（ADR-0024 D4：仓库内 `.ps1` 一律纯 ASCII；PL-026 落地后剩余豁免）

| ID | 规则 | 位置 | 理由 | 移除触发 |
|---|---|---|---|---|
| E-021 | file/pure-ascii-ps1 | `spikes/spike-a-notepad/probe-01-tree-survey.ps1:30` | **故意的非 ASCII 测试数据**——本 probe 就是要证明 UIA 能正确读写含中文文本，所以**测试数据本身**必须含非 ASCII。翻译它 = 改掉被测对象。 | **永不**（测试数据的语义就是非 ASCII） |
| E-022 | file/pure-ascii-ps1 | `spikes/spike-a-notepad/probe-02-text-and-timing.ps1:35` | 同上（另一处故意非 ASCII 测试数据） | **永不** |
| E-023 | file/pure-ascii-ps1 | `spikes/spike-a-notepad/probe-02-text-and-timing.ps1:81` | **故意非 ASCII 写入数据**——`$zh = "..."` 是 SetValue 写入测试，必须含中文/符号/多字节数字才能验证 UIA 的写入读回链路。 | **永不** |
