# ADR-0035　workspace `[lints.clippy]` 维持严格 deny 政策（B 路结论）

状态：**Accepted**（2026-09-19）
日期：2026-09-19　Supersedes：—　Superseded by：—

## 背景

TASK-051 / TASK-052 / TASK-053 / TASK-054 四轮清理后（commit `4ac7d19..781ab7b`），
原悬空 commit `10f78db` 留下的 `xtask` 护栏升级工作已落地。人类在 TASK-051
报告后裁决**B 路** = 纯重构 = **禁止动 workspace `[lints.clippy]` 加 `exceptions = [...]`**
（= 漂移触发器 ⑥ 不可忍受）。

本 ADR 把这条隐含 policy 显式化，并清点当前状态作为 baseline。

## 决策

**1. 不向 `[workspace.lints.clippy]` 增加任何 `exceptions = [...]` 字段。**

**2. per-line `#[allow(...)]` 仍允许使用**（ADR-0021 + ADR-0030 D3 的明示机制），
   但每个允许项必须：
   - 在该处 `// 注释：原因` 解释（per AGENTS.md §5.2 注释密度 + ADR-0030 D3 的可审计性）
   - 在 `tasks/<card>.md` §5 偏差节登记（避免下次会话重复辩论）
   - 仅限**已发现的具体模式**，不预设白名单

## 当前 per-line allow 清点（2026-09-19 baseline）

| 类别 | 数量 | 性质 | 处置 |
|---|---|---|---|
| `#[cfg(test)] #[allow(clippy::unwrap_used, ...)] mod tests {` | 4 模块各 1 处 | per-mod 测试 wrapper | **保留**（per-mod ≠ workspace 级；ADR-0021 允许）|
| `#[allow(dead_code)]` (card_check.rs line 24/26/28/30/33/35/37/129/270 + exemptions.rs line 45) | 9 + 1 | ADR-0031 D6 占位常量 + 预留函数 | **保留**（pre-existing，ADR-0031 显式要求）|
| `#[allow(clippy::single_char_pattern)]` (refscan.rs:94) | 1 | `replace("\r", "\n")` 的 API workaround | **TASK-055 清掉**（改 API 调用）|
| `#[allow(clippy::case_sensitive_file_extension_comparisons)]` (refscan.rs:101/103) | 2 | `lower.ends_with(".md")` 在 `to_ascii_lowercase()` 后 | **TASK-055 清掉**（改用 `Path::extension`）|
| `#[allow(clippy::unwrap_used, clippy::expect_used)]` (refscan.rs:render) | 0 | TASK-054 已改 render 返回 `Result<String, std::fmt::Error>` | — |

**清点后生产代码 per-line allow 目标 = 0**，仅保留测试 wrapper（合法）+ pre-existing dead_code（合法占位）。

## 不接受的反模式

1. **不**为「workspace deny 但代码模式永久正确」的 lint 增加 `exceptions = ["..."]`。
   反例：TASK-054 处理 writeln! to String 永不失败的方案 = 改 render 签名返回 `Result`，
   而不是「加 `unwrap_used` 豁免」。

2. **不**为「per-line allow 太啰嗦」累加 `exceptions = [...]`。
   反例：每处 per-line allow 配 `// 注释：原因`（AGENTS.md §5.2）+ 任务卡 §5 偏差登记 = 可审计；
   workspace-level exceptions 无法区分 production vs `#[cfg(test)]` 块，会破坏铁律 ①「禁止 silent skip」。

3. **不**为「不同 crate 不同容忍度」把 workspace-level 拆成 per-crate overrides。
   这违反 AGENTS.md §3「单 workspace 配置」原则；如有需求，开新 ADR。

## 替代路径

未来出现新的「workspace deny 但代码模式正确」的 lint 模式时，标准路径：

1. **首选**：改代码 / 改 API 签名（如 TASK-054 `render -> Result`）— 编译期保证 + 零 per-line allow
2. **次选**：per-line `#[allow(...)]` + 注释 + 任务卡 §5 登记（ADR-0021 机制）— 占用 1-3 行，零漂移
3. **最后**：开新 ADR 推翻本 ADR，加 `exceptions = [...]`（漂移触发器 ⑥ 兜底）— 仅在模式频繁 + 单调时考虑

## 改 workspace lints 的强制流程（自本 ADR 起）

任何动 `[workspace.lints.clippy]` 的提交必须：
1. 先开 DRIFT-N ticket（per AGENTS.md §4）
2. 引用本 ADR 编号 0035（标记推翻旧 policy）
3. 在 LEDGER.md 追加 `[supersedes:2026-09-19 ADR-0035]` 行
4. 人类裁决后才可合并

## 相关 ADR

- ADR-0021（per-line `#[allow]` 的允许与规范）
- ADR-0030（机器校验硬门禁 #12b）
- ADR-0031（任务卡一卡一文件，含 D3/D6）
- TASK-052 §5 偏差 #1（人类 B 路裁决来源）
- TASK-054 §5（本卡彻底化 writeln! to String 问题为 Result 签名）

## 相关 PL

- PL-NEW（已在 TASK-052 关闭，登记本 ADR 为其结论）
