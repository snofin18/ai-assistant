//! # verify-schemas 子命令
//!
//! 职责：检查 5 份 `protocol/**/*.json` schema 文件的存在 + JSON 合法性 +
//! 顶层版本字段 = 期望值 + ErrorCode 类别数 = 13。
//!
//! ## 边界（不做什么）
//! - 不实现完整 JSON Schema 校验（无第三方依赖；仅做结构性最低校验）
//! - 不做 codegen（那是 `codegen` 子命令）
//!
//! ## 不变量
//! 1. 五份 schema 必须全部存在 + JSON 合法 + 版本号匹配 → 否则 verdict FAILED
//! 2. error-codes 必须含**恰好 13** 个 category
//! 3. capability-matrix 必须有 ≥ 1 个 capability
//! 4. 任何 schema 的 `version` 字段必须是 string

#![allow(clippy::doc_markdown, clippy::format_push_string, clippy::if_not_else, clippy::needless_collect, clippy::needless_pass_by_value, clippy::collapsible_if, clippy::unnecessary_map_or, clippy::missing_const_for_fn, clippy::nonminimal_bool, clippy::unnecessary_operation, clippy::uninlined_format_args)]  // per ADR-0035; codegen output intentionally triggers these specific lints