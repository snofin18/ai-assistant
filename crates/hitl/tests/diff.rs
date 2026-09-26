//! Diff preparation contract tests.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use assistant_hitl::{
    DiffPreview, FieldDiff, FileChangeKind, FileDiff, HitlError, IrreversibleDiff, TextDiffKind,
    TextDiffPreview, UiStepDiff,
};

#[test]
fn test_text_diff_is_minimal_deterministic_and_line_numbers_are_one_based() {
    let diff = TextDiffPreview::prepare("alpha\nbeta\ngamma\n", "alpha\ndelta\ngamma\n", 64)
        .expect("diff");
    assert_eq!(diff.entries.len(), 2);
    let removed = diff.entries.first().expect("removed");
    assert_eq!(removed.kind, TextDiffKind::Removed);
    assert_eq!(removed.before_line, Some(2));
    assert_eq!(removed.text, "beta");
    let added = diff.entries.get(1).expect("added");
    assert_eq!(added.kind, TextDiffKind::Added);
    assert_eq!(added.after_line, Some(2));
    assert_eq!(added.text, "delta");
}

#[test]
fn test_text_diff_rejects_budget_instead_of_truncating() {
    let error = TextDiffPreview::prepare("one\ntwo", "three\nfour", 3)
        .expect_err("three by three matrix exceeds budget");
    assert!(matches!(
        error,
        HitlError::DiffBudgetExceeded {
            required_cells: 9,
            max_cells: 3
        }
    ));
}

#[test]
fn test_structured_diff_builders_validate_shape() {
    assert!(FieldDiff::new("document.title", "old", "new").is_ok());
    assert!(FieldDiff::new("document.title", "same", "same").is_err());

    assert!(FileDiff::new("report.txt", FileChangeKind::Removed, Some(12), None).is_ok());
    assert!(FileDiff::new("report.txt", FileChangeKind::Removed, None, Some(12)).is_err());

    assert!(UiStepDiff::new("s_1", "save", "report.txt", None).is_ok());
    assert!(UiStepDiff::new("", "save", "report.txt", None).is_err());

    let irreversible = DiffPreview::Irreversible(
        IrreversibleDiff::new("This action cannot be undone").expect("warning"),
    );
    assert!(irreversible.requires_secondary_confirmation());
}
