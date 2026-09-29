# TASK-210　UI 与提交质量门禁：Prettier / ESLint / Vitest / commitlint

- 状态：**Ready**
- 阶段：1　子阶段：1a 补救　批次：A5-REMEDIATION　依赖：039
- 预估：L　难度：M
- 本文件 = 卡片正文 ＋ 执行记录（ADR-0031）。
- 关联：gov §5.1/§6.3、`apps/desktop-ui/**`、`.github/workflows/**`、PL-093

## 目标

补齐非 Rust 侧的现行质量门禁：为 desktop-ui 引入并实际运行 Prettier、ESLint、
Vitest/Testing Library；为仓库引入 commitlint 或等价提交规范门禁。所有新增依赖先登记。

## In scope

- `apps/desktop-ui/package.json`、lockfile、配置与测试。
- `.github/workflows/ci.yml`。
- 必要的 `docs/DEPENDENCIES.md` 登记。
- 本卡记录与 memory。

## Out of scope

- 修改 Rust 业务实现。
- 新 UI 功能。
- 通过关闭规则、降低阈值或忽略目录来让门禁变绿。
- 在没有负向验证前宣称新硬门禁已完成。

## 必须遵守

- 先登记依赖，再引入。
- 所有 UI 检查在 CI 中每次 PR 运行，不允许仅本地脚本。
- 新 lint/test 门禁必须包含至少一个负向样本或 canary。
- commitlint 规则与现有提交风格一致，不得重写历史。
- 若依赖审批受阻，记录 DRIFT 并停止，不得绕过。

## 验收命令

```powershell
pnpm --dir apps/desktop-ui install --frozen-lockfile
pnpm --dir apps/desktop-ui lint
pnpm --dir apps/desktop-ui format:check
pnpm --dir apps/desktop-ui typecheck
pnpm --dir apps/desktop-ui test
pnpm --dir apps/desktop-ui build
cargo run -p xtask -- docscan
```

另须验证 commitlint 对合法/非法提交信息的正负样本。

## 完成定义（DoD）

- [ ] Prettier、ESLint、Vitest/Testing Library 已真实接入并有正负测试。
- [ ] CI 对每个 PR 运行 UI format/lint/typecheck/test/build。
- [ ] commitlint 或等价门禁已接入。
- [ ] 依赖登记和使用方同步。
- [ ] 全部本地与 CI 验证通过。
- [ ] 未修改 Out of scope 文件。

<!-- ══ 分界线：以上为**卡片正文**，Orchestrator 所有，Implementer 只读 ══
     以下由 Implementer 填写。改动分界线以上的任何一行 = 漂移触发器 ⑤（超出 write scope），
     并会被 `xtask card-check` 判为 Error（ADR-0031 D3 / D6）。 -->

## 执行记录（Implementer 填写）

### 1. 约束回执

### 2. 实际改动文件

### 3. 验收输出摘要

### 4. DoD 逐条核对

### 5. 偏差

### 6. 更合理做法

### 7. 遗留问题

### 8. 新增长期记忆

### 9. 给审阅者的关注点
