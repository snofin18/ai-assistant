//! 明文密钥材料的**唯一**容器。
//!
//! 边界：只负责"装住明文 + 用后清零 + 不泄露到 Debug/Serialize"，不做存取、不做审计。
//!
//! 不变量：
//!   1. 内部是 [`zeroize::Zeroizing`]：`Drop` 时清零（`Zeroizing` 自带 `Drop` 实现）
//!   2. **不实现** `Display` / `Serialize` / `Clone` —— 前者会让 `{}` 直接打印明文，
//!      中者会让它意外进 JSON 日志，后者会让明文多出不受控的副本
//!   3. `Debug` 固定输出 `SecretValue(<redacted> N bytes)`：只报长度，不报内容
//!   4. 取明文只有一个入口 [`SecretValue::expose`] —— 名字刻意取得刺眼，`rg expose` 即可列出全部调用点
//!
//! 相关：架构 v2 §12.5（严禁明文入 SQLite / 日志 / prompt / 崩溃报告 / 审计事件）

use std::fmt;

use zeroize::{Zeroize, Zeroizing};

use crate::error::SecretError;

/// 密钥值的最大字节长度。
///
/// 为什么是 8 KiB：密钥库条目应当很小（API key / token / 短口令）。上限的作用是**拒绝**
/// 把证书、私钥包、整份配置文件塞进来 —— 那些该走 blob 池并单独设计，不该悄悄变成"一条密钥"。
pub const MAX_SECRET_VALUE_LEN: usize = 8 * 1024;

/// 明文密钥材料。
///
/// 生命周期：从 [`SecretValue::new`] 到 `Drop` 之间，明文只存在于本对象内部的
/// `Zeroizing<String>` 里（`expose()` 借出去的那份除外，见其文档）。
pub struct SecretValue {
    inner: Zeroizing<String>,
}

impl SecretValue {
    /// 从明文构造。
    ///
    /// # Errors
    /// 超过 [`MAX_SECRET_VALUE_LEN`] → [`SecretError::InvalidValue`]（**不截断**：静默截断会把
    /// 一个"写错长度的密钥"变成"认证失败"，把真正的原因藏起来）。
    pub fn new(plaintext: impl Into<String>) -> Result<Self, SecretError> {
        let plaintext = plaintext.into();
        if plaintext.len() > MAX_SECRET_VALUE_LEN {
            return Err(SecretError::InvalidValue {
                detail: format!(
                    "密钥值 {} 字节，超过上限 {MAX_SECRET_VALUE_LEN} 字节",
                    plaintext.len()
                ),
            });
        }
        Ok(Self {
            inner: Zeroizing::new(plaintext),
        })
    }

    /// 取出明文。
    ///
    /// 安全性：调用方拿到的 `&str` **不受本 crate 保护** —— 它一旦被拷贝出去，那份副本的
    /// 清零责任就转移给调用方。这是本 crate 唯一的明文出口。
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.inner
    }

    /// 明文长度（字节）。**不含内容**，可安全进日志。
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// 是否为空。
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

impl Zeroize for SecretValue {
    fn zeroize(&mut self) {
        self.inner.zeroize();
    }
}

impl fmt::Debug for SecretValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // 只报长度：排查"值是不是被截断了"时够用，且不泄露内容
        write!(f, "SecretValue(<redacted> {} bytes)", self.inner.len())
    }
}
