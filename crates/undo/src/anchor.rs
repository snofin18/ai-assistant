//! Rollback anchors from architecture v2 section 9.3.
//!
//! Responsibility: validate content digests, shadow-copy paths, undo budgets, anchor payloads,
//! and the capability prerequisites used to choose an anchor strategy.
//!
//! Boundary: this module captures no content and copies no files. It models what the caller must
//! capture and lets the executor perform the corresponding restore.

use assistant_platform_api::Fingerprint;

use crate::error::{UndoError, UndoResult};
use crate::id::{AnchorId, StepId, TargetId};
use crate::recipe::{RollbackAction, RollbackRecipe};
use crate::reversibility::Reversibility;

const SHA256_HEX_LENGTH: usize = 64;

/// A content-addressed SHA-256 digest in lowercase hexadecimal form.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ContentDigest(String);

impl ContentDigest {
    /// Parses exactly 64 lowercase hexadecimal characters.
    ///
    /// # Errors
    ///
    /// Returns [`UndoError::InvalidContentDigest`] for any other length or character set.
    pub fn parse(value: impl Into<String>) -> UndoResult<Self> {
        let value = value.into();
        let is_lower_hex = value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
        if value.len() != SHA256_HEX_LENGTH || !is_lower_hex {
            return Err(UndoError::InvalidContentDigest {
                reason: "expected exactly 64 lowercase hexadecimal characters".to_owned(),
            });
        }
        Ok(Self(value))
    }

    /// Returns the digest as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Path to a shadow copy created before modifying a file.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ShadowCopyPath(String);

impl ShadowCopyPath {
    /// Parses a non-empty path with no NUL byte or `..` traversal segment.
    ///
    /// # Errors
    ///
    /// Returns [`UndoError::InvalidShadowCopyPath`] when the path is unsafe to pass to a storage
    /// executor. Existence and file integrity remain executor responsibilities.
    pub fn parse(value: impl Into<String>) -> UndoResult<Self> {
        let value = value.into();
        if value.is_empty() {
            return Err(UndoError::InvalidShadowCopyPath {
                reason: "must not be empty".to_owned(),
            });
        }
        if value.contains('\0') {
            return Err(UndoError::InvalidShadowCopyPath {
                reason: "must not contain NUL".to_owned(),
            });
        }
        if value.split(['/', '\\']).any(|segment| segment == "..") {
            return Err(UndoError::InvalidShadowCopyPath {
                reason: "must not contain a `..` traversal segment".to_owned(),
            });
        }
        Ok(Self(value))
    }

    /// Returns the path as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Shadow-copy metadata needed to restore a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShadowCopy {
    /// Path supplied by the storage subsystem.
    pub path: ShadowCopyPath,
    /// Digest of the captured content.
    pub expected_digest: ContentDigest,
}

impl ShadowCopy {
    /// Creates a validated shadow-copy anchor.
    #[must_use]
    pub const fn new(path: ShadowCopyPath, expected_digest: ContentDigest) -> Self {
        Self {
            path,
            expected_digest,
        }
    }
}

/// Bounded number of application undo steps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct UndoStepCount(u8);

impl UndoStepCount {
    /// Creates a count in the inclusive range 1 through 50.
    ///
    /// # Errors
    ///
    /// Returns [`UndoError::InvalidUndoStepCount`] outside that range.
    pub const fn new(value: u8) -> UndoResult<Self> {
        if value == 0 || value > 50 {
            return Err(UndoError::InvalidUndoStepCount { value });
        }
        Ok(Self(value))
    }

    /// Returns the count.
    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }
}

/// Lifetime of an undo-stack anchor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UndoValidity {
    /// Valid until the document closes.
    DocumentClose,
    /// Valid until the application session ends.
    SessionEnd,
    /// Valid until a caller-supplied UTC RFC 3339 timestamp.
    ExplicitDeadline {
        /// UTC RFC 3339 timestamp.
        timestamp: String,
    },
}

/// Undo-stack budget captured before a step executes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UndoBudget {
    /// Number of application undo steps attributable to this operation.
    pub recorded_steps: UndoStepCount,
    /// Lifetime during which those steps are expected to remain available.
    pub valid_until: UndoValidity,
}

impl UndoBudget {
    /// Creates an undo-stack budget.
    #[must_use]
    pub const fn new(recorded_steps: UndoStepCount, valid_until: UndoValidity) -> Self {
        Self {
            recorded_steps,
            valid_until,
        }
    }
}

/// High-level anchor strategy selected for one reversibility level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnchorStrategy {
    /// Use a bounded number of application undo steps.
    UndoStack,
    /// Restore a complete content snapshot.
    ContentSnapshot,
    /// Restore a file from a shadow copy.
    ShadowCopy,
    /// Execute a declared compensating recipe.
    CompensatingAction,
    /// Preserve evidence only; no rollback is promised.
    EvidenceOnly,
}

impl AnchorStrategy {
    /// Returns the stable strategy name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UndoStack => "undo_stack",
            Self::ContentSnapshot => "content_snapshot",
            Self::ShadowCopy => "shadow_copy",
            Self::CompensatingAction => "compensating_action",
            Self::EvidenceOnly => "evidence_only",
        }
    }
}

/// Whether a prerequisite was observed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceAvailability {
    /// The capability or artifact is available.
    Available,
    /// The capability or artifact is known to be unavailable.
    Unavailable,
    /// The caller did not probe the prerequisite.
    Unknown,
}

impl EvidenceAvailability {
    fn require(self, capability: &'static str, context: &str) -> UndoResult<()> {
        match self {
            Self::Available => Ok(()),
            Self::Unavailable | Self::Unknown => Err(UndoError::CapabilityMissing {
                capability,
                context: context.to_owned(),
            }),
        }
    }
}

/// Target shape used to choose between content snapshots and file shadow copies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnchorTargetKind {
    /// A file or file-like target.
    File,
    /// An in-memory document or structured object.
    Document,
    /// Unknown target shape.
    Unknown,
}

/// Known prerequisites for anchor selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnchorAvailability {
    /// Whether the application exposes an undo stack.
    pub undo_stack: EvidenceAvailability,
    /// Whether the full target content can be captured.
    pub complete_content: EvidenceAvailability,
    /// Whether a file shadow copy can be created.
    pub file_shadow_copy: EvidenceAvailability,
    /// Whether an adapter declared a compensating recipe.
    pub compensating_recipe: EvidenceAvailability,
    /// Shape of the target.
    pub target_kind: AnchorTargetKind,
}

impl AnchorAvailability {
    /// Creates a fully specified capability snapshot.
    #[must_use]
    pub const fn new(
        undo_stack: EvidenceAvailability,
        complete_content: EvidenceAvailability,
        file_shadow_copy: EvidenceAvailability,
        compensating_recipe: EvidenceAvailability,
        target_kind: AnchorTargetKind,
    ) -> Self {
        Self {
            undo_stack,
            complete_content,
            file_shadow_copy,
            compensating_recipe,
            target_kind,
        }
    }
}

/// Selects the required anchor strategy, failing closed when evidence is missing.
///
/// # Errors
///
/// Returns [`UndoError::CapabilityMissing`] when the declared reversibility level lacks its
/// required anchor capability.
pub fn plan_anchor_strategy(
    reversibility: Reversibility,
    availability: &AnchorAvailability,
) -> UndoResult<AnchorStrategy> {
    match reversibility {
        Reversibility::L0UndoStack => {
            availability.undo_stack.require(
                "app.undo_stack",
                "L0_undo_stack requires an adapter-declared undo stack",
            )?;
            Ok(AnchorStrategy::UndoStack)
        }
        Reversibility::L1Snapshot => match availability.target_kind {
            AnchorTargetKind::File => {
                availability.file_shadow_copy.require(
                    "storage.shadow_copy",
                    "file targets require a shadow copy for L1 recovery",
                )?;
                Ok(AnchorStrategy::ShadowCopy)
            }
            AnchorTargetKind::Document => {
                availability.complete_content.require(
                    "target.complete_content",
                    "document targets require a complete content snapshot for L1 recovery",
                )?;
                Ok(AnchorStrategy::ContentSnapshot)
            }
            AnchorTargetKind::Unknown => Err(UndoError::CapabilityMissing {
                capability: "target.kind",
                context: "cannot choose an L1 anchor without a known target shape".to_owned(),
            }),
        },
        Reversibility::L2Compensating => {
            availability.compensating_recipe.require(
                "adapter.compensating_recipe",
                "L2_compensating requires a declared rollback recipe",
            )?;
            Ok(AnchorStrategy::CompensatingAction)
        }
        Reversibility::L3Irreversible => Ok(AnchorStrategy::EvidenceOnly),
    }
}

/// Captured data needed for one anchor kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnchorKind {
    /// Bounded application undo steps.
    UndoStack {
        /// Captured undo budget.
        budget: UndoBudget,
        /// Optional L1 fallback captured because section 9.1 treats L0 and L1 as additive.
        fallback_snapshot: Option<ContentDigest>,
    },
    /// Content-addressable complete snapshot.
    ContentSnapshot {
        /// Digest of the captured content.
        content_digest: ContentDigest,
    },
    /// File shadow copy.
    ShadowCopy {
        /// Shadow-copy metadata.
        shadow_copy: ShadowCopy,
    },
    /// Compensating recipe supplied by the adapter.
    CompensatingAction {
        /// Recipe to execute.
        recipe: RollbackRecipe,
    },
    /// Evidence only; no rollback promise.
    EvidenceOnly,
}

impl AnchorKind {
    /// Returns the strategy represented by this payload.
    #[must_use]
    pub const fn strategy(&self) -> AnchorStrategy {
        match self {
            Self::UndoStack { .. } => AnchorStrategy::UndoStack,
            Self::ContentSnapshot { .. } => AnchorStrategy::ContentSnapshot,
            Self::ShadowCopy { .. } => AnchorStrategy::ShadowCopy,
            Self::CompensatingAction { .. } => AnchorStrategy::CompensatingAction,
            Self::EvidenceOnly => AnchorStrategy::EvidenceOnly,
        }
    }
}

/// Rollback anchor captured before a step executes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Anchor {
    id: AnchorId,
    step_id: StepId,
    target_id: TargetId,
    reversibility: Reversibility,
    pre_fingerprint: Fingerprint,
    post_fingerprint: Option<Fingerprint>,
    kind: AnchorKind,
}

impl Anchor {
    /// Creates an anchor and validates that its payload matches the reversibility level.
    ///
    /// # Errors
    ///
    /// Returns [`UndoError::AnchorKindMismatch`] for mismatched level and payload, and
    /// [`UndoError::RecipeAnchorMismatch`] for a compensating recipe belonging to another anchor.
    pub fn new(
        anchor_id: AnchorId,
        step_id: StepId,
        target_id: TargetId,
        reversibility: Reversibility,
        pre_fingerprint: Fingerprint,
        kind: AnchorKind,
    ) -> UndoResult<Self> {
        validate_anchor_kind(&anchor_id, reversibility, &kind)?;
        Ok(Self {
            id: anchor_id,
            step_id,
            target_id,
            reversibility,
            pre_fingerprint,
            post_fingerprint: None,
            kind,
        })
    }

    /// Returns the anchor identifier.
    #[must_use]
    pub const fn anchor_id(&self) -> &AnchorId {
        &self.id
    }

    /// Returns the step identifier.
    #[must_use]
    pub const fn step_id(&self) -> &StepId {
        &self.step_id
    }

    /// Returns the target identifier.
    #[must_use]
    pub const fn target_id(&self) -> &TargetId {
        &self.target_id
    }

    /// Returns the step reversibility level.
    #[must_use]
    pub const fn reversibility(&self) -> Reversibility {
        self.reversibility
    }

    /// Returns the fingerprint captured before the step.
    #[must_use]
    pub const fn pre_fingerprint(&self) -> &Fingerprint {
        &self.pre_fingerprint
    }

    /// Returns the fingerprint captured after the step, when it has been recorded.
    #[must_use]
    pub const fn post_fingerprint(&self) -> Option<&Fingerprint> {
        self.post_fingerprint.as_ref()
    }

    /// Returns the anchor payload.
    #[must_use]
    pub const fn kind(&self) -> &AnchorKind {
        &self.kind
    }

    /// Returns the strategy represented by the payload.
    #[must_use]
    pub const fn strategy(&self) -> AnchorStrategy {
        self.kind.strategy()
    }

    /// Records the post-step fingerprint exactly once.
    ///
    /// # Errors
    ///
    /// Returns [`UndoError::PostFingerprintAlreadyRecorded`] if a caller attempts to overwrite the
    /// post-step state.
    pub fn record_post_fingerprint(&mut self, fingerprint: Fingerprint) -> UndoResult<()> {
        if self.post_fingerprint.is_some() {
            return Err(UndoError::PostFingerprintAlreadyRecorded {
                anchor_id: self.id.clone(),
            });
        }
        self.post_fingerprint = Some(fingerprint);
        Ok(())
    }

    /// Returns the shadow-copy metadata embedded in this anchor, when present.
    #[must_use]
    pub const fn shadow_copy(&self) -> Option<&ShadowCopy> {
        match &self.kind {
            AnchorKind::ShadowCopy { shadow_copy } => Some(shadow_copy),
            _ => None,
        }
    }

    /// Returns the optional content-snapshot fallback embedded in an L0 anchor.
    #[must_use]
    pub const fn fallback_snapshot(&self) -> Option<&ContentDigest> {
        match &self.kind {
            AnchorKind::UndoStack {
                fallback_snapshot, ..
            } => fallback_snapshot.as_ref(),
            _ => None,
        }
    }
}

fn validate_anchor_kind(
    anchor_id: &AnchorId,
    reversibility: Reversibility,
    kind: &AnchorKind,
) -> UndoResult<()> {
    let strategy = kind.strategy();
    let matches_level = matches!(
        (reversibility, strategy),
        (Reversibility::L0UndoStack, AnchorStrategy::UndoStack)
            | (
                Reversibility::L1Snapshot,
                AnchorStrategy::ContentSnapshot | AnchorStrategy::ShadowCopy
            )
            | (
                Reversibility::L2Compensating,
                AnchorStrategy::CompensatingAction
            )
            | (Reversibility::L3Irreversible, AnchorStrategy::EvidenceOnly)
    );
    if !matches_level {
        return Err(UndoError::AnchorKindMismatch {
            reversibility,
            strategy,
        });
    }
    if let AnchorKind::CompensatingAction { recipe } = kind
        && recipe.anchor_id() != anchor_id
    {
        return Err(UndoError::RecipeAnchorMismatch {
            anchor_id: anchor_id.clone(),
            recipe_anchor_id: recipe.anchor_id().clone(),
        });
    }
    if let AnchorKind::CompensatingAction { recipe } = kind
        && !recipe
            .actions()
            .iter()
            .all(|action| matches!(action, RollbackAction::CompensatingAction { .. }))
    {
        return Err(UndoError::AnchorKindMismatch {
            reversibility,
            strategy,
        });
    }
    Ok(())
}
