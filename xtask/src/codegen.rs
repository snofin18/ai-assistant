//! # codegen 子命令
//!
//! 职责：从 `protocol/**/*.json` 生成 `crates/protocol/src/generated/*.rs`。
//!
//! ## 不变量
//! 1. 输出**确定**：同一 schema → 同一 generated 内容
//! 2. 生成文件首行 = `// GENERATED — DO NOT EDIT`
//! 3. `codegen --check`：drift = 退出 1（铁律 1）
//!
//! ## Lint policy (per ADR-0035)
//! - workspace 继承父约束
//! - 本模块按需 per-line `#[allow(...)]` + 注释

#![allow(clippy::format_push_string, clippy::if_not_else, clippy::needless_collect, clippy::needless_pass_by_value, clippy::doc_markdown)]  // per ADR-0035; codegen output intentionally triggers these specific lints