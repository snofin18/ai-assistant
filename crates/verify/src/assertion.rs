//! Evaluating one parsed postcondition against an observation.
//!
//! Responsibility: the three-valued decision at the heart of architecture v2 section 7.4.
//! [`AssertionOutcome::Satisfied`] means the observation proves the postcondition;
//! [`AssertionOutcome::Falsified`] means it proves the opposite; [`AssertionOutcome::NotEvaluable`]
//! means the observation does not contain what the postcondition needs.
//!
//! Boundary: no IO, no clock, no retry, no rollback, no prompting. It reads an [`Observation`]
//! the caller collected and returns a decision; acting on the decision belongs to the Host,
//! `undo`, and `hitl`.
//!
//! ## Why three values and not a boolean
//!
//! A two-valued result forces "we did not look" into either "yes" or "no". Turning it into "yes"
//! is the project's first red line (section 7.4: a failed verification must never look like a
//! success). Turning it into "no" would roll back work that may well have succeeded. So the third
//! value exists and propagates to `VerifyOutcome::Inconclusive`.
//!
//! ## Invariants
//!
//! 1. A missing observation never yields `Satisfied`; it yields `NotEvaluable` with a reason.
//! 2. A type mismatch is `NotEvaluable`, never `Falsified`: a string assertion compared against a
//!    numeric observation is a question about a different quantity, so we refuse to answer it.
//! 3. `state_changed` is `NotEvaluable` unless the observation carries a previous fingerprint,
//!    the elapsed time between the two fingerprints, and (when the postcondition pins one) a
//!    matching scope. `state_unchanged` needs the previous fingerprint and the scope.
//! 4. Every non-`Satisfied` outcome carries a machine-readable index upstream and a
//!    human-readable expected/actual or reason here (rule 1: no silent failure).

use serde::{Deserialize, Serialize};

use crate::observation::{FileSnapshot, Observation};
use crate::postcondition::{AssertValue, CompareOp, FileChangeKind, Postcondition, StateField};

/// The longest rendering of a text value inside a violation report.
///
/// A violation report is shown to the model and to the user (section 7.4 `escalate_to_user`), so
/// an unbounded echo of a document body would blow up the prompt and the card. The truncation is
/// announced with the number of dropped bytes so it is never mistaken for the whole value.
const REPORT_TEXT_LIMIT: usize = 120;

/// The three-valued result of evaluating one postcondition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum AssertionOutcome {
    /// The observation proves the postcondition holds.
    Satisfied,
    /// The observation proves the postcondition does not hold.
    Falsified {
        /// What the postcondition asked for, rendered for a human.
        expected: String,
        /// What the observation actually showed.
        actual: String,
    },
    /// The observation does not contain what the postcondition needs.
    NotEvaluable {
        /// What was missing, or why the observation cannot answer this question.
        reason: String,
    },
}

impl AssertionOutcome {
    /// Whether the postcondition holds.
    #[must_use]
    pub const fn is_satisfied(&self) -> bool {
        matches!(self, Self::Satisfied)
    }

    /// Whether the postcondition was disproved.
    #[must_use]
    pub const fn is_falsified(&self) -> bool {
        matches!(self, Self::Falsified { .. })
    }

    /// Whether the observation could not decide the postcondition.
    #[must_use]
    pub const fn is_not_evaluable(&self) -> bool {
        matches!(self, Self::NotEvaluable { .. })
    }
}

/// Renders a text value for a report, truncating at a character boundary.
#[must_use]
pub fn render_text(value: &str) -> String {
    if value.len() <= REPORT_TEXT_LIMIT {
        return format!("{value:?}");
    }
    let mut cut = REPORT_TEXT_LIMIT;
    while cut > 0 && !value.is_char_boundary(cut) {
        cut -= 1;
    }
    let head = value.get(..cut).unwrap_or_default();
    let dropped = value.len().saturating_sub(cut);
    format!("{head:?}\u{2026}(+{dropped} bytes)")
}

/// Turns a boolean decision into an outcome, keeping the evidence for the failing case.
fn decide(holds: bool, expected: String, actual: String) -> AssertionOutcome {
    if holds {
        AssertionOutcome::Satisfied
    } else {
        AssertionOutcome::Falsified { expected, actual }
    }
}

/// Builds a `NotEvaluable` outcome.
const fn not_evaluable(reason: String) -> AssertionOutcome {
    AssertionOutcome::NotEvaluable { reason }
}

/// Evaluates one postcondition against `observation`.
#[must_use]
pub fn evaluate_postcondition(
    postcondition: &Postcondition,
    observation: &Observation,
) -> AssertionOutcome {
    match postcondition {
        Postcondition::StateAssert { field, op, value } => {
            evaluate_state_assert(*field, *op, value, observation)
        }
        Postcondition::TextContains { text } => evaluate_contains(&observation.text, text, true),
        Postcondition::TextNotContains { text } => {
            evaluate_contains(&observation.text, text, false)
        }
        Postcondition::StateChanged {
            fingerprint_scope,
            within_ms,
        } => evaluate_state_changed(observation, fingerprint_scope.as_deref(), *within_ms),
        Postcondition::StateUnchanged { fingerprint_scope } => {
            evaluate_state_unchanged(observation, fingerprint_scope.as_deref())
        }
        Postcondition::ElementExists { selector } => evaluate_element(observation, selector, true),
        Postcondition::ElementGone { selector } => evaluate_element(observation, selector, false),
        Postcondition::ValueEquals { name, value } => {
            evaluate_value_equals(observation, name, value)
        }
        Postcondition::ValueInRange { name, min, max } => {
            evaluate_value_in_range(observation, name, *min, *max)
        }
        Postcondition::FileChanged { path, expect } => {
            evaluate_file_changed(observation, path, *expect)
        }
        Postcondition::AppReported { key, value } => evaluate_app_reported(observation, key, value),
    }
}

/// The canonical label of a [`StateField`], used in report text.
const fn state_field_label(field: StateField) -> &'static str {
    match field {
        StateField::Title => "title",
        StateField::Text => "text",
        StateField::Fingerprint => "fingerprint",
    }
}

fn evaluate_state_assert(
    field: StateField,
    op: CompareOp,
    expected: &AssertValue,
    observation: &Observation,
) -> AssertionOutcome {
    let actual: &str = match field {
        StateField::Title => &observation.title,
        StateField::Text => &observation.text,
        StateField::Fingerprint => observation.fingerprint.as_str(),
    };
    // Parsing already rejected a non-text value for these fields; this branch is defence in depth
    // so that a hand-built `Postcondition` cannot panic or silently pass.
    let AssertValue::Text(expected_text) = expected else {
        return not_evaluable(format!(
            "`state_assert` on `{}` requires a string value, got {}",
            state_field_label(field),
            expected.kind_name()
        ));
    };

    let label = state_field_label(field);
    let (holds, expected_rendering) = match op {
        CompareOp::Equals => (
            actual == expected_text,
            format!("{label} == {}", render_text(expected_text)),
        ),
        CompareOp::NotEquals => (
            actual != expected_text,
            format!("{label} != {}", render_text(expected_text)),
        ),
        CompareOp::Contains => (
            actual.contains(expected_text.as_str()),
            format!("{label} contains {}", render_text(expected_text)),
        ),
        CompareOp::NotContains => (
            !actual.contains(expected_text.as_str()),
            format!("{label} does not contain {}", render_text(expected_text)),
        ),
    };
    decide(
        holds,
        expected_rendering,
        format!("{label} = {}", render_text(actual)),
    )
}

fn evaluate_contains(text: &str, needle: &str, expect_present: bool) -> AssertionOutcome {
    let present = text.contains(needle);
    let expected = if expect_present {
        format!("text contains {}", render_text(needle))
    } else {
        format!("text does not contain {}", render_text(needle))
    };
    decide(
        present == expect_present,
        expected,
        format!("text = {}", render_text(text)),
    )
}

/// Checks the scope pinned by a `state_changed` / `state_unchanged` postcondition.
///
/// Comparing fingerprints taken at different scopes would compare different things, so a pinned
/// scope that does not match is unevaluable rather than satisfied or falsified.
fn scope_matches(observation: &Observation, scope: Option<&str>) -> Option<AssertionOutcome> {
    let scope = scope?;
    if observation.is_scope(scope) {
        return None;
    }
    Some(not_evaluable(format!(
        "postcondition pins scope {scope:?}, but the observation was taken for scope {:?}",
        observation.fingerprint_scope
    )))
}

fn evaluate_state_changed(
    observation: &Observation,
    scope: Option<&str>,
    within_ms: u64,
) -> AssertionOutcome {
    if let Some(outcome) = scope_matches(observation, scope) {
        return outcome;
    }
    let Some(previous) = observation.previous_fingerprint.as_ref() else {
        return not_evaluable(
            "`state_changed` needs the fingerprint from before the step, which was not recorded"
                .to_owned(),
        );
    };
    let Some(elapsed) = observation.elapsed_since_previous_ms else {
        return not_evaluable(
            "`state_changed` needs the elapsed time between the two fingerprints, which was not \
             recorded"
                .to_owned(),
        );
    };

    // The deadline is part of the claim: "the state changed within N ms". A transition observed
    // over a longer window therefore falsifies it, even though a change did happen.
    let changed = previous != &observation.fingerprint;
    decide(
        changed && elapsed <= within_ms,
        format!("fingerprint changes within {within_ms} ms"),
        format!(
            "fingerprint changed={changed}, elapsed={elapsed} ms, before={previous}, after={}",
            observation.fingerprint
        ),
    )
}

fn evaluate_state_unchanged(observation: &Observation, scope: Option<&str>) -> AssertionOutcome {
    if let Some(outcome) = scope_matches(observation, scope) {
        return outcome;
    }
    let Some(previous) = observation.previous_fingerprint.as_ref() else {
        return not_evaluable(
            "`state_unchanged` needs the fingerprint from before the step, which was not recorded"
                .to_owned(),
        );
    };
    if previous == &observation.fingerprint {
        AssertionOutcome::Satisfied
    } else {
        AssertionOutcome::Falsified {
            expected: "fingerprint is unchanged".to_owned(),
            actual: format!(
                "fingerprint changed from {previous} to {}",
                observation.fingerprint
            ),
        }
    }
}

fn evaluate_element(
    observation: &Observation,
    selector: &str,
    expect_present: bool,
) -> AssertionOutcome {
    let Some(element) = observation.elements.get(selector) else {
        return not_evaluable(format!(
            "element {selector:?} was not probed; an unprobed element is not an absent element"
        ));
    };
    let expected = if expect_present {
        format!("element {selector:?} is present")
    } else {
        format!("element {selector:?} is gone")
    };
    decide(
        element.present == expect_present,
        expected,
        format!(
            "element {selector:?} present={}, role={:?}, name={:?}",
            element.present, element.role, element.name
        ),
    )
}

fn evaluate_value_equals(
    observation: &Observation,
    name: &str,
    expected: &AssertValue,
) -> AssertionOutcome {
    let Some(actual) = observation.values.get(name) else {
        return not_evaluable(format!("value {name:?} was not observed"));
    };
    expected.equal_to(actual).map_or_else(
        || {
            not_evaluable(format!(
                "value {name:?} was observed as {}, but the postcondition expects {}",
                actual.kind_name(),
                expected.kind_name()
            ))
        },
        |holds| {
            decide(
                holds,
                format!("value {name:?} == {}", expected.render()),
                format!("value {name:?} == {}", actual.render()),
            )
        },
    )
}

fn evaluate_value_in_range(
    observation: &Observation,
    name: &str,
    min: f64,
    max: f64,
) -> AssertionOutcome {
    let Some(actual) = observation.values.get(name) else {
        return not_evaluable(format!("value {name:?} was not observed"));
    };
    let AssertValue::Number(number) = actual else {
        return not_evaluable(format!(
            "value {name:?} was observed as {}, which cannot be range-checked",
            actual.kind_name()
        ));
    };
    let Some(value) = number.as_f64() else {
        return not_evaluable(format!(
            "value {name:?} is not representable as a finite number"
        ));
    };
    decide(
        (min..=max).contains(&value),
        format!("{min} <= value {name:?} <= {max}"),
        format!("value {name:?} = {value}"),
    )
}

/// Renders one file snapshot for a report.
fn render_snapshot(snapshot: &FileSnapshot) -> String {
    if snapshot.exists {
        format!(
            "exists(size={}, mtime={}, digest={:?})",
            snapshot.size, snapshot.mtime_ms, snapshot.digest
        )
    } else {
        "absent".to_owned()
    }
}

fn evaluate_file_changed(
    observation: &Observation,
    path: &str,
    expect: FileChangeKind,
) -> AssertionOutcome {
    let Some(transition) = observation.files.get(path) else {
        return not_evaluable(format!("file {path:?} was not observed"));
    };
    let before = &transition.before;
    let after = &transition.after;
    let both_exist = before.exists && after.exists;

    match expect {
        FileChangeKind::Any => {
            // Creation and deletion are changes in their own right; when both snapshots exist the
            // comparison is over the attributes the caller recorded.
            let changed = if both_exist {
                before.size != after.size
                    || before.mtime_ms != after.mtime_ms
                    || before.digest != after.digest
            } else {
                before.exists != after.exists
            };
            decide(
                changed,
                format!("file {path:?} changed"),
                format!(
                    "file {path:?} before={}, after={}",
                    render_snapshot(before),
                    render_snapshot(after)
                ),
            )
        }
        FileChangeKind::Size => {
            if !both_exist {
                return not_evaluable(format!(
                    "file {path:?} must exist both before and after the step to compare sizes"
                ));
            }
            decide(
                before.size != after.size,
                format!("file {path:?} size changed"),
                format!("file {path:?} size {} -> {}", before.size, after.size),
            )
        }
        FileChangeKind::Mtime => {
            if !both_exist {
                return not_evaluable(format!(
                    "file {path:?} must exist both before and after the step to compare \
                     modification times"
                ));
            }
            decide(
                before.mtime_ms != after.mtime_ms,
                format!("file {path:?} modification time changed"),
                format!(
                    "file {path:?} mtime {} -> {}",
                    before.mtime_ms, after.mtime_ms
                ),
            )
        }
        FileChangeKind::Digest => {
            if !both_exist {
                return not_evaluable(format!(
                    "file {path:?} must exist both before and after the step to compare digests"
                ));
            }
            let (Some(before_digest), Some(after_digest)) =
                (before.digest.as_ref(), after.digest.as_ref())
            else {
                return not_evaluable(format!(
                    "file {path:?} needs both content digests, but the observer recorded only one"
                ));
            };
            decide(
                before_digest != after_digest,
                format!("file {path:?} content changed"),
                format!("file {path:?} digest {before_digest:?} -> {after_digest:?}"),
            )
        }
    }
}

fn evaluate_app_reported(
    observation: &Observation,
    key: &str,
    expected: &AssertValue,
) -> AssertionOutcome {
    let Some(actual) = observation.app_reported.get(key) else {
        return not_evaluable(format!(
            "the application did not report {key:?}; a missing report is not a negative report"
        ));
    };
    expected.equal_to(actual).map_or_else(
        || {
            not_evaluable(format!(
                "the application reported {key:?} as {}, but the postcondition expects {}",
                actual.kind_name(),
                expected.kind_name()
            ))
        },
        |holds| {
            decide(
                holds,
                format!("application reports {key:?} == {}", expected.render()),
                format!("application reports {key:?} == {}", actual.render()),
            )
        },
    )
}
