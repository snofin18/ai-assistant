//! 密钥名（架构 v2 L2624 的 `key_ref` 取值空间）。
//!
//! 边界：只做**名字**的构造与校验 —— 不碰任何后端、不碰明文、不做 IO。
//!
//! 不变量：
//!   1. **构造即校验**：不合法直接返回 `SecretError::InvalidName`，不存在"先造出来再校验"的路径
//!   2. **密钥名不是秘密**：它进日志、进审计记录、进错误文本；受保护的是明文
//!   3. 字符集与长度上限是**契约**（[`MAX_SECRET_NAME_LEN`] + [`SecretName::new`] 的判据）：
//!      放宽它们会让后端的 `service:username` 拼接产生歧义
//!
//! 相关：`docs/spec/naming.md` §5（newtype id）、架构 v2 §12.5 / L2624

use std::fmt;

use crate::error::SecretError;

/// 密钥名的最大字节长度。
///
/// 为什么是 128：Windows 凭据管理器的 `UserName` 字段上限是 256 字符，而后端会把
/// `service` 与 `username` 拼成 target name；留一半余量给 service 前缀，且远超任何
/// 合理的人写名字（如 `model.openai.api_key`）。
pub const MAX_SECRET_NAME_LEN: usize = 128;

/// 密钥名允许的字符：ASCII 字母 / 数字 / `.` / `_` / `-`。
///
/// 为什么不放开任意 Unicode：后端会把名字拼进平台层的 target name（Windows 是
/// `service:username`），冒号、空格、控制字符会造成**拼接歧义**或平台侧截断。
pub const fn is_allowed_name_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'.' || byte == b'_' || byte == b'-'
}

/// 密钥名（newtype）。
///
/// 语义：一个**引用**，不是密钥本身 —— 因此可以安全地进日志与审计记录。
///
/// 错误语义：构造失败只可能是 [`SecretError::InvalidName`]。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SecretName(String);

impl SecretName {
    /// 校验并构造。
    ///
    /// # Errors
    /// 空 / 超长 / 含字符集外的字节 → [`SecretError::InvalidName`]。
    pub fn new(value: impl Into<String>) -> Result<Self, SecretError> {
        let value = value.into();
        if value.is_empty() {
            return Err(SecretError::InvalidName {
                detail: "密钥名不得为空".to_string(),
            });
        }
        if value.len() > MAX_SECRET_NAME_LEN {
            return Err(SecretError::InvalidName {
                detail: format!(
                    "密钥名 {} 字节，超过上限 {MAX_SECRET_NAME_LEN} 字节",
                    value.len()
                ),
            });
        }
        if let Some(position) = value.bytes().position(|byte| !is_allowed_name_char(byte)) {
            return Err(SecretError::InvalidName {
                detail: format!(
                    "密钥名第 {} 字节不是允许的字符（只允许 ASCII 字母 / 数字 / . / _ / -）",
                    position + 1
                ),
            });
        }
        Ok(Self(value))
    }

    /// 名字本体（**可安全进日志与审计**）。
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SecretName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
