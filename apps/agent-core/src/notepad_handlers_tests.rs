//! Unit tests for the Notepad handler helpers.
//!
//! Moved out of `notepad_handlers.rs` so that file stays under the 600-line
//! guideline (gov section 5.4); the parent module pulls this in with
//! `#[path = "notepad_handlers_tests.rs"]`.

use super::normalize_line_endings;

#[test]
fn test_normalize_line_endings_handles_crlf_and_cr() {
    assert_eq!(normalize_line_endings("a\r\nb\rc\n"), "a\nb\nc\n");
}
