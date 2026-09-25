//! Boundary tests that exercise every fail-closed validation branch.

use assistant_policy::{
    PathValidationRequest, PolicyError, TextLimits, UrlValidationRequest, validate_integer_range,
    validate_path, validate_regular_expression, validate_text_limits, validate_url,
};
use assistant_protocol::ErrorCode;

fn assert_path_rejected(raw_path: &str, root: &str, resolved_path: &str) {
    assert!(
        validate_path(&PathValidationRequest {
            raw_path,
            root,
            resolved_path,
        })
        .is_err()
    );
}

#[test]
fn test_error_variants_have_codes_and_readable_messages() {
    let cases = [
        (
            PolicyError::InvalidRuleSet {
                reason: "bad rule".to_owned(),
            },
            ErrorCode::ToolInvalidArgs,
        ),
        (
            PolicyError::RuleSetConflict {
                rule_id: "duplicate".to_owned(),
            },
            ErrorCode::ToolInvalidArgs,
        ),
        (
            PolicyError::PathRejected {
                reason: "bad path".to_owned(),
            },
            ErrorCode::ToolInvalidArgs,
        ),
        (
            PolicyError::UrlRejected {
                reason: "bad url".to_owned(),
            },
            ErrorCode::ToolInvalidArgs,
        ),
        (
            PolicyError::TextRejected {
                reason: "bad text".to_owned(),
            },
            ErrorCode::ToolInvalidArgs,
        ),
        (
            PolicyError::IntegerOutOfRange {
                value: 2,
                minimum: 0,
                maximum: 1,
            },
            ErrorCode::ToolInvalidArgs,
        ),
        (
            PolicyError::RegexRejected {
                reason: "bad regex".to_owned(),
            },
            ErrorCode::ToolInvalidArgs,
        ),
        (
            PolicyError::ProtocolProjection {
                reason: "bad projection".to_owned(),
            },
            ErrorCode::Fatal,
        ),
    ];
    for (error, expected) in cases {
        assert_eq!(error.error_code(), expected);
        assert!(!error.to_string().is_empty());
    }
}

#[test]
fn test_path_rejects_malformed_roots_and_paths() {
    assert_path_rejected("", "C:/root", "C:/root/file");
    assert_path_rejected("file", "", "C:/root/file");
    assert_path_rejected("file", "relative", "C:/root/file");
    assert_path_rejected("file", "C:/root", "");
    assert_path_rejected("file", "C:/root", "relative/file");
    assert_path_rejected("C:file", "C:/root", "C:/root/file");
    assert_path_rejected("file%2e%2e/secret", "C:/root", "C:/root/file");
    assert_path_rejected("file\nname", "C:/root", "C:/root/file");
    assert_path_rejected(" file", "C:/root", "C:/root/file");
    assert_path_rejected("C:/outside/file", "C:/root", "C:/root/file");
    assert_path_rejected("dir:name/file", "C:/root", "C:/root/file");
    assert_path_rejected("file.", "C:/root", "C:/root/file");
    assert_path_rejected("CON", "C:/root", "C:/root/CON");
    assert_path_rejected("file", "//server/share", "//server/share/file");
    assert_path_rejected("file", "C:/root", "D:/root/file");
    let long_path = "a".repeat(4097);
    assert_path_rejected(&long_path, "C:/root", "C:/root/file");
}

#[test]
fn test_path_accepts_absolute_path_inside_root() {
    let result = validate_path(&PathValidationRequest {
        raw_path: "C:/root/file",
        root: "c:/ROOT",
        resolved_path: "C:/root/file",
    });
    assert_eq!(
        result.map(|path| path.as_str().to_owned()),
        Ok("C:/root/file".to_owned())
    );
}

#[test]
fn test_url_rejects_malformed_and_unsupported_forms() {
    let schemes = ["https", "notes"];
    let hosts = ["docs.example.com"];
    let cases = [
        "",
        "://missing-scheme",
        "1https://docs.example.com",
        "file:///secret",
        "ftp://docs.example.com",
        "https:docs.example.com",
        "https://",
        "https://user@docs.example.com",
        "https://%64ocs.example.com",
        "https://bad_host.example.com",
        "https://docs.example.com:0/",
        "https://docs.example.com:bad/",
        "https://docs.example.com:443:1/",
        "https://2130706433/",
        "https://0x7f000001/",
        "custom:",
        "custom://host",
        "https://docs.example.com/#fragment",
        "https://docs.example.com/a b",
    ];
    for url in cases {
        assert!(
            validate_url(&UrlValidationRequest {
                url,
                allowed_schemes: &schemes,
                allowed_hosts: &hosts,
            })
            .is_err(),
            "{url}"
        );
    }
}

#[test]
fn test_url_accepts_exact_suffix_and_custom_scheme() {
    assert!(
        validate_url(&UrlValidationRequest {
            url: "https://docs.example.com/guide?q=1",
            allowed_schemes: &["https"],
            allowed_hosts: &["docs.example.com"],
        })
        .is_ok()
    );
    assert!(
        validate_url(&UrlValidationRequest {
            url: "https://help.example.com/guide",
            allowed_schemes: &["https"],
            allowed_hosts: &[".example.com"],
        })
        .is_ok()
    );
    assert!(
        validate_url(&UrlValidationRequest {
            url: "notes:open?name=today",
            allowed_schemes: &["notes"],
            allowed_hosts: &[],
        })
        .is_ok()
    );
}

#[test]
fn test_text_rejects_invalid_limits_and_all_size_dimensions() {
    let limits = TextLimits {
        max_characters: 4,
        max_bytes: 8,
        max_lines: 2,
    };
    assert!(
        validate_text_limits(
            "text",
            TextLimits {
                max_characters: 0,
                ..limits
            }
        )
        .is_err()
    );
    assert!(validate_text_limits("12345", limits).is_err());
    assert!(
        validate_text_limits(
            "abcd",
            TextLimits {
                max_bytes: 2,
                ..limits
            }
        )
        .is_err()
    );
    assert!(validate_text_limits("a\nb\nc", limits).is_err());
    assert!(validate_text_limits("ab\0", limits).is_err());
    assert!(validate_text_limits("", limits).is_ok());
}

#[test]
fn test_integer_range_rejects_inverted_bounds() {
    assert!(validate_integer_range(1, 2, 1).is_err());
}

#[test]
fn test_regex_rejects_lexical_and_structural_errors() {
    let cases = [
        "",
        "\\",
        r"\1",
        r"\x41",
        "[abc",
        "a)",
        "a{2}",
        "*abc",
        "a**",
        "a*a*a*a*a*",
        "(a|b)+",
        "[[a]]",
        r"[\x41]",
        "(?=secret)",
        "nonascii\u{00e9}",
    ];
    for pattern in cases {
        assert!(validate_regular_expression(pattern).is_err(), "{pattern}");
    }
    assert!(validate_regular_expression(r"^[a-z]+\.txt$").is_ok());
}

#[test]
fn test_dsl_rejects_all_structural_variants() {
    let invalid = [
        "{",
        "[]",
        r#"{"rules":[]}"#,
        r#"{"default":7,"rules":[]}"#,
        r#"{"default":{},"rules":[]}"#,
        r#"{"default":{"effect":7},"rules":[]}"#,
        r#"{"default":{"effect":"allow"},"rules":[]}"#,
        r#"{"default":{"effect":"deny"},"rules":7}"#,
        r#"{"default":{"effect":"deny"},"rules":[7]}"#,
        r#"{"default":{"effect":"deny"},"rules":[{"id":"x","when":{"effect":"read"},"effect":"allow","extra":1}]}"#,
        r#"{"default":{"effect":"deny"},"rules":[{"when":{"effect":"read"},"effect":"allow"}]}"#,
        r#"{"default":{"effect":"deny"},"rules":[{"id":7,"when":{"effect":"read"},"effect":"allow"}]}"#,
        r#"{"default":{"effect":"deny"},"rules":[{"id":"","when":{"effect":"read"},"effect":"allow"}]}"#,
        r#"{"default":{"effect":"deny"},"rules":[{"id":"x","effect":"allow"}]}"#,
        r#"{"default":{"effect":"deny"},"rules":[{"id":"x","when":7,"effect":"allow"}]}"#,
        r#"{"default":{"effect":"deny"},"rules":[{"id":"x","when":{"unknown":1},"effect":"allow"}]}"#,
        r#"{"default":{"effect":"deny"},"rules":[{"id":"x","when":{"effect":"unknown"},"effect":"allow"}]}"#,
        r#"{"default":{"effect":"deny"},"rules":[{"id":"x","when":{"effect":7},"effect":"allow"}]}"#,
        r#"{"default":{"effect":"deny"},"rules":[{"id":"x","when":{"risk_level":"unknown"},"effect":"allow"}]}"#,
        r#"{"default":{"effect":"deny"},"rules":[{"id":"x","when":{"reversibility":"unknown"},"effect":"allow"}]}"#,
        r#"{"default":{"effect":"deny"},"rules":[{"id":"x","when":{"egress":"unknown"},"effect":"allow"}]}"#,
        r#"{"default":{"effect":"deny"},"rules":[{"id":"x","when":{"unattended":"yes"},"effect":"allow"}]}"#,
        r#"{"default":{"effect":"deny"},"rules":[{"id":"x","when":{"target_app":[7]},"effect":"allow"}]}"#,
        r#"{"default":{"effect":"deny"},"rules":[{"id":"x","when":{"effect":"write"},"effect":7}]}"#,
        r#"{"default":{"effect":"deny"},"rules":[{"id":"x","when":{"effect":"write"},"effect":"unknown"}]}"#,
        r#"{"default":{"effect":"deny"},"rules":[{"id":"x","when":{"effect":"read"},"effect":"allow","reason":"x"}]}"#,
        r#"{"default":{"effect":"deny"},"rules":[{"id":"x","when":{"effect":"write"},"effect":"allow_with_confirmation"}]}"#,
        r#"{"default":{"effect":"deny"},"rules":[{"id":"x","when":{"effect":"write"},"effect":"allow_with_confirmation","confirmation":{"scope_options":["once"],"show_diff":true,"extra":1}}]}"#,
        r#"{"default":{"effect":"deny"},"rules":[{"id":"x","when":{"effect":"write"},"effect":"allow_with_confirmation","confirmation":{"show_diff":true}}]}"#,
        r#"{"default":{"effect":"deny"},"rules":[{"id":"x","when":{"effect":"write"},"effect":"allow_with_confirmation","confirmation":{"scope_options":["forever"],"show_diff":true}}]}"#,
        r#"{"default":{"effect":"deny"},"rules":[{"id":"x","when":{"effect":"write"},"effect":"allow_with_confirmation","confirmation":{"scope_options":["once"],"show_diff":"yes"}}]}"#,
        r#"{"default":{"effect":"deny"},"rules":[{"id":"x","when":{"effect":"destroy"},"effect":"deny"}]}"#,
        r#"{"default":{"effect":"deny"},"rules":[{"id":"x","when":{"effect":"destroy"},"effect":"deny","confirmation":{"scope_options":["once"],"show_diff":true},"reason":"x"}]}"#,
        r#"{"default":{"effect":"deny"},"rules":[{"id":"x","when":{"effect":"read"},"effect":"allow"},{"id":"x","when":{"effect":"read"},"effect":"allow"}]}"#,
    ];
    for json in invalid {
        assert!(
            assistant_policy::RuleSet::from_json(json).is_err(),
            "{json}"
        );
    }
}
