//! 状态指纹（架构 v2 §7.3）。
//!
//! 职责：给"这个目标还是不是刚才那个状态"一个**不可与其它字符串混淆**的类型。
//! 边界：**不计算**指纹 —— 计算需要平台树访问（TASK-017）；本 crate 只定义**形状与校验**。
//!
//! ## 不变量
//! 1. 合法值**必须**是 `sha256:` + 64 位**小写**十六进制（与审计 hash chain 同口径，架构 v2 §15.3）。
//! 2. 相等即"同一状态" —— 因此 `PartialEq` 就是 §7.3 的用途 1/2/3（变化检测 / 幂等判定 / 回放对齐）。
//! 3. 指纹**必须足够敏感又不过度敏感**（§7.3 末段）：忽略字段由 per-adapter 配置，
//!    **不在**本类型里做（否则就成了隐式全局规则）。

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::ErrorCode;
use crate::error::{PlatformError, PlatformResult};

/// 目标状态指纹（`sha256:<64 位小写 hex>`）。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Fingerprint {
    /// 完整形态（含 `sha256:` 前缀）。
    value: String,
}

impl Fingerprint {
    /// 前缀常量（与审计 hash chain 同口径）。
    pub const PREFIX: &'static str = "sha256:";

    /// 校验并构造指纹。
    ///
    /// # Errors
    /// 前缀不是 `sha256:`，或摘要不是 64 位小写十六进制 → `VerifyFailed`
    /// （指纹是**验证**用的输入；格式错说明平台实现给了坏值，绝不能"当成有效指纹"继续比）。
    pub fn parse(value: impl Into<String>) -> PlatformResult<Self> {
        let value = value.into();
        let Some(digest) = value.strip_prefix(Self::PREFIX) else {
            return Err(PlatformError::new(
                ErrorCode::VerifyFailed,
                format!("fingerprint must start with `{}`", Self::PREFIX),
            ));
        };
        if digest.len() != 64 {
            return Err(PlatformError::new(
                ErrorCode::VerifyFailed,
                format!(
                    "fingerprint digest must be 64 hex chars, got {}",
                    digest.len()
                ),
            ));
        }
        if !digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(PlatformError::new(
                ErrorCode::VerifyFailed,
                "fingerprint digest must be lowercase hex (0-9a-f)",
            ));
        }
        Ok(Self { value })
    }

    /// 完整形态（含前缀）。
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.value
    }

    /// 摘要部分（不含 `sha256:` 前缀）。
    ///
    /// 用 `get(..)` 而不是切片：`indexing_slicing` 在本 workspace 是 deny（AGENTS.md §5.3）。
    #[must_use]
    pub fn digest(&self) -> &str {
        self.value.get(Self::PREFIX.len()..).unwrap_or_default()
    }
}

impl fmt::Display for Fingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.value)
    }
}
