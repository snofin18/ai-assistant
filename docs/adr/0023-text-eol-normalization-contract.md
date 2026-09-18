# ADR-0023　文本进出口的换行符归一化契约（含每应用「习惯表」）

状态：Accepted　日期：2026-09-18　Supersedes：—　Superseded by：—
关联：ADR-0021（应用档案）、ADR-0022（定位契约）、`docs/memory/apps/notepad.md`、
`spikes/spike-a-notepad/probe-03-eol-matrix.ps1`、`probe-04-write-path-eol.ps1`、
架构 v2 §9（撤销/postcondition）、人类指示 #2（2026-09-18）

## 背景（为什么现在要决定）

Spike A 先导实测（2026-09-17）出现一次**假阴性**：磁盘文件 62 字符（CRLF），
`ValuePattern.GetValue` 读回只有 59 字符，于是探针报告 `value == file content ? False`，
看起来像「记事本吞了数据」。真相是 **UIA 返回的段落分隔符是裸 `\r`**，字符数当然对不上。

人类指示 #2（2026-09-18）：

> 「这个问题，需要在进出口的地方做一下约束或转换，对于已知的程序，可以尝试做个列表存储一下习惯，
> 以后可以直接在列表里找，实际运行的时候检测一下没有问题即可。如果有问题，网络搜一下官方新闻和文档，
> 确认是否新版本更新了回车换行方式。」

即三件事：① 在**进出口**做约束/转换；② 建**每应用习惯表**；③ 运行时**检测验证**，
并用官方文档确认换行方式是否随版本变化。本 ADR 逐条落实。

## 证据一：官方文档 —— 裸 `\r` 是 RichEdit 的**内部表示**，不是 bug，也不是版本回退

| 来源 | 原文（关键句） | 含义 |
|---|---|---|
| learn.microsoft.com `Windows.UI.Text.TextGetOptions` | `UseCrlf = 2`："Use carriage return/line feed (CR/LF) **in place of a carriage return**" | ★ 官方明示：富文本的**默认**段落分隔符就是**裸 CR**，CRLF 是需要**显式请求**的选项 → 读回裸 `\r` 属预期行为 |
| learn.microsoft.com《About Rich Edit Controls》 | "The **end-of-paragraph mark (CR)** can also handle CR/LF" | 记事本 11 的编辑区 ClassName = `RichEditD2DPT`，属 RichEdit 家族 → 同一套段落语义 |
| devblogs.microsoft.com/commandline《Extended EOL in Notepad》（Michel Lopez [MSFT]） | Notepad 支持 **LF / CR / CRLF**，**保留文件原有 EOL**，新建文件默认 CRLF，状态栏显示检测到的 EOL | 换行方式**没有**在新版本被改回或改动；且「文件 EOL」与「UIA 文本表示」是**两个独立变量** |

→ 人类指示 #2 的第三问「是否新版本更新了回车换行方式」的答案：**没有**。
裸 CR 来自 RichEdit 的内部段落模型；文件 EOL 由记事本自己检测并保留。

## 证据二：本机实测矩阵（probe-03，读入方向）

同一内容（`AAA<sep>BBB<sep>CCC<sep>`）写成 5 种磁盘形态，每次全新启动记事本、
按 ADR-0022 D1 的 nonce 锚点定位窗口后读取编辑区：

| 磁盘形态 | 磁盘 EOL 计数 | **UIA 返回** EOL 计数 | 磁盘字符 | UIA 字符 | 原样相等 | 归一化后相等 | 状态栏显示 |
|---|---|---|---|---|---|---|---|
| CRLF | CR=3 LF=3 | **CR=3 LF=0** | 15 | 12 | False | **True** | ` Windows (CRLF)` |
| LF | CR=0 LF=3 | **CR=3 LF=0** | 12 | 12 | False | **True** | ` Unix (LF)` |
| CR | CR=3 LF=0 | **CR=3 LF=0** | 12 | 12 | **True** | **True** | ` Macintosh (CR)` |
| MIXED（CRLF+LF+CR） | CR=2 LF=2 | **CR=3 LF=0** | 13 | 12 | False | **True** | ` Windows (CRLF)` |
| CJK（`中文一<CRLF>中文二<CRLF>`） | CR=2 LF=2 | **CR=2 LF=0** | 10 | 8 | False | **True** | ` Windows (CRLF)` |

> **[2026-09-18 补测]** CJK 行首次跑出的是**被污染的数据**（`disk_chars=14` / `disk_bytes=34` /
> `disk_eol=[code10=2 code13=1]` / `uia_chars=13`）。原因：当时 `probe-03` 脚本里直接写了 CJK 字面量，
> 而 PS 5.1 按 **GBK** 解码无 BOM 脚本 → **字面量本身就已不是我们以为的那串字符**，
> 于是 `norm_eq=True` 是**假阳性**（两边同样被糟蹋）—— 这比假阴性更危险。
> 把脚本改为**纯 ASCII + 用码点构造 CJK 样本**（`[char]0x4E2D` …，ADR-0024 D4）后重跑，得到上表数据：
> 磁盘 10 字符 / **22 字节**（(3×3+2)×2，UTF-8），UIA 读回 **8 字符**（6 个 CJK + 2 个裸 CR），
> 归一化后逐字符相等 → **CJK 无损，且结论 R1 对 CJK 同样成立**。
> 其余 4 个用例重跑结果与首次**逐字节一致** → 证明改写未改变探针逻辑。
> 旧输出已留存：`D:\csart\eol-probe\RESULT-pre-ascii-fix-20260918.txt`。

**结论 R1**：UIA **一律**把所有段落分隔符表示为裸 `\r`，与磁盘 EOL 无关（LF-only 文件读回也是 CR）。
**结论 R2**：因此**文本层不可恢复**「文件原本是 LF 还是 CRLF」——这个信息只存在于
① 磁盘字节、② 记事本状态栏（本地化文本）。
**结论 R3**：状态栏确实独立报告文件 EOL（`Windows (CRLF)` / `Unix (LF)` / `Macintosh (CR)`），
可作为**交叉验证**信号，但它是本地化 `Name`，按 ADR-0022 D5 只能当辅助。

## 证据三：本机实测矩阵（probe-04，写回 + 保存方向）

`SetValue` 写入不同分隔符 → 读回编辑器 → `Ctrl+S` 保存 → 读磁盘字节：

| 磁盘原形态 | 写入分隔符 | 写入字符 | **UIA 读回** | **保存后磁盘** | 磁盘 EOL 风格是否保留 | CJK 是否完好 |
|---|---|---|---|---|---|---|
| CRLF | LF | 15（含 7 个 CJK） | CR=2 LF=0，15 字符 | CR=2 LF=2 | ✅ | ✅ **True** |
| CRLF | CRLF | 10 | CR=2 LF=0，8 字符 | CR=2 LF=2 | ✅ | n/a |
| CRLF | CR | 8 | CR=2 LF=0，8 字符 | CR=2 LF=2 | ✅ | n/a |
| LF | LF | 8 | CR=2 LF=0，8 字符 | CR=0 LF=2 | ✅ | n/a |
| LF | CRLF | 10 | CR=2 LF=0，8 字符 | CR=0 LF=2 | ✅ | n/a |

五例的 `ASSERT uia_equals_write_normalized` 与 `ASSERT disk_equals_write_normalized` **全部 True**；
`ASSERT disk_eol_style_preserved` **全部 True**；状态栏在保存前后不变。

**结论 W1**：`SetValue` 对 LF / CR / CRLF **一视同仁**，内部统一存为裸 CR（写 CRLF 时 LF 被丢弃，10→8 字符）。
→ **写入时用哪种分隔符都不影响结果**，因此可以固定用**内部规范形**。
**结论 W2**：保存后的**磁盘 EOL 跟随文件原有风格，而不是跟随我们写入的分隔符**
（CRLF 文件写 LF 仍存成 CRLF；LF 文件写 CRLF 仍存成 LF）。
→ **走应用自身的保存路径不会破坏文件的换行风格**。这是最好的结果，无需额外还原逻辑。
**结论 W3**：CJK 经 `SetValue` → 保存 → 磁盘 **完整无损**（7 个 CJK 字符全部存活）。
**结论 W4**（推论，需在阶段 1 验证）：只有当我们**绕过应用直接写文件**（L1 文件契约）时，
才必须自己复现原 EOL；此时「原 EOL」的唯一权威来源是**磁盘字节**，不是状态栏。

## 决策

### D1　内部规范形（canonical form）= **LF**

项目内部（Core / policy / task-engine / 审计 / fixture / prompt 里的文本）**只允许 `\n`**。
任何来自应用或磁盘的文本，**入口**必须归一化；任何发往应用或磁盘的文本，**出口**必须按目标约定转换。

### D2　入口归一化：三态全归一，**顺序不可换**

```text
canonical = raw.replace("\r\n", "\n").replace("\r", "\n")
```

顺序错了会怎样：先替换 `\r` 会把 CRLF 变成 `<LF><LF>`（多出一个空行），这是**不可逆**的破坏。
→ 该函数必须是**唯一**的归一化实现（放 `crates/core` 的文本工具模块，禁止各处手写）。
**只做 `Replace("\r\n","\n")` 是不够的** —— probe-03 的 CR-only 与 MIXED 用例证明孤立 `\r` 真实存在。

### D3　出口转换：由**每应用习惯表**决定，不硬编码

出口需要两个独立决定，**不得混为一谈**：

| 决定 | 依据 | 记事本的答案 |
|---|---|---|
| ① 交给应用时用什么分隔符 | 应用是否自己归一化（W1） | 任意 → 统一用 **LF**（最简，且已证明等价） |
| ② 直接写磁盘时用什么 EOL | 文件原 EOL（W4） | **必须复现原 EOL**（CRLF/LF/CR），来源 = 读文件时记下的字节级 EOL |

### D4　每应用「习惯表」（`text_conventions`）落在应用档案里

人类指示 #2 要求的「列表」= `docs/memory/apps/<app>.md` 的第 4 节（结构见 ADR-0021）。
每应用**必须**记录下列字段（未测则写 `UNTESTED`，不得留空、不得猜）：

```yaml
app_id: windows.notepad
app_version_tested: 11.2607.14.0
uia_text_separator: CR            # UIA 读回的段落分隔符（R1）
uia_text_options_available: [TextGetOptions.UseCrlf]   # 能否让应用直接吐 CRLF
accepts_on_write: [LF, CR, CRLF]  # 写入方向被归一化的分隔符集合（W1）
canonical_write_separator: LF     # 本项目固定使用的写入分隔符
save_preserves_file_eol: true     # 应用保存是否保留文件原 EOL（W2）
detects_eol_from_content: true    # 是否自动检测（混合文件 -> Windows (CRLF)）
new_file_default_eol: CRLF        # 新建文件的默认 EOL
eol_status_signal:                # 可用于交叉验证的信号（本地化，仅辅助）
  location: "status bar Text[aid=ContentTextBlock]"
  values: { CRLF: "Windows (CRLF)", LF: "Unix (LF)", CR: "Macintosh (CR)" }
  caveat: "localized Name; ADR-0022 D5 -> 辅助信号，禁止作主判据"
encoding_observed: UTF-8          # 实测编码
bom_stripped_on_read: UNTESTED
direct_file_write_requires_eol_restore: true   # W4
postcondition_compare_mode: normalized         # 见 D5
verified_at: 2026-09-18
verified_by: probe-03 / probe-04
reverify_trigger: "app_version 变化，或 postcondition 连续 2 次失败"
```

> 阶段 1 起，这张表升级为 `protocol/` 下的 **schema + 机器可读数据**（Adapter 清单的一部分），
> 并纳入 `xtask verify-schemas`。阶段 0 先用 Markdown 表（**已登记 `docs/memory/open.md`**）。

### D5　postcondition 比较**必须**在规范形上做

铁律 4 要求「每个写操作必须有 postcondition」。本 ADR 补充其**比较规则**：

- 文本类 postcondition 一律比较 `canonical(expected) == canonical(actual)`；
- **禁止**裸比较（probe-03 的假阴性就是这么来的）；
- 若同时需要断言 EOL 风格（例如「不得改变用户文件的换行风格」），那是**独立的一条**
  postcondition，比较对象是**磁盘字节的 EOL 计数**，不是 UIA 文本；
- 该规则写进 `docs/spec/`（阶段 1 的 `postcondition.md`），并在契约测试里固化
  「15 磁盘字符 ↔ 12 UIA 字符」这一具体换算作为回归用例。

### D6　运行时自检（人类指示 #2 的「实际运行的时候检测一下」）

Adapter 首次绑定某应用时执行一次**廉价自检**，把结果与习惯表比对：

1. 读一个已知内容的小 fixture（含 CRLF 与孤立 CR 各一处）；
2. 断言 `canonical(uia_text) == canonical(fixture)`；
3. 断言 `uia_text` 中**不含** `\n`（若含 → 说明该版本已改为返回 LF/CRLF，习惯表过期）；
4. 不一致 → **显式失败**并提示「应用版本 <X> 的文本约定与档案不符，请重跑 probe」，
   **禁止**静默按旧表继续（铁律 1）。

自检成本 = 一次读 + 两次字符串比较（probe-03 实测 UIA 读 59 字符 ≈ 0.11 ms），可每次绑定都跑。

## 影响（需要改的文件）

- `docs/memory/apps/notepad.md`：写入上面的习惯表实例（第 4 节）。
- `spikes/spike-a-notepad/`：`probe-03` / `probe-04` 即本 ADR 的证据来源，README 需登记。
- `docs/spike-reports/SPIKE-A.md`：§4 坑 1 升级为「已定契约（ADR-0023）」。
- 阶段 1：`crates/core` 的文本工具模块（唯一归一化实现）+ `protocol/` 的 `text_conventions` schema
  + `docs/spec/postcondition.md`。

## 考虑过的选项（含被否理由）

| # | 选项 | 结论 | 理由 |
|---|---|---|---|
| 1 | 内部规范形用 CRLF | ❌ 否决 | 与「全仓库 LF」（ADR-0016）、fixture、跨平台（macOS/Linux 目标应用多为 LF）全部冲突 |
| 2 | 不归一化，各处比较时自己处理 | ❌ 否决 | 已实证会产生假阴性；且每个 Adapter 各写一套必然出现只做 `\r\n→\n` 的半吊子实现 |
| 3 | 用 `TextGetOptions.UseCrlf` 直接取 CRLF，绕开归一化 | ⏸ 部分采纳 | 官方支持且优雅，但**只解决读**、且只对有 `TextPattern` 的应用可用；`ValuePattern` 降级路径仍需归一化 → 记入习惯表 `uia_text_options_available`，作为**优先路径**而非唯一路径 |
| 4 | 靠状态栏判断文件 EOL | ❌ 否决（降为交叉验证） | 状态栏是本地化 `Name`（ADR-0022 D5），且 MIXED 文件会显示 CRLF，与真实字节不完全对应 |
| 5 | 每次都重读磁盘字节确定 EOL | ✅ 采纳（限 L1 直接写文件路径） | W4：只有绕过应用写文件时才需要，此时磁盘字节是唯一权威 |
| 6 | **LF 规范形 + 三态归一 + 每应用习惯表 + 绑定期自检** | ✅ **采纳** | 覆盖读写双向、覆盖版本漂移、失败模式是显式的 |

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| 习惯表随应用更新过期 | D6 绑定期自检 + `reverify_trigger` 字段；自检失败=显式失败 |
| 「归一化后再比较」掩盖真实的丢字问题 | 除规范形比较外，**另外**记录并断言原始长度差（如 15↔12），差值必须能被 EOL 计数完全解释 |
| 孤立 `\r`（老 Mac 风格）在真实用户文件里罕见，测试覆盖不足 | probe-03 已含 CR-only 与 MIXED 用例；契约测试固化这两种 |
| 出口 EOL 还原写错 → 静默改变用户整个文件的换行风格（diff 巨大） | W2 表明走应用保存路径无此风险；L1 直写路径必须先读原 EOL 再写，且该动作**风险级上调 + 强制人工确认** |
| BOM 处理未测 | 习惯表标 `UNTESTED`，列为 TASK-002 剩余项 |

## 验证方式

1. probe-03 / probe-04 的断言全 True（已于 2026-09-18 实测通过，输出存 `D:\csart\eol-probe\RESULT*.txt`）。
2. 阶段 1 的契约测试：喂 `\r\n` / `\n` / `\r` / 混合四种输入，断言归一化输出**逐字节**等于预期。
3. 故意把归一化顺序改成「先 `\r`→`\n`」→ 契约测试必须变红（负向验证，ADR-0019 N1）。
4. TASK-002 正式会话补测 BOM 与「另存为」改编码/改 EOL 的情形。

## 重新评估的触发条件

- 任何目标应用的 `uia_text_separator` 不是 CR（例如某应用直接返回 LF）→ 说明 D1/D2 需要按应用分叉。
- 记事本新版本改变「保存保留原 EOL」行为（W2 失效）→ 必须引入出口 EOL 还原逻辑并重估风险级。
- 阶段 1 把习惯表升级为 schema 后 → 本 ADR 的 Markdown 表转为示例，正文保留契约部分。
