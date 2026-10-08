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
/// Tab-count readout target id.
///
/// Unlike the six targets above this one is **optional**: the real Notepad pack
/// exposes no stable tab-count element, so requiring it would force that pack to
/// declare a bogus target. An adapter that declares it gets tab-count
/// observation; one that does not keeps failing closed, and the error names the
/// missing declaration instead of inventing a number.
pub(crate) const TAB_COUNT_TARGET: &str = "tab_count";

/// Required target ids for the Notepad adapter package.
///
/// Per ADR-0084 the loader no longer hardcodes this set: each adapter passes its
/// own required list to [`NotepadTargetCatalog::load`], so a second adapter
/// (Paint) can declare a different required set without a second loader.
pub(crate) const NOTEPAD_REQUIRED_TARGETS: &[&str] = &[
    MAIN_WINDOW_TARGET,
    EDITOR_TARGET,
    ADD_TAB_BUTTON_TARGET,
    SAVE_AS_DIALOG_TARGET,
    SAVE_AS_FILENAME_TARGET,
    SAVE_AS_SAVE_BUTTON_TARGET,
];

/// Targets an adapter may declare without being required to.
///
/// Declaring one opts that adapter into the capability; the loader validates it
/// exactly like a required target when present.
const OPTIONAL_TARGETS: &[&str] = &[TAB_COUNT_TARGET];

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
#[derive(Debug)]
pub(crate) struct NotepadTargetCatalog {
    app_id: String,
    descriptors: BTreeMap<String, TargetDescriptor>,
}

impl NotepadTargetCatalog {
    /// Loads and validates `selectors/targets.json`.
    ///
    /// `required_targets` is the adapter's own required id list (ADR-0084 D1):
    /// the loader rejects a package that omits any of them, but does not impose
    /// a Notepad-shaped set on a different adapter.
    pub(crate) fn load(path: &Path, required_targets: &[&str]) -> Result<Self, TargetCatalogError> {
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
        // Window-scope descriptors carry only window candidates; element-scope descriptors carry
        // the app window anchor plus their element candidates. Passing one chain to both fields
        // (the old behavior) silently ignored the declared `scope` and let element selectors run
        // in the window-resolution path.
        let window_anchor = declared
            .targets
            .iter()
            .find(|target| target.id == MAIN_WINDOW_TARGET)
            .map(|target| target.candidates.clone())
            .ok_or_else(|| TargetCatalogError::MissingTarget {
                target: MAIN_WINDOW_TARGET.to_owned(),
            })?;
        let mut descriptors = BTreeMap::new();
        for target in declared.targets {
            if descriptors.contains_key(&target.id) {
                return Err(TargetCatalogError::Malformed {
                    reason: format!("duplicate target id `{}`", target.id),
                });
            }
            let (window_candidates, element_candidates) = match target.scope.as_str() {
                "window" => (target.candidates, Vec::new()),
                "element" => (window_anchor.clone(), target.candidates),
                other => {
                    return Err(TargetCatalogError::Malformed {
                        reason: format!(
                            "target `{}` has unsupported scope `{other}` (expected `window` or `element`)",
                            target.id
                        ),
                    });
                }
            };
            let descriptor = TargetDescriptor::new(
                "2.0".to_owned(),
                declared.app_id.clone(),
                window_candidates,
                element_candidates,
                policy.clone(),
            );
            descriptors.insert(target.id, descriptor);
        }
        for required in required_targets {
            if !descriptors.contains_key(*required) {
                return Err(TargetCatalogError::MissingTarget {
                    target: (*required).to_owned(),
                });
            }
        }
        // Optional targets are validated when an adapter declares them. Their
        // absence is not an error: it means "this adapter offers no observation
        // for that capability", and the caller must fail closed rather than
        // fabricate one.
        for optional in OPTIONAL_TARGETS {
            if let Some(descriptor) = descriptors.get(*optional) {
                descriptor
                    .validate()
                    .map_err(|error| TargetCatalogError::Malformed {
                        reason: format!("optional target `{optional}`: {error}"),
                    })?;
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
    scope: String,
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

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        EDITOR_TARGET, MAIN_WINDOW_TARGET, NOTEPAD_REQUIRED_TARGETS, NotepadTargetCatalog,
        SAVE_AS_DIALOG_TARGET,
    };

    fn workspace_root() -> PathBuf {
        let canonical = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .expect("workspace root");
        let text = canonical.to_string_lossy();
        PathBuf::from(text.strip_prefix(r"\\?\").unwrap_or(&text))
    }

    #[test]
    fn test_scope_routes_window_and_element_candidates() {
        let path = workspace_root().join("adapters/com.microsoft.notepad/selectors/targets.json");
        let catalog = NotepadTargetCatalog::load(&path, NOTEPAD_REQUIRED_TARGETS)
            .expect("adapter target catalog");
        let window = catalog.descriptor(MAIN_WINDOW_TARGET).expect("main window");
        assert!(
            !window.window_candidates().is_empty(),
            "window scope must keep window candidates"
        );
        assert!(
            window.element_candidates().is_empty(),
            "window scope must not expose selector chains as element candidates"
        );
        let editor = catalog.descriptor(EDITOR_TARGET).expect("editor");
        assert!(
            !editor.window_candidates().is_empty(),
            "element scope needs a non-empty window anchor for descriptor validation"
        );
        assert!(
            !editor.element_candidates().is_empty(),
            "element scope must keep its declared element candidates"
        );
        let dialog = catalog
            .descriptor(SAVE_AS_DIALOG_TARGET)
            .expect("save as dialog");
        assert!(
            dialog.element_candidates().is_empty(),
            "the save-as dialog is window-scoped"
        );
    }

    #[test]
    fn test_unknown_scope_is_rejected() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "notepad-targets-scope-{}-{unique}.json",
            std::process::id()
        ));
        let body = r#"{
  "schema_version": 1,
  "app_id": "com.example.scope",
  "policy": {
    "on_ambiguous": "error_and_ask",
    "on_not_found": "escalate_to_human",
    "min_score_to_try": 0.1,
    "max_resolve_ms": 3000
  },
  "targets": [
    {"id": "main_window", "scope": "window", "candidates": [{"id": "w", "kind": "automation_id", "value": {"text": "W"}, "score": 1.0, "locale_dependent": false}]},
    {"id": "editor", "scope": "element", "candidates": [{"id": "e", "kind": "automation_id", "value": {"text": "E"}, "score": 1.0, "locale_dependent": false}]},
    {"id": "add_tab_button", "scope": "element", "candidates": [{"id": "a", "kind": "automation_id", "value": {"text": "A"}, "score": 1.0, "locale_dependent": false}]},
    {"id": "save_as_dialog", "scope": "window", "candidates": [{"id": "d", "kind": "automation_id", "value": {"text": "D"}, "score": 1.0, "locale_dependent": false}]},
    {"id": "save_as_filename", "scope": "element", "candidates": [{"id": "f", "kind": "automation_id", "value": {"text": "F"}, "score": 1.0, "locale_dependent": false}]},
    {"id": "save_as_save_button", "scope": "element", "candidates": [{"id": "s", "kind": "automation_id", "value": {"text": "S"}, "score": 1.0, "locale_dependent": false}]},
    {"id": "broken_scope", "scope": "hover", "candidates": [{"id": "x", "kind": "automation_id", "value": {"text": "X"}, "score": 1.0, "locale_dependent": false}]}
  ]
}"#;
        std::fs::write(&path, body).expect("write temp targets");
        let result = NotepadTargetCatalog::load(&path, NOTEPAD_REQUIRED_TARGETS);
        let _ = std::fs::remove_file(&path);
        let error = result.expect_err("an unknown scope must be rejected");
        assert!(
            error.to_string().contains("unsupported scope"),
            "unexpected scope error: {error}"
        );
    }

    /// ADR-0084 D1: a second adapter declares its own required target set, and
    /// the loader must not impose the Notepad set on it.
    #[test]
    fn test_second_adapter_loads_with_its_own_required_targets() {
        const PAINT_REQUIRED_TARGETS: &[&str] = &[
            "main_window",
            "canvas",
            "rectangle_tool_button",
            "foreground_color_button",
        ];
        let path = workspace_root().join("adapters/com.microsoft.paint/selectors/targets.json");
        let catalog = NotepadTargetCatalog::load(&path, PAINT_REQUIRED_TARGETS)
            .expect("paint target catalog");
        assert_eq!(catalog.app_id(), "com.microsoft.paint");
        let canvas = catalog.descriptor("canvas").expect("canvas descriptor");
        assert!(
            !canvas.element_candidates().is_empty(),
            "canvas is element-scoped and keeps its declared candidates"
        );
        assert!(
            !canvas.window_candidates().is_empty(),
            "element scope needs the main-window anchor"
        );
        assert!(
            catalog.descriptor("editor").is_err(),
            "the Notepad required set must not be imposed on another adapter"
        );
    }
}
