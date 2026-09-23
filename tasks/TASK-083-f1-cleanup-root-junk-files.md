# TASK-083　F-1 清理：根目录 17 个 octal-escaped 乱码文件名

- 状态：**InProgress**
- 阶段：0　子任务：stage-0 收尾（不是 B 工作）　预估：XS（~10 min）　阻塞主线：否
- write scope：repo root 删除 17 个乱码文件 + git rm + 本卡 + commit
- background：这些是早期阶段脚本（大概是一个 bash heredoc 路径转义失败的产物）创建的 0 字节文件，文件名是 octal-escaped UTF-8。git status 一直显示为 untracked，污染 git status 输出。

<!-- ══ 分界线 ══ -->

## 执行记录（Implementer 填写）

### 1. 约束回执

```text
【任务】TASK-083 F-1 root 目录清理
【write scope】仅：repo root 17 个乱码 0-byte 文件 + 本卡 + commit
【铁律】AGENTS.md §3+§6 / 不改公共热点外的文件 / 不引入新依赖
【禁止】不要碰 spikes / crates / docs / tests / etc 任何合法文件 / 不要删除任何 git tracked 文件
【验收】git status clean for untracked junk；cargo test / xtask 全 PASSED；本卡 §1-9 + commit
【依赖】TASK-001 + TASK-073 (closeout 已在 audit §5 标记 F-1)
【疑问】无
```

### 2-9 实施后填入
