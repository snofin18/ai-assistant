# TASK-015　xtask 护栏增强：内化 refscan/docscan + 豁免清单读取 + card-check 子命令

- 状态：**Ready**
- 阶段：1　子阶段：**1a**　批次：**A1（地基层，必须串行）**　依赖：TASK-001　预估：M（≤1 会话）
- 本文件 = **卡片正文 ＋ 执行记录**（ADR-0031「一卡一文件」）。分界线**以上**是正文（Orchestrator 所有，Implementer **只读**）；**以下**是执行记录（Implementer 填写）。
- 阶段级信息见 `plans/stage-1-pilots.md`。
- ★ **关键路径节点** —— 后续所有卡的护栏都依赖本卡落地的机器校验。本卡完成后 `cargo run -p xtask --` 必须能用 hygiene / memory-counts / adr-index / refscan / docscan / card-check 六个子命令。

---

- 依赖：TASK-001　预估：M（≤1 会话）　难度：M
- **write scope**：`xtask/src/**`、`xtask/Cargo.toml`、`xtask/README.md`、`docs/spec/tool-schema.md`（若需要落 Tool/Adapter/审计事件的最小 schema）、`docs/PARKING_LOT.md`（追加关闭行）、`LEDGER.md`、`docs/memory/{facts,pitfalls,decisions}.md`、`MEMORY.md` §1（快照 + 规模表）、`docs/adr/0034-*.md`（新增 ADR-0034）
- **关联**：`docs/adr/0032-doc-rule-exemption-mechanism.md`（豁免清单机制，本卡的机器读入依据）、`docs/adr/0030-machine-verified-memory-counts-and-adr-index.md`（已有 memory-counts / adr-index 的样板）、`docs/adr/0028-file-rewrite-mutex-protocol.md`（CLI 风格 + guard 用法）、`docs/adr/0025-hygiene-rule-count-unification.md`（hygiene 总数 13 的口径来源）、`docs/governance-ai-agent-execution.md` §5.1（CI 门禁清单）

**Out of scope**：`crates/**`、`apps/**`、任何产品代码；`docs/spec/*` 完整 schema（仅最小够用）；修改 ADR-0030 / 0032 / 0033 已决内容

**步骤**

1. **内化 `refscan.py` 为 `xtask refscan` 子命令**：原脚本在 `%TEMP%\refscan.py`，扫描全仓 `.md` + `.rs` 的：
   - `adr/number-range-notation`：两个 4 位 ADR 号之间夹 `~` / `～` / `-` → 命中后**读 ADR-0032 豁免清单跳过**
   - `adr/bare-pending-reference`：ADR-00NN 出现在 `docs/adr/` 之外且 NN 属待建号集合 → 命中后**读豁免清单跳过**
   - 新增 `file/pure-ascii-ps1`：扫描 `*.ps1` 文件字节，凡 > 127 的字节即违规（**读豁免清单跳过**）
   - 输出格式与原 `refscan.py` 保持一致（`scanned=N RANGE ... BARE-PENDING ...`），便于人类审计
   - 退出码：发现未豁免违规 → 1；否则 0

2. **内化 `docscan.py` 为 `xtask docscan` 子命令**：
   - 破表扫描（`exp=N got=M`）
   - setext 风险扫描（`---` 前一行非空且不是另一根 `---`）
   - 文件编码扫描：UTF-8 BOM / CRLF / 末尾换行形态（缺 `LF` / 双 `LF`）
   - **hygiene 第 13 项（末行换行）由 Warning 升 Error**（ADR-0025 D4 的第二阶段；现在仓库 0 违规 = 升安全）
   - 输出格式与原 `docscan.py` 保持一致（`files=N md=M broken=X settext=Y encoding=Z`）
   - 退出码：发现 Error 级违规 → 1；否则 0

3. **实现 `xtask card-check` 子命令**（当前仅 `%TEMP%\cardchk.py` 原型）：
   - canonical 分界线**从 gov §3.4 现场读取**（不硬编码 = 避免 PL-031 类漂移）
   - 状态行唯一性
   - 分界线唯一性（每文件恰好 1 条，顶格）
   - 记录区 9 节标题齐全（**Warning**，Ready 豁免；**历史上 9 节之前的格式**用对照表豁免）
   - `plans/*.md` 不得出现 `# TASK-0NN` / `## TASK-0NN` / `### TASK-0NN` 形态（**Error**，防形态回退）
   - 状态非 `Ready` 的卡必须有对应 `tasks/TASK-NNN-*.md` 文件（**Error**，索引表脱节）
   - 输出格式与原型一致
   - 退出码：发现 Error 级违规 → 1；否则 0

4. **CI 集成**：把这三个新子命令加入 `.github/workflows/ci.yml` 的 `doc-consistency` job（ADR-0030 #12b）。命令形如 `cargo run -p xtask -- refscan / docscan / card-check`，三个全 0 exit = job 绿。

5. **更新 `xtask/src/deferred.rs`**：把 `card-check` 从「[未实现 · 待补卡]」移到「[已实现 · TASK-015]」（其他子命令状态保持）

6. **更新 `xtask/README.md`**：加 refscan / docscan / card-check 三个子命令的用法、退出码、ADR-0032 豁免读取逻辑说明

7. **新增 ADR-0034**：记录「`xtask card-check` 子命令机器化 ADR-0031 D6 四条判据」的决策，与 ADR-0031 的影响表对齐

8. **写 ADR-0030 修订（不在本卡范围）**：暂记 `PARKING_LOT.md` 留到阶段末统一

**go 判据（全部满足）**

- `cargo test --workspace` 全绿（含新子命令的单元测试 ≥ 6 个）
- `cargo run -p xtask -- refscan / docscan / card-check / hygiene / memory-counts / adr-index` 全部 `verdict=PASSED, errors=0`
- `cargo run -p xtask -- card-check --repo .` 输出含 12 个 stage-0 卡 + 2 个已迁 stage-1 卡 + 0 个 plans 卡片正文 = 0 错
- ADR-0032 豁免清单 23 条在 refscan 输出中正确跳过
- 故意制造一个未豁免违规 → refscan 报 1 个违规、退出码 1（**负向实证**）

**no-go 后果**：若 xtask 零第三方依赖约束与机器读入 ADR-0032 冲突 → 在 `PARKING_LOT.md` 新增 PL-036，重审 ADR-0032 豁免清单的格式（机器可解析性）。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写；9 节骨架见 gov §3.4 / ADR-0031 D4）
