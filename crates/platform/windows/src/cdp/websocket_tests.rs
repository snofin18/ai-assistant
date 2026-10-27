//! Private WebSocket framing and upgrade validation tests.

use super::{MAX_STATUS_LINE_BYTES, validate_upgrade_status, websocket_accept};

#[test]
fn test_websocket_accept_matches_rfc6455_example() {
    assert_eq!(
        websocket_accept("dGhlIHNhbXBsZSBub25jZQ=="),
        "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="
    );
}

#[test]
fn test_upgrade_status_accepts_101_with_any_reason_phrase() {
    assert!(validate_upgrade_status("HTTP/1.1 101 Switching Protocols").is_ok());
    assert!(validate_upgrade_status("HTTP/1.1 101 WebSocket Protocol Handshake").is_ok());
    assert!(validate_upgrade_status("HTTP/1.1 101").is_ok());
}

#[test]
fn test_upgrade_status_rejects_malformed_or_non_101() {
    assert!(validate_upgrade_status("HTTP/1.0 101 Switching Protocols").is_err());
    assert!(validate_upgrade_status("HTTP/1.1 20 OK").is_err());
    assert!(validate_upgrade_status("HTTP/1.1 200 OK").is_err());
    assert!(validate_upgrade_status("HTTP/1.1 1010 Invalid").is_err());
}

#[test]
fn test_upgrade_status_rejects_oversized_line() {
    let oversized = format!("HTTP/1.1 101 {}", "x".repeat(MAX_STATUS_LINE_BYTES));
    assert!(validate_upgrade_status(&oversized).is_err());
}
