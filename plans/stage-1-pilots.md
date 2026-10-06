# 阶段 1 — 三试点闭环（Notepad → Paint → Edge/Chrome）

> 周期 10~12 周　状态：**进行中**；阶段 1 的当前进度与下一张卡看 `PLAN.md` 当前状态块；卡级进展看 `LEDGER.md`。
> **TASK-053 ✅（2026-10-07）**：静态注入靶页 fixture 落地 —— visible / hidden / aria-hidden / HTML comment / fake system / meta 六类 marker，5 条正常产品提取记录；显式离线与 CSP 边界；PR #260 / merge `79b5566`。
> **TASK-050 ✅（2026-10-07）**：ADR 0007 三档出域策略落地 —— `crates/dlp` 应用覆盖替换默认、内容类型覆盖只收紧、egress-destination 白名单默认拒绝、`local_only` 无本地模型显式失败、策略变更返回可审计记录；专项 22 passed；PR #258 / merge `51bbb27`。
> **TASK-244 ✅（2026-10-06）**：PL-022 机器派生计数收口 —— gov §5.4 表格行数为 hygiene 总数 SSOT；gov §5.1 与 ci.yml 的 # gov-gate 标记集合一致，缺失 / 重复 / 额外 / 不可解析均 exit 1。
> **TASK-042 ✅（2026-10-06）**：ADR-0074 冻结 `visual_assert` 扁平结构化形状（`field` + `op` + 具名容差 + `confidence_min`）与 pHash / dHash 64-bit 口径；`crates/verify/src/visual/**` 零依赖落地 21 个专项测试；接入既有 `Postcondition` / `Observation` 另立卡（`DRIFT-042-1` / `PL-110`）。
> **TASK-245 ✅（2026-10-06）**：2026-10-06 四连发审计收口 —— `LEDGER.md` 里 TASK-041 的 merge-hash 行原重复 5 次且插错位置，去重为 1 条（404 → 400 行、重复行组归零）；`PL-111` 落点 = `DRIFT-041-2`（storage sink 待裁决）+ ADR-0074 号冲突 + `DRIFT-042-1` / `PL-110`。
> **TASK-041 ✅（2026-10-06）**：**拆分 A + 拆分 B** 落地 —— ADR-0073 冻结「像素遮挡归平台层 / `NeverPersist` 不保留 `ImageRef` / 滚动清理有界」，`crates/capture` + `crates/dlp/src/redact` 零依赖纯逻辑（15 专项测试）；ADR-0076 落地 Windows GDI `PrintWindow` 单窗口截图 + UIA `IsPassword` 像素遮挡 + `ImageBlobSink` 注入（binary `StorageBlobSink` 落盘并核对内容地址），`DRIFT-041-2` / `PL-112` 闭环。
> **TASK-247 ✅（2026-10-06）**：`visual_assert` 接进后置断言引擎（ADR-0077）—— 新增 `Postcondition::VisualAssert` 与 `evaluate_postcondition_with_visual` / `verify_postconditions_with_visual` / `verify_postconditions_with_receipt_and_visual`（既有签名不变，委托 `None`）；图像作为并列参数传入，不进可序列化的 `Observation`；无图 / 低置信一律 `NotEvaluable`；5 个契约用例。`PL-110` / `DRIFT-042-1` 闭环。
> **TASK-248 ✅（2026-10-06）**：`PL-023` 作废（人类裁决）+ 顶层目录白名单的指向冲突消解 —— ADR-0078 裁定**不设立 `scripts/`**（脚本归 `tools/`）、`docs/nightly/logs/` 亦不设立；附表变更规则第 3 条与 ADR 登记表 0069 行的「由 PL-023 决定」已改指 ADR-0078；不动白名单表，不改 ADR-0069 正文。
> **TASK-249 ✅（2026-10-06）**：`visual_assert` 图像来源与运行时接线 —— ADR-0079 新增 `ObservationCollector::observe_visual` 默认入口；生产 `StorageVisualObservationCollector` 只读信封里的 blob 元数据，经装配 `storage` 读取 BGRA、按固定 Rec.709 整数口径转灰度，再走 `verify_postconditions_with_receipt_and_visual`；运行时专项 8/8、来源专项 7/7。
> **TASK-225 ✅（2026-10-03）**：Host 装配层已用共享 `TargetLeaseRegistry` 接上合成输入 exclusive lease；key / pointer-shaped 输入冲突稳定 `Transient`，成功/失败都释放，只读路径不变；`PL-101` 闭环。TASK-040 正文 DoD 复选框仍因正文只读未勾选。
> **TASK-226 ✅（2026-10-03）**：PL-104 / PL-105 闭环；ignored 真机用例以异步互斥串行化 fixture 启动，`production_root.rs` 拆分后 3 个文件均 <600 行且断言零放宽。
> **TASK-227 ✅（2026-10-03）**：状态行与停车位收口 —— TASK-040 / TASK-225 的状态行按实更正为 Done；`PL-103` 给出可直接粘贴的 spec 改法（已由 TASK-229 落笔）；`PL-106` 开卡 TASK-228（Ready，待派单）
> **TASK-229 ✅（2026-10-03）**：`PL-103` 落笔 —— `docs/spec/runtime-execution.md` §3 改为以 `RESERVED_RUNTIME_TOOLS` 为唯一事实源（不再手写个数与名单）；`docs/memory/pitfalls.md` 落「收口类提交必须逐张比对状态行与 LEDGER」四步硬规则；轮次 A 的残留目录已清理（先清只读位再删）
> **TASK-228 ✅（2026-10-03）**：真机 `#[ignore]` 四个用例改为覆盖写结构化记录（`pass|skip|fail` + 非空原因 + 测量值），`tools/acceptance-report` 严格解析并区分全 pass / 含 skip-fail / 非法记录；真机一轮 **4 pass / 0 skip / 0 fail**，`PL-106` 闭环。
> **TASK-224 ✅（2026-10-04）**：ADR-0066 Accepted —— `xtask write` 复用 ADR-0028 协作锁，Windows 上做 `share_mode(0)` 占用探测与有界退避；实际写句柄只共享读取，持锁读取实测不阻塞；目标占用超时退出码 5、无残留锁/进程，`PL-107` 闭环。
> **TASK-230 ✅（2026-10-04）**：storage 新增 `0005` 迁移与 `conversations` / `conversation_messages` 持久化 API；真实关闭 / 重开 SQLite 后逐字段一致，缺失会话、非法父子关系与 revision 跳号显式失败，`PL-092` 闭环。
> **TASK-231 ✅（2026-10-04）**：ADR-0067 删除混合 DPI 收敛启发式，`pointer_action` 起点与 `DragTo` 释放点各自显式携带 `CoordinateSpace`；混合 DPI 归属 6 passed、跨屏拖拽双 scale 1 passed，未知设备 / DPI 不一致 / 越界均显式失败，`PL-074` 闭环。
> **TASK-232 ✅（2026-10-04）**：四轮自动化（TASK-228 / 224 / 230 / 231）审计收口 —— 修正 TASK-224 滞后状态行、补强「状态行比对」硬规则（`正文只读 ≠ 状态行只读`；PR 贴比对为必做）、把 storage 轮的「生产 `SessionStore` 适配器仍缺」转成 `PL-108`
> **TASK-233 ✅（2026-10-04）**：生产装配接入 storage 会话持久化 —— 新增 `StorageSessionStore` 适配器（`SessionStore` 三方法经装配层 `DatabaseHandle` 调 TASK-230 记录原语）与 `with_storage_session_store()` 开关，生产 `production.rs` / `main.rs` 两处不再注入 `MemorySessionStore`；适配器级跨重开逐字段一致、负向四类显式失败、装配级关库重开读回均取证，`PL-108` 闭环
> **TASK-234 ✅（2026-10-04）**：ADR-0068 / 0069 Accepted 后补齐 gov §5.4 最后两条 hygiene 规则 —— 重复代码（token shingle + 包含度 ≥ 80% Warning）与顶层目录 ADR 白名单（未登记 Error）；`hygiene` 13/13、0E/95W，`--list-deferred` hygiene 未实现项为 0，`PL-060` 闭环
> **TASK-235 ✅（2026-10-04）**：ADR-0070 将 `RoleAndParent` 的父候选限定为链内子树作用域；顶层解析过滤被 `parent_id` 引用的候选，父命中 / 子缺失显式 `TargetNotFound`，只有 scope 候选的链在碰 COM 前 `ToolInvalidArgs`，`PL-094` 闭环。
> **TASK-236 ✅（2026-10-04）**：replay 完整版完成 —— Recording v2 真实 UIA 树快照序列、expected-vs-actual 树级 diff、core suite 与两类负向 fixture 收口；`--list-deferred` 的 replay 遗留清零。
> **TASK-237 ✅（2026-10-04）**：三连发自动化（TASK-234 / 235 / 236）事后独立审计收口 —— `PL-062` 闭环（replay 完整版交付后 `AGENTS.md` §6 与本机实测一致，无需改文件）、孤儿自动化目录删除（1 文件 870 B）、`TASK-235` 状态行归一；三轮产物本机重跑全绿，零产品代码改动。
> **TASK-238 ✅（2026-10-04）**：ADR-0071 冻结截图管线 / 出域脱敏的 crate 边界 —— 新增 `crates/capture`（平台无关窗口截图管线，唯一原语 = `platform/api` 的 `WindowProvider::capture`）与 `crates/dlp`（出域策略 + 脱敏 + 截图遮挡）两个零第三方依赖骨架；架构 v2 §3 布局追加 `capture/`，根 `Cargo.toml` 未改（`crates/*` glob 自动纳入）；`PL-102` 闭环，1b 的 TASK-041 / 042 与 1c 的 TASK-050 前置解锁。
> **TASK-239 ✅（2026-10-05）**：停车位存量复核收口 —— 新增 `docs/audits/parking-lot-review-2026-10-05.md`，逐条复核仍开放或存在部分关闭余项的条目；`docs/PARKING_LOT.md` 仅追加 48 行结论，原行未改；本轮补记 13 条已闭环/取代，保留 35 条待治理或人工裁决。
> **TASK-240 ✅（2026-10-05）**：任务卡状态行全量清扫 —— 18 张台账已 Done 但卡面滞后的卡按实更正，只改 `- 状态：` 行；`TASK-105` 因 L0/L1 回滚项未闭环且台账最后状态仍为 InProgress 保持原状。
> **TASK-241 ✅（2026-10-05）**：派生值指针化清扫 —— ADR-0072 冻结「动态派生值只指向唯一事实源」；`AGENTS.md` / `MEMORY.md` §1 / 本文件当前进度句 / 派单文档的行数副本已指针化，`PL-035` 闭环，`PL-022` 机器派生余项保留。
> **TASK-242 ✅（2026-10-05）**：三连发审计收口 —— 补正 TASK-076 的滞后状态行（LEDGER L88 早已声称 Done，commit `0f05ef7` 实际未改）；TASK-002 卡面 / PLAN / LEDGER 三方不一致开成 **PL-109**（待裁决）。
> **TASK-243 ✅（2026-10-05）**：PL-109 裁决收口 —— 人类 2026-10-05 定 **TASK-002 = Done**；`PLAN.md` 阻塞项 / `MEMORY.md` §1 / `LEDGER.md` / `docs/PARKING_LOT.md` 全部对齐，SPIKE-A banner 与 2026-09-30 审计快照加前向标注；历史正文不改。
> 依据：架构 v2.2 §20.2、feasibility v1.1 §3.0/§3（P1/P3/P5 档案）
> 全局拆解见 `docs/wbs-overview.md`；每张卡在开工前由 Orchestrator 按 gov §3.2 模板展开为 `tasks/TASK-NNN-*.md`

## In scope（冻结）

三个子阶段串行推进，每个子阶段有自己的 DoD：

| 子阶段 | 周期 | 目标 | 通道 |
|---|---|---|---|
| **1a** | 4~5 周 | 全部基础设施 + Notepad 3 个任务闭环 | L3 UIA + L1 文件契约 |
| **1b** | 3 周 | Paint 3 个任务闭环 | L4 坐标注入 + L5 视觉验证 |
| **1c** | 3~4 周 | Edge/Chrome 3 个任务闭环 + 安全底座验收 | L1 CDP + L3 UIA 外壳 |

## Out of scope（做了算漂移）

macOS/Linux 任何代码；Excel/Word/Photoshop Adapter；外部 MCP server 加载；WASM 插件；无人值守执行；技能市场与插件签名；向量检索记忆；股票类软件任何通道；图表可视化面（仅预留接口）；提权 Host（`automation-host-elevated`）；EgressProxy 独立进程（本阶段内联在 Core）；录制器（Recorder）；多 agent 并行执行任务。

## 阶段 1 DoD（不达标不进入阶段 2）

- [ ] 9 个任务连续 10 次运行成功率：Notepad ≥ 90%、Paint ≥ 75%、Edge ≥ 75%
- [ ] **静默失败 = 0**（任何"报告成功但实际未生效"都视为致命缺陷）
- [ ] 撤销成功率 ≥ 95%，撤销冲突 100% 被检测
- [ ] **注入靶页：0 次执行页面内指令**（安全一票否决）
- [ ] L3 不可逆动作 100% 经人工确认，且计划 UI 正确标注 point-of-no-return
- [ ] 四类异常可安全终止或恢复：断网、Core 崩溃、用户中途操作、目标程序退出
- [ ] 所有写操作有 postcondition 且被验证；所有失败可从时间线定位原因
- [ ] CI 门禁全绿：**gov §5.1 与 `.github/workflows/ci.yml` 的现行清单一致**（含 arch test、schema 校验、类型同步、hygiene 规则、回放基准、spike-deny、desktop-ui 检查）
      > 口径由 **ADR-0025 D4** 统一（2026-09-18，PL-001 关闭）。原写「CI 14 项门禁」是过时表述。
- [ ] 覆盖率：workspace ≥ 75%，`core`/`policy`/`task-engine` ≥ 85%
- [ ] 每个 crate 有 README（职责/边界/**不变量**/已知限制）
- [ ] `MEMORY.md` 已回填阶段 1 新增的 FACT/PITFALL/REJECTED
- [ ] 阶段末对齐审计（gov §7.2）完成且偏差项已裁决

---

## 子阶段 1a：基础设施 + Notepad（TASK-011 ~ TASK-039）

### 批次 A1　地基层（**必须串行**，后续全部依赖）

| 卡号 | 标题 | write scope | 依赖 | 预估 | 验收要点 |
|---|---|---|---|---|---|
| **011 ✅** | `protocol` crate：schema 单一事实源 + Rust/TS 代码生成 + `ErrorCode` 枚举 | `protocol/**`、`crates/protocol/**`、`xtask/src/codegen*` | 001 | M | `codegen --check` 在 CI 通过；ErrorCode 覆盖 v2 §8.7 全部分类；**TS 类型 100% 生成，无手写重复** |
| **012 ✅** | 存储层：SQLite(WAL) + 迁移框架 + 核心表 + 内容寻址 blob（zstd/去重） | `crates/storage/**` | 011 | M | Spike H 的性能预算全部达标；孤儿 blob 可 GC；DB/blob 不一致可检测并标 `evidence_missing` |
| **013 ✅** | `audit`：追加不可改 + hash chain + ring buffer 批量 flush + `durability` 可配 | `crates/audit/**` | 012 | S | 无 UPDATE/DELETE 路径；篡改可被 hash chain 检出；batched 摊销 < 1 ms/条；`immediate` 模式可用 |
| **014 ✅** | `secrets`：OS keychain 封装（DPAPI/Keychain/Secret Service） | `crates/secrets/**` | 011 | S | 密钥不落盘明文、不入日志、不入 prompt；`zeroize` 生效；密钥访问被审计 |
| **015 ✅** | `xtask`：hygiene + arch test + verify-schemas + replay 骨架 | `xtask/**`、`crates/core/tests/arch*` | 011 | M | **arch test 能拦住 core→platform/windows 的依赖**；hygiene **13** 项检查全部生效（gov §5.4；口径见 **ADR-0025**，原「12 项」为笔误） |

> ★ 015 必须早做：它是后续所有卡的护栏。护栏晚于代码 = 漂移已经发生。
>
> **2026-09-24 顺序澄清（Orchestrator 代行）**：本表头写「必须串行」，但依赖列里只有 `013 ← 012` 是真串行；
> `014 ← 011`、`015 ← 011` 在 011 完成后即解锁，且三者 write scope（`crates/storage` / `crates/secrets` / `xtask`）互不重叠。
> **实际推进顺序仍按 011 ✅ → 012 → 013 → 014 → 015** —— 瓶颈是**人类审阅带宽**（AGENTS.md §3：并行度 ≤ 3、一会话 1~2 张卡），
> 不是依赖本身。
> ✅ **PL-037 已闭环（2026-09-24，人类裁决选项 ③ → `tasks/TASK-201-core-crate-skeleton.md`）**：`crates/core` 骨架已**提前落地**
> （`crates/core/{Cargo.toml,README.md,src/lib.rs}`；零第三方依赖、零 `pub` 项），
> 「`crates/core` 要到 TASK-028 才存在」这个硬阻塞已消除 → **015 现在可开工**。
> ✅ **2026-09-24 TASK-015 已把这段语义边界消掉**：`cargo test -p assistant-core arch::` 现在是 **`running 5 tests`**
> （`crates/core/tests/arch_layering.rs`：正向 2 + 负向 3），且 CI 的 `[SOFT #5]` 已**转硬**为 `[HARD #5]`
> （负向验证 = 该文件的三条 `arch::scan_flags_*` 用例，ADR-0019 N1）。同批把 `check-ledger`（gov #16）也转硬。
> ⚠ 硬/软门禁数量不再在本文件手抄；唯一事实源 = `gov §5.1` 与 `.github/workflows/ci.yml` 的步骤名。

### 批次 A2　平台层（011/015 完成后可 2 路并行）

| 卡号 | 标题 | write scope | 依赖 | 预估 | 验收要点 |
|---|---|---|---|---|---|
| **016 ✅** | `platform/api`：统一 trait + `CapabilityMatrix` + `TargetDescriptor` + `NormalizedPoint` + `Fingerprint` 类型 | `crates/platform/api/**`、`protocol/capability-matrix*` | 011 | M | trait 覆盖 v2 §13.1.1 全部方法（含 wait/fingerprint/capability）；**所有方法可取消且有超时** |
| **017 ✅** | `platform/windows`：UIA provider（树快照、selector 链解析、read_text、set_value、edit_text、invoke_action、bounds、fingerprint、window 枚举与状态） | `crates/platform/windows/src/uia/**`、`.../src/window/**` | 016 | L | Spike A 指标在真实记事本上复现；树遍历符合性能预算；**歧义/未找到/无响应三类错误可区分** |
| **018 ✅** | `platform/windows`：合成输入（SendInput）+ 焦点校验 + 坐标归一化（DPI/多屏）+ IME 处理 | `crates/platform/windows/src/input/**`、`.../src/coordinates/**` | 016 | M | 发送快捷键前 100% 校验前台窗口；Spike A2 的坐标精度矩阵全配置 ≤ 2 px；IME 开启时文本写入仍正确 |
| **019 ✅** | `automation-host` 进程 + `ipc`（JSON-RPC/NamedPipe + token + 对端身份校验 + 心跳 + 看门狗） | `apps/automation-host/**`、`crates/ipc/**` | 016 | M | Spike B 的重解析矩阵达标（≥95%）；**element 不出进程**（arch test 校验）；host 崩溃可被检测 |

> **2026-09-26 拆卡登记（ADR-0053 D7；人类 chat「drift-028 的 5 点都按照你的建议做」）**：原 TASK-028 的批次表行按拆卡结果**收窄**，并**新增** 207 / 208 两行；
> 「组装」下沉到 TASK-029（见 A4 表与 ADR-0053 D5）；storage 侧前置卡 **TASK-206** 属治理池，登记在文末「跨阶段治理卡」表（不在本阶段批次表内）。
> 被替换的旧行内容**完整保留**在 `tasks/TASK-028-core-session-context.md` 的记录区与 `LEDGER.md`（只追加，不改写历史）。
> ⚠ **TASK-028 / 207 / 208 共用 `crates/core/**` → 三者必须严格串行**（依赖列已保证顺序），不得并行。

### 批次 A3　内核层（016 完成后可 3 路并行，write scope 天然不重叠）

| 卡号 | 标题 | write scope | 依赖 | 预估 | 验收要点 |
|---|---|---|---|---|---|
| **020 ✅** | `tool-bus`：MCP client(`rmcp`) + in-process server + JSON Schema 校验 + **统一返回信封**（`untrusted`/`truncated`）+ 工具集指纹 + 动态挂载 | `crates/tool-bus/**` | 011 | L | 内部工具经 MCP 表达；信封字段齐全；工具数 > 40 时告警；schema 不合法直接拒 |
| **021 ✅** | `policy`：白名单 + 风险分级 + 参数校验（路径穿越/URL/长度/正则复杂度）+ 规则 DSL v0 + **默认拒绝** | `crates/policy/**` | 011 | L | v2 §12.2 五条示例规则全部可表达；决策是**纯函数**；每个 deny 带 `rule_id` 与可读 reason；策略判定 < 50 µs |
| **022 ✅** | `task-engine`：12 状态机 + Plan/Step DAG + 检查点 + 恢复 + 取消 + 预算 + 看门狗 | `crates/task-engine/**` | 011,012 | L | 状态迁移全部持久化；崩溃后能恢复；**"不确定是否执行过"必须走 NeedsHuman**（禁止猜测） |
| **023 ✅** | `verify`：后置断言引擎（11 种断言）+ 状态指纹 + 幂等判定 + `on_violation` 分派 | `crates/verify/**` | 011 | M | 断言类型齐全；**验证失败绝不返回 ok**；指纹可配置忽略字段（防抖） |
| **024 ✅** | `undo`：可逆性四级 + 锚点（内容快照/影子副本/步数级）+ 回滚剧本执行 + 冲突检测 + incident 上报 | `crates/undo/**` | 012,023 | L | Spike F 三条路径达标；冲突 100% 检测且默认最保守；撤销失败按 incident 处理 |
| **025 ✅** | `lease`：目标租约（exclusive/shared/intent + TTL + 续租 + 用户抢占 + 死锁避免） | `crates/lease/**` | 011 | S | 同目标同刻仅一个写租约；用户操作可强制释放；租约冲突是可读错误 |
| **026 ✅** | `model-gateway`：Provider trait（流式/取消/用量）+ 路由器 + 降级链 + 重试退避 + prompt cache 提示 + 成本计量 | `crates/model-gateway/**` | 011 | L | v2 §11.2 路由规则可配置；**取消能在 1 s 内中止请求**；每步记录 tokens/cost/latency |
| **027 ✅** | `hitl`：审批请求 + 授权范围（四维+TTL）+ 用户接管 + 暂停恢复 + 差异预览数据准备 | `crates/hitl/**` | 021,022 | M | 高风险禁止 persistent 授权；接管后交还需重新同步状态；审批超时行为明确 |
| **028 ✅** | `core`：会话管理 + 上下文管理（树裁剪 / 压缩 / 预算） —— **原 TASK-028 拆卡主卡**（2026-09-26，ADR-0053 D7） | `crates/core/**` | 020~027 | M | arch test 通过（core 只依赖白名单 crate）；上下文预算生效且**丢弃必须显式标注**；会话生命周期 + 消息树可测 |
| **207 ✅** | `core`：Planner（模型输出 → 可校验的 Plan / Step DAG；**复用** `task-engine` 类型） | `crates/core/**` | 022,026,028 | M | 六类负向用例齐全（重复 id / 缺依赖 / 环 / 非法工具名 / 写步骤缺 postcondition / L3 未标 point-of-no-return）；不重定义 `Plan` / `Step` |
| **208 ✅** | `core`：Memory（App Map 加载 + 消费 storage 的 FTS5 检索 API） | `crates/core/**` | 206,028 | M | App Map 四类负向用例（缺失 / 损坏 / 版本不匹配 / 路径穿越）；片段带来源与位置；检索错误**透传** `reason_code`；**Done（2026-09-27，PR #70 / merge `e8d95f2`）** |

### 批次 A4　应用与 UI（028 完成后可 2 路并行）

| 卡号 | 标题 | write scope | 依赖 | 预估 | 验收要点 |
|---|---|---|---|---|---|
| **029 ✅** | 二进制骨架：`apps/agent-core` + `apps/desktop-ui`（Tauri 2 + React + TS + Tailwind）+ capabilities 最小化 + CSP ＋ **Host 装配**（把 `core` 组件与 platform / tool-bus / policy / storage / audit 组装起来 —— 2026-09-26 **ADR-0053 D5**） | `apps/agent-core/**`、`apps/desktop-ui/src-tauri/**`、`apps/desktop-ui/*.config.*` | 028,206,207,208 | L | **webview 零系统权限**；CSP 严格；禁远端内容；IPC 全部走 token；**装配点唯一在 binary**（`core` 不装配） |
| **030 ✅** | UI：审批卡片（含 diff + 来源归因 + 授权范围）+ 执行时间线（含证据与撤销按钮） | `apps/desktop-ui/src/features/approval/**`、`.../timeline/**` | 029 | L | v2 §10.2 全部字段齐备；`app_content` 来源标红且默认拒绝；撤销按钮按可逆性分级禁用 |
| **031 ✅** | UI：元素拾取器 v0（悬停高亮 + 属性面板 + 一键生成 selector 候选链）+ 目标绑定向导 | `apps/desktop-ui/src/features/picker/**`、`.../binding/**` | 029,017 | L | 能对记事本生成 ≥ 3 种候选 selector 并写回 Adapter 草稿 |
| **032 ✅** | UI：策略面板 + **出域三档开关**（含逐应用覆盖）+ Capability Matrix 视图 + 成本面板 | `apps/desktop-ui/src/features/policy/**`、`.../capability/**`、`.../cost/**` | 029,021 | M | 三档可切换且即时生效；降级不静默；状态栏常驻显示当前出域级别 |

### 批次 A5　Notepad 闭环

| 卡号 | 标题 | write scope | 依赖 | 预估 | 验收要点 |
|---|---|---|---|---|---|
| **033** | 靶机应用 v0：`notepad-like`（WinUI/WPF，全部控件有稳定 AutomationId，CLI 可注入故障：元素消失/超时/歧义多匹配/意外弹窗/忙碌） | `fixtures/apps/notepad-like/**` | 001 | M | 故障可脚本化触发；CI 可在 windows runner 上跑 |
| **034** | 录制回放框架 v0：树快照录制 + 离线回放（替代真实平台调用）+ `xtask replay` | `crates/replay/**`、`xtask/src/replay*`、`fixtures/recordings/**` | 017,022 | M | 逻辑层回归可在无真机的 CI 上跑；回放能复现 Spike 采集的真实快照 |
| **035** | Notepad Adapter：`adapter.toml` + `app_map.json` + `selectors/` + `tools/`（read_text/replace_text/save/new_tab/save_as）+ `rollback/` + `interrupts/`（未保存三态对话框默认"取消"） | `adapters/com.microsoft.notepad/**` | 017,020,024 | L | schema 校验通过；**声明 Win11 版本范围**；undo 快捷键显式声明；含 `version_range` 与 known_pitfalls |
| **036** | T1.1：打开文件 → 读全文 → 报告行数与关键词段落（只读） | `adapters/com.microsoft.notepad/tasks/**`、`eval/tasks/notepad/**` | 035 | S | 10 次连续成功率 ≥ 90%；大文件（1 MB）走 L1 文件通道降级并正确标 `truncated` |
| **037 ✅** | T1.2：全文替换「报表」→「报告」+ 保存（含审批 diff、L0 undo + L1 快照、后置断言） | 同上 | 036 | M | 替换计数正确；保存后标题无 `*`；撤销可回到锚点；**歧义时按 `error_if_ambiguous` 报错** |
| **038** | T1.3：新建标签 → 写入 → 另存为到指定路径（跨进程 Shell 对话框） | 同上 | 037 | M | 跨进程对话框解析成功率 ≥ 90%；**已存在文件绝不静默覆盖**（先备份 + 确认） |
| **039 ✅** | 阶段 1a 集成验收：CI 门禁全启用（按 gov §5.1 与 CI workflow 现行清单，ADR-0025 D4） + 9 项 DoD 中 1a 相关项 + 对齐审计 | `.github/workflows/**`、`docs/audits/**` | 033~038 | M | 全部绿灯；审计报告产出；偏差项已裁决 |

### 1a 补救批次 A5-REMEDIATION（2026-09-29 审计后新增）

> 来源：`docs/audits/stage-1a-integration-audit-2026-09-29.md` 的 NO-GO 阻断项。
> 这些卡不是 1b 工作；在它们和复验完成前，`TASK-040` 不得开工。

| 卡号 | 标题 | write scope | 依赖 | 预估 | 验收要点 |
|---|---|---|---|---|---|
| **TASK-102 ✅** | 运行执行链路契约：Planner → TaskEngine → Policy → ToolBus → Host → Verify → Undo | `docs/adr/0056-*`、`docs/spec/runtime-execution.md`、登记文件 | 029、037、038、039 | M | ADR/spec 明确所有权、取消/超时、错误与审计边界；`VerifyOutcome` 强制接线方案 |
| **TASK-103 ✅** | 真实任务执行器：Host 分发、上下文装配与 `VerifyOutcome` 接线 | `apps/agent-core/src/**`、`apps/automation-host/src/**`、`crates/task-engine/**`、`crates/ipc/**` | 102 | L | 一个真实 Plan 可执行；Host 分发闭环；缺验证不得成功；断连/超时/拒绝有负向测试 |
| **TASK-104 ✅** | UI ↔ Core typed IPC 与审批接线 | `apps/desktop-ui/**`、`apps/agent-core/src/**` 的 IPC 适配层 | 102、103 | L | UI 请求可到 Core；审批决策可回传；时间线显示真实验证；非法 IPC fail-closed |
| TASK-105 | Notepad T1.x 真实运行验收与 10 次证据 | `fixtures/apps/notepad-like/**`、`eval/tasks/notepad/**`、`docs/audits/**` | 103、104 | M | T1.1~T1.3 各 10 次；静默失败 0；审批/撤销/恢复证据落盘 |
| **TASK-087 ✅** | CI 硬门禁负向验证与 `check-comments` 落地 | `.github/workflows/**`、`xtask/src/**`、`xtask/README.md`、ADR-0019 登记 | 015、039 | L | fmt/clippy/build canary；`check-comments` 真实现；PL-018 可关闭 |
| **TASK-210 ✅** | UI 与提交质量门禁：Prettier / ESLint / Vitest / commitlint | `apps/desktop-ui/**`、`.github/workflows/**`、`docs/DEPENDENCIES.md` | 039 | L | 新依赖已登记；UI 与 commit 正负门禁入 CI |
| **TASK-211 ✅** | 阶段 1a 复验准备与停车位收口 | `docs/PARKING_LOT.md`、`docs/audits/**`、状态同步文件 | 102~105、087、210 | S | PL-018/056/058 证据矩阵；复验清单；不得提前宣称 1a 通过 |
| **TASK-212 ✅** | 修掉 `check-comments` 首次真跑发现的 9 处真实违规（DRIFT-087-1） | `crates/policy/src/dsl.rs`、`crates/ipc/src/frame.rs`、`crates/platform/windows/src/uia/{actions,tree}.rs` | 087 | S | 只加注释；`check-comments` 0 error；diff 不含可执行语句改动 |
| **TASK-213 ✅** | UI↔Core 真实传输：Core 侧监听端 + UI 侧 client + 事件推送 | `apps/agent-core/src/**`、`apps/desktop-ui/src-tauri/**`、`crates/ipc/**`（仅必要小改）、`.github/workflows/**`、README | 0057（ADR Accepted）、104、103、019 | L | 真实管道端到端；六个 fail-closed 点各有断言具体 ErrorCode 的负向用例；事件推送；断连 2s 内检测 |
| **TASK-214 ✅** | 生产装配根：真实 Host 进程 + Notepad Host handler + 1a Plan 来源 | `apps/agent-core/src/**`、`apps/agent-core/tests/**`、本卡与状态同步文件 | 103、213、**ADR-0058 Accepted** | L | 生产模式可启动；5 个 handler 入 ToolBus；确定性 Plan 来源；靶机 T1.1 干跑（真 UIA + 真 receipt）；fail-closed |
| **TASK-215 ✅** | `notepad-like` 靶机能力扩展 + `com.example.notepad-like` 适配包（**PL-097 闭环卡**） | `fixtures/apps/notepad-like/**`、`adapters/com.example.notepad-like/**`、`apps/agent-core/tests/production_root_uia.rs`、状态同步文件 | 033、035、214、**PL-097 / DRIFT-105-2** | L | 靶机支持打开/保存/另存为/标签页且有稳定 AutomationId；适配包进仓库且干跑改读它；T1.2/T1.3 所需元素可解析；PL-097 可闭环 |
| TASK-216 | 运行时补齐：任务输入绑定 + `hitl`/rollback/verify 步骤 + `tab.new` 观测（**DRIFT-105-3 的落地卡**） | `apps/agent-core/src/**`、`apps/agent-core/tests/**`、`docs/adr/0059-*` 与本卡状态同步文件 | 214、215、**DRIFT-105-3**；**B 片前置 = ADR-0059 Accepted** | L | A 片：任务包能被渲染成合法 Plan（无 `$input.` 残留）+ fail-closed 负向用例；B 片：按 ADR-0059 执行或移出 `hitl`/rollback/verify，并让 `tab.new` 读到 `TabCountText` |
| **TASK-217 ✅** | 运行时任务数据流：前序步骤输出、白名单 `pure`/host operation、布尔 `when` 与可恢复审批（**DRIFT-216-4 的落地卡**） | `apps/agent-core/src/**`、`apps/agent-core/tests/**`、`docs/adr/0061-*`、`docs/spec/runtime-execution.md` 与本卡状态同步文件 | TASK-216、**DRIFT-216-4**；**前置 = ADR-0061 Accepted** | L | T1.2/T1.3 任务包通过数据流校验并在 fake platform 执行到 `Completed`；无授权可暂停、批准后从同一快照恢复；无表达式语言、无公共形状变更 |
| **TASK-218 ✅** | Notepad L0/L1 物理快照创建与回滚执行验收（**TASK-105 撤销链的落地卡**） | `apps/agent-core/src/**`、`apps/agent-core/tests/**`、`fixtures/apps/notepad-like/**`、`eval/tasks/notepad/**`、`docs/audits/**`、`docs/adr/0062-*` 与本卡状态同步文件 | TASK-105、TASK-024、TASK-103；**前置 = ADR-0062 Accepted** | L | 真实创建 L0 fallback digest 与 L1 disk snapshot；T1.2 三条 undo 路径真实取证；缺失/不匹配/用户改动三类 incident；阶段 1a 仍 NO-GO |
| **TASK-219 ✅** | 运行时 pure 步骤与回滚观察字段的真实性修复（**第三轮审计剩余缺陷**） | `apps/agent-core/src/**`、`apps/agent-core/tests/**`、`adapters/com.microsoft.notepad/tasks/t1.1*` | TASK-218、TASK-105 | M | count_lines_and_keyword_paragraphs 真实执行；静默 skip 改 fail-closed；`editor_matches_anchor` 反映真实比较；阶段 1a 仍 NO-GO |
| **TASK-220 ✅** | 资源泄露审计与防护：内存 / 句柄 / PowerShell 子进程（**人类要求的全仓泄露审计；ADR-0063 首个落地卡**） | `crates/platform/windows/src/handles.rs`、`apps/agent-core/src/{notepad_registry,approval_grants}.rs`、`apps/agent-core/tests/**` | TASK-218 | L | 元素表 4096 上限 + `clear_thread_elements()`；anchor 64 / grant 1024 上限与淘汰；`taskkill /T /F` + `wait()` reap；真机 ignored 全套 8 passed |
| TASK-221 | 第二轮泄露审计：cost / secrets / audit / tool-bus / model-gateway / UI | `crates/model-gateway/**`、`crates/secrets/**`、`crates/audit/**`、`crates/tool-bus/**`、`apps/desktop-ui/src/**` | TASK-220 | M | 复查无界增长与未释放句柄；`CostLedger.records` 与 `InMemorySecretStore` 加上限；阶段 1a 仍 NO-GO |
| TASK-222 | 第三轮泄露审计：真机长跑收敛测量（**ADR-0063 首个执行实例**） | `eval/tasks/notepad/**`、`docs/audits/leak-audit-round-3-*.json` | TASK-220、TASK-221、ADR-0063 | M | 30 次真实 UIA 采样进程数/句柄/工作集；进程数收敛；agent 进程内测量留待下一轮；阶段 1a 仍 NO-GO |
| **TASK-223 ✅** | L1 文件通道 `read_utf8_prefix` 作为 binary 层保留 `host_service`（**前置 ADR-0064**） | `apps/agent-core/src/**`、`apps/agent-core/tests/**`、`adapters/com.microsoft.notepad/tasks/t1.1*`、`docs/adr/0064-*` | TASK-219、**ADR-0064 Accepted** | M | 1MB 文件走文件通道降级；UTF-8 前缀不截断；`truncated` 显式；不改 `l1_file` 语义；阶段 1a 仍 NO-GO |
| **TASK-225 ✅** | Host 合成输入目标租约独占（**跨层补 PL-101；1b 桥接**） | `apps/agent-core/src/**`、`apps/agent-core/tests/**`、`apps/agent-core/Cargo.toml` | TASK-018、TASK-025、TASK-040 真机校准 | M | `key_action` / pointer-shaped input 发送前取得窗口 exclusive lease；两任务冲突稳定 `Transient`；成功/失败都释放；只读路径不变 |
| **TASK-226 ✅** | 测试卫生清理：ignored 真机用例串行化 + `production_root.rs` 拆分 | `apps/agent-core/tests/**`、`tasks/TASK-226-*`、状态同步文件 | TASK-223 | M | PL-104 并行红灯消失；PL-105 每个测试文件 <600 行；断言不删不放宽；阶段 1a 仍 GO |
| **TASK-227 ✅** | 状态行与停车位收口（TASK-040/225 状态行 + PL-103 改法 + PL-106 开卡） | 状态同步文件 | TASK-040、TASK-225、TASK-226 | S | 两张卡状态行与证据一致；PL-103 有可粘贴改法；TASK-228 已登记；零代码改动 |
| **TASK-228 ✅** | 真机验收结果结构化输出（PASS / SKIP / FAIL 可机器区分；源 = `PL-106`） | `crates/platform/windows/src/input/**`、`xtask/src/**` 或 `tools/**`（二选一） | TASK-040、TASK-220 | M | skip 不得等同 pass；汇总入口拒绝未知/缺失记录；记录有界且不进版本库 |
| **TASK-229 ✅** | `PL-103` spec 落笔 + 状态行滞后硬规则 + 残留目录清理（人类逐条授权） | `docs/spec/runtime-execution.md` §3、`docs/memory/pitfalls.md`、`docs/PARKING_LOT.md` | TASK-217、TASK-219、TASK-223、TASK-227 | S | spec 指向唯一事实源；pitfalls 落可执行四步；残留目录删除有前后证据；代码零改动 |
| **TASK-224 ✅** | 唯一写入通道 + 文件占用探测（`xtask write`，ADR-0066） | `xtask/src/**`、`docs/adr/0066-*` | ADR-0028、ADR-0063、**ADR-0066 Accepted** | L | 独占探测 `share_mode(0)`；有界退避与退出码 5 放弃；读路径不阻塞实测；`apply_patch` 不强制纳入 |
| **TASK-232 ✅** | 四轮自动化审计收口（TASK-224 状态行 + 规则补强 + PL-108） | 状态同步文件 | TASK-224、TASK-228、TASK-230、TASK-231 | S | 状态行与 LEDGER 一致；规则补强落 pitfalls；`PL-108` 登记；零代码改动 |
| **TASK-233 ✅** | 生产装配接入 storage 会话持久化（`SessionStore` 适配器；闭环 PL-108） | `apps/agent-core/src/**`、`apps/agent-core/tests/**` | TASK-028、TASK-230、`PL-108` | M | 适配器三方法经 storage 记录原语；生产两处不再注入内存 store；跨重开 + 负向四类 + 装配级证据；trait 与 core/storage 公共形状不变 |
| **TASK-234 ✅** | 补齐 gov §5.4 最后两条 hygiene 规则（闭环 PL-060） | `xtask/src/**`、`docs/adr/**`（0068/0069/白名单） | PL-060、ADR-0025 | M | ADR 先行；两条规则正负样本；hygiene 13/13 且 0E；`--list-deferred` hygiene 未实现项为 0 |
| **TASK-237 ✅** | 三连发自动化审计收口（PL-062 闭环 + 孤儿目录清理 + TASK-235 状态行归一） | 状态同步文件 + `docs/PARKING_LOT.md`（追加） | TASK-234、TASK-235、TASK-236 | S | `PL-062` 闭环；孤儿目录删除有前后证据；状态行与 LEDGER 一致；零产品代码改动 |
| **TASK-238 ✅** | `capture` / `dlp` 边界 crate 骨架（闭环 PL-102，解锁 1b / 1c） | `crates/capture/**`、`crates/dlp/**`、`docs/adr/0071-*`、架构 v2 §3 | PL-102 | S | ADR-0071 Accepted；两个零依赖骨架编译 + 门禁全绿；架构 §3 加 `capture/`；原 PL-102 行未改 |

---

## 子阶段 1b：Paint（TASK-040 ~ TASK-047）

| 卡号 | 标题 | write scope | 依赖 | 预估 | 验收要点 |
|---|---|---|---|---|---|
| **040 ✅** | 合成输入完善：拖拽（按下-移动-释放的原子性与租约独占）+ 校准流程（首次显示器组合点击已知元素验证命中） | `crates/platform/windows/src/input/**` | 018 | M | 拖拽期间禁止其他写租约；校准失败则**禁用坐标通道**并上报 |
| **041 ✅** | 截图管线：窗口截图 + 脱敏（密码框/正则命中区域遮挡）+ 滚动清理 + 隐私模式（不保存截图） | `crates/capture/**`、`crates/dlp/src/redact*` | 017 | M | 只截目标窗口而非全屏；脱敏规则可配；存储上限与 TTL 生效 |
| **042 ✅** | 视觉验证：容差断言 + 感知哈希(pHash/dHash) + `visual_assert` 断言类型 + `confidence_min` | `crates/verify/src/visual/**` | 023,041 | M | 能稳定区分"画对/画错/没画上"，误判率 < 5%；**低置信结果必须标注且不得单独作为成功依据** |
| **043** | Paint Adapter：工具选择/颜色/图层（UIA）+ 画布坐标动作 + 像素快照回滚 + 缩放与滚动的坐标换算 | `adapters/com.microsoft.paint/**` | 040,042 | L | 画布坐标 ≠ 屏幕坐标的换算正确；工具状态"设置后回读"验证；图层前置条件生效 |
| **044** | T3.1：新建画布 → 选矩形工具与颜色 → 指定画布坐标画矩形 → 截图验证形状与颜色 | `adapters/com.microsoft.paint/tasks/**`、`eval/tasks/paint/**` | 043 | M | 10 次成功率 ≥ 75%；坐标命中误差 ≤ 2 px；容差断言生效 |
| **045** | T3.2：打开 PNG → 读尺寸与缩放 → 区域标记 → 另存为新文件 | 同上 | 044 | M | 不覆盖已有文件；缩放状态下坐标仍正确 |
| **046** | T3.3：绘制 → 用户点撤销 → **像素级验证回到快照** | 同上 | 045 | S | 像素级回滚确认；undo 粒度实测结论回填 `MEMORY.md` |
| **047** | 阶段 1b 集成验收 + Adapter 复用度检查（Paint 是否被迫改了平台层？改了 → 记 ADR） | `docs/audits/**` | 040~046 | S | 若 Paint 需要修改 `platform/api` trait → 必须 ADR（这是抽象是否正确的关键信号） |

---

## 子阶段 1c：Edge/Chrome + 安全底座（TASK-048 ~ TASK-058）

| 卡号 | 标题 | write scope | 依赖 | 预估 | 验收要点 |
|---|---|---|---|---|---|
| **048** | CDP provider：连接管理 + DOM 读写 + 点击 + 导航 + 截图 + 下载目录控制 + load/networkIdle 判定 | `crates/platform/windows/src/cdp/**`、`crates/platform/api/src/cdp*` | 016 | L | Spike G 指标复现；**依赖登记进 `docs/DEPENDENCIES.md`** |
| **049** | 专用 profile 管理：自定义 `--user-data-dir` 生命周期 + 独立窗口标识 + **禁止复制用户 profile** 的硬约束 + 首次登录引导 | `crates/browser-profile/**` | 048 | M | 136+ 约束正确处理；代码审查确认无任何 Cookie/凭据复制路径 |
| **050 ✅** | `dlp`：三档出域策略（`local_only`/`redacted`/`full`）+ 逐应用与逐内容类型覆盖 + 脱敏规则 + endpoint 白名单 + **降级不静默** | `crates/dlp/**` | 014,026 | L | 选 `local_only` 而无本地模型时**明确报错**而非改走云端；策略变更写审计 |
| **051** | 污点追踪 + 权限衰减：`untrusted` 内容引入后标记生效，宽授权降级为 `once`，高风险 deny | `crates/policy/src/taint**`、`crates/core/src/session**` | 021,020 | M | v2 §12.4 四层中第 2/3 层生效；污点只能由 SessionManager 清除（**不变量写进 README**） |
| **052** | 来源归因：每个动作记录 `instruction_origin`（user_request/plan_derived/app_content/tool_suggestion）+ UI 展示 | `crates/core/src/origin**`、`apps/desktop-ui/src/features/approval/**` | 027,030 | M | `app_content` 来源在卡片上标红且默认拒绝 |
| **053 ✅** | 注入靶页 fixture：可见指令 / 隐藏元素指令 / HTML 注释 / 伪系统提示 + 一个正常提取任务 | `fixtures/web/injection-target/**` | 001 | S | 页面可本地打开（`file://` 或本地 http），无需外网 |
| **054** | 干净上下文复核（第 4 层）：高风险动作前用不含外部内容的小模型复核一致性 | `crates/core/src/verify_review**`、`crates/model-gateway/**` | 026,051 | M | 不一致 → 拒绝并告警；复核调用本身不计入污点上下文 |
| **055** | Edge Adapter：CDP 工具集（读取/填表/导航/下载）+ 外壳 UIA 工具（地址栏/标签/下载栏）+ 站点黑名单（银行/支付/密码修改）+ interrupts（Cookie 横幅/登录墙/验证码 → NeedsHuman） | `adapters/browser.edge/**`、`adapters/browser.chrome/**` | 048,049 | L | **禁止绕过验证码**；黑名单命中即拒绝；标签身份用 URL+标题+自定义标记组合，不用 tab index |
| **056** | T5.1：打开指定站点 → 提取列表页结构化数据 → 写入本地 CSV | `adapters/browser.edge/tasks/**`、`eval/tasks/edge/**` | 055 | M | 10 次成功率 ≥ 75%；异步加载判定误判率 < 5% |
| **057** | T5.2：表单填写 → **停在提交前** → 展示 diff 与来源归因 → 用户确认后提交 | 同上 | 056 | M | L3 动作 100% 经确认；计划 UI 正确标注 point-of-no-return |
| **058** | T5.3：注入靶页安全验收 + **安全回归常驻 CI** + 阶段 1c/阶段 1 总验收与对齐审计 | 同上、`.github/workflows/**`、`docs/audits/**` | 053,054,055 | M | **0 次执行页面内指令**（一票否决）；安全回归在 CI 中每次 PR 都跑；阶段 1 全部 DoD 达成 |

---

## 并行编排建议（write scope 不重叠，≤3 agent）

```text
批次 A1（串行，1 agent）      011 → 012 → 013 → 014 → 015
批次 A2（2 agent 并行）       {016} → {017, 019} 并行；018 接在 017 后
批次 A3（3 agent 并行）       {020, 021, 026} → {022, 023, 025} → {024, 027} → {028}
批次 A4（2 agent 并行）       {029} → {030, 032} 并行；031 需 017 就绪
批次 A5（1~2 agent）          033 ∥ 034 → 035 → 036 → 037 → 038 → 039
批次 1b（2 agent）            {040, 041} → 042 → 043 → 044 → 045 → 046 → 047
批次 1c（2~3 agent）          {048, 050, 053} → {049, 051, 052} → 054 → 055 → 056 → 057 → 058
```

**关键路径**：`011 → 012 → 016 → 017 → 020/021/022 → 028 → 029 → 035 → 036~038 → 039`（约 6~7 周），因此 **011/012/016/017 四张卡不得延误**，且应由最强的 agent（或人类直接审阅）负责。

**人类审阅瓶颈提示**：批次 A3 有 8 张内核卡、每张 diff 可达 400 行 → 若并行 3 个 agent，人类每天需审阅约 1200 行。**建议 A3 批次并行度降为 2**，或把 022/024/028 三张最关键的卡改为人类逐行审阅。

---

## 卡片正文的位置（ADR-0031：**一卡一文件**）

> 本文件**只保留阶段级信息**：In/Out scope、阶段 DoD、批次表与并行建议。
> 每张卡的**正文**（目标 / write scope / In scope / 必须遵守 / 验收命令 / DoD）与**执行记录**都在
> `tasks/TASK-NNN-<slug>.md` 里，由 Orchestrator 在**开工前**按 gov §3.2 模板展开（会话启动只读自己那一个）。
> **本文件不再放任何卡片正文** —— 出现即被 `xtask card-check` 判为 Error（ADR-0031 D6 判据 ④）。
> 模板与填写要求见 **gov §3.2（模板）与 §3.4（9 节执行记录骨架 + 分界线原文）**。

**已展开的卡片文件**（其余 46 张在开工前逐张展开；本表只登记已存在的文件，**只记「完成标记」** —— 该卡是否 Done（**ADR-0041 D1**）；
状态的**权威落点**仍是卡片文件自身，ADR-0031 D2）：

| 卡号 | 批次 | 卡片文件（正文 + 执行记录） | 备注 |
|---|---|---|---|
| **TASK-011 ✅** | A1 | `tasks/TASK-011-protocol-schema-codegen.md` | 完整卡（原 plans 的示例逐字搬运）；**已 Done** |
| **TASK-035 ✅** | A5 | `tasks/TASK-035-notepad-adapter.md` | Done（PR #86 / merge `9a979f4`；声明式 Adapter 包 v0） |
| **TASK-012 ✅** | A1 | `tasks/TASK-012-storage-layer-sqlite-wal-blob.md` | **完整卡**（2026-09-24 Orchestrator 展开）；**已 Done（2026-09-24）** |
| **TASK-013 ✅** | A1 | `tasks/TASK-013-audit-append-hash-chain-flush.md` | **完整卡**（2026-09-24 Orchestrator 展开；**已 Done**） |
| **TASK-014 ✅** | A1 | `tasks/TASK-014-secrets-os-keychain-wrapper.md` | **完整卡**（2026-09-24 Orchestrator 代行展开：`keyring` 4.2 后端 + `zeroize` + 访问审计注入点（fail-closed）；write scope 含 `docs/DEPENDENCIES.md` —— 登记表规则 1「先登记后引入」）；**已 Done（2026-09-24）** |
| **TASK-015 ✅** | A1 | `tasks/TASK-015-xtask-hygiene-archtest-replay-skeleton.md` | **完整卡**（2026-09-24 Orchestrator 展开：`docscan` 4 条结构规则 + `crates/core/tests/arch*` 分层断言 + PL-047 迁移登记表扫描 + ADR-0039 D3 的 `check-ledger` 两条规则）；**已 Done（2026-09-24）** |
| **TASK-016 ✅** | A2 | `tasks/TASK-016-platform-api-trait-capability-matrix.md` | **完整卡**（2026-09-24 Orchestrator 代行展开：零平台依赖的纯类型 + 3 个 trait 形状 + `CapabilityMatrix::validate()` 的 4 条不变量；**卡面有 3 个待裁决项 Q1 / Q2 / Q3**）；**已 Done（2026-09-24）** |
| **TASK-017 ✅** | A2 | `tasks/TASK-017-platform-windows-uia-provider.md` | **完整卡**（2026-09-24 Orchestrator 代行展开：UIA provider 全 9 方法 + `WindowProvider` 全 5 方法 + 句柄纪律源码扫描断言；**卡面有 4 个待裁决项 Q1 ~ Q4**）；**已 Done（2026-09-24）** |
| **TASK-018 ✅** | A2 | `tasks/TASK-018-platform-windows-synthetic-input-ime.md` | **完整卡**（2026-09-25 Orchestrator 代行展开：`SendInput` VK + `KEYEVENTF_UNICODE` 路径 + 发送前 100% 前台校验 + 显示器枚举 / DPI 换算 / 虚拟屏幕归一化 + `is_ime_open`；**卡面有 3 个待裁决项 Q1 ~ Q3**）；**已 Done（2026-09-25）** |
| **TASK-019 ✅** | A2 | `tasks/TASK-019-automation-host-ipc-named-pipe.md` | **已 Done（2026-09-25）** |
| **TASK-020 ✅** | A2 | `tasks/TASK-020-tool-bus-mcp-rmcp-server.md` | **完整卡**（正文含 4 个待裁决项 Q1 ~ Q4；Q5 为实现时新命中，见卡 §5）；**已 Done（2026-09-25）** |
| **TASK-021 ✅** | A2 | `tasks/TASK-021-policy-whitelist-risk-default-deny.md` | **已 Done（2026-09-25）** |
| **TASK-022 ✅** | A2 | `tasks/TASK-022-task-engine-state-machine-dag-checkpoint.md` | **完整卡**（2026-09-25 由 16:30 那次 automation 代 Orchestrator 展开正文，见卡 §5 DRIFT-022-1）；**已 Done（2026-09-25）** |
| **TASK-023 ✅** | A2 | `tasks/TASK-023-verify-postcondition-assertion-engine.md` | **已 Done（2026-09-25）** |
| **TASK-024 ✅** | A2 | `tasks/TASK-024-undo-four-level-rollback-anchor.md` | **已 Done（2026-09-26）** |
| **TASK-025 ✅** | A2 | `tasks/TASK-025-lease-target-exclusive-shared-intent.md` | **已 Done（2026-09-26）** |
| **TASK-026 ✅** | A2 | `tasks/TASK-026-model-gateway-provider-router-fallback.md` | **已 Done（2026-09-26）** |
| TASK-027 ✅ | A2 | `tasks/TASK-027-hitl-approval-scope-takeover.md` | **已 Done（2026-09-26）** |
| TASK-028 | A2 | `tasks/TASK-028-core-session-context.md` | **完整卡**（2026-09-26 Orchestrator 代行展开：会话管理 + 上下文管理（树裁剪 / 压缩 / 预算）；**原 TASK-028 的拆卡主卡**，见 ADR-0053 D7；文件名由 `TASK-028-core-session-context-planner-memory.md` 改名，**号不变**）；**Ready** |
| **TASK-207 ✅** | A2 | `tasks/TASK-207-core-planner-plan-step-dag.md` | **完整卡**（2026-09-26 Orchestrator 代行展开：Planner —— 模型输出 → 可校验的 Plan / Step DAG）；**Done（2026-09-27，PR #66 / merge `78acfed`）** |
| **TASK-208 ✅** | A2 | `tasks/TASK-208-core-memory-app-map-fts-retrieval.md` | **完整卡**（2026-09-26 Orchestrator 代行展开：Memory —— App Map 加载 + 消费 storage 的检索 API）；**Done（2026-09-27，PR #70 / merge `e8d95f2`）** |
| **TASK-029 ✅** | A3 | `tasks/TASK-029-binary-skeleton-agent-core-desktop-ui.md` | Done（2026-09-27，PR #72 / merge `21e35bc`） |
| **TASK-030 ✅** | A3 | `tasks/TASK-030-ui-approval-card-timeline-evidence.md` | Done（PR #74 / merge `4823cfd`；审批卡片 + 时间线） |
| **TASK-031 ✅** | A3 | `tasks/TASK-031-ui-element-picker-selector-candidates.md` | Done（独立 review P0/P1 清零；PL-094 跟踪 RoleAndParent 契约治理） |
| **TASK-032 ✅** | A3 | `tasks/TASK-032-ui-policy-panel-egress-capability-cost.md` | Done（PR #78 / merge `ed9fbfd`；策略 / 能力 / 成本面板） |
| **TASK-033 ✅** | A3 | `tasks/TASK-033-target-app-notepad-like-fault-injection.md` | Done（PR #82 / merge `8adb118`；UIA 六模式专项测试） |
| **TASK-034 ✅** | A3 | `tasks/TASK-034-record-replay-framework-xtask-replay.md` | Done（PR #84 / merge `167c3f2`；16/16 CI；Recording v1 + 离线 provider + `xtask replay`） |
| **TASK-036 ✅** | A5 | `tasks/TASK-036-t1-1-open-read-full-text.md` | Done（PR #88 / merge `0b64f85`；T1.1 声明式任务包 + 10 用例评测集） |
| **TASK-037 ✅** | A5 | `tasks/TASK-037-t1-2-replace-save-approval-diff-undo.md` | Ready（批次表占位派单前补全） |
| **TASK-038 ✅** | A5 | `tasks/TASK-038-t1-3-newtab-saveas-cross-process-dialog.md` | Ready（批次表占位派单前补全） |
| **TASK-039 ✅** | A5 | `tasks/TASK-039-stage-1a-integration-audit.md` | Done（阶段 1a 独立复验全绿并翻转 GO；TASK-105 3×10 证据就位） |
| **TASK-102 ✅** | A5-REMEDIATION | `tasks/TASK-102-runtime-execution-contract.md` | Ready（1a 执行链路契约） |
| **TASK-103 ✅** | A5-REMEDIATION | `tasks/TASK-103-runtime-executor-host-dispatch-verifyoutcome.md` | Ready（真实执行器） |
| **TASK-104 ✅** | A5-REMEDIATION | `tasks/TASK-104-ui-core-ipc-approval-wiring.md` | Ready（UI/IPC/审批接线） |
| TASK-105 | A5-REMEDIATION | `tasks/TASK-105-notepad-t1-runtime-validation.md` | Ready（T1 真实运行验收） |
| **TASK-087 ✅** | A5-REMEDIATION | `tasks/TASK-087-ci-negative-verification-and-check-comments.md` | Ready（CI 硬门禁与 check-comments） |
| **TASK-210 ✅** | A5-REMEDIATION | `tasks/TASK-210-ui-and-commit-quality-gates.md` | Ready（UI/提交质量门禁） |
| **TASK-212 ✅** | A5-REMEDIATION | `tasks/TASK-212-check-comments-violation-fix.md` | Ready（DRIFT-087-1 的 9 处违规修复） |
| **TASK-211 ✅** | A5-REMEDIATION | `tasks/TASK-211-stage1a-reaudit-and-parking-closeout.md` | Ready（复验与停车位收口） |
| **TASK-213 ✅** | A5-REMEDIATION | `tasks/TASK-213-ui-core-ipc-transport.md` | Ready（**ADR-0057 转 Accepted 后才可开工**） |
| **TASK-214 ✅** | A5-REMEDIATION | `tasks/TASK-214-production-composition-root-notepad-handlers.md` | Ready（**ADR-0058 转 Accepted 后才可开工**） |
| **TASK-215 ✅** | A5-REMEDIATION | `tasks/TASK-215-fixture-capability-extension-and-adapter-pack.md` | Done（适配包 + 文件读写 + 标签页 + 跨进程 Save As；PL-097 闭环） |
| **TASK-216 ✅** | A5-REMEDIATION | `tasks/TASK-216-runtime-completion-input-binding-and-step-kinds.md` | Done（A/B 片由 TASK-217 落地并收口；`DRIFT-216-4` 闭环） |
| **TASK-217 ✅** | A5-REMEDIATION | `tasks/TASK-217-runtime-task-dataflow-resumable-approval.md` | Done（T1.2/T1.3 fake platform Completed；UI approve/resume；DRIFT-216-4 闭环） |
| **TASK-218 ✅** | A5-REMEDIATION | `tasks/TASK-218-notepad-rollback-execution-verification.md` | Done（真实 UIA 完整 L1 恢复 + fake-platform fallback/负向；阶段 1a 仍 NO-GO） |
| **TASK-219 ✅** | A5-REMEDIATION | `tasks/TASK-219-runtime-pure-step-and-rollback-observability-fix.md` | Done（count_lines_and_keyword_paragraphs 真实执行；未知 pure operation fail-closed；editor_matches_anchor 真实比较） |
| **TASK-220 ✅** | A5-REMEDIATION | `tasks/TASK-220-resource-leak-audit-and-guards.md` | Done（元素表 4096 / anchor 64 / grant 1024 上限与淘汰；进程树 kill+reap；真机 ignored 全套 8 passed；`DRIFT-220-1` 闭环） |
| **TASK-221 ✅** | A5-REMEDIATION | `tasks/TASK-221-leak-audit-round-2.md` | Done（第二轮泄露审计：cost/secrets 加上限，audit/tool-bus/UI 复查无问题） |
| **TASK-222 ✅** | A5-REMEDIATION | `tasks/TASK-222-leak-audit-round-3-convergence.md` | Done（第三轮真机收敛首测：进程数收敛；agent 进程内测量留待下一轮） |
| **TASK-223 ✅** | A5-REMEDIATION | `tasks/TASK-223-l1-file-channel-read-utf8-prefix.md` | Done（ADR-0064 文件通道 + ADR-0065 前置初始指纹；T1.1 大文件 fake 与真机双证据；`DRIFT-223-1` / `PL-100` 闭环） |
| **TASK-225 ✅** | 1b bridge | `tasks/TASK-225-synthetic-input-target-lease.md` | Done（共享 `TargetLeaseRegistry` + input gate；两任务冲突 / 释放 / pointer-shaped 专项 5 passed；PL-101 闭环） |
| **TASK-226 ✅** | A5-REMEDIATION | `tasks/TASK-226-test-hygiene-cleanup.md` | Done（PL-104 / PL-105 测试卫生清理） |
| **TASK-227 ✅** | 治理池 | `tasks/TASK-227-status-and-parking-closeout.md` | Done（TASK-040/225 状态行更正 + PL-103 改法 + TASK-228 开卡） |
| **TASK-228 ✅** | 治理池 | `tasks/TASK-228-structured-real-machine-acceptance-record.md` | Done（真机结构化记录 + 严格汇总入口；`PL-106` 闭环） |
| **TASK-229 ✅** | 治理池 | `tasks/TASK-229-pl103-spec-fix-and-status-rule.md` | Done（PL-103 落笔 + 状态行硬规则 + 残留目录清理） |
| **TASK-224 ✅** | 治理池 | `tasks/TASK-224-single-write-channel-lock-queue.md` | Done（ADR-0066 命令行 write 通道 + Windows 独占占用探测 + 有界退避；PL-107 闭环） |
| **TASK-232 ✅** | 治理池 | `tasks/TASK-232-four-round-automation-audit-closeout.md` | Done（四轮审计收口：TASK-224 状态行 + 规则补强 + `PL-108`） |
| **TASK-233 ✅** | 治理池 | `tasks/TASK-233-storage-session-store-adapter.md` | Done（生产装配接入 storage 会话持久化：`StorageSessionStore` 适配器 + `with_storage_session_store()`；`PL-108` 闭环） |
| **TASK-234 ✅** | 治理池 | `tasks/TASK-234-hygiene-final-rules.md` | Done（ADR-0068 / 0069 + hygiene 13/13；`PL-060` 闭环；PR #218 / `e417a2a`） |
| **TASK-235 ✅** | 治理池 | `tasks/TASK-235-role-and-parent-resolution-semantics.md` | Done（ADR-0070；`RoleAndParent` 父候选仅作用域，`PL-094` 闭环） |
| **TASK-236 ✅** | 治理池 | `tasks/TASK-236-replay-sequence-diff-full.md` | Done（replay 完整版：真实 UIA 树快照序列 + 树级 diff + `--suite core`；`--list-deferred` 清零） |
| **TASK-237 ✅** | 治理池 | `tasks/TASK-237-automation-round-audit-closeout.md` | Done（三连发自动化审计收口：`PL-062` 闭环 + 孤儿目录清理 + `TASK-235` 状态行归一） |
| **TASK-238 ✅** | 治理池 | `tasks/TASK-238-capture-dlp-crate-skeletons.md` | Done（ADR-0071 `capture` / `dlp` 边界骨架；`PL-102` 闭环；解锁 1b TASK-041 / 042 与 1c TASK-050） |
| **TASK-239 ✅** | 治理池 | `tasks/TASK-239-parking-lot-review-2026-10-05.md` | Done（停车位存量复核：新增 2026-10-05 审计报告；PARKING_LOT 只追加；13 条补记闭环/取代、35 条复核保留） |
| **TASK-240 ✅** | 治理池 | `tasks/TASK-240-card-status-sweep.md` | Done（全量状态行清扫：18 张滞后卡按 LEDGER/main/测试证据更正；零产品代码改动） |
| **TASK-241 ✅** | 治理池 | `tasks/TASK-241-derived-value-sweep.md` | Done（ADR-0072 冻结派生值只指向唯一事实源；PL-035 闭环，PL-022 保留机器派生余项，PL-065 / PL-066 复核确认） |
| **TASK-242 ✅** | 治理池 | `tasks/TASK-242-round-audit-closeout-076-and-002.md` | Done（三连发审计收口：TASK-076 状态行按实改 Done + TASK-002 三方不一致开 PL-109） |
| **TASK-243 ✅** | 治理池 | `tasks/TASK-243-pl109-adjudication-and-consistency.md` | Done（PL-109 裁决收口：TASK-002 = Done，PLAN 阻塞项 / MEMORY §1 / LEDGER / PARKING_LOT 全部协调一致） |
| **TASK-244 ✅** | 治理池 | `tasks/TASK-244-derived-counts.md` | Done（PL-022 闭环：ADR-0075；gov §5.4 运行时派生 + gov §5.1 ↔ ci.yml 标记集合校验） |
| **TASK-245 ✅** | 治理池 | `tasks/TASK-245-round4-audit-closeout.md` | Done（四连发审计收口：LEDGER 去重 404→400 + `PL-111` WIP 落点 / ADR-0074 号冲突登记） |
| **TASK-040 ✅** | 1b | `tasks/TASK-040-synthetic-input-drag-lease-calibration.md` | Done（真机四用例取证；跨层 lease 由 TASK-225 闭环，`PL-101` 关闭） |
| **TASK-231 ✅** | 1b bridge | `tasks/TASK-231-pointer-coordinate-space-dpi.md` | Done（ADR-0067 pointer 显式坐标空间；`PL-074` 闭环） |
| **TASK-041 ✅** | 1b | `tasks/TASK-041-capture-window-redact-privacy.md` | Done（拆分 A + B：ADR-0073 / ADR-0076 + `crates/capture` 纯管线 + Windows GDI 截图 / 像素遮挡 + `ImageBlobSink` 注入） |
| **TASK-042 ✅** | 1b | `tasks/TASK-042-visual-verify-phash-dhash-confidence.md` | Done（ADR-0074 视觉验证纯逻辑；`crates/verify/src/visual/**` 零依赖 + 21 专项测试；`DRIFT-042-1` / `PL-110`） |
| **TASK-249 ✅** | 1b bridge | `tasks/TASK-249-visual-observation-source-runtime-wiring.md` | Done（ADR-0079 宿主 blob→BGRA→GrayImage→VisualObservation；运行时 `observe_visual` + 生产 collector；专项 15 passed） |
| TASK-043 | 1b | `tasks/TASK-043-paint-adapter-tools-canvas-coords.md` | Ready（批次表占位派单前补全） |
| TASK-044 | 1b | `tasks/TASK-044-t3-1-newcanvas-rect-color-screenshot.md` | Ready（批次表占位派单前补全） |
| TASK-045 | 1b | `tasks/TASK-045-t3-2-png-open-read-region-saveas.md` | Ready（批次表占位派单前补全） |
| TASK-046 | 1b | `tasks/TASK-046-t3-3-draw-undo-pixel-snapshot-verify.md` | Ready（批次表占位派单前补全） |
| TASK-047 | 1b | `tasks/TASK-047-stage-1b-integration-adapter-reuse.md` | Ready（批次表占位派单前补全） |
| TASK-048 | 1c | `tasks/TASK-048-cdp-provider-connect-dom-nav-download.md` | Ready（批次表占位派单前补全） |
| TASK-049 | 1c | `tasks/TASK-049-browser-profile-no-copy-user-profile.md` | Ready（批次表占位派单前补全） |
| **TASK-050 ✅** | 1c | `tasks/TASK-050-dlp-three-tier-egress-local-only-redacted-full.md` | Done（ADR 0007；`crates/dlp` 22 专项测试） |
| TASK-051 | 1c | `tasks/TASK-051-taint-tracking-permission-decay.md` | Ready（批次表占位派单前补全） |
| TASK-052 | 1c | `tasks/TASK-052-instruction-origin-attribution-ui.md` | Ready（批次表占位派单前补全） |
| **TASK-053 ✅** | 1c | `tasks/TASK-053-injection-target-fixture-visible-hidden.md` | Done（静态注入靶页 fixture；PR #260 / merge `79b5566`） |
| TASK-054 | 1c | `tasks/TASK-054-clean-context-review-small-model-fourth-layer.md` | Ready（批次表占位派单前补全） |
| TASK-055 | 1c | `tasks/TASK-055-edge-adapter-cdp-ua-blacklist-interrupts.md` | Ready（批次表占位派单前补全） |
| TASK-056 | 1c | `tasks/TASK-056-t5-1-open-site-extract-list-write-csv.md` | Ready（批次表占位派单前补全） |
| TASK-057 | 1c | `tasks/TASK-057-t5-2-form-fill-stop-before-submit-diff-origin.md` | Ready（批次表占位派单前补全） |
| TASK-058 | 1c | `tasks/TASK-058-t5-3-injection-target-security-ci-audit.md` | Ready（批次表占位派单前补全） |
| **TASK-085 ✅** | XTASK 池 | `tasks/TASK-085-xtask-hygiene-rust-source-rules.md` | Done（gov §5.4 第 1 / 2 / 3 / 10 / 11 项 = Rust 源码结构规则；实现覆盖 8/13） |
| **TASK-086 ✅** | XTASK 池 | `tasks/TASK-086-xtask-hygiene-file-level-and-registry-rules.md` | **完整卡**（2026-09-24 PL-059 归属修正新建：gov §5.4 第 8 / 9 / 13 项 = 文件级 + 依赖登记规则）；**Done（hygiene 11/13）** |


> **迁移零丢失核对**（ADR-0031 验证方式 3）：两段正文共 **30** 行非空内容（TASK-011 28 行 + TASK-035 2 行），
> 迁移后逐行同序一致地出现在新卡片文件的正文区（脚本校验 missing=0、same-order=True）。
> 搬运方式是**逐字节复制、不改一个字**；新增的只有元数据块、分界线注释与执行记录骨架。


## 跨阶段治理卡（不在本阶段批次表内）

| 卡号 | 号段 | 卡片文件 | 备注 |
|---|---|---|---|
| TASK-200 | 治理池 200~299（ADR-0037 D1） | `tasks/TASK-200-fix-spec-contract-drafts.md` | `docs/spec/*` 7 份契约草案的系统性缺陷（PL-038）；**已 Done（2026-09-24）** |
| TASK-201 | 治理池 200~299（ADR-0037 D1） | `tasks/TASK-201-core-crate-skeleton.md` | `crates/core` 骨架提前（PL-037 选项 ③ 的落地物）；**已 Done（2026-09-24）** |
| TASK-202 | 治理池 200~299（ADR-0037 D1） | `tasks/TASK-202-storage-migration-registry.md` | 存储迁移注册表（**ADR-0038**：storage 只提供机制、各 crate 自持迁移 + 唯一装配点）；PL-046 的落地物；**已 Done（2026-09-24）** |
| TASK-203 | 治理池 200~299（ADR-0037 D1） | `tasks/TASK-203-audit-log-column-semantics.md` | `audit_logs` 列语义去重 + 显式链序（**ADR-0040**：删与 `id` 同义的 `hash`、加 `sequence`；迁移 0003 重建表）；PL-043 / PL-045 的落地物；**已 Done（2026-09-24）** |
| TASK-204 | 治理池 200~299（ADR-0037 D1） | `tasks/TASK-204-draft07-keyword-verdict.md` | `crates/tool-bus` 的 draft-07 关键字判据硬化（三张显式拒绝表 + `$schema` 方言校验 + **`pattern` / `format` 永久放弃**）；TASK-020 §9 关注点 3「最大设计负债」的落地物；**已 Done（2026-09-25）** |
| **TASK-205 ✅** | 治理池 200~299（ADR-0037 D1） | `tasks/TASK-205-schema-module-split.md` | `crates/tool-bus/src/schema.rs`（892 行，TASK-204 收尾时只剩 8 行余量）按**职责**拆分为模块目录（注册期 schema 检查 / 运行期实例校验），让每个文件回到 gov §5.4 的 600 行建议线以下、**行为零变化**；人类 2026-09-25 裁决「**合适的时候立卡，拆文件吧**」；**Ready（2026-09-25）** |
| **TASK-206 ✅** | 治理池 200~299（ADR-0037 D1） | `tasks/TASK-206-storage-memory-fts5-search.md` | `crates/storage` 的 `memory_fts`（FTS5）迁移 + 检索 API + 存储侧测试 —— 原 TASK-028 的 **DRIFT-028-1** 前置卡（**ADR-0053 D6**）；**Done（2026-09-27，PR #68 / merge `6da9007`）** |
| **TASK-209 ✅** | 治理池 200~299 | `tasks/TASK-209-audit-round-2-governance-remediation.md` | 第二轮审计治理整改：文档漂移、卡片状态、ADR 断表、UI/xtask CI 空转与缺失硬门禁；产品最后一公里另立卡 |
| **TASK-230 ✅** | 治理池 200~299（ADR-0037 D1） | `tasks/TASK-230-storage-conversation-message-tree.md` | `PL-092` 落地物：storage 的 `conversations` + message-tree 迁移与持久化记录 API；**Done（2026-10-04）** |

## 任务卡号段分配（ADR-0037, 2026-09-20 起生效）

按 [docs/adr/0037-task-card-number-allocation-strategy.md](../../docs/adr/0037-task-card-number-allocation-strategy.md)：

| 号段 | 用途 | 现状 |
|---|---|---|
| 001~010 | stage-0 已 Done | 10 张（TASK-001~010）|
| 011 / 012 / 035 | stage-1 已迁入卡片文件 | 3 张（011 protocol / 012 storage = **完整正文**；035 notepad-adapter = **仅要点摘录、未展开**）|
| 013~034 / 036~058 | **stage-1 批次表占位**（本文件批次表）| **45 张 Ready**（开工前由 Orchestrator 逐张展开正文）|
| 059~070 | 已用 = xtask 护栏升级（6 张）+ governance（6 张）| 12 张 Done |
| 071 | ADR-0037 实施卡（号段分配策略；本 ADR 生效前建的治理卡）| 1 张 Done |
| 072~099 | XTASK 池 | 已用 072~079（stage-0 b1 探针卡）+ 083 / 084 + **085 / 086**（PL-059 归属修正新建）= **12 张**；**空位 080~082 / 087~099** |
| 100~199 | 业务池 | 已用 100 / 101（Win32-Input + probe 替换）；空位 102~199 |
| 200~299 | 治理池（audit/docs/memory 治理）| 已用 200（spec 修复卡）/ **201**（`crates/core` 骨架提前，PL-037）/ **202**（存储迁移注册表，ADR-0038，PL-046）/ **203**（`audit_logs` 列语义，ADR-0040）/ **204**（draft-07 关键字判据，TASK-020 §9）/ **205**（`schema.rs` 拆文件，TASK-204 收尾遗留）；**206**（`crates/storage` 的 `memory_fts`（FTS5）检索 + 存储侧测试，ADR-0053 D6）/ **207**（`core` Planner，ADR-0053 D7）/ **208**（`core` Memory，ADR-0053 D7）；空位 209~299 |

**未来 xtask 护栏扩张** = 用 072~099；用满后用 200~299。sub-suffix 永久禁用。
