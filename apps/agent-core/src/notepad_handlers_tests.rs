//! Unit tests for the Notepad handler helpers.
//!
//! Moved out of `notepad_handlers.rs` so that file stays under the 600-line
//! guideline (gov section 5.4); the parent module pulls this in with
//! `#[path = "notepad_handlers_tests.rs"]`.

use super::{normalize_line_endings, parse_tab_count};

#[test]
fn test_normalize_line_endings_handles_crlf_and_cr() {
    assert_eq!(normalize_line_endings("a\r\nb\rc\n"), "a\nb\nc\n");
}

#[test]
fn test_parse_tab_count_reads_the_fixture_readout() {
    assert_eq!(parse_tab_count("Tabs: 1").ok(), Some(1));
    assert_eq!(parse_tab_count("Tabs: 12").ok(), Some(12));
}

#[test]
fn test_parse_tab_count_rejects_a_readout_that_is_not_a_count() {
    let error = parse_tab_count("no tabs here").err();
    assert!(matches!(
        error,
        Some(assistant_tool_bus::ToolBusError::Mcp { code: -32_601, .. })
    ));
}
