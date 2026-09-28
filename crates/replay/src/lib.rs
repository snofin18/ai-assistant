//! # assistant-replay —— 录制树快照与离线平台回放
//!
//! 职责：定义 Recording v1 数据模型，严格校验树与引用，并用它实现离线
//! `UiAutomationProvider` / `WindowProvider`，让逻辑层回归无需真实桌面。
//!
//! 边界：不采集真实桌面、不驱动应用、不修改 `assistant-platform-api` 的公共形状。
//! v0 只回放树解析、窗口元数据、整窗指纹与录制文本；未录制的写动作显式返回
//! `CapabilityMissing`，绝不假装成功。
//!
//! 相关：架构 v2 §17.4、`docs/spec/testing.md`、TASK-034。

#![deny(unsafe_code)]

mod error;
mod model;
mod provider;

pub use error::ReplayError;
pub use model::{RECORDING_VERSION, RecordedNode, RecordedReadText, RecordedWindow, Recording};
pub use provider::{ReplaySession, ReplayUiAutomationProvider, ReplayWindowProvider};
