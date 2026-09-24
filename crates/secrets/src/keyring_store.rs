//! `keyring` 4.2 真实后端（Windows 凭据管理器 / macOS Keychain / Linux Secret Service）。
//!
//! 边界：只做 OS 密钥库读写 —— **不做**审计（由 [`crate::AuditedSecretStore`] 装饰）、
//! **不做**缓存（缓存明文会制造出 `zeroize` 管不到的副本）。
//!
//! 不变量：
//!   1. 明文只在 `get_password()` 返回的 `String` 里短暂存在，立刻搬进 [`SecretValue`]
//!      （内部 `Zeroizing`）；本模块不保留任何副本
//!   2. 错误里只放 `keyring::Error` 的文本 —— 平台报错不含密钥内容
//!   3. `service` 在**构造期**固定，`username` 用密钥名 —— Windows 的 target name 因此是
//!      `<service>:<name>`；`SecretName` 禁止冒号正是为了不让这个拼接产生歧义
//!   4. **单测不碰本模块的真实路径**（CI 上没有可用的 OS 密钥库）→ 见 `crates/secrets/README.md`
//!
//! 相关：架构 v2 §12.5、`docs/DEPENDENCIES.md`（`keyring` 4.2 已登记）

use crate::error::{SecretError, SecretResult};
use crate::secret_name::is_allowed_name_char;
use crate::{SecretName, SecretStore, SecretValue};

/// `service` 的最大字节长度。
///
/// 与 [`crate::MAX_SECRET_NAME_LEN`] 同量级：保证 `<service>:<name>` 拼出来不触平台上限。
pub const MAX_SERVICE_NAME_LEN: usize = 64;

/// 基于 `keyring` 的 OS 密钥库后端。
#[derive(Debug, Clone)]
pub struct KeyringSecretStore {
    service: String,
}

impl KeyringSecretStore {
    /// 用应用标识（`service`）构造。
    ///
    /// # Errors
    /// `service` 为空 / 超长 / 含字符集外的字节 → [`SecretError::InvalidName`]。
    pub fn new(service: impl Into<String>) -> SecretResult<Self> {
        let service = service.into();
        if service.is_empty() {
            return Err(SecretError::InvalidName {
                detail: "service 不得为空".to_string(),
            });
        }
        if service.len() > MAX_SERVICE_NAME_LEN {
            return Err(SecretError::InvalidName {
                detail: format!(
                    "service {} 字节，超过上限 {MAX_SERVICE_NAME_LEN} 字节",
                    service.len()
                ),
            });
        }
        if service.bytes().any(|byte| !is_allowed_name_char(byte)) {
            return Err(SecretError::InvalidName {
                detail: "service 只允许 ASCII 字母 / 数字 / . / _ / -".to_string(),
            });
        }
        Ok(Self { service })
    }

    /// 本后端使用的应用标识。
    #[must_use]
    pub fn service(&self) -> &str {
        &self.service
    }

    /// 查询后端是否已就绪（不会因后端缺失而 panic）。
    ///
    /// 语义：`Ok(())` = 平台密钥库可用；`Err` = 不可用（平台不支持 / Secret Service 未运行等）。
    ///
    /// # Errors
    /// 后端不可用 → [`SecretError::KeychainUnavailable`] / [`SecretError::AccessDenied`]。
    pub fn check_backend() -> SecretResult<()> {
        match keyring::Entry::store_status() {
            Ok(()) => Ok(()),
            Err(error) => Err(map_error(error)),
        }
    }

    fn entry(&self, name: &SecretName) -> SecretResult<keyring::Entry> {
        keyring::Entry::new(&self.service, name.as_str()).map_err(|error| map_error(&error))
    }
}

impl SecretStore for KeyringSecretStore {
    fn get(&self, name: &SecretName) -> SecretResult<Option<SecretValue>> {
        let entry = self.entry(name)?;
        match entry.get_password() {
            Ok(password) => match SecretValue::new(password) {
                Ok(value) => Ok(Some(value)),
                Err(error) => Err(SecretError::CorruptEntry {
                    detail: format!("库中的值不满足当前上限：{error}"),
                }),
            },
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(other) => Err(map_error(&other)),
        }
    }

    fn set(&self, name: &SecretName, value: &SecretValue) -> SecretResult<()> {
        let entry = self.entry(name)?;
        entry
            .set_password(value.expose())
            .map_err(|error| map_error(&error))
    }

    fn delete(&self, name: &SecretName) -> SecretResult<bool> {
        let entry = self.entry(name)?;
        match entry.delete_credential() {
            Ok(()) => Ok(true),
            Err(keyring::Error::NoEntry) => Ok(false),
            Err(other) => Err(map_error(&other)),
        }
    }

    fn contains(&self, name: &SecretName) -> SecretResult<bool> {
        // 后端没有"仅探测存在性"的原语，只能走一次读 —— 代价是明文会被短暂取出并随即清零。
        // 这里不复用默认实现只是为了让"为什么仍要读一次"留在代码里。
        self.get(name).map(|value| value.is_some())
    }
}

/// `keyring::Error` → [`SecretError`]。
///
/// 为什么保留 `_ =>` 分支：`keyring::Error` 是 `#[non_exhaustive]`，升级 `keyring` 会新增变体；
/// 落进兜底分支比 `compile_error` 更安全（未知错误按后端失败处理，仍不静默）。
fn map_error(error: &keyring::Error) -> SecretError {
    match error {
        keyring::Error::NoStorageAccess(inner) => SecretError::AccessDenied {
            detail: inner.to_string(),
        },
        keyring::Error::NoDefaultStore => SecretError::KeychainUnavailable {
            detail: "平台没有可用的默认密钥库（平台不支持或后端初始化失败）".to_string(),
        },
        keyring::Error::PlatformFailure(inner) => SecretError::BackendFailure {
            detail: inner.to_string(),
        },
        keyring::Error::NotSupportedByStore(reason) => SecretError::UnsupportedOperation {
            detail: reason.clone(),
        },
        keyring::Error::Invalid(parameter, reason) => SecretError::KeychainUnavailable {
            detail: format!("{parameter}: {reason}"),
        },
        keyring::Error::TooLong(attribute, limit) => SecretError::InvalidName {
            detail: format!("{attribute} 超过平台上限 {limit}"),
        },
        keyring::Error::BadEncoding(_) => SecretError::CorruptEntry {
            detail: "取回的内容不是合法 UTF-8".to_string(),
        },
        keyring::Error::BadDataFormat(_, inner) => SecretError::CorruptEntry {
            detail: inner.to_string(),
        },
        keyring::Error::BadStoreFormat(reason) => SecretError::CorruptEntry {
            detail: reason.clone(),
        },
        keyring::Error::Ambiguous(_) => SecretError::BackendFailure {
            detail: "同名条目多于一条，无法确定用哪条".to_string(),
        },
        // `NoEntry` 在调用点已单独处理；这里落兜底只为穷尽匹配。
        _ => SecretError::BackendFailure {
            detail: error.to_string(),
        },
    }
}
