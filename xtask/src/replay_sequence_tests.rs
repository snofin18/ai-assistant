//! Targeted regression tests for Recording v2 sequence replay and tree diff.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};

use super::{run_path, run_suite};

fn recording_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("fixtures")
        .join("recordings")
        .join(relative)
}

fn run_and_capture(path: &Path) -> (u8, String) {
    let mut output = Vec::new();
    let code = run_path(path, &mut output).expect("replay should not have an IO error");
    (
        code,
        String::from_utf8(output).expect("replay output is UTF-8"),
    )
}

#[test]
fn test_sequence_fixture_reports_tree_diff() {
    let path = recording_path("core/notepad-like-sequence.sequence.json");
    let (code, output) = run_and_capture(&path);

    assert_eq!(code, 0, "fixture must replay cleanly\n{output}");
    assert!(
        output.contains("text_changed"),
        "text diff must be visible\n{output}"
    );
    assert!(
        output.contains("node_added"),
        "node addition must be visible\n{output}"
    );
    assert!(output.contains("-- verdict: PASSED"), "{output}");
}

#[test]
fn test_tampered_missing_node_is_rejected() {
    let path = recording_path("core/negative/tampered-missing-node.json");
    let (code, output) = run_and_capture(&path);

    assert_eq!(code, 1, "tampered fixture must fail\n{output}");
    assert!(output.contains("MISMATCH"), "{output}");
    assert!(
        output.contains("node_added"),
        "missing node must be locatable\n{output}"
    );
}

#[test]
fn test_tampered_text_is_rejected() {
    let path = recording_path("core/negative/tampered-text.json");
    let (code, output) = run_and_capture(&path);

    assert_eq!(code, 1, "tampered fixture must fail\n{output}");
    assert!(output.contains("MISMATCH"), "{output}");
    assert!(output.contains("text_changed"), "{output}");
    assert!(
        output.contains("EditorTextBox"),
        "node label must be locatable\n{output}"
    );
}

#[test]
fn test_unknown_suite_is_an_explicit_failure() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut output = Vec::new();
    let error =
        run_suite(&root, "no-such-suite", &mut output).expect_err("unknown suite must fail");

    assert!(error.contains("unknown replay suite"), "{error}");
}

#[test]
fn test_missing_suite_directory_is_an_explicit_failure() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("does-not-exist");
    let mut output = Vec::new();
    let error = run_suite(&root, "core", &mut output).expect_err("missing suite dir must fail");

    assert!(
        error.contains("missing replay fixture suite directory"),
        "{error}"
    );
}

#[test]
fn test_legacy_v1_fixture_still_uses_legacy_path() {
    let path = recording_path("core/notepad-like-basic.json");
    let (code, output) = run_and_capture(&path);

    assert_eq!(code, 0, "legacy fixture must remain readable\n{output}");
    assert!(output.contains("version: 1"), "{output}");
}
