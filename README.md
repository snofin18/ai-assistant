# 跨平台本地 AI 助理（Cross-Platform Local AI Assistant）

让大模型**受控地**操作你电脑上已有的常用应用（记事本、画图、Edge/Chrome、Excel、
Photoshop…）：模型负责理解与规划，所有动作都通过**注册的工具**执行，
全过程可审计、可撤销、可回放。**默认拒绝**，不可逆动作必须人工确认。

> 状态：**阶段 1（三试点闭环：Notepad → Paint → Edge/Chrome）** —— 阶段 0（文档与 Spike）已于 2026-09-20 closeout；
> 产品代码自阶段 1 起才落地（`crates/protocol` / `crates/storage` / `crates/audit` / `crates/core` 骨架 / `crates/secrets` /
> `xtask` 护栏 / **`crates/platform/api`**（平台抽象层：4 个纯类型 + 3 个 trait 形状 + 能力矩阵）已完成，
> 下一张是 `crates/platform/windows`（Win32 / UIA 实现））。**当前阶段详情以 `PLAN.md` 为准**。

---

## 这个项目最特别的一点

它主要由 **AI coding agent**（Codex / opencode / Claude Code）实现，人类负责规划、审阅与裁决。
因此仓库里有一半的"代码"其实是**约束 agent 的机制**：
任务卡、write scope、约束回执、漂移触发器、机器护栏、长期记忆文件。
如果你只想看架构，直接读架构文档；如果你想让 agent 在这个仓库里干活，**必须先读 `AGENTS.md`**。

---

## 产品层 vs 工程元层（读任何文档前先分清这一条）

本项目由 AI coding agent 实现、人类审阅，因此仓库里有大量**不属于产品**的东西。
判定标准只有一句话：**「删掉它，产品的行为会变吗？」不会 → 工程元层。**

| 层 | 包含 | 会被构建进发布物吗 | 受什么约束 |
|---|---|---|---|
| **产品层** | `crates/`、`protocol/`、`adapters/`、`adapters-private/`、`apps/`、`fixtures/`、`eval/` | ✅ 是 | `docs/spec/*` 的契约 ＋ `AGENTS.md` |
| **工程元层** | `AGENTS.md`、`docs/governance-ai-agent-execution.md`、`docs/subagent-orchestration.md`、`docs/overnight-automation-charter.md`、`docs/nightly/*`、`MEMORY.md` ＋ `docs/memory/*`、`LEDGER.md`、`docs/PARKING_LOT.md`、`docs/adr/*`、`xtask/`、`.github/workflows/*` | ❌ 否 | 只受 `AGENTS.md` 约束 |

**为什么要正式区分**（ADR-0029 D5）：这条边界此前只存在于口头，造成三个真实症状 ——
① 文档地图把「夜间自动化章程」与「存储设计」并列，读者（尤其是将来开源后的外部读者）
分不清哪些是产品的设计、哪些是「我们怎么干活」；② 每次讨论夜间自动化都要重新解释一遍
「它不影响 `crates/` 里的任何代码」；③ 护栏工具与治理文档的体积已接近产品文档，
若不分类，「阶段 0 零产品代码」这个事实会被掩盖（此前只能靠一句免责声明打补丁，那是补丁不是结构）。
边界不清的真实代价是**注意力错配**：工程元层的讨论会被误当成产品需求变更，反之亦然。

> **推论**：工程元层的 ADR（夜间自动化机制、护栏工具口径、ADR 编号治理…）**不构成产品决策**，
> 也不改变任何 `docs/spec/*`。审阅它们不需要产品上下文；反过来，审阅产品 spec 也不需要读它们。

---

## 文档地图（按需读，不要全读）

| 你想做什么 | 读什么 |
|---|---|
| 让 AI agent 在本仓库干活 | **`AGENTS.md`**（唯一入口，含十条铁律与会话协议） |
| 了解整体架构（分层、进程边界、安全、平台差异） | `cross-platform-ai-assistant-architecture-v2.md` |
| 了解"某个应用能不能接、怎么接" | `target-apps-feasibility.md` |
| 了解现在做什么、不做什么 | `PLAN.md` → `plans/<当前阶段>.md` |
| 回忆项目已知什么 / 否决过什么 / 踩过什么坑 | `MEMORY.md`（**L0 索引**，≤150 行，含路由表）→ 按路由跳读 `docs/memory/*` |
| 了解治理机制（任务卡、CI 门禁、代码规范、验收分离） | `docs/governance-ai-agent-execution.md` |
| 了解多 agent 如何分工 | `docs/subagent-orchestration.md` |
| 了解存储方案与性能预算 | `docs/storage-design.md` |
| 了解全项目拆解顺序 | `docs/wbs-overview.md` |
| 了解夜间无人值守自动化的**纪律与边界** | `docs/overnight-automation-charter.md`（章程 v1.4 §11：一主一备 —— 主 = Codex 原生 scheduled tasks，备 = 任务计划程序 + `codex exec`） |
| 查夜间自动化的**具体操作**（建 / 改 / 删 / 立即运行 / 暂停 / 停止 / 恢复） | `docs/nightly/codex-automations-operations.md`（ADR-0029 的主交付物；每条结论都带 `[官方]` / `[实测]` / `[未验证]` 证据标签） |
| 查契约细节 | `docs/spec/`（naming 已就位，其余陆续产出） |
| 查"为什么当初这么决定" | `docs/adr/README.md`（**编号登记表**：哪些号已有文件 / 哪些只是待建）→ 具体 `docs/adr/NNNN-*.md` |
| 查台账 / 停车位 / 依赖登记 | `LEDGER.md` / `docs/PARKING_LOT.md` / `docs/DEPENDENCIES.md` |

---

## 当前阶段

**阶段 1 — 三试点闭环**（Notepad → Paint → Edge/Chrome；TASK-011 ~ TASK-058）。
子阶段 1a 已开工：地基层的 `crates/protocol`（schema + codegen）/ `crates/storage`（SQLite WAL + 迁移注册表 + blob）/
`crates/audit`（append-only hash chain）/ `crates/core` 骨架 / `crates/secrets`（OS keychain 封装）/
`xtask` 护栏（`docscan` 4 条结构规则 + `check-ledger` + `check-migrations` + `crates/core` 分层断言）/
**`crates/platform/api`**（铁律 7 的唯一平台入口：`TargetDescriptor` / `NormalizedPoint` / `Fingerprint` /
`CapabilityMatrix` + `PlatformService` / `WindowProvider` / `UiAutomationProvider` 三个 trait 形状）**均已落地**；
跨阶段治理卡 TASK-200 / 201 / 202 / 203 已 Done。
**下一张 = TASK-017（`crates/platform/windows`：Win32 / UIA provider 实现）**。＋ Notepad 的 3 个任务闭环。
阶段 0（文档与 Spike）已于 2026-09-20 closeout —— 它的产出是 Spike 报告，**不是**产品代码。
详见 `plans/stage-1-pilots.md`。

平台基线：**Windows 11 24H2+**（唯一正式基线；Windows 10 已 EOL，仅 C 级尽力）。
Linux 侧 **Wayland-first**（GNOME 50 已移除 X11 后端）。

---

## 构建与验证

前置：Rust stable（由 `rust-toolchain.toml` 固定）。Windows 需要 MSVC 生成工具 + Windows SDK。

```powershell
# 安装工具链（Windows）
winget install Rustlang.Rustup

# 三项基本门禁
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace

# 仓库卫生护栏（gov §5.4；当前实现 3/13 项，工具会自己声明 —— ADR-0025）
cargo run -p xtask -- hygiene
cargo run -p xtask -- --list-deferred   # 查看"还缺哪些检查、归属哪张卡"

# 文档一致性护栏（ADR-0030；派生计数与 ADR 编号不再靠人记）
cargo run -p xtask -- memory-counts     # MEMORY.md 规模表 ↔ docs/memory/ 实测
cargo run -p xtask -- adr-index         # ADR 登记表 ↔ docs/adr/*.md ↔ decisions.md

# 改写公共热点文件（LEDGER.md / docs/memory/* / docs/PARKING_LOT.md）前先取锁（ADR-0028）
cargo run -p xtask -- guard acquire MEMORY.md --owner <会话级唯一标识> --intent "<要干什么>"
cargo run -p xtask -- guard status
cargo run -p xtask -- guard release MEMORY.md --owner <同上>
```

`xtask` 是**零第三方依赖**的只读护栏工具。它对未实现的子命令**显式返回失败**
（退出码 3）并指出归属卡号 —— 本项目不允许任何形式的静默失败。

CI：`.github/workflows/ci.yml`（三平台矩阵；**硬门禁 10 项**已上线 —— fmt / clippy / test / verify-schemas(#6) /
codegen --check(#7) / deny / build / hygiene / spike-deny(#8b) / doc-consistency(#12b)；另有 **7 项软门禁**标
`continue-on-error` 并注明启用卡号；#6/#7 的注入式负向验证在 `gate-negative` job，单测侧负向用例在
`xtask/src/{verify_schemas,codegen}.rs`）。

---

## 最近进展（2026-09-24：阶段 1 地基层 + 平台抽象层 + 治理池收口 + 护栏补齐 = TASK-011 / 012 / 013 / 014 / 015 / 016 / 200 / 201 / 202 / 203）

阶段 1 的地基层已经落地（含密钥层），治理池把 TASK-013 现场撞出的三个**结构性**缺陷一次性收口，
`xtask` 护栏同批补齐并把两条 CI 门禁由软转硬。

| 卡 | 内容 | 状态 |
|---|---|---|
| TASK-011 | `crates/protocol`：JSON Schema → Rust/TS 类型 codegen；`codegen --check` 变成**真门禁**（drift 会 exit 1） | ✅ Done |
| TASK-012 | `crates/storage`：SQLite WAL + 迁移框架 + blob 池（zstd + sha256 内容寻址） | ✅ Done |
| TASK-013 | `crates/audit`：append-only + SHA-256 hash chain + ring buffer 批量 flush（摊销 < 1 ms/条） | ✅ Done |
| TASK-200 | `docs/spec/*` 7 份契约草案的系统性结构缺陷（外来模板块 / 空节 / 断链引用）—— PL-038 闭环 | ✅ Done |
| TASK-201 | `crates/core` 骨架提前（PL-037 闭环） | ✅ Done |
| TASK-202 | 存储**迁移注册表**：storage 只提供机制、各 crate 自持迁移 + 唯一装配点（**ADR-0038**，PL-046 闭环） | ✅ Done |
| TASK-203 | `audit_logs` 列语义去重 + 显式链序：删与 `id` 同义的 `hash`、加 `sequence`（**ADR-0040**，PL-043 / PL-045 闭环） | ✅ Done |
| TASK-014 | `crates/secrets`：OS keychain 封装（`keyring` 4.2 / DPAPI·Keychain·Secret Service）+ `zeroize` + 访问审计注入点（fail-closed） | ✅ Done |
| TASK-016 | `crates/platform/api`：**铁律 7 的唯一平台入口** —— 4 个纯类型（`TargetDescriptor` / `NormalizedPoint` / `Fingerprint` / `CapabilityMatrix`）+ 3 个 trait 形状（RPITIT，**零 `async-trait` 依赖**）+ 能力矩阵 4 条不变量的机器校验；`ResolvedWindow` / `ResolvedElement` = 不透明句柄（铁律 8 有源码扫描断言，含负向样本） | ✅ Done |
| TASK-015 | `xtask` 护栏补齐：`docscan` 4 条结构规则（裸 NUL / 数字节号重复 / 整节为空 / 标题文字重复）+ `crates/core/tests/arch*` 分层断言（`arch::` 由 0 → 5 个测试）+ `check-ledger`（ADR-0039 D3）+ `check-migrations`（PL-047）；同批修 PL-048（flaky）与 PL-051（裸 NUL） | ✅ Done |

TASK-016 把**平台抽象层**立起来：`core` / `Host` 从此只能经 `assistant-platform-api` 的 trait 用平台能力
（铁律 7），且**该 crate 全 crate 零 `#[allow]`、零 `unsafe`、零第三方依赖**（除已登记的 `serde`）——
`f64 → i32` 这类"`as` 会饱和 + pedantic 会拦"的转换改用整数域逐位合成，越界一律报 `TargetNotFound`。

治理机制同步前进：**ADR-0039** 把「卡 Done = 同一 PR 内同步 `PLAN.md` + `README.md`」写成硬契约
（DRIFT-202-2 闭环），其机器判据 `check-ledger` 已由 **TASK-015** 落地并**由软门禁转硬**（CI 的 `[HARD #16]`）；
同批把 gov §5.1 **#5 arch test** 也转硬 —— 它此前是 `running 0 tests` 的假绿，现在是真的 5 个断言。

### 历史小节（按时间倒序）

- **2026-09-19 TASK-059/060/061/062/063/064（xtask 护栏升级六连发）**：
  `xtask` 从 4 个子命令扩到 7 个（新增 `refscan` / `docscan` / `card-check`），
  原 `hygiene / memory-counts / adr-index / guard` 保持；全部在 CI 硬门禁 #12b（`doc-consistency`）下统一跑过。
  `docscan` 扫破表（PL-031）+ setext 风险 + UTF-8 BOM/CRLF/缺末行 LF；`card-check` 是 ADR-0031 D6 的机器化；
  同一批清掉了 `xtask/src` 生产代码里的全部 per-line `#[allow]`（**ADR-0034** / **ADR-0035**）。
### 历史小节（按时间倒序）

- **2026-09-19 TASK-064**（promote `extension_is` helper + 收 repowalk.rs:172 per-line allow）:
    - 新增 `pub fn extension_is(path: &Path, expected: &str) -> bool` 到 `xtask/src/repowalk.rs`（紧邻 `has_rust_extension`）；同步把 refscan.rs 的 local `ext_is` closure 删掉、import 此 helper
    - `collect_repo_files_recursively` 内 2 处 `lower.ends_with(...)` 改 `extension_is(&path, ...)`；删函数级 `#[allow(clippy::case_sensitive_file_extension_comparisons)]`
    - 加 4 条 `extension_is` 单元测试：positive match / case-insensitive / 非 UTF-8 返回 false / 无扩展名返回 false（cargo test 279 → 283 passed）
    - **副作用**（行为变化）：`.ends_with(&format!(".{ext}"))` 在 caller 传大写 ext（如 `"MD"`）时会漏匹配，`extension_is` 修复了此 bug —— **现状** caller 全部传小写（card_check `["md"]` / docscan `["md"]` / refscan `["md","rs","ps1"]`）故无回归；**测试覆盖** test_extension_is_is_case_insensitive 显式断言 4 种大小写组合
    - ADR-0035 §1 baseline 表补登 `repowalk.rs:172` 已清 + §决策 2 加一条「抽到 `pub fn` 替代 per-line allow 必须同步登记」（教训：TASK-063 提升到 helper 后才发现 repowalk.rs:172 还有同型 allow）
    - **真正的最终态**：全 xtask/src 生产代码 per-line allow = 0（refscan:0 + repowalk:0 + docscan:0 + card_check:0 + exemptions:0；保留 = test wrapper 4 处合法 + card_check 9 + exemptions 1 共 10 处 `#[allow(dead_code)]` 占位常量）
    - 详见 `tasks/TASK-064-promote-extension-is-helper.md` 执行记录 9 节 + LEDGER.md 2026-09-19 行
- **2026-09-19 TASK-063**（xtask refscan 清最后 3 处 per-line allow — 达到「生产代码 per-line allow = 0」）:
  - refscan.rs:94 `#[allow(clippy::single_char_pattern)]` → `replace("\r", "\n")` 改 `replace('\r', "\n")`（char 字面量，触发 Pattern impl 即可）
  - refscan.rs:101/103 `#[allow(clippy::case_sensitive_file_extension_comparisons)]` → `lower.ends_with(".md"/".rs"/".ps1")` 改 `Path::extension().and_then(to_str).is_some_and(eq_ignore_ascii_case)`（closure `ext_is` 复用）
  - 删除 `let lower = rel_path.to_ascii_lowercase();` 与配套注释（5 处出现 → 0）
  - 行为不变：refscan 仍报 151 errors（baseline 一致）；UTF-8 非合法扩展名按 `to_str` 失败语义 = 与原本 `ends_with`(`&str`) 在非 UTF-8 路径上失败同形
  - **复核 GAP**：repowalk.rs:336 `#[allow(clippy::case_sensitive_file_extension_comparisons)]` **未在本卡 scope 内**（卡面标题与 In scope 严格限制 refscan.rs），且 ADR-0035 baseline 表也漏列；建议下一卡 `TASK-064` 或新卡处理（统一改 `Path::extension` 模式）
  - 详见 `tasks/TASK-063-clear-last-3-per-line-allows.md` 执行记录 9 节 + LEDGER.md 2026-09-19 行
- **2026-09-19 TASK-062**（xtask render() 返回 Result — 消除 per-line allow）:
  - `pub fn render(findings: &[Finding]) -> String` → `pub fn render(findings: &[Finding]) -> Result<String, std::fmt::Error>`
  - 删除 `#[allow(clippy::unwrap_used, clippy::expect_used)]` per-line allow（TASK-060 遗留）
  - `writeln!(..).expect("...")` → `writeln!(..)?` + `.map_err(|e| e.to_string())?` 在 `run()` 中
  - 行为不变（writeln! to String 永不失败；Result 类型强制 caller 处理 = 编译期保证）
- **2026-09-19 TASK-061**（xtask lint cleanup pass 2 — TASK-060 遗留 5 处 str[Range] + 1 处 stale dead_code + 3 处 test f[0]）:
  - card_check.rs 5 处 str[Range] → 全替换为 `.get(range)` + `?` operator 重构
  - refscan.rs:290 stale `#[allow(dead_code)]` 删除（render 已被 run() 调用）
  - docscan/card_check 测试代码 `f[0]` → `f.first().expect("non-empty")`（3 处）
- **2026-09-19 TASK-059 / TASK-060**（xtask 护栏升级 + 纯重构）：
  - TASK-059 cherry-pick 悬空 commit `10f78db` 收回 → 4 子命令可执行 + ADR-0034 注册
  - TASK-060 人类裁决 B 路（漂移不可忍受）= 纯重构 = 4 模块顶部 `#![allow(...)` 块全清
    （**DoD 硬证据**：`grep '^#![allow' xtask/src/{refscan,docscan,card_check,exemptions}.rs` = 0 命中）
    + 32 处 `indexing_slicing` 用 `while let Some + .get(n).expect()` 全替换
    + `docs/PARKING_LOT.md` PL-NEW 关闭
  - 防御性 PITFALL：`docs/memory/pitfalls.md` 追加「禁止 sub-card 后缀；commit 标题必须对应 `tasks/TASK-NNN-*.md`」
  - 详见 `LEDGER.md` 2026-09-19 三行 + `tasks/TASK-059-...md` / `tasks/TASK-060-...md` 执行记录
- **2026-09-17 TASK-001**：仓库骨架 + CI 8 硬门禁 + ADR 编号登记表建立

## 许可证

`MIT`（见 `LICENSE`）。

> 说明：内部项目，暂不公开。计划中的双许可 `MIT OR Apache-2.0` 尚未最终确认
> （`docs/memory/open.md` 的 **M5** 待裁决项）；当前先以 MIT 落地，**该决定可逆**。
> 开源前需要完成脱敏，清单待建（见 `docs/PARKING_LOT.md` PL-005）。
