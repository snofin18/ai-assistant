//! `Fingerprint` 的形态校验：正向 + 负向（ADR-0019 N1）。
//!
//! 为什么放在 `tests/`：只依赖公开 API。断言风格见 `tests/common/mod.rs`。

mod common;

use assistant_platform_api::{ErrorCode, Fingerprint};
use common::ok_or_fail;

/// 一个合法指纹（全 `0` 摘要）。
fn valid() -> String {
    format!("sha256:{}", "0".repeat(64))
}

#[test]
fn test_parse_accepts_canonical_form() {
    let fingerprint = ok_or_fail(Fingerprint::parse(valid()), "规范形态必须可解析");
    assert_eq!(fingerprint.as_str(), valid());
    assert_eq!(fingerprint.digest().len(), 64);
    assert_eq!(fingerprint.to_string(), valid());
}

#[test]
fn test_parse_rejects_missing_prefix() {
    let parsed = Fingerprint::parse("0".repeat(64));
    assert_eq!(
        parsed.err().map(|error| error.code()),
        Some(ErrorCode::VerifyFailed)
    );
}

#[test]
fn test_parse_rejects_wrong_digest_length() {
    for length in [63_usize, 65] {
        assert!(
            Fingerprint::parse(format!("sha256:{}", "0".repeat(length))).is_err(),
            "摘要长度 {length} 必须被拒绝"
        );
    }
}

#[test]
fn test_parse_rejects_uppercase_and_non_hex() {
    // 与审计 hash chain 同口径：只认小写 hex（大写会让同一状态算出两种指纹）。
    assert!(
        Fingerprint::parse(format!("sha256:{}", "A".repeat(64))).is_err(),
        "大写 hex 必须被拒绝"
    );
    assert!(Fingerprint::parse(format!("sha256:{}", "z".repeat(64))).is_err());
    assert!(
        Fingerprint::parse(format!("sha256:{}!", "0".repeat(63))).is_err(),
        "非 hex 字符必须被拒绝"
    );
}

#[test]
fn test_digest_strips_only_the_prefix() {
    let fingerprint = ok_or_fail(
        Fingerprint::parse(format!("sha256:{}", "a".repeat(64))),
        "规范形态必须可解析",
    );
    assert_eq!(fingerprint.digest(), "a".repeat(64));
    assert_eq!(fingerprint.digest().len(), 64);
    assert!(fingerprint.as_str().starts_with(Fingerprint::PREFIX));
}

#[test]
fn test_equality_is_state_equality() {
    let a = ok_or_fail(Fingerprint::parse(valid()), "a 必须可解析");
    let b = ok_or_fail(Fingerprint::parse(valid()), "b 必须可解析");
    let c = ok_or_fail(
        Fingerprint::parse(format!("sha256:{}", "1".repeat(64))),
        "c 必须可解析",
    );
    assert_eq!(a, b, "同摘要 = 同状态（§7.3 变化检测）");
    assert_ne!(a, c, "不同摘要 = 状态已变");
    assert!(a.to_string().starts_with("sha256:"));
}
