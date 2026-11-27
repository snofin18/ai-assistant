# Stage 2 Excel T2.2 production save boundary

日期：2026-11-27
范围：TASK-420

## 1. 结论先行

当前 **NO-GO**：不得填写 production `tools.json`、注册 `excel.*` ToolBus Tool、
编写 production rollback/assembly，或把 TASK-407/409/415/418/419 的 test-only 证据
宣称为 production save 已完成。

若 production 定义要求覆盖 Host 进程整体崩溃后的 Excel ownership 收敛，则下一步必须先
通过新 ADR/spec 定义独立 helper process；否则只能接受同进程 save caller 的明确能力上限。
本审计建议采用 helper process 作为长期 production 方案，但该决定尚未获得 ADR 批准。

## 2. 已满足的私有证据

- TASK-407：真实 COM writable open、scalar write、recalculate、save、close 路径已在
  human-present ignored test 中校准；产物是 test-only harness。
- TASK-409：真实 COM 10/10 重复写入、saved digest 变化、shadow restore、failure injection、
  process convergence 和 `EXCEL.EXE=0` 证据已存在；仍是 test-only harness。
- TASK-415：L1 scalar file transaction 已组合 anchor、write-port、file-shadow、post-fingerprint
  与 saved-file digest 双门回滚。
- TASK-418：test-only task transaction owner 已提供 running/closing/terminal、单 active
  transaction 与 committed/rollback cleanup。
- TASK-419：owner 的 duplicate execute、rollback failure、cleanup failure 路径已证明保留
  证据且不误报 terminal 成功。

这些证据足以支撑继续设计 production caller，不足够直接开放 production Tool surface。

## 3. 当前实际缺口

1. 没有 production in-place save caller 把 L1 receipt、真实 COM Save 与 task-end 收敛串起来。
2. 没有 production 保存后的文件 digest 回传、冲突检测、恢复失败 incident 和人工接管路径。
3. 没有独立 helper process 的启动、发现、认证、生命周期与崩溃恢复协议。
4. 没有 production task assembly / rollback recipe / ToolBus registration。
5. 没有人类在场的 Open/Close/Quit hang 与 Host/child 崩溃注入证据。

## 4. 方案对比

| 方案 | 优点 | 缺点 | 适用结论 |
|---|---|---|---|
| A. 同进程 production save caller | 复用当前 owner 与 runner，改动较小 | Host 整体崩溃后无存活执行体，无法保证 owned Excel 被收敛 | 只能作为无 Host-crash 保证的过渡方案，需 ADR 明确限制 |
| B. 独立 helper process | Host 崩溃后仍有执行体可读 journal、验证 identity 并清理 Excel | 新增进程边界、IPC、认证、权限、发现、回收和测试成本 | 若 production 要求 Host crash recovery，推荐 |
| C. 保持 private surface | 风险最低，可继续验证 | 无法完成任务闭环 | 当前状态 |

## 5. 推荐实施顺序

1. **ADR/spec 决策**：确认 Host 整体崩溃是否属于 production 必需能力；若需要，选择 helper
   process，并定义 IPC、认证、发现、journal 所有权、陈旧进程处理和失败上报。
2. **helper process contract test**：只做 fake/journal/temp-file 测试，不注册 Tool。
3. **private save caller integration**：连接 COM save、saved digest、L1 owner 与 cleanup。
4. **human-present crash acceptance**：Open/Close/Quit hang、Host kill、child kill、
   retry/recovery、`EXCEL.EXE=0` 与文件 digest 证据。
5. **production assembly**：只有前四项全部通过后，才填写 `tools.json`、rollback recipe、
   assembly 并注册 ToolBus；另开卡处理。

## 6. 明确的禁止项

- 不把同进程 thread watchdog 当作 Host 崩溃恢复。
- 不把 test-only runner handle 暴露成模型可见 Tool。
- 不用裸 PID 或仅路径终止 Excel。
- 不把 shadow restore 宣称为 production atomic replacement。
- 不在没有 ADR/spec 的情况下修改公共 API 或新增 IPC。

## 7. 验证

本审计只读；验收命令见 TASK-420。production `tools.json`、ToolBus、rollback、assembly、
schema、公共 API 和依赖均保持零改动。
