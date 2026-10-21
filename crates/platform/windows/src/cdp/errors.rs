//! CDP runtime 常见错误。

use assistant_platform_api::{ErrorCode, PlatformError};

/// CDP 失败 wrapper。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct CdpError {
    target_url: String,
    target_title: String,
    cause: String,
}

impl CdpError {
    /// 构造 CDP 失败 wrapper。
    #[must_use]
    pub const fn new(target_url: String, target_title: String, cause: String) -> Self {
        Self {
            target_url,
            target_title,
            cause,
        }
    }

    /// 目标 URL。
    #[must_use]
    pub fn target_url(&self) -> &str {
        &self.target_url
    }

    /// 目标标题。
    #[must_use]
    pub fn target_title(&self) -> &str {
        &self.target_title
    }

    /// 原因。
    #[must_use]
    pub fn cause(&self) -> &str {
        &self.cause
    }

    /// 转成 `PlatformError`。
    #[must_use]
    pub fn to_platform_error(&self, code: ErrorCode) -> PlatformError {
        PlatformError::new(
            code,
            format!(
                "CDP target {:?} / {:?}: {}",
                self.target_url, self.target_title, self.cause
            ),
        )
    }
}
