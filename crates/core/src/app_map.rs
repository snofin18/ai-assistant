//! Versioned App Map loading and validation.
//!
//! App Map files are untrusted inputs. This module owns their JSON shape,
//! version gate, path safety, source references, and token estimates. It does
//! not perform filesystem access; callers inject [`AppMapFileReader`].

use std::fmt;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use assistant_protocol::serde_json::{self, Map, Value};

use crate::error::{CoreError, CoreResult};
use crate::identifiers::TokenCount;

/// Supported App Map schema version.
pub const APP_MAP_VERSION: u32 = 1;

const MAX_APP_MAP_CONTENT_BYTES: usize = 16_384;

/// File-reader failure returned by an injected App Map reader.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum AppMapReadError {
    /// The requested App Map file does not exist.
    NotFound,
    /// The file exists but could not be read.
    Unreadable(String),
}

impl fmt::Display for AppMapReadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => write!(formatter, "App Map file was not found"),
            Self::Unreadable(detail) => write!(formatter, "App Map file is unreadable: {detail}"),
        }
    }
}

impl std::error::Error for AppMapReadError {}

/// Injected file access for App Map loading.
pub trait AppMapFileReader: Send + Sync {
    /// Reads one already-validated relative App Map path.
    ///
    /// # Errors
    ///
    /// Returns [`AppMapReadError::NotFound`] or
    /// [`AppMapReadError::Unreadable`]. Implementations must not receive an
    /// absolute path or a path containing parent traversal.
    fn read_to_string(&self, relative_path: &Path) -> Result<String, AppMapReadError>;
}

/// One validated App Map entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppMapEntry {
    id: String,
    title: String,
    content: String,
    source_reference: String,
    entry_index: usize,
    token_estimate: TokenCount,
}

impl AppMapEntry {
    /// Stable entry id.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Human-readable entry title.
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Text eligible for injection.
    #[must_use]
    pub fn content(&self) -> &str {
        &self.content
    }

    /// Source file and position reference.
    #[must_use]
    pub fn source_reference(&self) -> &str {
        &self.source_reference
    }

    /// Zero-based position of this entry in the App Map array.
    #[must_use]
    pub const fn entry_index(&self) -> usize {
        self.entry_index
    }

    /// Caller-supplied token estimate.
    #[must_use]
    pub const fn token_estimate(&self) -> TokenCount {
        self.token_estimate
    }
}

/// A validated App Map document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppMap {
    version: u32,
    app_id: String,
    entries: Vec<AppMapEntry>,
}

impl AppMap {
    /// Schema version.
    #[must_use]
    pub const fn version(&self) -> u32 {
        self.version
    }

    /// Stable application identifier.
    #[must_use]
    pub fn app_id(&self) -> &str {
        &self.app_id
    }

    /// Validated entries in file order.
    #[must_use]
    pub fn entries(&self) -> &[AppMapEntry] {
        &self.entries
    }

    /// Finds one entry by id.
    #[must_use]
    pub fn entry(&self, entry_id: &str) -> Option<&AppMapEntry> {
        self.entries.iter().find(|entry| entry.id == entry_id)
    }
}

/// App Map loader backed by an injected file reader.
pub struct AppMapLoader {
    reader: Arc<dyn AppMapFileReader>,
}

impl fmt::Debug for AppMapLoader {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AppMapLoader")
            .finish_non_exhaustive()
    }
}

impl AppMapLoader {
    /// Creates a loader.
    #[must_use]
    pub fn new(reader: Arc<dyn AppMapFileReader>) -> Self {
        Self { reader }
    }

    /// Loads and validates a relative App Map path.
    ///
    /// Paths are logical `/`-separated relative paths. Absolute paths, `..`,
    /// `.` components, and backslashes are rejected before the reader is called.
    ///
    /// # Errors
    ///
    /// - [`CoreError::AppMapPathTraversal`] for unsafe path shapes;
    /// - [`CoreError::AppMapMissing`] / [`CoreError::AppMapUnreadable`] from the reader;
    /// - [`CoreError::AppMapCorrupt`] for malformed JSON or fields;
    /// - [`CoreError::AppMapVersionMismatch`] for unsupported versions.
    pub fn load(&self, relative_path: &str) -> CoreResult<AppMap> {
        let validated = validate_app_map_path(relative_path)?;
        let raw = self
            .reader
            .read_to_string(&validated)
            .map_err(|error| match error {
                AppMapReadError::NotFound => CoreError::AppMapMissing {
                    path: relative_path.to_owned(),
                },
                AppMapReadError::Unreadable(reason) => CoreError::AppMapUnreadable {
                    path: relative_path.to_owned(),
                    reason,
                },
            })?;
        parse_app_map(&raw, relative_path)
    }
}

fn validate_app_map_path(relative_path: &str) -> CoreResult<PathBuf> {
    if relative_path.is_empty() || relative_path.contains('\\') || relative_path.contains('\0') {
        return Err(CoreError::AppMapPathTraversal {
            path: relative_path.to_owned(),
        });
    }
    let path = Path::new(relative_path);
    if path.is_absolute() {
        return Err(CoreError::AppMapPathTraversal {
            path: relative_path.to_owned(),
        });
    }
    for component in path.components() {
        if !matches!(component, Component::Normal(_)) {
            return Err(CoreError::AppMapPathTraversal {
                path: relative_path.to_owned(),
            });
        }
    }
    Ok(path.to_path_buf())
}

fn parse_app_map(raw: &str, path: &str) -> CoreResult<AppMap> {
    let value: Value = serde_json::from_str(raw).map_err(|error| CoreError::AppMapCorrupt {
        path: path.to_owned(),
        reason: error.to_string(),
    })?;
    let object = value.as_object().ok_or_else(|| CoreError::AppMapCorrupt {
        path: path.to_owned(),
        reason: "root must be an object".to_owned(),
    })?;
    reject_unknown_fields(object, &["version", "app_id", "entries"], path)?;
    let version = required_u64(object, "version", path)?;
    if version != u64::from(APP_MAP_VERSION) {
        return Err(CoreError::AppMapVersionMismatch {
            expected: APP_MAP_VERSION,
            found: version,
        });
    }
    let app_id = required_string(object, "app_id", path)?;
    let entries_value = object
        .get("entries")
        .ok_or_else(|| CoreError::AppMapCorrupt {
            path: path.to_owned(),
            reason: "missing entries".to_owned(),
        })?;
    let entries_array = entries_value
        .as_array()
        .ok_or_else(|| CoreError::AppMapCorrupt {
            path: path.to_owned(),
            reason: "entries must be an array".to_owned(),
        })?;
    if entries_array.is_empty() {
        return Err(CoreError::AppMapCorrupt {
            path: path.to_owned(),
            reason: "entries must not be empty".to_owned(),
        });
    }
    let mut entries = Vec::with_capacity(entries_array.len());
    let mut seen_ids = std::collections::BTreeSet::new();
    for (index, value) in entries_array.iter().enumerate() {
        let entry = parse_entry(value, path, index)?;
        if !seen_ids.insert(entry.id.clone()) {
            return Err(CoreError::AppMapCorrupt {
                path: path.to_owned(),
                reason: format!("duplicate entry id {}", entry.id),
            });
        }
        entries.push(entry);
    }
    Ok(AppMap {
        version: APP_MAP_VERSION,
        app_id,
        entries,
    })
}

fn parse_entry(value: &Value, path: &str, index: usize) -> CoreResult<AppMapEntry> {
    let object = value.as_object().ok_or_else(|| CoreError::AppMapCorrupt {
        path: path.to_owned(),
        reason: format!("entries[{index}] must be an object"),
    })?;
    reject_unknown_fields(
        object,
        &[
            "id",
            "title",
            "content",
            "source_reference",
            "token_estimate",
        ],
        path,
    )?;
    let id = required_string(object, "id", path)?;
    let title = required_string(object, "title", path)?;
    let content = required_string(object, "content", path)?;
    if content.len() > MAX_APP_MAP_CONTENT_BYTES {
        return Err(CoreError::AppMapCorrupt {
            path: path.to_owned(),
            reason: format!("entries[{index}].content exceeds {MAX_APP_MAP_CONTENT_BYTES} bytes"),
        });
    }
    // A file-provided source_reference is only a hint. Provenance exposed to
    // callers is generated from the validated path and array position.
    let source_reference = format!("{path}#/entries/{index}");
    let token_estimate = required_u64(object, "token_estimate", path)?;
    if token_estimate == 0 {
        return Err(CoreError::AppMapCorrupt {
            path: path.to_owned(),
            reason: format!("entries[{index}].token_estimate must be positive"),
        });
    }
    let conservative_tokens =
        token_estimate.max(u64::try_from(content.chars().count()).unwrap_or(u64::MAX));
    Ok(AppMapEntry {
        id,
        title,
        content,
        source_reference,
        entry_index: index,
        token_estimate: TokenCount::new(conservative_tokens),
    })
}

fn reject_unknown_fields(
    object: &Map<String, Value>,
    allowed: &[&str],
    path: &str,
) -> CoreResult<()> {
    for field in object.keys() {
        if !allowed.contains(&field.as_str()) {
            return Err(CoreError::AppMapCorrupt {
                path: path.to_owned(),
                reason: format!("unknown field {field:?}"),
            });
        }
    }
    Ok(())
}

fn required_string(object: &Map<String, Value>, field: &str, path: &str) -> CoreResult<String> {
    let value =
        object
            .get(field)
            .and_then(Value::as_str)
            .ok_or_else(|| CoreError::AppMapCorrupt {
                path: path.to_owned(),
                reason: format!("missing or invalid string field {field:?}"),
            })?;
    if value.trim().is_empty() {
        return Err(CoreError::AppMapCorrupt {
            path: path.to_owned(),
            reason: format!("field {field:?} must not be empty"),
        });
    }
    Ok(value.to_owned())
}

fn required_u64(object: &Map<String, Value>, field: &str, path: &str) -> CoreResult<u64> {
    object
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| CoreError::AppMapCorrupt {
            path: path.to_owned(),
            reason: format!("missing or invalid integer field {field:?}"),
        })
}
