//! Typed rollback recipes and the recipe builder for each anchor kind.
//!
//! Responsibility: validate tool names, action counts, undo limits, and anchor-to-action shape.
//!
//! Boundary: recipe actions are declarative. This module does not execute them and does not
//! validate tool arguments against a tool schema; that belongs to the executor/tool bus.

use assistant_platform_api::Fingerprint;

use crate::anchor::{Anchor, AnchorKind, ContentDigest, ShadowCopyPath, UndoStepCount};
use crate::error::{UndoError, UndoResult};
use crate::id::{AnchorId, TargetId};

/// Maximum number of actions in one rollback recipe.
pub const MAX_ROLLBACK_ACTIONS: usize = 64;

/// A fully qualified tool name in `<app>.<domain>.<action>` form.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ToolName(String);

impl ToolName {
    /// Parses a three-segment lowercase tool name.
    ///
    /// # Errors
    ///
    /// Returns [`UndoError::InvalidToolName`] for empty or extra segments and for characters
    /// outside lowercase ASCII letters, digits, and underscores.
    pub fn parse(value: impl Into<String>) -> UndoResult<Self> {
        let value = value.into();
        let segments: Vec<&str> = value.split('.').collect();
        let is_valid_segment = |segment: &str| {
            !segment.is_empty()
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        };
        if segments.len() != 3 || !segments.iter().copied().all(is_valid_segment) {
            return Err(UndoError::InvalidToolName { value });
        }
        Ok(Self(value))
    }

    /// Returns the tool name as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A compensating action invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolInvocation {
    /// Tool to invoke.
    pub tool_name: ToolName,
    /// Target of the compensating action.
    pub target_id: TargetId,
    /// Opaque JSON arguments validated by the executor before invocation.
    pub arguments_json: String,
}

impl ToolInvocation {
    /// Creates a compensating invocation with non-empty argument text.
    ///
    /// # Errors
    ///
    /// Returns [`UndoError::InvalidCompensatingArguments`] when arguments are empty or contain NUL.
    pub fn new(
        tool_name: ToolName,
        target_id: TargetId,
        arguments_json: impl Into<String>,
    ) -> UndoResult<Self> {
        let arguments_json = arguments_json.into();
        if arguments_json.is_empty() {
            return Err(UndoError::InvalidCompensatingArguments {
                reason: "must not be empty".to_owned(),
            });
        }
        if arguments_json.contains('\0') {
            return Err(UndoError::InvalidCompensatingArguments {
                reason: "must not contain NUL".to_owned(),
            });
        }
        Ok(Self {
            tool_name,
            target_id,
            arguments_json,
        })
    }
}

/// One atomic rollback action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RollbackAction {
    /// Ask the application to undo a bounded number of steps.
    UndoStack {
        /// Number of undo invocations.
        times: UndoStepCount,
        /// Fingerprint that must be reached.
        until_fingerprint: Fingerprint,
    },
    /// Restore a content-addressed snapshot.
    RestoreContentSnapshot {
        /// Digest of the snapshot content.
        content_digest: ContentDigest,
    },
    /// Restore a file from a shadow copy.
    RestoreShadowCopy {
        /// Shadow-copy path.
        path: ShadowCopyPath,
        /// Digest expected after restoration.
        expected_digest: ContentDigest,
    },
    /// Execute one semantic compensating action.
    CompensatingAction {
        /// Invocation to execute.
        invocation: ToolInvocation,
    },
}

/// Ordered rollback actions belonging to one anchor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RollbackRecipe {
    anchor_id: AnchorId,
    actions: Vec<RollbackAction>,
}

impl RollbackRecipe {
    /// Creates a non-empty, bounded rollback recipe.
    ///
    /// # Errors
    ///
    /// Returns [`UndoError::EmptyRollbackRecipe`] or [`UndoError::RollbackRecipeTooLong`].
    pub fn new(anchor_id: AnchorId, actions: Vec<RollbackAction>) -> UndoResult<Self> {
        if actions.is_empty() {
            return Err(UndoError::EmptyRollbackRecipe { anchor_id });
        }
        if actions.len() > MAX_ROLLBACK_ACTIONS {
            return Err(UndoError::RollbackRecipeTooLong {
                count: actions.len(),
                maximum: MAX_ROLLBACK_ACTIONS,
            });
        }
        Ok(Self { anchor_id, actions })
    }

    /// Returns the anchor this recipe belongs to.
    #[must_use]
    pub const fn anchor_id(&self) -> &AnchorId {
        &self.anchor_id
    }

    /// Returns the ordered actions.
    #[must_use]
    pub fn actions(&self) -> &[RollbackAction] {
        &self.actions
    }
}

/// Builds the executable recipe for an anchor.
///
/// # Errors
///
/// Returns [`UndoError::IrreversibleStep`] for an evidence-only anchor, and propagates validation
/// errors from an embedded L2 recipe.
pub fn build_rollback_recipe(anchor: &Anchor) -> UndoResult<RollbackRecipe> {
    match anchor.kind() {
        AnchorKind::UndoStack { budget, .. } => RollbackRecipe::new(
            anchor.anchor_id().clone(),
            vec![RollbackAction::UndoStack {
                times: budget.recorded_steps,
                until_fingerprint: anchor.pre_fingerprint().clone(),
            }],
        ),
        AnchorKind::ContentSnapshot { content_digest } => RollbackRecipe::new(
            anchor.anchor_id().clone(),
            vec![RollbackAction::RestoreContentSnapshot {
                content_digest: content_digest.clone(),
            }],
        ),
        AnchorKind::ShadowCopy { shadow_copy } => RollbackRecipe::new(
            anchor.anchor_id().clone(),
            vec![RollbackAction::RestoreShadowCopy {
                path: shadow_copy.path.clone(),
                expected_digest: shadow_copy.expected_digest.clone(),
            }],
        ),
        AnchorKind::CompensatingAction { recipe } => {
            if &recipe.anchor_id != anchor.anchor_id() {
                return Err(UndoError::RecipeAnchorMismatch {
                    anchor_id: anchor.anchor_id().clone(),
                    recipe_anchor_id: recipe.anchor_id.clone(),
                });
            }
            Ok(recipe.clone())
        }
        AnchorKind::EvidenceOnly => Err(UndoError::IrreversibleStep {
            step_id: anchor.step_id().as_str().to_owned(),
        }),
    }
}

/// Builds the optional L1 fallback recipe embedded in an L0 anchor.
///
/// # Errors
///
/// Propagates recipe construction errors. Returns `Ok(None)` when the anchor has no fallback.
pub fn build_fallback_recipe(anchor: &Anchor) -> UndoResult<Option<RollbackRecipe>> {
    let Some(content_digest) = anchor.fallback_snapshot() else {
        return Ok(None);
    };
    RollbackRecipe::new(
        anchor.anchor_id().clone(),
        vec![RollbackAction::RestoreContentSnapshot {
            content_digest: content_digest.clone(),
        }],
    )
    .map(Some)
}

/// Validates that a prebuilt recipe is legal for a one-step reversibility level.
///
/// This helper is used by callers that reconstruct recipes from storage instead of calling
/// [`build_rollback_recipe`].
///
/// # Errors
///
/// Returns [`UndoError::AnchorKindMismatch`] when the first action does not match the anchor kind.
pub fn validate_rollback_recipe(anchor: &Anchor, recipe: &RollbackRecipe) -> UndoResult<()> {
    if &recipe.anchor_id != anchor.anchor_id() {
        return Err(UndoError::RecipeAnchorMismatch {
            anchor_id: anchor.anchor_id().clone(),
            recipe_anchor_id: recipe.anchor_id.clone(),
        });
    }
    let is_valid = match anchor.kind() {
        AnchorKind::UndoStack { budget, .. } => matches!(
            recipe.actions.as_slice(),
            [RollbackAction::UndoStack {
                times,
                until_fingerprint
            }] if *times == budget.recorded_steps
                && until_fingerprint == anchor.pre_fingerprint()
        ),
        AnchorKind::ContentSnapshot { content_digest } => matches!(
            recipe.actions.as_slice(),
            [RollbackAction::RestoreContentSnapshot {
                content_digest: candidate
            }] if candidate == content_digest
        ),
        AnchorKind::ShadowCopy { shadow_copy } => matches!(
            recipe.actions.as_slice(),
            [RollbackAction::RestoreShadowCopy {
                path,
                expected_digest
            }] if path == &shadow_copy.path && expected_digest == &shadow_copy.expected_digest
        ),
        AnchorKind::CompensatingAction {
            recipe: expected_recipe,
        } => recipe == expected_recipe,
        AnchorKind::EvidenceOnly => false,
    };
    if is_valid {
        Ok(())
    } else {
        Err(UndoError::AnchorKindMismatch {
            reversibility: anchor.reversibility(),
            strategy: anchor.strategy(),
        })
    }
}
