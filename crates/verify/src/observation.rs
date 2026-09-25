//! The observed world state an assertion is evaluated against.
//!
//! Responsibility: carry everything the 11 assertion kinds may need, in a form that makes
//! "not observed" distinguishable from "observed as absent". That distinction is the whole
//! point of invariant 2: a key missing from these maps means *the caller did not look*, so
//! the assertion is unevaluable -- never "the element is gone".
//!
//! Boundary: this crate does not collect any of it. Windows/UIA, the file channel, and the
//! application interfaces fill these structures in; `verify` only reads them.
//!
//! Invariants:
//! 1. A map entry exists **only** for something the observer actually probed.
//! 2. `files` holds a before/after pair per path, because `file_changed` is inherently a
//!    transition.
//! 3. `previous_fingerprint` and `elapsed_since_previous_ms` are `Option`: a postcondition
//!    that needs them is unevaluable when either is missing.

use std::collections::BTreeMap;

use assistant_platform_api::Fingerprint;
use serde::{Deserialize, Serialize};

use crate::postcondition::AssertValue;

/// What the observer learned about one element.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservedElement {
    /// Whether the element was present.
    pub present: bool,
    /// The control role (`Edit`, `Button`, ...).
    pub role: String,
    /// The element name, already truncated by the observer when necessary.
    pub name: String,
    /// Whether the element was enabled.
    pub enabled: bool,
    /// Whether the element had keyboard focus.
    pub focused: bool,
}

impl ObservedElement {
    /// Creates an element observation.
    #[must_use]
    pub fn new(
        present: bool,
        role: impl Into<String>,
        name: impl Into<String>,
        enabled: bool,
        focused: bool,
    ) -> Self {
        Self {
            present,
            role: role.into(),
            name: name.into(),
            enabled,
            focused,
        }
    }
}

/// One file's attributes at one moment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileSnapshot {
    /// Whether the file existed.
    pub exists: bool,
    /// Size in bytes (meaningless when `exists` is false).
    pub size: u64,
    /// Modification time in milliseconds since the Unix epoch.
    pub mtime_ms: i64,
    /// Content digest, when the observer computed one.
    pub digest: Option<String>,
}

impl FileSnapshot {
    /// Creates a snapshot of an existing file.
    #[must_use]
    pub const fn present(size: u64, mtime_ms: i64, digest: Option<String>) -> Self {
        Self {
            exists: true,
            size,
            mtime_ms,
            digest,
        }
    }

    /// Creates a snapshot of a file that did not exist.
    #[must_use]
    pub const fn absent() -> Self {
        Self {
            exists: false,
            size: 0,
            mtime_ms: 0,
            digest: None,
        }
    }
}

/// A file's before/after pair.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileTransition {
    /// The file state before the step ran.
    pub before: FileSnapshot,
    /// The file state after the step ran.
    pub after: FileSnapshot,
}

impl FileTransition {
    /// Creates a transition from its two snapshots.
    #[must_use]
    pub const fn new(before: FileSnapshot, after: FileSnapshot) -> Self {
        Self { before, after }
    }
}

/// Everything a postcondition may be evaluated against.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Observation {
    /// Which target or subtree the fingerprint describes (for example `document.body`).
    pub fingerprint_scope: String,
    /// The target title at observation time.
    pub title: String,
    /// The target's visible text at observation time.
    pub text: String,
    /// The fingerprint after the step ran.
    pub fingerprint: Fingerprint,
    /// The fingerprint before the step ran, when the caller recorded one.
    pub previous_fingerprint: Option<Fingerprint>,
    /// Milliseconds between the two fingerprints, when the caller recorded them.
    pub elapsed_since_previous_ms: Option<u64>,
    /// Probed elements, keyed by selector.
    pub elements: BTreeMap<String, ObservedElement>,
    /// Probed named values, keyed by name.
    pub values: BTreeMap<String, AssertValue>,
    /// Probed files, keyed by path.
    pub files: BTreeMap<String, FileTransition>,
    /// Values the application reported through its own interface, keyed by key.
    pub app_reported: BTreeMap<String, AssertValue>,
}

impl Observation {
    /// Creates an observation with empty element, value, file, and application maps and no
    /// previous fingerprint.
    #[must_use]
    pub fn new(
        fingerprint_scope: impl Into<String>,
        title: impl Into<String>,
        fingerprint: Fingerprint,
    ) -> Self {
        Self {
            fingerprint_scope: fingerprint_scope.into(),
            title: title.into(),
            text: String::new(),
            fingerprint,
            previous_fingerprint: None,
            elapsed_since_previous_ms: None,
            elements: BTreeMap::new(),
            values: BTreeMap::new(),
            files: BTreeMap::new(),
            app_reported: BTreeMap::new(),
        }
    }

    /// Whether the observation was taken for `scope`.
    ///
    /// A `state_changed` / `state_unchanged` assertion that pins a different scope is
    /// unevaluable: comparing fingerprints across scopes would compare different things.
    #[must_use]
    pub fn is_scope(&self, scope: &str) -> bool {
        self.fingerprint_scope == scope
    }
}
