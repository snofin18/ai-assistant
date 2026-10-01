# ADR-0016　仓库文本统一使用 LF

状态：**Draft**（自动工作单 W4，待人类批准）　日期：2026-10-02　Supersedes：decisions.md `[ADR:待建 0016]`　Superseded by：—
关联：`.gitattributes`、`rustfmt.toml`、`.github/workflows/ci.yml`、`tasks/TASK-001-repo-skeleton.md`、`docs/memory/rejected.md`

## 背景（为什么现在要决定）

项目从第一天就面向 Windows / macOS / Linux 三平台，并在三平台 CI 矩阵上运行
`cargo fmt --all --check`。若文本文件跟着开发机的 Git 配置在 CRLF / LF 之间变化：

1. Linux / macOS runner 与 Windows 开发机可能得到不同字节；
2. `cargo fmt --check` 与 shell 脚本会把换行差异误报成内容差异；
3. 问题会依赖个人 `core.autocrlf` 配置出现，难以稳定复现。

TASK-001 为避免这一环境差异，已经把仓库文本策略固定为 LF。本 ADR 把该既有决定
从 `decisions.md` 的待建条目落成可审查的正式记录。

## 决策（一句话）

**仓库内文本一律使用 LF，并通过 `.gitattributes` 与 `rustfmt.toml` 双重声明，
不由开发机的 `core.autocrlf` 决定文件字节。**

决策细化：

| # | 内容 |
|---|---|
| **D1** | `.gitattributes` 使用 `* text=auto eol=lf` 声明文本文件的仓库内与签出形式均为 LF。 |
| **D2** | 二进制文件必须显式标记为 `binary`，禁止 Git 对图片、压缩包、数据库、音视频与可执行文件做换行转换。 |
| **D3** | `rustfmt.toml` 使用稳定选项 `newline_style = "Unix"`，与 `.gitattributes` 保持同一口径。 |
| **D4** | 不依赖个人 `core.autocrlf`、编辑器默认值或平台约定来决定仓库文件形态。 |

## 考虑过的选项（至少 2 个，含被否理由）

| # | 选项 | 结论 | 理由 |
|---|---|---|---|
| 1 | **仓库统一 LF（本 ADR）** | ✅ 采纳 | 三平台 CI 得到同一字节表示；文本差异只反映真实内容变化。 |
| 2 | 仓库统一 CRLF | ❌ 否决 | Linux / macOS runner 与大量工具链默认 LF；`cargo fmt --check` 会形成永久红灯。 |
| 3 | 只依赖 `core.autocrlf` | ❌ 否决 | 文件长什么样由每个开发机的 Git 配置决定，环境差异不可审计。 |
| 4 | 混合换行，由各文件自行选择 | ❌ 否决 | 同一仓库出现多套规则会让 diff、格式化和脚本行为不稳定。 |

## 影响（需要改的 spec / 代码 / 文档 / 任务卡）

- `.gitattributes`：文本 LF 规则与二进制显式标记。
- `rustfmt.toml`：`newline_style = "Unix"`。
- `.github/workflows/ci.yml`：三平台 `cargo fmt --all --check` 是该决定的持续验证。
- `docs/memory/rejected.md`：记录“CRLF 会导致 Linux CI 永久红灯”的否决理由。

本 ADR 不改变任何产品 API、依赖或许可证。

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| Windows 旧工具或模板文件可能偏好 CRLF | 对确有需要的生成物使用生成器显式控制；不得用个人 Git 配置改变仓库真值。 |
| 二进制文件被当作文本转换 | `.gitattributes` 按扩展名显式标记常见二进制格式；新增格式时补标记。 |
| 已有多平台文件出现历史换行差异 | 用一次性规范化提交处理，之后由 `cargo fmt --check` 与文档检查持续守门。 |

## 验证方式（怎么知道这个决策是对的；何时应重新评估）

1. `cargo fmt --all --check` 在三平台 CI 全绿。
2. `.gitattributes` 含 `* text=auto eol=lf` 与实际二进制标记。
3. `rustfmt.toml` 含 `newline_style = "Unix"`，且不保留 stable 通道无效的格式化选项。
4. 抽样 `git ls-files --eol` 时，仓库文本应稳定为 LF；任何平台特有 EOL 都应有显式规则。
5. **重新评估触发**：某类必须保留 CRLF 的文本无法通过生成器或按扩展名规则表达时，
   新 ADR 给出局部例外；不得恢复为全局依赖 `core.autocrlf`。
