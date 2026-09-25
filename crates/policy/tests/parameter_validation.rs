//! Positive and negative contract tests for pure parameter validators.

use assistant_policy::{
    PathValidationRequest, TextLimits, UrlValidationRequest, validate_integer_range, validate_path,
    validate_regular_expression, validate_text_limits, validate_url,
};

#[test]
fn test_path_validation_accepts_contained_resolved_path() {
    let result = validate_path(&PathValidationRequest {
        raw_path: "notes/today.txt",
        root: "C:/workspace",
        resolved_path: "C:/workspace/notes/today.txt",
    });
    assert_eq!(
        result.map(|path| path.as_str().to_owned()),
        Ok("C:/workspace/notes/today.txt".to_owned())
    );
}

#[test]
fn test_path_validation_rejects_traversal_unc_device_and_symlink_escape() {
    for request in [
        PathValidationRequest {
            raw_path: "../secret.txt",
            root: "C:/workspace",
            resolved_path: "C:/secret.txt",
        },
        PathValidationRequest {
            raw_path: "//server/share/file.txt",
            root: "C:/workspace",
            resolved_path: "C:/workspace/file.txt",
        },
        PathValidationRequest {
            raw_path: r"\\?\C:\secret.txt",
            root: "C:/workspace",
            resolved_path: "C:/workspace/secret.txt",
        },
        PathValidationRequest {
            raw_path: "link/file.txt",
            root: "C:/workspace",
            resolved_path: "C:/outside/file.txt",
        },
    ] {
        assert!(validate_path(&request).is_err());
    }
}

#[test]
fn test_url_validation_accepts_allowed_domain() {
    let result = validate_url(&UrlValidationRequest {
        url: "https://docs.example.com/guide",
        allowed_schemes: &["https"],
        allowed_hosts: &["docs.example.com"],
    });
    assert!(result.is_ok());
}

#[test]
fn test_url_validation_rejects_file_private_ip_userinfo_and_host_mismatch() {
    for request in [
        UrlValidationRequest {
            url: "file:///C:/secret.txt",
            allowed_schemes: &["file"],
            allowed_hosts: &[],
        },
        UrlValidationRequest {
            url: "https://127.0.0.1/admin",
            allowed_schemes: &["https"],
            allowed_hosts: &["127.0.0.1"],
        },
        UrlValidationRequest {
            url: "https://user@example.com/path",
            allowed_schemes: &["https"],
            allowed_hosts: &["example.com"],
        },
        UrlValidationRequest {
            url: "https://evil.example.net/path",
            allowed_schemes: &["https"],
            allowed_hosts: &["example.com"],
        },
    ] {
        assert!(validate_url(&request).is_err());
    }
}

#[test]
fn test_text_validation_rejects_length_lines_and_carriage_return() {
    let limits = TextLimits {
        max_characters: 8,
        max_bytes: 16,
        max_lines: 1,
    };
    assert!(validate_text_limits("123456789", limits).is_err());
    assert!(validate_text_limits("one\ntwo", limits).is_err());
    assert!(validate_text_limits("line\r\n", limits).is_err());
    assert!(validate_text_limits("12345678", limits).is_ok());
}

#[test]
fn test_integer_range_rejects_out_of_bounds_value() {
    assert!(validate_integer_range(51, 1, 50).is_err());
    assert_eq!(validate_integer_range(50, 1, 50), Ok(50));
}

#[test]
fn test_regex_validation_accepts_simple_pattern_and_rejects_redos_shapes() {
    assert!(validate_regular_expression(r"^[a-z]+\.txt$").is_ok());
    for pattern in [
        r"(a+)+",
        r"(a)\1",
        r".*.*",
        r"(?=secret)",
        r"a{1,50}",
        r"a|*",
    ] {
        assert!(validate_regular_expression(pattern).is_err(), "{pattern}");
    }
}
