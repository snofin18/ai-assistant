# xtask — 仓库护栏与开发任务工具

## 职责

把「靠自觉」的规范变成**机器可执行的检查**。当前提供：

- `hygiene`：仓库卫生检查（gov §5.4 的 **13** 项中已实现 3 项（口径见 ADR-0025），工具会主动声明覆盖范围）
- `--list-deferred`：打印**未实现**的子命令与规则，含归属任务卡号
- 其余子命令（`verify-schemas` / `codegen` / `replay` / `check-comments` / `check-ledger` /
  `card-check`）已登记但**未实现**，运行会以退出码 3 显式失败

## 边界（不做什么）

- **只读**：不创建、修改、删除任何文件，不访问网络
- **不参与产品运行时**：不被任何 `crates/*` 或 `apps/*` 依赖
- **零第三方依赖**：只用 `std`。护栏工具自身必须无供应链风险，且编译要快到
  "每次提交都能跑"（当前全量编译 < 1 s）
- 不做规则以外的判断：例如"这个文件该不该拆"由人决定，工具只报告"它超过了阈值"

## 不变量（改动前必读）

1. **规则判定必须是纯函数**：输入源码文本 → 输出 `Finding`。IO 只在 `repowalk.rs`
   与 `main.rs`。这是白盒测试能覆盖每条规则而不需要造临时目录的前提。
2. **无静默失败**：未实现的子命令必须显式失败并指出归属卡号；扫到 0 个文件必须告警；
   IO 错误必须带上具体路径向上抛。
3. **输出确定性**：同一仓库状态两次运行输出逐字节相同（目录遍历排序 + 发现项排序 +
   相对路径统一用 `/`）。否则 CI 输出无法 diff、无法缓存。
4. **阈值与规则标识符是契约**：`hygiene/*.` 规则名、`FILE_LINES_*` 等阈值被 CI 与任务卡
   引用，改动需 ADR。
5. **不放宽护栏来让自己通过**：xtask 自己也受 hygiene 检查（`cargo run -p xtask -- hygiene`
   会扫 `xtask/src`）。当前 7 个源文件全部 < 600 行。

## 模块分层

| 模块 | 职责 | 碰文件系统 |
|---|---|---|
| `cli.rs` | 解析 argv、用法文本 | 否 |
| `repowalk.rs` | 定位仓库根、遍历源文件、路径归一化 | **是** |
| `rustscan.rs` | 把源码拆成「注释列表」与「降噪代码」两个视图 | 否 |
| `hygiene.rs` | gov §5.4 卫生规则判定 | 否 |
| `deferred.rs` | 未实现项登记表 | 否 |
| `report.rs` | `Finding` / `Report` 模型与渲染 | 否 |
| `main.rs` | 分派、读文件内容、呈现、退出码 | 是（只读） |

`rustscan` 存在的理由：直接对原始文本做子串匹配会误判 —— `let url = "http://x";`
里的 `//` 不是注释，字符串里的 `TODO` 字样也不是待办。抹平字面量与注释之后，
剩下的才是"真正的代码"。

## 退出码

| 码 | 含义 |
|---|---|
| 0 | 通过（可能有 Warning） |
| 1 | 存在 Error 级发现项 → 阻塞合并 |
| 2 | 用法错误 |
| 3 | 子命令已登记但未实现（**故意不是 0**） |
| 4 | IO 或内部错误 |

## 已实现的规则

| 规则标识符 | 级别 | 依据 |
|---|---|---|
| `hygiene/file-too-long` | > 600 行 Warning；> 900 行 Error | gov §5.4 |
| `hygiene/banned-comment-tag` | Error | naming §8（禁用标签） |
| `hygiene/missing-card-reference` | Error | naming §8（待办/占位必须带卡号） |
| `hygiene/commented-out-code` | Error（连续 ≥5 行形似代码的 `//`） | gov §5.4 / §6.1.2 |
| `xtask/no-source-files` | Warning | 本工具自证「扫到 0 个文件」不是通过 |

## 已知限制 / 技术债

- 8 项 gov §5.4 规则未实现（函数行数、参数个数、圈复杂度、重复代码、顶层目录白名单、
  依赖登记比对、空实现 stub、被跳过的测试）→ **TASK-015**
- `check-comments` / `check-ledger` / `card-check` **无任务卡认领** → `docs/PARKING_LOT.md` PL-002
- 「文档注释里用反引号引用的标签字样」是否豁免，尚未裁决 → PL-004
- `rustscan` 不是完整的 Rust 解析器：它只需要区分 代码 / 注释 / 字符串 / 字符字面量 /
  生命周期。宏内部的复杂 token 序列可能被误判（当前规则不依赖宏内部结构，故可接受）

## 运行

```powershell
cargo run -p xtask -- hygiene
cargo run -p xtask -- hygiene --list-deferred
cargo run -p xtask -- hygiene --repo <路径>     # 指定仓库根（默认由编译期常量推导）
cargo run -p xtask -- --help
```
