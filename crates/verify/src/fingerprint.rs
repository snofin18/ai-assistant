//! State fingerprints (architecture v2 section 7.3): canonical form + SHA-256.
//!
//! Responsibility: turn a caller-supplied [`FingerprintSubject`] into the canonical byte string
//! section 7.3 hashes, and hash it into the project's one fingerprint shape
//! (`sha256:<64 lowercase hex>`, `assistant_platform_api::Fingerprint`).
//!
//! Boundary: **this crate does not walk any UI tree.** Reading titles, control ids, roles,
//! document samples, and scroll positions needs platform access, which lives in
//! `crates/platform/<os>` (rule 7: `core` and this layer never call platform APIs). The caller
//! collects a [`FingerprintSubject`]; this module only canonicalises and hashes it.
//!
//! ## Why the canonical form is length-prefixed
//!
//! Fingerprint material is attacker-influenced text (a document title can contain `|`, `=`,
//! newlines, or anything else). A separator-based encoding would let two different subjects
//! collide by moving a separator into a value. Every field is therefore written as
//! `name=<byte-length>:<raw bytes>\n`, so the encoding is unambiguous for *any* byte content,
//! and a decoder can always resynchronise. `FingerprintIgnore` writes the literal `ignored`
//! marker instead of the value, so two different ignore sets never produce the same bytes.
//!
//! ## Invariants
//!
//! 1. `canonical_form` is a pure function: equal subjects and equal ignore sets give equal bytes.
//! 2. Ordering is explicit (control ids keep their caller order) — no `HashMap` iteration order
//!    ever reaches the digest, so a fingerprint is reproducible across runs and processes.
//! 3. Ignores are **explicit and per-adapter** (section 7.3 last paragraph). This crate has no
//!    default ignore list, because a global default would be an implicit rule nobody can audit.
//! 4. The digest is always `sha256:` + 64 lowercase hex, or the function fails.

use std::collections::BTreeSet;

use assistant_platform_api::Fingerprint;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{VerifyError, VerifyResult};

/// Version tag of the canonical encoding.
///
/// Bumping this changes every fingerprint. It is part of the hashed bytes on purpose: without it,
/// a later encoding change would silently compare new fingerprints against old ones.
const CANONICAL_VERSION: &str = "fingerprint-canonical-v1";

/// Marker written in place of an ignored field.
const IGNORED: &str = "<ignored>";

/// Marker written for an absent optional field (`document`, `scroll`).
const ABSENT: &str = "<absent>";

/// One component of the fingerprint, used to build a [`FingerprintIgnore`] set.
///
/// The set mirrors the inputs of section 7.3's formula, so "which fields does this adapter
/// ignore?" is answerable without reading code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum FingerprintField {
    /// The target window or document title.
    Title,
    /// The ordered set of visible control ids.
    ControlIds,
    /// The role and state of the key controls.
    ControlStates,
    /// The document digest (length plus head/tail sample plus sampled hash).
    Document,
    /// The scroll position.
    ScrollPosition,
    /// Whether a modal dialog was present.
    ModalDialog,
}

impl FingerprintField {
    /// The stable name this field is written under in the canonical form.
    #[must_use]
    pub const fn canonical_name(self) -> &'static str {
        match self {
            Self::Title => "title",
            Self::ControlIds => "control_ids",
            Self::ControlStates => "control_states",
            Self::Document => "document",
            Self::ScrollPosition => "scroll",
            Self::ModalDialog => "modal_dialog",
        }
    }

    /// Every field, in canonical order. Useful for adapters that build an ignore set.
    pub const ALL: [Self; 6] = [
        Self::Title,
        Self::ControlIds,
        Self::ControlStates,
        Self::Document,
        Self::ScrollPosition,
        Self::ModalDialog,
    ];
}

/// The set of fields an adapter wants excluded from its fingerprints.
///
/// Section 7.3 requires fingerprints to be "sensitive enough but not over-sensitive": a caret
/// blink or an animation timestamp must not look like a state change. Which fields those are is a
/// per-adapter decision, so this type carries no default.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FingerprintIgnore {
    /// The excluded fields; a set so that duplicates collapse.
    fields: BTreeSet<FingerprintField>,
}

impl FingerprintIgnore {
    /// Ignores nothing (the strictest, most sensitive fingerprint).
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }

    /// Ignores exactly `fields`; duplicates collapse.
    #[must_use]
    pub fn excluding(fields: impl IntoIterator<Item = FingerprintField>) -> Self {
        Self {
            fields: fields.into_iter().collect(),
        }
    }

    /// Whether `field` is excluded.
    #[must_use]
    pub fn ignores(&self, field: FingerprintField) -> bool {
        self.fields.contains(&field)
    }

    /// The excluded fields in canonical order.
    #[must_use]
    pub fn fields(&self) -> Vec<FingerprintField> {
        self.fields.iter().copied().collect()
    }
}

/// A document's coarse digest: length, head and tail samples, and a sampled hash.
///
/// Section 7.3 deliberately hashes a *summary* rather than the whole document: hashing megabytes
/// on every step would dominate the step's cost, and the head/tail sample is what actually
/// changes when a user edits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentDigest {
    /// Document length in characters (or bytes — the caller must be consistent).
    pub length: u64,
    /// The first N characters.
    pub head: String,
    /// The last N characters.
    pub tail: String,
    /// A hash over a fixed sample of the document, computed by the caller.
    pub sampled_hash: String,
}

/// A scroll position in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScrollPosition {
    /// Horizontal offset.
    pub horizontal: i64,
    /// Vertical offset.
    pub vertical: i64,
}

/// Role and state of one key control.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControlState {
    /// Stable control id (not the visible label — see AGENTS.md section 7).
    pub id: String,
    /// The control role (`Edit`, `Button`, ...).
    pub role: String,
    /// Whether the control was enabled.
    pub enabled: bool,
    /// Whether the control had keyboard focus.
    pub focused: bool,
    /// Whether the control was selected.
    pub selected: bool,
}

/// Everything a fingerprint is computed from (the arguments of section 7.3's formula).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FingerprintSubject {
    /// Which target or subtree this fingerprint describes (for example `document.body`).
    pub scope: String,
    /// The target window or document title.
    pub title: String,
    /// Visible control ids, in the order the observer walked the tree.
    pub control_ids: Vec<String>,
    /// Role and state of the key controls.
    pub control_states: Vec<ControlState>,
    /// The document digest, when the target has a document body.
    pub document: Option<DocumentDigest>,
    /// The scroll position, when the target scrolls.
    pub scroll: Option<ScrollPosition>,
    /// Whether a modal dialog was present.
    pub has_modal_dialog: bool,
}

impl FingerprintSubject {
    /// Creates a subject with no document, no scroll position, and no modal dialog.
    ///
    /// Callers fill the remaining fields with struct-update syntax; every field is public, so a
    /// new component added here forces every construction site to be revisited rather than
    /// silently defaulting.
    #[must_use]
    pub fn new(scope: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            scope: scope.into(),
            title: title.into(),
            control_ids: Vec::new(),
            control_states: Vec::new(),
            document: None,
            scroll: None,
            has_modal_dialog: false,
        }
    }
}

/// Appends one `name=<length>:<value>\n` field to `out`.
///
/// Length-prefixing (rather than escaping) is what makes the encoding injective: the byte count
/// says exactly how many bytes belong to the value, so no value can impersonate a separator.
fn push_field(out: &mut String, name: &str, value: &str) {
    out.push_str(name);
    out.push('=');
    out.push_str(&value.len().to_string());
    out.push(':');
    out.push_str(value);
    out.push('\n');
}

/// Appends one length-prefixed inner element (`<length>:<value>`) used inside list fields.
fn push_prefixed(out: &mut String, value: &str) {
    out.push_str(&value.len().to_string());
    out.push(':');
    out.push_str(value);
}

/// The lowercase hex alphabet used to render a digest.
const HEX_ALPHABET: &[u8; 16] = b"0123456789abcdef";

/// Maps one nibble (0..=15) to its lowercase hex byte.
///
/// `get` rather than an index keeps this panic-free even though every caller masks the nibble
/// first (`indexing_slicing` is denied workspace-wide, and a fallback byte is safer than a panic in
/// a function that feeds a verification decision). `slice::get` is not const-stable, so this stays
/// a plain function.
fn hex_digit(nibble: u8) -> u8 {
    HEX_ALPHABET
        .get(usize::from(nibble))
        .map_or(b'0', |byte| *byte)
}

/// Renders `true`/`false` as `1`/`0` so the canonical bytes are locale- and case-free.
const fn flag(value: bool) -> char {
    if value { '1' } else { '0' }
}

/// Builds the canonical byte string of `subject` under `ignore`.
///
/// This is the exact input to the SHA-256 in [`state_fingerprint`], exposed separately so tests
/// and diagnostics can show *what* was hashed instead of only the digest.
#[must_use]
pub fn canonical_form(subject: &FingerprintSubject, ignore: &FingerprintIgnore) -> String {
    let mut out = String::new();
    push_field(&mut out, "version", CANONICAL_VERSION);
    push_field(&mut out, "scope", &subject.scope);

    if ignore.ignores(FingerprintField::Title) {
        push_field(&mut out, "title", IGNORED);
    } else {
        push_field(&mut out, "title", &subject.title);
    }

    if ignore.ignores(FingerprintField::ControlIds) {
        push_field(&mut out, "control_ids", IGNORED);
    } else {
        let mut encoded = format!("n={}", subject.control_ids.len());
        for id in &subject.control_ids {
            encoded.push('|');
            push_prefixed(&mut encoded, id);
        }
        push_field(&mut out, "control_ids", &encoded);
    }

    if ignore.ignores(FingerprintField::ControlStates) {
        push_field(&mut out, "control_states", IGNORED);
    } else {
        let mut encoded = format!("n={}", subject.control_states.len());
        for state in &subject.control_states {
            encoded.push('|');
            push_prefixed(&mut encoded, &state.id);
            encoded.push(',');
            push_prefixed(&mut encoded, &state.role);
            encoded.push(',');
            encoded.push(flag(state.enabled));
            encoded.push(flag(state.focused));
            encoded.push(flag(state.selected));
        }
        push_field(&mut out, "control_states", &encoded);
    }

    if ignore.ignores(FingerprintField::Document) {
        push_field(&mut out, "document", IGNORED);
    } else {
        match &subject.document {
            None => push_field(&mut out, "document", ABSENT),
            Some(document) => {
                let mut encoded = format!("length={}", document.length);
                encoded.push_str("|head=");
                push_prefixed(&mut encoded, &document.head);
                encoded.push_str("|tail=");
                push_prefixed(&mut encoded, &document.tail);
                encoded.push_str("|hash=");
                push_prefixed(&mut encoded, &document.sampled_hash);
                push_field(&mut out, "document", &encoded);
            }
        }
    }

    if ignore.ignores(FingerprintField::ScrollPosition) {
        push_field(&mut out, "scroll", IGNORED);
    } else {
        match subject.scroll {
            None => push_field(&mut out, "scroll", ABSENT),
            Some(scroll) => push_field(
                &mut out,
                "scroll",
                &format!("x={},y={}", scroll.horizontal, scroll.vertical),
            ),
        }
    }

    if ignore.ignores(FingerprintField::ModalDialog) {
        push_field(&mut out, "modal_dialog", IGNORED);
    } else {
        push_field(
            &mut out,
            "modal_dialog",
            if subject.has_modal_dialog {
                "true"
            } else {
                "false"
            },
        );
    }

    out
}

/// Hashes [`canonical_form`] into the project's fingerprint shape.
///
/// # Errors
///
/// Returns [`VerifyError::InvalidFingerprint`] if the digest could not be rendered into the
/// canonical `sha256:<64 lowercase hex>` shape. This is unreachable for a correct SHA-256
/// encoder, but the function stays fallible rather than unwrapping: a silent `unwrap` here would
/// turn an internal defect into a wrong fingerprint, and a wrong fingerprint is a wrong decision
/// about whether a step happened.
pub fn state_fingerprint(
    subject: &FingerprintSubject,
    ignore: &FingerprintIgnore,
) -> VerifyResult<Fingerprint> {
    let canonical = canonical_form(subject, ignore);
    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());
    let digest = hasher.finalize();

    let mut encoded = String::with_capacity(Fingerprint::PREFIX.len() + 64);
    encoded.push_str(Fingerprint::PREFIX);
    let mut raw = Vec::with_capacity(64);
    for byte in digest {
        raw.push(hex_digit(byte >> 4));
        raw.push(hex_digit(byte & 0x0f));
    }
    let digest_hex = String::from_utf8(raw).map_err(|error| VerifyError::InvalidFingerprint {
        reason: format!("SHA-256 encoder produced a non-ASCII digest: {error}"),
    })?;
    encoded.push_str(&digest_hex);

    Fingerprint::parse(encoded).map_err(|error| VerifyError::InvalidFingerprint {
        reason: format!("SHA-256 encoder produced an invalid fingerprint: {error}"),
    })
}

/// Validates a fingerprint string written by a tool author.
///
/// # Errors
///
/// Returns [`VerifyError::InvalidFingerprint`] when the value is not
/// `sha256:` + 64 lowercase hex characters.
pub fn parse_fingerprint(value: &str) -> VerifyResult<Fingerprint> {
    Fingerprint::parse(value).map_err(|error| VerifyError::InvalidFingerprint {
        reason: error.message().to_owned(),
    })
}
