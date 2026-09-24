# assistant-platform-api

> **铁律 7 指定的唯一平台入口**：`core` / `Host` 只能经本 crate 的 trait 使用平台能力，
> **不得**直接依赖 `windows` / `objc2` / `gtk` / `atspi` 等平台实现 crate。
> 分层断言见 `crates/core/tests/arch_layering.rs`（gov §5.1 门禁 #5）。

## 职责

1. **纯类型**（可序列化、可跨进程）：`TargetDescriptor` / `NormalizedPoint` / `Fingerprint` /
   `CapabilityMatrix`（+ 它携带的 `ProbeContext`：探测时刻 / 平台 / 会话状态）。
2. **不透明句柄**（**不可**序列化、**不得**跨进程）：`ResolvedWindow` / `ResolvedElement`。
3. **trait 形状**：`PlatformService` / `WindowProvider` / `UiAutomationProvider`（架构 v2 §13.1.1）。
4. **能力矩阵不变量**：`CapabilityMatrix::validate()` 把 `docs/spec/capability-matrix.md` §4 的
   4 条不变量变成**会失败的返回值**，而不是一句注释。

## 边界（不做什么）

- **不实现任何平台**（Win32 / UIA / AT-SPI / CDP 归 `crates/platform/windows` 等，TASK-017 / 018 / 040 / 048）。
- **不做策略判定**（默认拒绝的放行点是 `crates/policy`，TASK-021）；本 crate 只给矩阵数据 + 不变量。
- **不做 Tool ↔ capability 绑定**（归 `docs/spec/tool-schema.md`）。
- **不新增 `ErrorCode`**：复用 `assistant_protocol::ErrorCode`（新增 = ADR）。
- **零平台实现依赖**：本 crate 的 `[dependencies]` 只许有 `serde`（已登记）与 `assistant-protocol`。

## 不变量

1. **句柄不得跨进程**（铁律 8）：`ResolvedWindow` / `ResolvedElement` **不派生** `Serialize` / `Deserialize`，
   且内部**不持有**任何平台对象（只持 Host 本地 id）。机器校验见 `tests/handles_not_serializable.rs`。
2. **错误必带 `ErrorCode`**（铁律 1）：`PlatformError` 的三个字段都不可为空；`evidence_ref` 由
   `ErrorDefinition` 派生，调用方不得手写。
3. **能力矩阵单调升级**：风险只升不降；`L3+ ⇒ Approval = required`、`L5 ⇒ Approval = forbidden`。
4. **坐标只有一个规范空间**：内部统一 = **全局逻辑坐标、原点主显示器左上**（架构 v2 §6.9 规则 1）；
   换算只经 `NormalizedPoint::to_physical`，不允许各平台自己约定。
5. **判定逻辑是纯函数**：`validate()` / 坐标换算 / 指纹比较都不碰 IO，同样的输入必得同样的输出
   （`docs/memory/pitfalls.md` 2026-09-24 的 flaky 教训）。

## 测试

单测住在 `tests/`（正向 + **负向**，ADR-0019 N1），按**契约**而不是按内部实现组织：

| 文件 | 覆盖 |
|---|---|
| `tests/capability_matrix.rs` | 4 条不变量逐条正/负用例 + `ProbeContext` 读取侧不漂移 |
| `tests/target_descriptor.rs` | 候选链 / 解析策略 / `validate()` 的正负用例 |
| `tests/geometry.rs` | 坐标校验、`to_physical` 的舍入与 **`i32` 边界**（含越界一格） |
| `tests/fingerprint.rs` | 指纹形态（前缀 / 长度 / 大小写 / 非 hex） |
| `tests/handles_not_serializable.rs` | **铁律 8** 的机器校验：扫 `src/handle.rs` 源码，禁止句柄获得序列化能力 |

**本 crate 的测试不使用 `expect` / `unwrap` / `panic!`**（`tests/common/mod.rs` 的 `ok_or_fail`
等价替代）：`[lints]` 继承 workspace 的 `deny`，而 TASK-016 的 Out of scope 禁止 `#[allow]` 放宽 ——
所以整个 crate（含测试）**零 `#[allow]`**。断言优先比较**整个 `Result`**（`.ok()` / `.err()` / `matches!`），
这样"该失败却成功"和"该成功却失败"都会被抓住。

## 已知限制

- trait 方法用 **RPITIT**（`-> impl Future<Output = ...> + Send`）而不是 `async fn` in trait：
  这样既不需要 `async-trait` 依赖，也不会触发 `async_fn_in_trait` lint（**零 `#[allow]`**）。
  **代价**：这些 trait **不是 `dyn`-compatible**。若 TASK-017 需要 `dyn`，届时按需裁决
  （`#[trait_variant]` 或手写 `Pin<Box<dyn Future>>`），仍不引入依赖。
- `NormalizedPoint::to_physical` 的 `f64 → i32` 换算**不用 `as`**：`as` 在越界时**饱和**
  （`1e9 as i32 == i32::MAX`）且不报错（铁律 1 的静默失败形态），而 `clippy::cast_possible_truncation`
  （pedantic，本 workspace = deny）会拦下它，标准库又没有 `TryFrom<f64> for i32`。
  实现改用**整数域逐位合成**（`f64::from(u32)` 精确 + 比较 + 减法，见 `src/geometry.rs` 的
  `exact_i32_from_f64`）：**零 `unsafe`、零 `#[allow]`**，越界一律返回 `TargetNotFound`。
  若将来人类允许一个窄范围的 `#[allow(clippy::cast_possible_truncation)]`，可换回 `as` + 范围检查。
- `CapabilityMatrix` 在本 crate 指**运行时探测结果**（架构 v2 §13.1.2）；
  `protocol/capability-matrix/capability-1.0.json` 是**稳定能力标识目录**（`<layer>.<capability>`）。
  两者同名不同物，改名（→ `CapabilityCatalog`）要改契约 → 需 ADR（`docs/PARKING_LOT.md` 待裁决项）。

## 相关文档

`cross-platform-ai-assistant-architecture-v2.md` §3.1 / §6.2 / §6.9 / §7.3 / §13.1.1 / §13.1.2、
`docs/spec/capability-matrix.md`、`docs/spec/naming.md` §7、`tasks/TASK-016-*.md`。
