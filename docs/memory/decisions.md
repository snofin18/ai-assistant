# decisions.md — 已决策索引（DECISION）

> 标签 `[DECISION]` = 已定案的决策，**详情一律在 ADR / v2 对应章节**，本文件只做索引。
> 改决策 = 新 ADR + 在此追加一行（不删旧行）。

---

> **本文件是 `MEMORY.md` 分层结构（ADR-0021）的 L1 层之一。**
> 写入规则见 `MEMORY.md`「条目格式」与「更新职责」；路由规则见 `docs/memory/README.md`。
> **只追加，不改写他人条目**；更正用新条目 + `[supersedes:日期]` 标注。
> 应用专属的条目**不进本文件**，进 `docs/memory/apps/<app>.md`。

- [2026-09-16][DECISION][ADR:待建 0001] 核心语言 **Rust**；UI 用 **Tauri 2 + React/TS**；工具协议 **MCP-first（`rmcp`）**，内部工具同样以 MCP 表达。
- [2026-09-16][DECISION][ADR:待建 0002] **API 优先**为纲领原则一：L1 应用接口 > L2 命令 > L3 无障碍 > L4 合成输入 > L5 视觉。
- [2026-09-16][DECISION][ADR:待建 0003] **Linux Wayland-first**，X11 仅作 XWayland 兼容通道；按合成器家族分流；只承诺支持矩阵内组合。
- [2026-09-16][DECISION][ADR:待建 0004] **element/句柄不跨进程**：Host 边界切在"定位之后"，对外只传可序列化 descriptor。
- [2026-09-16][DECISION][ADR:待建 0005] **可逆性四级模型（L0~L3）**；撤销快捷键必须由 Adapter 显式声明；不可逆动作后置。
- [2026-09-16][DECISION][ADR:待建 0006] **无人值守暂不支持但三处预留**（类型/契约/能力 + UI 占位），未来仅白名单应用。
- [2026-09-16][DECISION][ADR:待建 0007] **出域策略用户可选三档**（`local_only`/`redacted`/`full`）+ 逐应用与逐内容类型覆盖；默认 `redacted`；降级不静默。
- [2026-09-16][DECISION][ADR:待建 0008] **股票类软件只读 + 解读 + 图形展示**；`TradingGate` 恒拒绝并预留，未来只经厂商官方接口开放。
- [2026-09-16][DECISION][ADR:待建 0009] 平台基线 **Windows 11 24H2+**；Win10 仅对跨版本一致目标提供 C 级尽力支持。
- [2026-09-16][DECISION][ADR:待建 0010] 版本支持：**Office 2019+**（2016 技术可用不承诺）；**Photoshop 最低 2021(v22)**（2020 无 UXP，列尽力而为）。
- [2026-09-16][DECISION][ADR:待建 0011] 试点顺序 **Notepad → Paint → Edge/Chrome**（阶段 1）→ Excel（2）→ Photoshop（3）；理由：零许可零登录依赖、三种通道覆盖、安全底座提前。
- [2026-09-16][DECISION][ADR:待建 0012] 存储四层：**内存热缓存 / SQLite(WAL) / 内容寻址 blob(zstd+去重) / 冷归档**；只有 Core 写库。
- [2026-09-16][DECISION][ADR:待建 0013] 分发：**当前内部使用，但按开源规范建设**（建议 `MIT OR Apache-2.0`），`adapters/` 与 `adapters-private/` 从第一天分开。
- [2026-09-16][DECISION][ADR:待建 0014] 执行模式：**AI agent 实现 + 人类裁决**；防漂移靠 SSOT 分层、任务卡 write scope、约束回执、14 项 CI 门禁、验收分离。
- [2026-09-16][DECISION][ADR:待建 0015] 命名要求"一眼可懂"+ **受控词汇表**；注释密度偏高，公共 API 100% 文档注释，坑用 `PITFALL(app=…)` 结构化标签。
- [2026-09-16][DECISION][ADR:待建 0016] **全仓库统一 LF**：`.gitattributes` 设 `* text=auto eol=lf` + 显式二进制标记，`rustfmt.toml` 设 `newline_style = "Unix"`。理由见 §4 的对应 REJECTED 条目。
- [2026-09-16][DECISION][ADR:待建 0017] **未实现项必须显式登记 + 显式失败**：xtask 未实现的子命令以退出码 3 失败并打印归属卡号；`hygiene` 每次运行都声明"11 项只实现 3 项"；CI 有 `deferred-inventory` job 专门验证"未实现子命令仍然失败"。理由：一条"因为没写所以返回 0"的门禁比没有这条门禁更危险（制造虚假安全感）。
- [2026-09-16][DECISION][ADR:待建 0018] 夜间自动化用 **heartbeat**（每日 23:30 与 02:30 唤醒当前 thread）：每夜 ≤3 轮、每次唤醒 ≤2 轮、`.nightly.lock` 互斥、只在 `nightly/<date>` 分支提交、不合并 main、不操作真实应用、仅失败时通知、晨间报告落 `docs/nightly/<date>-report.md`。详见 `docs/overnight-automation-charter.md` §11。
- [2026-09-16][DECISION][ADR:待建 0019] **`#[allow]` 的唯一合法位置**是 `#[cfg(test)] mod tests` 上的 `clippy::unwrap_used` / `expect_used` / `panic`（gov §5.2 授权：测试失败就该炸）。产品代码里加 allow = 放宽护栏 = 漂移触发器，夜间自动化一律禁止。需写进 `docs/spec/testing.md`。
- [2026-09-16][DECISION][ADR:待建 0020] TASK-001 先落 **MIT 单许可**（而非 gov 建议的 `MIT OR Apache-2.0`）：内部阶段不需要双许可复杂度，且该决定**可逆**（追加 Apache-2.0 是加法）。M5 仍待人类在阶段 0 结束前正式确认。
- [2026-09-16][DECISION][ADR:待建 0018 补充] 夜间工作的**选取顺序是三级回退**：① `tasks/` 下 Ready 任务卡 → ② 章程 §13「当前夜间工作单」（人类白天维护，夜间只能执行不能改写）→ ③ 白名单 P2~P8。理由：阶段 0 的 10 张卡**没有一张是夜间安全的**（Spike 全都要真机 GUI 或新增依赖，而黑名单两者都禁止），若坚持"必须有 Ready 卡"，夜间自动化会永久空转。首批工作单 W1（rustscan 函数扫描器）/ W2（hygiene 函数级 3 条规则）/ W3（testing spec 草案）/ W4（ADR 0016~0020 草稿）已写入章程 §13。

---

---

## 2026-09-18 追加（ADR-0021 ~ ADR-0025，均 Accepted）

- [2026-09-18][DECISION][src:ADR-0021] **长期记忆分层**：`MEMORY.md` 降级为 L0 索引（≤150 行），全部具体条目移入 `docs/memory/{facts,pitfalls,rejected,decisions,open}.md`；新增 L2 应用档案 `docs/memory/apps/<app>.md`（8 个固定小节）；L3 `archive/` **按体积（>400 行）而非按时间**触发。读取路由表见 `docs/memory/README.md`。（人类指示 #11）
- [2026-09-18][DECISION][src:ADR-0022] **Windows 目标身份与 UIA selector 稳定性**：禁用启动 PID 定位、禁用 `MainWindowHandle`；启动走 **AUMID + `IApplicationActivationManager::ActivateApplication`**，属主 PID 只从 `GetWindowThreadProcessId` 取；selector = **有序候选链 + `app_version_range` + 状态指纹**；本地化禁令扩到 5 项；`RuntimeId` 仅会话内有效；DPI 用**物理像素 + Per-Monitor V2**；会话恢复 / TeachingTip / 标签拖撕 需显式处理。（人类指示 #1 #5）
- [2026-09-18][DECISION][src:ADR-0023] **文本 EOL 归一化契约**：规范形 = **LF**；归一化顺序**铁律** `replace("\r\n","\n").replace("\r","\n")`（不可颠倒）；出口按**每应用习惯表**（`text_conventions`）；postcondition 必须用规范形比较，且原始长度差必须能被 EOL 计数**完全解释**；Adapter 绑定期必须自检一次。→ 记事本的实例见 `apps/notepad.md` §4。（人类指示 #2）
- [2026-09-18][DECISION][src:ADR-0024 D1 + **D1a 实证修正**] **Rust 侧 UIA 一律用 `windows` crate 的 COM 绑定**，否决 `uiautomation` crate。**D1a 关键修正：feature 名是 `Win32_UI_Accessibility`，不是 `Win32_UI_UIAutomation`（后者不存在）**，且必须同时开 `Win32_System_Ole`（`VARIANT` 被双重 gate）。实证载体 `spikes/spike-a-notepad/src/bin/uia_dep_proof.rs`，E1~E6 全 PASS。（人类指示 #6）
- [2026-09-18][DECISION][src:ADR-0024 D2] **PL-019 关闭**：`spikes/` 单独一道 **`spike-deny`** 硬门禁（gov §5.1 **#8b**），枚举 `spikes/*/Cargo.toml` 逐个跑 `cargo deny check licenses sources`；**只查这两项**（免联网）；无 manifest 时必须**显式打印「无」再 exit 0**（铁律 1）。canary 在 `gate-selftest.yml`，ADR-0019 登记表已补 #8b 行。fmt/clippy/test 对 spikes 的豁免是**有意的**。（人类指示 #7）
- [2026-09-18][DECISION][src:ADR-0024 D3] **控件普查工具 = `inspect.exe` + 自写 UIA 树导出**，否决补装 Accessibility Insights（输出不可 diff / 不可计时 / 不可重复）。官方 `winapp ui inspect` 列为候选，试用归 TASK-003。（人类指示 #8）
- [2026-09-18][DECISION][src:ADR-0024 D4] **仓库内 `.ps1` 一律纯 ASCII**：PS 5.1 按 GBK 解码无 BOM 脚本会导致行号错乱 + `` `r `` 被吞。中文说明一律放 `.md`。（`pwsh` 7 保留为逃生口，已否决作为默认。）
- [2026-09-18][DECISION][src:ADR-0018 状态变更] **夜间自动化的投递机制 = Windows 任务计划程序 + `codex exec`**，**放弃** Codex automation（heartbeat / cron 两条路都已被探针否决：投递机制产生畸形 `function_call_output`，缺 `call_id`）。章程 §11 据此重写，验收测试见 `docs/nightly/scheduler-acceptance-test.md`。（人类指示 #10 + 2026-09-17 探针结论）
- [2026-09-18][DECISION][src:ADR-0025] **仓库卫生规则总数 = 13**（gov §5.4 表格行数即唯一事实源）：原 11 项 + 「禁 CRLF」（**Error**）+ 「必须以单个 `\n` 结尾」（**先 Warning，清扫完 15 个既有文件后升 Error**）。PL-001 / PL-011 / PL-020 **三条一并关闭**；「hygiene 12 项」= 笔误，「CI 14 项门禁」= 过时表述。**CI 口径同时定案**：gov §5.1 = **17 行清单** ↔ **16 个 CI 步骤（7 硬 + 9 软）**；主编号**永不重排**，新增门禁用子编号（`#8b`）。⚠️ 本文件里 2026-09-16 的两条旧 DECISION（「14 项 CI 门禁」与「11 项只实现 3 项」）**按原样保留**，以本条为准。（人类指示 #12）
- [2026-09-18][DECISION][src:人类指示 #9 + rollout 取证] **任务卡会话由人类手工新建，不由 agent 调 `create_thread` 派生**（至少在上游 openai/codex#36315 关闭前）。已落成 `docs/subagent-orchestration.md` **§10.1**（v1.0 → **v1.1**）与 §10 表新行。**勿混淆**：`spawn_agent`（subagent）**仍可用且仍是主路径**，受限的只是「在 Codex 应用里新建人类可见的独立任务」这一条路径。

## 2026-09-18 追加（ADR-0026 D2：编号 0019 → 0027）

- [2026-09-18][DECISION][ADR:待建 0027][supersedes:2026-09-16 的 [ADR:待建 0019] 条目] **`#[allow]` 的唯一合法位置**是 `#[cfg(test)] mod tests` 上的 `clippy::unwrap_used` / `expect_used` / `panic`（gov §5.2 授权：测试失败就该炸）。产品代码里加 allow = 放宽护栏 = 漂移触发器，夜间自动化一律禁止。需写进 `docs/spec/testing.md`。**改号理由**：0019 号已被 `docs/adr/0019-hard-gate-negative-verification.md`（硬门禁负向验证）占用，两条完全不同的决策静默共号；ADR-0026 D2 把这条**尚未建文件的待建号**改为 0027（改预留号代价≈0，改已建文件号要同步十余处引用）。原 2026-09-16 条目按「只追加不改写」的规矩**保留原样**，以本条为准；0019 号在 `docs/adr/README.md` §3 登记为**已退役**（专指硬门禁负向验证，永不再分配给本条决策）。今后同类事故由 `cargo run -p xtask -- adr-index` 的 `adr/number-collision` 与 `adr/retired-number-reallocated` 两条规则机器拦截（ADR-0030 D3）。

## 2026-09-18 追加（ADR-0028 / ADR-0029 / ADR-0030，均 Accepted）

- [2026-09-18][DECISION][src:ADR-0028] **文件改写互斥锁 = `xtask guard`**（协作式、按**仓库相对路径**为粒度、锁文件放 `target/locks/`，不入库）。要点：① 原子原语用 `OpenOptions::create_new(true)`（POSIX `O_CREAT|O_EXCL` / Windows `CREATE_NEW`，两平台语义一致，不需要第三方锁库）；② 锁记录是**行式 `key=value`**（不是 JSON —— xtask 零第三方依赖，手写 JSON 解析器的转义坑比行式格式多），且**必须带人类可读字段**（`owner` / `pid` / `task` / `intent` / ISO 时间），因为超时放弃时等待方要能说出「谁、什么时候、为了什么」拿着锁；③ 默认 `--timeout 30` 秒、退避轮询 100 ms，超时即**放弃**并四件事通报 —— 退出码 **5**、机器可读 `-- guard-result: ABANDONED …` 行、`target/locks/abandonments.log` 追加、**在 `LEDGER.md` 追加一行**（前三件工具做，第四件是 `AGENTS.md` 的协议义务；缺第四件就只是「日志」不是「通报」）；④ 陈旧锁是**纯时间判据**（`--stale-after` 默认 900 s，接管必须打印被接管者的完整记录），**刻意不做 PID 存活探测**（要第三方依赖，且 PID 复用会误判成「还活着」→ 永久死锁）；⑤ 多文件**先按路径排序再逐个获取**，任一失败即**回滚全部已获取的锁**（死锁避免）；⑥ 时钟回拨时判「等待」而非「陈旧」并标注 clock anomaly。`xtask` 的「只读」不变量按 D8 **受限放宽**：只读**仓库内容**，唯一例外是 `target/locks/`。**已知弱点**：协作式锁无法技术强制 → 靠 `AGENTS.md` 义务 + review agent + `memory-counts`/`adr-index` 事后发现「条目数变少」三层兜住。（人类指示 #1 的追加硬性要求）
- [2026-09-18][DECISION][src:ADR-0029 D1/D2/D3/D4] **夜间自动化的目标机制改回 Codex 原生 scheduled tasks**，首选形态 = **standalone（cron 类）**（每次 run 开全新 chat，天然避免「毒项沉入长驻会话」与上下文累积漂移）；**heartbeat 永久不作为夜间主方案**（只在 GATE-0 里作为最小可验证单位用一次）。ADR-0018（Windows 任务计划程序 + `codex exec`）状态改为 `Accepted → Superseded by ADR-0029`，但**保留为回退方案**（它是本机唯一被证据支持可运行的路径；`docs/nightly/scheduler-acceptance-test.md` 不删除、只加横幅，章程 §11.8 的 10 条 `codex exec` 实测全部保留并加归属声明）。**一主一备，同一时刻只允许一种 Active**（否决双轨并行：「两个都是主方案」= 没有主方案）。**本轮范围 = 只调研落档，不做端到端调试**：交付操作手册 `docs/nightly/codex-automations-operations.md`（建/改/删/立即运行/暂停/停止/恢复逐条写法，每条结论带 `[官方]`/`[实测]`/`[未验证]` 标签）+ 其 §8 验收清单 + 章程 §11 重写为 **v1.4**。**启用前置门禁 GATE-0**：必须在标题以 `nightly-probe-` 开头的**新建废弃 thread** 上实测全链路成功，**绝不在主线工作 thread 上实验**（毒项会永久损坏它，损失不可逆），且**不得**用 `automation_update` 的 `mode = "view"` 判断存在性（它对不存在的 id 也渲染空卡 → 存在性只认磁盘 `automation.toml`）。**本轮没有创建、修改或删除任何真实 automation**。（人类指示 #2）
  - [supersedes:2026-09-24] → **ADR-0029 的「生效前提」已满足**：D3 的启用前置门禁 **GATE-0 于 2026-09-24 实测通过**（cron + heartbeat 两种形态；未触碰主线 thread；探针已清理）。关键新事实：**automation 的投递形态已变为正常 user message** → ADR-0018 的 E1 根因（`call_id: None` 的合成 `function_call_output`）**消失**、毒项失效模式未复现。→ ADR-0029 从「方向已定、未启用」转为「**已启用（尚未排期）**」；章程 §11 状态字段回填为 **v1.5**；证据 = `docs/nightly/codex-automations-operations.md` §2 / §8.2（18 行新基线）与 `docs/memory/facts.md` 2026-09-24 条目。**D1~D6 一字未改**。人类 2026-09-24 指示：**何时排期另行说明**。
- [2026-09-18][DECISION][src:ADR-0029 D5] **「产品层 / 工程元层」的边界正式落档到 `README.md`**。判定标准一句话：**「删掉它，产品的行为会变吗？」不会 → 工程元层**。产品层 = `crates/`、`protocol/`、`adapters/`、`adapters-private/`、`apps/`、`fixtures/`、`eval/`（会被构建进发布物，受 `docs/spec/*` + `AGENTS.md` 约束）；工程元层 = `AGENTS.md`、gov、`subagent-orchestration.md`、夜间章程与 `docs/nightly/*`、`MEMORY.md` + `docs/memory/*`、`LEDGER.md`、`docs/PARKING_LOT.md`、`docs/adr/*`、`xtask/`、`.github/workflows/*`（不进产品构建，只受 `AGENTS.md` 约束）。**推论**：工程元层的 ADR（夜间机制、护栏口径、编号治理…）**不构成产品决策**，也不改变任何 spec；审阅它们不需要产品上下文。（回答人类 Q1「夜间自动化、memory 分层是项目之外的辅助设计环节吧？」= **是**；此前这条边界只存在于口头，已造成三个真实症状，见 ADR-0029 诉求 B）
- [2026-09-18][DECISION][src:ADR-0030] **「记忆规模计数」与「ADR 编号登记表」由手工维护改为机器校验**：`xtask memory-counts`（**8** 条规则）+ `xtask adr-index`（**11** 条规则），CI 新增硬门禁 **#12b** `doc-consistency`（gov §5.1 用**子编号**，沿用 `#8b` 先例；gov §10 口径改为「18 行清单 ↔ 17 步骤（8 硬 + 9 软）」）。动机是**同一根因已造成两次真实事故**（第二次就发生在刚修完第一次之后）：同一个派生事实手写多处就必然漂移，靠人工核对等于把护栏建在注意力上。**D2 是重点：消除重复书写比校验重复书写更重要** —— 删掉 `MEMORY.md` 正文里「L1 现合计 207 条」这个派生合计（规则 `memory/derived-total-in-prose` 禁止「现合计」字样复发），迁移核对块的 **155 / 200 / 7** 三个数字**保留但显式标注为历史快照**（它们是历史事实，不随后续追加变化）。**D3 引入退役清单**（`docs/adr/README.md` §3 的 `已退役编号：` 行）：`decisions.md` 只追加，0019 的原始待建条目会永远留着，不排除退役号就会永久红灯。**表结构解析不到必须报错而不是静默当作一致**（`memory/scale-table-unparsable`、`adr/registry-section-missing`，铁律 1）。**D4 刻意不实现**裸引用检查 → 拆出 **PL-032**。（人类指示 #4「修复，避免今后再发生」）
- [2026-09-18][DECISION][src:ADR-0026 人类确认] **ADR-0026 由 `Proposed` 转为 `Accepted`**，D2 / D4 随之执行：① `docs/memory/decisions.md` 追加 `[ADR:待建 0027]` 条目（`#[allow]` 的唯一合法位置 = `#[cfg(test)] mod tests`），原 `[ADR:待建 0019]` 条目按「只追加不改写」保留原样；② 章程 §13 W4 的 write scope 改为 **0016 / 0017 / 0020 / 0027** 四个号（逐个列出，**禁止写编号范围形式**）、「禁止」项改为「新建 **0028 及以后**的编号」+「不得复用已退役编号 0019」、验收改为 **4 个文件** 并新增 `xtask adr-index` 须 0 Error、write scope 额外含 `docs/adr/README.md`（仅回填 —— ADR-0030 之后「新建后必回填」是机器强制的）；③ `docs/adr/README.md` §3 登记 **已退役编号：0019**，**下一个可用编号 = 0031**；④ `docs/memory/open.md` **N8 关闭**。
- [2026-09-18][DECISION][src:本轮实施，PL-033 记录口径矛盾] **单文件行数超限的处置方式 = 用 `#[path]` 把私有单测外置，而不是提高阈值、也不是放宽 lint**。四个模块（`adr_index` / `adr_registry` / `guard` / `guard_runner`）的 `#[cfg(test)] mod tests` 外置到同名 `*_tests.rs`。外置**不改变可见性语义**（仍是父模块的私有子模块，`use super::*` 照旧能访问私有项），因此白盒测试能力完全不受影响。**为什么不改阈值**：gov 里存在两套互相矛盾的口径（§5.4「>600 警告 / >900 失败」vs 另一处「≤400 软 / 600 硬」，`AGENTS.md §5.3` 抄的是后者），改任一边都是「放宽护栏」= 漂移触发器；拆文件能让**两种口径同时满足**，把矛盾消解在事实上而不需要裁决。矛盾本身记为 **PL-033**（需 ADR 裁决）。

## 2026-09-18 追加（ADR-0031，Accepted）

- [2026-09-18][DECISION][src:ADR-0031] **任务卡改为「一卡一文件」**：卡片正文与执行记录同在 `tasks/TASK-NNN-<slug>.md`，文件内用一条 HTML 注释**分界线**把「正文区（Orchestrator 所有，Implementer **只读**）」与「执行记录区（Implementer 所有，固定 9 节，骨架见 gov §3.4）」分开；`plans/<阶段>.md` 退回**阶段索引** —— 只留阶段级 In/Out scope、阶段 DoD、卡号索引与批次/并行建议，**不放卡片正文**。七条子决策里最需要记住的五点：① **D2 状态只有一个落点**（卡片文件的 `- 状态：` 行），`plans` 的索引表**刻意不设「状态」列** —— 与 ADR-0030 同源，同一事实手写两处必然漂移，而状态是每次会话都要改的高频值；② **D3 分界线用 HTML 注释而不是三个连字符** —— 后者在 Markdown 里会把上一行变成 setext 标题，本项目已为此设了扫描判据；③ **D5 write scope 默认项**：`tasks/TASK-NNN-*.md`（**仅本卡号那一个文件**）的记录区默认在每张卡的 scope 内、正文区不在，于是并行 agent 天然不重叠（一人一卡一文件）；④ **D6 四条机器判据已写死**（正文区 diff 非空 → Error；状态非 `Ready` 的卡必须有文件；记录区缺 9 节 → Warning，`Ready` 豁免；`plans/*.md` 出现卡片正文形态的二级 TASK 标题 → Error 防形态回退），**实现归 PL-002 的 `card-check`，本轮不写代码**；⑤ **D7 `<slug>` 与分支名 `task/TASK-0NN-<slug>` 用同一个 slug**，阶段末归档到 `docs/history/tasks-stage-N/`（按体积/阶段归档，不按时间删）。**已诚实记录的退步**：正文与记录同文件 = 执行者技术上能改自己的考核标准，靠「分界线注释 + D6 判据 ① + review agent 检查项」三层兜；但真正拦住粉饰的是**验收命令的机器输出**，不是文件隔离（这一点在「正文留 `plans`」的方案下也一样）。**已执行的迁移**：stage-0 的 10 张卡正文从 `plans/stage-0-spikes.md`（385 → 83 行）逐字节搬到 10 个卡片文件，**零丢失核对通过**（原 253 行非空正文与新文件正文区逐行同序一致，两套独立实现各核对一次）。**采纳理由的关键**：这不是新发明，而是把三处**已经存在但从未写明**的假定落成规矩（gov §3.2 模板本身是文件级 H1 形态、`AGENTS.md` §1 已按 `tasks/` 独立文件路由、§8 那句「不可改 In/Out scope」只在一卡一文件下才逐字自洽）；且 stage-1 的 48 张卡正文**还没写**，现在是唯一一次零迁移成本的定型时机。（人类裁决 DRIFT-001-3：「先做 DRIFT-001-3，按你说的一卡一文件做，把 0031 号 ADR 写出来」）

## 2026-09-18 追加（ADR-0032，Accepted）

- [2026-09-18][DECISION][src:ADR-0032 D1] **文档护栏规则的豁免清单 = 唯一机器可读 SSOT**：物理文件 `docs/adr/0032-doc-rule-exemption-registry.md`（与本 ADR 配套），每条豁免 5 列（ID / 规则 / 位置 / 理由 / 移除触发）。机器规则的 xtask 实现**仅读这张表**，不在源码里 hardcode。本 ADR 只规定机制，**不实现机器规则的豁免读入逻辑**；读入逻辑归 PL-002 / PL-015。
- [2026-09-18][DECISION][src:ADR-0032 D2/D4] **豁免清单的添加 / 移除流程**：① 添加 = 追加一行 + 在 `decisions.md` 留一条 `[DECISION][src:ADR-0032] 豁免 E-NNN`（与 ADR-0028 加锁记录的规矩一致）；② 移除 = 行尾加 `[supersedes:YYYY-MM-DD]`、**不删原行**（与 `decisions.md` 与 `open.md` 的 `[supersedes]` 规矩一致）；豁免本身是派生事实的二次派生（机器规则重复才能跳过），所以也需要被机器检查。
- [2026-09-18][DECISION][src:ADR-0032 D5] **“永不”豁免的处置**：豁免表里“移除触发”写「**永不**」的条目 = 该文件是“只追加”类（LEDGER、PARKING_LOT、长期记忆类 ADR 正文）= 这些违规会**永远**留在仓库里。这不是 bug 是事实（只追加文件的语义就是不能改）；XTASK 该在打印豁免时把“永不”**显式标红 / 标 [PERMANENT]** 供人类季度审视“这些豁免还合理吗”。

## 2026-09-18 追加（ADR-0033，Accepted）

- [2026-09-18][DECISION][src:ADR-0033 D1] **单文件行数口径不改、两套阈值各管一摊**：**写作规范 400/600**仍在 gov §6.2 (Rust) 「模块规模」行作为提醒性上限；**CI 门禆 600/900** 仍由 `xtask/src/hygiene.rs` `FILE_LINES_WARN` / `FILE_LINES_ERROR` 实装 = gov §5.4 「file-too-long」。两套不冲突，是常见的「建议 vs 强制」双层护栏：作者先按 400/600 拆，工具按 600/900 拦超标。
- [2026-09-18][DECISION][src:ADR-0033 D2/D3/D4] **口径不动、只加标注**：§5.4 表行末尾加「(CI 门禆；作者上限见 §6.2)」、§6.2 (Rust) 「模块规模」行末尾加「(写作规范，不是 CI 门禆)」、`AGENTS.md §5.3` 同步加标注。阈值数字本身**一个都未动**；改的是**阈值的角色说明**。
- [2026-09-18][DECISION][src:ADR-0033 D1 隐含推辑] **§6.2 (Rust) 不拆到新节、原位置保留、只加同行说明**：§6.2 是**Rust 语言专项规范**的一部分（诺多语言不共享「写作规范」这个概念）；独立成节会造成「谁才是写作规范」的难以回答问题。**少改动 = 少拖雲**（本就是 PL-017 裁决重复提醒的同一件事）。

## 2026-09-18 追加（ADR-0033，Accepted）

- [2026-09-18][DECISION][src:ADR-0033 D1] **单文件行数口径不改、两套阈值各管一摊**：**写作规范 400/600**仍在 gov §6.2 (Rust) 「模块规模」行作为提醒性上限；**CI 门禆 600/900** 仍由 `xtask/src/hygiene.rs` `FILE_LINES_WARN` / `FILE_LINES_ERROR` 实装 = gov §5.4 「file-too-long」。两套不冲突，是常见的「建议 vs 强制」双层护栏：作者先按 400/600 拆，工具按 600/900 拦超标。
- [2026-09-18][DECISION][src:ADR-0033 D2/D3/D4] **口径不动、只加标注**：§5.4 表行末尾加「(CI 门禆；作者上限见 §6.2)」、§6.2 (Rust) 「模块规模」行末尾加「(写作规范，不是 CI 门禆)」、`AGENTS.md §5.3` 同步加标注。阈值数字本身**一个都未动**；改的是**阈值的角色说明**。
- [2026-09-18][DECISION][src:ADR-0033 D3 隐含推輛] **§6.2 (Rust) 不拆到新节、原位置保留、只加同行说明**：§6.2 是**Rust 语言专项规范**的一部分（诺多语言不共享「写作规范」这个概念）；独立成节会造成「谁才是写作规范」的难以回答问题。**少改动 = 少拖雲**（本就是 PL-017 裁决重复提醒的同一件事）。

## 2026-09-18 追加（M5：MIT OR Apache-2.0 落地）

- [2026-09-18][DECISION][src:M5 裁决实施] **仓库许可证为 MIT OR Apache-2.0 双许可证**：**`LICENSE`** 重写为双许可证（MIT 全文 + Apache-2.0 全文，顶部明确“事人可任选一」、顶部 1 行干净指向合规源）；**`NOTICE`** 新增（10 行，Apache-2.0 §4(d) 硬要求“Derivative Works 必须在 NOTICE 里带起归属”）。选型论据：Rust 生态中 tokio / serde / hyper 都走这个双许可证，**“推荐不强制、但留出专利授权选项”**是经典 dual 模式。**不改 `deny.toml`**：其 `licenses.allow` 表列表中已包含了 `"MIT"` + `"Apache-2.0"`（ADR-0025 D1 机械化的“许可证名单”顶多余走上路子，而不是“项目用哪个就列哪个”）。上线后、仅需要在第一个引入的第三方依赖里选择 `MIT` 或 `Apache-2.0` 作为其许可证名即可。
- [2026-09-18][DECISION][src:M5 隐含推輛] **`NOTICE` 文件是 Apache-2.0 §4(d) 的硬要求“必须带”项—— 不是选项**：未来任何派生工作只要分发 `LICENSE` 中的任伀部分子集，必须同时携带 `NOTICE` 文件。这是 Apache-2.0 的硬约束——**不是 MIT 要求的、也不是选择性的**，是选了 Apache-2.0 那一叶就能报走的。


## 2026-09-20 追加（更正：line 75-79 + 81-85 重复 ADR-0033 DECISION）

- [2026-09-20][DECISION][src:TASK-069 audit cleanup] **decisions.md line 75-79 与 line 81-85 内容逐字重复**（两个 `## 2026-09-18 追加（ADR-0033，Accepted）` 段 + 重复 3 条 DECISION 条目）。**根因**：2026-09-18 batch 追加时复制粘贴失误。**处置**：按 MEMORY.md "只追加不改写" 规则，**新增此更正行**而非删原行；原 line 75-79 + line 81-85 全部保留（事实记录 = 当时确实写了两遍）。**xtask card-check 不报**（双 DECISION block 不在 card-check 判据 ①②③④ 范围内；判据 ⑤「编号唯一性」= ADR 编号唯一性，与 DECISION block 重复不同 = 不报）。**本卡实质修改**：0（仅追加更正行）。
- [2026-09-24][DECISION][src:ADR-0038] **存储迁移注册表**：`crates/storage` 不再拥有「全部表的迁移清单」，改为**只提供机制**（`Migration` / `MigrationSet` / `MigrationSetError` + `Database::open(paths, clock, &MigrationSet)`）；每个**拥有表**的 crate 用 `include_str!` 内嵌自己的迁移 SQL 并公开 `pub const MIGRATIONS: &[Migration]`（`crates/storage/migrations/0001_init.sql` → `assistant_storage::MIGRATIONS`；`crates/audit/migrations/0002_audit_logs.sql` → `assistant_audit::MIGRATIONS`）；应用侧在**唯一装配点**合并后开库（本轮 = `crates/audit/tests/common/mod.rs`，正式落点 = Host 装配 TASK-019~028）。**加一张表 = 只改自己那个 crate**（TASK-013 的 DRIFT-013-1 根因 **PL-046** 据此闭环）。**D5 = 删 `pub const SCHEMA_VERSION`**（注册表下它必然说谎），替代 = `MigrationSet::expected_version()`；这是**删公共 API**（漂移 ⑫），由本 ADR 授权。**D4** = 版本号分配以 `docs/storage-design.md` §3.4 登记表为 SSOT，装配时 `register()` 拦重号、`validate()` 拦缺号；「xtask 扫描迁移文件名」这条机器化护栏**本轮不做**（要动 gov §5.4 规则计数 + ADR-0025/0030 口径，属 TASK-015 地盘）→ 记 **PL-047**。落地卡 = `tasks/TASK-202-storage-migration-registry.md`。

## 2026-09-24 追加（ADR-0039 / ADR-0040，Accepted）

- [2026-09-24][DECISION][src:ADR-0039] **任务结束时的状态同步契约**：**每张卡 Done 时，必须在同一 PR 内同步 `PLAN.md` 的「当前状态」块 4 行 + `README.md` 的三处（状态行 / `## 当前阶段` / `## 最近进展`）**，**无阶段变化也必须改日期并追加「最近进展」**（沉默跳过 = 违反铁律 1）。**D4** = `PLAN.md` 仍是 Orchestrator 所有，但 Implementer **被授权**在卡 Done 时代写「当前状态」块（其余段落仍只读）；**D5** = `README.md` 的可写面从「仅状态行」扩到三处。**D3** = 可机器校验的部分（① `PLAN.md` 更新日期不得早于 `LEDGER.md` 末行日期；② `README.md` 必须有 `> 状态：` 行且含当前阶段名）归 **TASK-015** 的 `check-ledger`，先 Warning。**明确不做自动写回**（ADR-0030 选项 1 已否决：掩盖作者意图）。**根因** = DRIFT-202-2：`PLAN.md` 长期停在 TASK-012 开工前，因为 §11.1 的表里**没有它**、且它被标为 Orchestrator-only → **无人可写的文件必然腐化**。
- [2026-09-24][DECISION][src:ADR-0040] **`audit_logs` 列语义去重 + 显式链序**：列集合由 `(id, prev_hash, …, hash)` 改为 `(sequence, id, prev_hash, ts, actor, task_id, step_id, event_type, detail_json)`。**D2** = `sequence INTEGER PRIMARY KEY AUTOINCREMENT` 是显式链序（单调 + 永不复用已用号），链尾查询与读取顺序都按它 → **PL-043 闭环**（`VACUUM` 再也不能影响链序）；**D3** = `id TEXT NOT NULL UNIQUE` = 本条 `self_hash`，删掉与它完全同义的 `hash` 列 → **PL-045 闭环**；**D4** = 迁移 `0003_audit_logs_semantics` **重建表**（create new → `INSERT ... SELECT ORDER BY rowid` → `DROP` → `RENAME` → 重建索引与两个 append-only 触发器），**`0002` 一字不改**（checksum 记账）；**D6** = 本 ADR 授权更新**架构 v2 §15.1 line 2594** 的 `audit_logs(...)` 列清单（改 DB schema 属漂移 ③ + 改架构已决事项 ④，本 ADR 即授权）。**选项 4（UUIDv7 代理键）被否决**：引 `uuid` 依赖（漂移 ①）且时间有序 ≠ 链有序。落地卡 = `tasks/TASK-203-audit-log-column-semantics.md`。

## 2026-09-24 追加（ADR-0041，Accepted）

- [2026-09-24][DECISION][src:ADR-0041] **计划文件的「可写面」= 只允许改「完成状态」与「当前进度」**：人类 2026-09-24 chat 澄清「禁止改 plan.md」的确切含义 —— **禁止**改原有 plan 的**排期**与**各点要做的内容**；**必须**实时更新「是否完成」的状态（做完的在条目前打勾，按该文件既有定义，未必是真的 `✅`），且**打勾不得改动该条目的正文**；若有「当前状态 / 当前进度」块，该块内容也必须跟着进度**实时更新**。**D1** = 允许写：完成标记 / 「当前进度」块 / 新增任务行；**D2** = 禁止写：排期与周期、批次顺序、条目正文（名称、write scope、验收要点、依赖、预估）、已在跑的排期时刻；**D3** = 标记只加在**行首**，要改正文走 DRIFT + 人类裁决；**D4** = 状态更新与产生该状态的提交**同批**（延续 ADR-0039 D1/D2 的「同一 PR 内」）；**D5** = 本 ADR 授权修订 `AGENTS.md` §8 write scope 表 + §11 + `docs/overnight-automation-charter.md` §3 第 3 条。**根因** = 章程 §3 黑名单禁止夜间 agent 改 `PLAN.md` / `plans/*`，与 ADR-0039 D1/D2 冲突 → 夜间 agent 做完卡到「同步进度」必撞黑名单 → **「按计划自动逐卡推进」在机制上无法完成**（本次调查实测）。**选项 1（维持全只读）与选项 2（全可写）均否决**：前者把正常工作挡死，后者等于让执行者改考核标准。

## 2026-09-24 追加（ADR-0042，Accepted）

- [2026-09-24][DECISION][src:ADR-0042] **能力命名三分**：**`CapabilityCatalog`** = 稳定能力标识目录（`protocol/capability-matrix/capability-1.0.json`，schema `title` 改名）；**`CapabilityMatrix`** = **运行时探测结果**（架构 v2 §13.1.2，Rust `assistant_platform_api::CapabilityMatrix`，**保留原义、现存引用一处不改**）；**`CapabilityEntry`** = 单条能力的风险/审批元数据（策略引擎输入，TASK-021）。**D4** = 目录名 `protocol/capability-matrix/` **不改**（codegen 与 `verify-schemas` 的硬编码路径）。**D5** = 授权改 schema `title` + spec 名称澄清段。**根因** = PL-064：同一个名字在 schema / spec / 代码三处指两件事，读契约的人会把「目录」当「探测结果」。**选项 1（靠上下文区分）与选项 2（改 Rust 类型名）均否决**：前者已被 PL-064 证明会造成误读，后者要动 SSOT 且「矩阵」对探测结果更贴切。落地实测：codegen 的 `render_capability()` 不读 `title`、`verify_schemas` 不校验 `title` → 零代码影响。
