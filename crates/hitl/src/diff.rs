//! Display-independent diff data for approval cards.

use crate::error::{HitlError, HitlResult};

/// One prepared diff payload.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DiffPreview {
    /// Line-level text changes.
    Text(TextDiffPreview),
    /// Field-level before/after values.
    Fields(Vec<FieldDiff>),
    /// File metadata changes.
    Files(Vec<FileDiff>),
    /// UI action sequence preview.
    UiSteps(Vec<UiStepDiff>),
    /// Point-of-no-return warning requiring secondary confirmation.
    Irreversible(IrreversibleDiff),
}

impl DiffPreview {
    /// Returns whether the UI must require a second confirmation.
    #[must_use]
    pub const fn requires_secondary_confirmation(&self) -> bool {
        matches!(self, Self::Irreversible(_))
    }
}

/// One line-level text change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextDiffEntry {
    /// Added or removed line.
    pub kind: TextDiffKind,
    /// One-based source line for removals.
    pub before_line: Option<usize>,
    /// One-based destination line for additions.
    pub after_line: Option<usize>,
    /// Pre-redacted line text.
    pub text: String,
}

/// Direction of a line-level change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum TextDiffKind {
    /// Line was added.
    Added,
    /// Line was removed.
    Removed,
}

/// Prepared line-level text changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextDiffPreview {
    /// Minimal additions and removals from an LCS diff.
    pub entries: Vec<TextDiffEntry>,
}

impl TextDiffPreview {
    /// Computes a deterministic line diff without truncation.
    ///
    /// `max_cells` bounds the LCS dynamic-programming matrix. If the requested
    /// texts exceed that budget, the call returns
    /// [`HitlError::DiffBudgetExceeded`] instead of displaying a partial diff.
    ///
    /// # Errors
    ///
    /// Returns [`HitlError::DiffBudgetExceeded`] when `(before_lines + 1) *
    /// (after_lines + 1)` exceeds `max_cells`, or [`HitlError::InvalidValue`]
    /// when arithmetic overflows.
    pub fn prepare(before: &str, after: &str, max_cells: usize) -> HitlResult<Self> {
        let matrix = prepare_text_matrix(before, after, max_cells)?;
        let entries =
            collect_text_entries(&matrix.before_lines, &matrix.after_lines, &matrix.lengths)?;
        Ok(Self { entries })
    }
}

struct PreparedTextMatrix<'text> {
    before_lines: Vec<&'text str>,
    after_lines: Vec<&'text str>,
    lengths: Vec<Vec<u32>>,
}

/// Builds the bounded LCS matrix without indexing outside the checked shape.
fn prepare_text_matrix<'text>(
    before: &'text str,
    after: &'text str,
    max_cells: usize,
) -> HitlResult<PreparedTextMatrix<'text>> {
    let before_lines: Vec<&str> = before.lines().collect();
    let after_lines: Vec<&str> = after.lines().collect();
    let rows = before_lines
        .len()
        .checked_add(1)
        .ok_or_else(|| invalid_value("before line count overflowed"))?;
    let columns = after_lines
        .len()
        .checked_add(1)
        .ok_or_else(|| invalid_value("after line count overflowed"))?;
    let required_cells = rows
        .checked_mul(columns)
        .ok_or_else(|| invalid_value("diff cell count overflowed"))?;
    if required_cells > max_cells {
        return Err(HitlError::DiffBudgetExceeded {
            required_cells,
            max_cells,
        });
    }

    let mut lengths = vec![vec![0_u32; columns]; rows];
    for before_index in (0..before_lines.len()).rev() {
        for after_index in (0..after_lines.len()).rev() {
            let equal = before_lines.get(before_index) == after_lines.get(after_index);
            let value = if equal {
                1_u32.saturating_add(length_at(&lengths, before_index + 1, after_index + 1))
            } else {
                length_at(&lengths, before_index + 1, after_index).max(length_at(
                    &lengths,
                    before_index,
                    after_index + 1,
                ))
            };
            set_length(&mut lengths, before_index, after_index, value)?;
        }
    }
    Ok(PreparedTextMatrix {
        before_lines,
        after_lines,
        lengths,
    })
}

/// Traverses an LCS matrix and emits only changed lines.
fn collect_text_entries(
    before_lines: &[&str],
    after_lines: &[&str],
    lengths: &[Vec<u32>],
) -> HitlResult<Vec<TextDiffEntry>> {
    let mut entries = Vec::new();
    let mut before_index = 0_usize;
    let mut after_index = 0_usize;
    while before_index < before_lines.len() && after_index < after_lines.len() {
        if before_lines.get(before_index) == after_lines.get(after_index) {
            before_index += 1;
            after_index += 1;
        } else if length_at(lengths, before_index + 1, after_index)
            >= length_at(lengths, before_index, after_index + 1)
        {
            entries.push(removed_entry(before_lines, before_index)?);
            before_index += 1;
        } else {
            entries.push(added_entry(after_lines, after_index)?);
            after_index += 1;
        }
    }
    while before_index < before_lines.len() {
        entries.push(removed_entry(before_lines, before_index)?);
        before_index += 1;
    }
    while after_index < after_lines.len() {
        entries.push(added_entry(after_lines, after_index)?);
        after_index += 1;
    }
    Ok(entries)
}

fn removed_entry(lines: &[&str], index: usize) -> HitlResult<TextDiffEntry> {
    let text = lines
        .get(index)
        .ok_or_else(|| invalid_value("before line disappeared during diff traversal"))?;
    Ok(TextDiffEntry {
        kind: TextDiffKind::Removed,
        before_line: Some(index + 1),
        after_line: None,
        text: (*text).to_owned(),
    })
}

fn added_entry(lines: &[&str], index: usize) -> HitlResult<TextDiffEntry> {
    let text = lines
        .get(index)
        .ok_or_else(|| invalid_value("after line disappeared during diff traversal"))?;
    Ok(TextDiffEntry {
        kind: TextDiffKind::Added,
        before_line: None,
        after_line: Some(index + 1),
        text: (*text).to_owned(),
    })
}

/// One field-level change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldDiff {
    /// Field path or stable field name.
    pub field: String,
    /// Pre-redacted previous value.
    pub before: String,
    /// Pre-redacted next value.
    pub after: String,
}

impl FieldDiff {
    /// Creates and validates a field change.
    ///
    /// # Errors
    ///
    /// Returns [`HitlError::InvalidValue`] for an empty field or an unchanged
    /// value, which is not a useful approval diff.
    pub fn new(
        field: impl Into<String>,
        before: impl Into<String>,
        after: impl Into<String>,
    ) -> HitlResult<Self> {
        let field = field.into();
        let before = before.into();
        let after = after.into();
        if field.trim().is_empty() {
            return Err(invalid_value("diff field must not be empty"));
        }
        if before == after {
            return Err(invalid_value("field diff must contain a change"));
        }
        Ok(Self {
            field,
            before,
            after,
        })
    }
}

/// One file summary change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDiff {
    /// Stable path or display label.
    pub path: String,
    /// File change kind.
    pub kind: FileChangeKind,
    /// Size before the action, when the file existed.
    pub before_bytes: Option<u64>,
    /// Size after the action, when the file remains.
    pub after_bytes: Option<u64>,
}

impl FileDiff {
    /// Creates and validates a file summary change.
    ///
    /// # Errors
    ///
    /// Returns [`HitlError::InvalidValue`] for an empty path or an impossible
    /// size combination.
    pub fn new(
        path: impl Into<String>,
        kind: FileChangeKind,
        before_bytes: Option<u64>,
        after_bytes: Option<u64>,
    ) -> HitlResult<Self> {
        let path = path.into();
        if path.trim().is_empty() {
            return Err(invalid_value("file diff path must not be empty"));
        }
        let valid_sizes = match kind {
            FileChangeKind::Added => before_bytes.is_none() && after_bytes.is_some(),
            FileChangeKind::Removed => before_bytes.is_some() && after_bytes.is_none(),
            FileChangeKind::Modified => before_bytes.is_some() && after_bytes.is_some(),
        };
        if !valid_sizes {
            return Err(invalid_value(
                "file diff sizes do not match the change kind",
            ));
        }
        Ok(Self {
            path,
            kind,
            before_bytes,
            after_bytes,
        })
    }
}

/// Kind of file change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum FileChangeKind {
    /// File is created.
    Added,
    /// File is removed.
    Removed,
    /// File content or metadata changes.
    Modified,
}

/// One UI step preview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiStepDiff {
    /// Step identifier.
    pub step_id: String,
    /// Human-readable action.
    pub action: String,
    /// Target label.
    pub target_label: String,
    /// Optional screenshot evidence reference.
    pub screenshot_ref: Option<String>,
}

impl UiStepDiff {
    /// Creates and validates one UI step preview.
    ///
    /// # Errors
    ///
    /// Returns [`HitlError::InvalidValue`] when a required text field is empty.
    pub fn new(
        step_id: impl Into<String>,
        action: impl Into<String>,
        target_label: impl Into<String>,
        screenshot_ref: Option<String>,
    ) -> HitlResult<Self> {
        let step_id = step_id.into();
        let action = action.into();
        let target_label = target_label.into();
        for (name, value) in [
            ("step_id", step_id.as_str()),
            ("action", action.as_str()),
            ("target_label", target_label.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(invalid_value(&format!("{name} must not be empty")));
            }
        }
        Ok(Self {
            step_id,
            action,
            target_label,
            screenshot_ref,
        })
    }
}

/// Warning data for an irreversible action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrreversibleDiff {
    /// Warning shown before the secondary confirmation.
    pub warning: String,
}

impl IrreversibleDiff {
    /// Creates and validates an irreversible warning.
    ///
    /// # Errors
    ///
    /// Returns [`HitlError::InvalidValue`] for an empty warning.
    pub fn new(warning: impl Into<String>) -> HitlResult<Self> {
        let warning = warning.into();
        if warning.trim().is_empty() {
            return Err(invalid_value("irreversible warning must not be empty"));
        }
        Ok(Self { warning })
    }
}

fn length_at(lengths: &[Vec<u32>], row: usize, column: usize) -> u32 {
    lengths
        .get(row)
        .and_then(|values| values.get(column))
        .copied()
        .unwrap_or(0)
}

fn set_length(lengths: &mut [Vec<u32>], row: usize, column: usize, value: u32) -> HitlResult<()> {
    let cell = lengths
        .get_mut(row)
        .and_then(|values| values.get_mut(column))
        .ok_or_else(|| invalid_value("diff matrix cell is missing"))?;
    *cell = value;
    Ok(())
}

fn invalid_value(reason: &str) -> HitlError {
    HitlError::InvalidValue {
        reason: reason.to_owned(),
    }
}
