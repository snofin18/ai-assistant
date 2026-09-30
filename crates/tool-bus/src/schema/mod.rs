//! JSON Schema（draft-07 **子集**）校验：**不支持即拒绝**（fail-closed）。
//!
//! ## 为什么手写最小子集（TASK-020 Q2 裁决，见任务卡 §5）
//!
//! `jsonschema` crate 会把 `borrow-or-share`（`MIT-0`）带进依赖图，而 `deny.toml` 白名单不含它
//! （放宽 = 漂移触发器 ⑥，需 ADR）；`rmcp` 自带校验经查**不存在**（泛型 `ServerHandler` 直接调用
//! `call_tool`，从不读 `get_tool`）。故选**零新增依赖**的自写子集。
//!
//! 判据方向很重要：**宁可让工具注册失败，也绝不放过一条自己无法强制的约束**。
//! 「不支持就放行」会把约束变成装饰，直接违反铁律 1 与铁律 4。
//!
//! ## 支持的子集（白名单见 [`SUPPORTED_KEYWORDS`]）
//!
//! 类型/取值 `type`/`enum`/`const`；对象 `properties`/`required`/`additionalProperties`/
//! `minProperties`/`maxProperties`；数组 `items`（**单 schema 形式**）/`minItems`/`maxItems`；
//! 字符串 `minLength`/`maxLength`（按 **Unicode 码点**计数）；数字 `minimum`/`maximum`/
//! `exclusiveMinimum`/`exclusiveMaximum`；组合 `allOf`/`anyOf`/`oneOf`/`not`。
//!
//! ## 忽略的注解（不算「支持」，但也不拒绝）
//!
//! `title`/`description`/`default`/`examples`/`$comment`/`deprecated`/`readOnly`/`writeOnly`/
//! `$id` —— 它们**不改变校验结果**，忽略是忠实的（`$id` 无 `$ref` 故无解析语义）。
//!
//! ## 永久放弃 / 尚未实现 / 其它方言
//!
//! 三张表逐条给出关键字 + 理由/方言 +（能给的）替代：永久放弃见 [`REJECTED_FOREVER_KEYWORDS`]
//! （`$ref`/`definitions`、`pattern`/`patternProperties`/`format`、`if`-`then`-`else`/
//! `dependencies`、元组 `items`/`additionalItems`、`contentEncoding`/`contentMediaType`），
//! 缺口见 [`REJECTED_FOR_NOW_KEYWORDS`]（`multipleOf`/`uniqueItems`/`contains`/`propertyNames`），
//! 其它草案见 [`NON_DRAFT07_KEYWORDS`]。**三张表之外的任何关键字 → 一律拒绝**（兜底同时挡拼写错误）。
//!
//! 替代方案一律是「`enum`/`const` 收窄取值」或「**由 handler 校验值**」—— 校验器管**形状**，
//! handler 管**值的语义安全**（参数化查询、路径规范化、不拼 shell）。
//!
//! ## `$schema` 不忽略，改做**方言校验**
//!
//! `$schema` 声明的是「这份文档按哪一版语义解释」：缺省 = draft-07，声明 draft-07 = 放行，
//! 声明**别的方言 = 拒绝**。否则我们会用 draft-07 的语义去校验一份 2020-12 的文档 —— 那正是
//! 铁律 1 禁止的「用自己的语义冒充别人的语义」。
//!
//! 相关：`docs/spec/tool-schema.md` §4 不变量 2 / 6 / **8**、架构 v2 §5.2、
//! `crates/tool-bus/README.md`「已知限制」、`docs/memory/rejected.md`。

mod instance;
mod registration;
mod shared;

pub use instance::validate_arguments;
pub use registration::{
    ANNOTATION_KEYWORDS, NON_DRAFT07_KEYWORDS, REJECTED_FOR_NOW_KEYWORDS,
    REJECTED_FOREVER_KEYWORDS, SUPPORTED_KEYWORDS, collect_unenforceable_constructs,
};
