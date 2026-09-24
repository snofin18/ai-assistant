//! 单元级契约：`SecretName` 校验 / `SecretValue` 脱敏与 `Zeroize` / 内存后端语义 / 错误码映射。
//!
//! 这些断言**不碰任何真实 OS 密钥库**（CI 上没有可用的），见 `crates/secrets/README.md`。
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use assistant_protocol::ErrorCategory;
use assistant_secrets::{
    InMemorySecretStore, KeyringSecretStore, MAX_SECRET_NAME_LEN, SecretError, SecretName,
    SecretStore, SecretValue,
};
use zeroize::Zeroize;

const CANARY: &str = "sk-CANARY-do-not-leak-9f3a1c";

#[test]
fn test_secret_name_with_valid_charset_is_accepted() {
    let name = SecretName::new("model.openai.api_key").expect("合法名字");
    assert_eq!(name.as_str(), "model.openai.api_key");
    assert_eq!(name.to_string(), "model.openai.api_key");
}

#[test]
fn test_secret_name_empty_is_rejected() {
    let error = SecretName::new("").expect_err("空名字必须被拒绝");
    assert_eq!(error.reason_code(), "invalid_name");
    assert!(matches!(error, SecretError::InvalidName { .. }));
}

#[test]
fn test_secret_name_over_limit_is_rejected() {
    let too_long = "a".repeat(MAX_SECRET_NAME_LEN + 1);
    let error = SecretName::new(too_long).expect_err("超长名字必须被拒绝");
    assert_eq!(error.reason_code(), "invalid_name");
}

#[test]
fn test_secret_name_with_disallowed_chars_is_rejected() {
    // 冒号是重点：后端会把 service 与 username 拼成 `service:username`，名字里再带冒号就有歧义
    for candidate in ["a:b", "a b", "a/b", "密钥", "a\nb"] {
        let error = SecretName::new(candidate).expect_err("非法字符必须被拒绝");
        assert_eq!(
            error.reason_code(),
            "invalid_name",
            "candidate={candidate:?}"
        );
    }
}

#[test]
fn test_secret_value_expose_returns_plaintext() {
    let value = SecretValue::new(CANARY).expect("构造");
    assert_eq!(value.expose(), CANARY);
    assert_eq!(value.len(), CANARY.len());
    assert!(!value.is_empty());
}

#[test]
fn test_secret_value_debug_does_not_leak_plaintext() {
    let value = SecretValue::new(CANARY).expect("构造");
    let rendered = format!("{value:?}");
    assert!(!rendered.contains(CANARY), "Debug 泄露了明文：{rendered}");
    assert!(
        rendered.contains("<redacted>"),
        "Debug 应显式标注已脱敏：{rendered}"
    );
}

#[test]
fn test_secret_value_over_limit_is_rejected() {
    let too_long = "x".repeat(assistant_secrets::MAX_SECRET_VALUE_LEN + 1);
    let error = SecretValue::new(too_long).expect_err("超长值必须被拒绝");
    assert_eq!(error.reason_code(), "invalid_value");
    assert!(matches!(error, SecretError::InvalidValue { .. }));
}

#[test]
fn test_secret_value_implements_zeroize() {
    // 编译期断言：`Zeroize` 是"用后清零"的机器可读证据。
    // 注意：**不**声称运行期可观测 —— 无 `unsafe` 无法在测试里观察已清零的内存。
    fn assert_zeroize<T: Zeroize>() {}
    assert_zeroize::<SecretValue>();

    let mut value = SecretValue::new(CANARY).expect("构造");
    value.zeroize();
    assert!(value.expose().is_empty(), "显式 zeroize 后内部字符串应为空");
}

#[test]
fn test_memory_store_set_then_get_roundtrips() {
    let store = InMemorySecretStore::new();
    let name = SecretName::new("model.openai.api_key").expect("名字");
    let value = SecretValue::new(CANARY).expect("值");

    store.set(&name, &value).expect("写");
    let read_back = store.get(&name).expect("读").expect("应存在");
    assert_eq!(read_back.expose(), CANARY);
    assert_eq!(store.len().expect("条目数"), 1);
}

#[test]
fn test_memory_store_get_missing_returns_none_not_error() {
    let store = InMemorySecretStore::new();
    let name = SecretName::new("model.openai.api_key").expect("名字");
    // 铁律 1：「没有这条」是 Ok(None)，不是 Err
    assert!(store.get(&name).expect("读不存在的键不应报错").is_none());
    assert!(!store.contains(&name).expect("探测"));
    assert!(store.is_empty().expect("空判断"));
}

#[test]
fn test_memory_store_delete_missing_returns_false() {
    let store = InMemorySecretStore::new();
    let name = SecretName::new("model.openai.api_key").expect("名字");
    assert!(!store.delete(&name).expect("删不存在的键不应报错"));
}

#[test]
fn test_memory_store_delete_then_contains_is_false() {
    let store = InMemorySecretStore::new();
    let name = SecretName::new("model.openai.api_key").expect("名字");
    let value = SecretValue::new(CANARY).expect("值");
    store.set(&name, &value).expect("写");

    assert!(
        store.delete(&name).expect("删").eq(&true),
        "第一次删应真的删掉"
    );
    assert!(!store.contains(&name).expect("探测"));
    assert!(store.get(&name).expect("读").is_none());
}

#[test]
fn test_memory_store_overwrite_keeps_single_entry() {
    let store = InMemorySecretStore::new();
    let name = SecretName::new("token").expect("名字");
    store
        .set(&name, &SecretValue::new("first").expect("值"))
        .expect("写 1");
    store
        .set(&name, &SecretValue::new("second").expect("值"))
        .expect("写 2");

    assert_eq!(store.len().expect("条目数"), 1);
    assert_eq!(
        store.get(&name).expect("读").expect("存在").expose(),
        "second"
    );
}

#[test]
fn test_memory_store_debug_does_not_leak_plaintext() {
    let store = InMemorySecretStore::new();
    let name = SecretName::new("token").expect("名字");
    store
        .set(&name, &SecretValue::new(CANARY).expect("值"))
        .expect("写");

    let rendered = format!("{store:?}");
    assert!(
        !rendered.contains(CANARY),
        "store 的 Debug 泄露了明文：{rendered}"
    );
    assert!(
        !rendered.contains("token"),
        "store 的 Debug 不该列出键名：{rendered}"
    );
}

#[test]
fn test_keyring_store_rejects_invalid_service() {
    for candidate in ["", "with space", "with:colon"] {
        let error = KeyringSecretStore::new(candidate).expect_err("非法 service 必须被拒绝");
        assert_eq!(
            error.reason_code(),
            "invalid_name",
            "candidate={candidate:?}"
        );
    }
}

#[test]
fn test_keyring_store_accepts_valid_service_and_exposes_it() {
    let store = KeyringSecretStore::new("ai-assistant").expect("合法 service");
    assert_eq!(store.service(), "ai-assistant");
    // 只构造、**不**调用 get/set —— CI 上没有可用的真实密钥库（README「已知限制」）
}

#[test]
fn test_secret_error_reason_codes_are_stable() {
    // reason_code 进日志与审计，改名 = 破坏可回放性，故逐条钉住
    let cases: Vec<(SecretError, &str, ErrorCategory)> = vec![
        (
            SecretError::InvalidName {
                detail: String::new(),
            },
            "invalid_name",
            ErrorCategory::Fatal,
        ),
        (
            SecretError::InvalidValue {
                detail: String::new(),
            },
            "invalid_value",
            ErrorCategory::Fatal,
        ),
        (
            SecretError::KeychainUnavailable {
                detail: String::new(),
            },
            "keychain_unavailable",
            ErrorCategory::PlatformPermission,
        ),
        (
            SecretError::AccessDenied {
                detail: String::new(),
            },
            "access_denied",
            ErrorCategory::PlatformPermission,
        ),
        (
            SecretError::UnsupportedOperation {
                detail: String::new(),
            },
            "unsupported_operation",
            ErrorCategory::CapabilityMissing,
        ),
        (
            SecretError::BackendFailure {
                detail: String::new(),
            },
            "backend_failure",
            ErrorCategory::Fatal,
        ),
        (
            SecretError::CorruptEntry {
                detail: String::new(),
            },
            "corrupt_entry",
            ErrorCategory::Fatal,
        ),
        (
            SecretError::AuditRejected {
                detail: String::new(),
            },
            "audit_rejected",
            ErrorCategory::Fatal,
        ),
    ];

    for (error, reason, category) in cases {
        assert_eq!(error.reason_code(), reason);
        assert_eq!(error.error_category(), category, "reason={reason}");
        assert_eq!(error.requires_human(), category == ErrorCategory::Fatal);
    }
}
