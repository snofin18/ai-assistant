//! Load the Notepad Adapter target descriptors used by the production Host.
//!
//! Responsibilities:
//! - turn `selectors/targets.json` into the platform's serializable
//!   [`TargetDescriptor`] values;
//! - reject missing or unsupported target declarations before the Host starts.
//!
//! Boundaries:
//! - does not resolve windows or elements;
//! - does not execute tools or grant permission;
//! - does not contain platform handles.
//!
//! Invariants:
//! 1. the declared app id is non-empty and every required target exists;
//! 2. only the platform's `error_and_ask` ambiguity policy is accepted;
//! 3. descriptors retain the adapter's ordered candidate chains unchanged.
//!
//! Related documents: ADR-0022, ADR-0043, ADR-0058, and
//! `adapters/com.microsoft.notepad/selectors/targets.json`.

use std::collections::BTreeMap;
use std::path::Path;

use assistant_platform_api::{
    OnAmbiguous, OnNotFound, ResolutionPolicy, SelectorCandidate, TargetDescriptor,
};
use serde::Deserialize;
use thiserror::Error;

/// Stable target ids consumed by the 1a handlers.
pub(crate) const MAIN_WINDOW_TARGET: &str = "main_window";
/// Editor target id.
pub(crate) const EDITOR_TARGET: &str = "editor";
/// Add-tab button target id.
pub(crate) const ADD_TAB_BUTTON_TARGET: &str = "add_tab_button";
/// Save-as dialog target id.
pub(crate) const SAVE_AS_DIALOG_TARGET: &str = "save_as_dialog";
/// Save-as filename field target id.
pub(crate) const SAVE_AS_FILENAME_TARGET: &str = "save_as_filename";
/// Save-as final button target id.
pub(crate) const SAVE_AS_SAVE_BUTTON_TARGET: &str = "save_as_save_button";

const REQUIRED_TARGETS: &[&str] = &[
    MAIN_WINDOW_TARGET,
    EDITOR_TARGET,
    ADD_TAB_BUTTON_TARGET,
    SAVE_AS_DIALOG_TARGET,
    SAVE_AS_FILENAME_TARGET,
    SAVE_AS_SAVE_BUTTON_TARGET,
];

/// Failure to load the target descriptor package.
#[derive(Debug, Error)]
#[non_exhaustive]
pub(crate) enum TargetCatalogError {
    /// The file could not be read.
    #[error("target descriptor file `{path}` could not be read: {reason}")]
    Read {
        /// Attempted path.
        path: String,
        /// I/O failure.
        reason: String,
    },

    /// The JSON shape or a descriptor value is invalid.
    #[error("target descriptor file is invalid: {reason}")]
    Malformed {
        /// Failure detail.
        reason: String,
    },

    /// A required adapter target is absent.
    #[error("target descriptor package is missing required target `{target}`")]
    MissingTarget {
        /// Missing target id.
        target: String,
    },

    /// The adapter asks for behavior this binary does not implement.
    #[error("target descriptor policy is unsupported: {reason}")]
    UnsupportedPolicy {
        /// Why the policy cannot be used.
        reason: String,
    },
}

/// Ordered target descriptors loaded from one adapter package.
pub(crate) struct NotepadTargetCatalog {
    app_id: String,
    descriptors: BTreeMap<String, TargetDescriptor>,
}

impl NotepadTargetCatalog {
    /// Loads and validates `selectors/targets.json`.
    pub(crate) fn load(path: &Path) -> Result<Self, TargetCatalogError> {
        let body = std::fs::read_to_string(path).map_err(|error| TargetCatalogError::Read {
            path: path.display().to_string(),
            reason: error.to_string(),
        })?;
        let declared: DeclaredTargetsFile =
            serde_json::from_str(&body).map_err(|error| TargetCatalogError::Malformed {
                reason: error.to_string(),
            })?;
        if declared.app_id.trim().is_empty() {
            return Err(TargetCatalogError::Malformed {
                reason: "app_id must not be blank".to_owned(),
            });
        }
        let policy = build_resolution_policy(&declared.policy)?;
        let mut descriptors = BTreeMap::new();
        for target in declared.targets {
            if descriptors.contains_key(&target.id) {
                return Err(TargetCatalogError::Malformed {
                    reason: format!("duplicate target id `{}`", target.id),
                });
            }
            let descriptor = TargetDescriptor::new(
                "2.0".to_owned(),
                declared.app_id.clone(),
                target.candidates.clone(),
                target.candidates,
                policy.clone(),
            );
            descriptors.insert(target.id, descriptor);
        }
        for required in REQUIRED_TARGETS {
            if !descriptors.contains_key(*required) {
                return Err(TargetCatalogError::MissingTarget {
                    target: (*required).to_owned(),
                });
            }
        }
        for descriptor in descriptors.values() {
            descriptor
                .validate()
                .map_err(|error| TargetCatalogError::Malformed {
                    reason: error.to_string(),
                })?;
        }
        Ok(Self {
            app_id: declared.app_id,
            descriptors,
        })
    }

    /// Returns the adapter app id.
    pub(crate) fn app_id(&self) -> &str {
        &self.app_id
    }

    /// Returns one required descriptor.
    pub(crate) fn descriptor(&self, target: &str) -> Result<&TargetDescriptor, TargetCatalogError> {
        self.descriptors
            .get(target)
            .ok_or_else(|| TargetCatalogError::MissingTarget {
                target: target.to_owned(),
            })
    }
}

#[derive(Debug, Deserialize)]
struct DeclaredTargetsFile {
    app_id: String,
    policy: DeclaredPolicy,
    targets: Vec<DeclaredTarget>,
}

#[derive(Debug, Deserialize)]
struct DeclaredTarget {
    id: String,
    candidates: Vec<SelectorCandidate>,
}

#[derive(Debug, Deserialize)]
struct DeclaredPolicy {
    on_ambiguous: String,
    on_not_found: String,
    min_score_to_try: f64,
    max_resolve_ms: u64,
}

fn build_resolution_policy(
    declared: &DeclaredPolicy,
) -> Result<ResolutionPolicy, TargetCatalogError> {
    if declared.on_ambiguous != "error_and_ask" {
        return Err(TargetCatalogError::UnsupportedPolicy {
            reason: format!(
                "on_ambiguous=`{}` is not `error_and_ask`",
                declared.on_ambiguous
            ),
        });
    }
    let then_escalate = match declared.on_not_found.as_str() {
        "escalate_to_human" => true,
        "return_not_found" => false,
        other => {
            return Err(TargetCatalogError::UnsupportedPolicy {
                reason: format!("on_not_found=`{other}` is unknown"),
            });
        }
    };
    ResolutionPolicy::new(
        OnAmbiguous::ErrorAndAsk,
        OnNotFound::new(Vec::new(), then_escalate),
        declared.max_resolve_ms,
        declared.min_score_to_try,
    )
    .map_err(|error| TargetCatalogError::Malformed {
        reason: error.to_string(),
    })
}
