//! Recording data model and validation.

use std::collections::{HashMap, HashSet};

use assistant_protocol::serde_json::{Map, Value};

use crate::error::ReplayError;

/// Supported recording schema version.
pub const RECORDING_VERSION: u32 = 1;

/// A recorded UI tree snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Recording {
    schema_version: u32,
    id: String,
    captured_at: String,
    application: String,
    window: RecordedWindow,
    nodes: Vec<RecordedNode>,
    read_text: Vec<RecordedReadText>,
}

impl Recording {
    /// Parse a recording from JSON.
    ///
    /// # Errors
    /// Returns [`ReplayError`] if JSON shape or recording invariants are invalid.
    pub fn from_json(raw: &str) -> Result<Self, ReplayError> {
        let value: Value = assistant_protocol::serde_json::from_str(raw)
            .map_err(|error| ReplayError::InvalidJson(error.to_string()))?;
        let object = value.as_object().ok_or_else(|| ReplayError::InvalidField {
            field: "$".to_string(),
            message: "top-level value must be an object".to_string(),
        })?;

        let version_value = required_u64(object, "schema_version")?;
        let schema_version =
            u32::try_from(version_value).map_err(|_| ReplayError::InvalidField {
                field: "schema_version".to_string(),
                message: "does not fit in u32".to_string(),
            })?;
        if schema_version != RECORDING_VERSION {
            return Err(ReplayError::UnsupportedVersion(schema_version));
        }

        let recording = Self {
            schema_version,
            id: required_string(object, "recording_id")?,
            captured_at: required_string(object, "captured_at")?,
            application: required_string(object, "application")?,
            window: parse_window(required_object(object, "window")?)?,
            nodes: parse_nodes(required_array(object, "nodes")?)?,
            read_text: parse_read_text(required_array(object, "read_text")?)?,
        };
        recording.validate()?;
        Ok(recording)
    }

    /// Validate tree handles, parent links, cycles, and recorded references.
    ///
    /// # Errors
    /// Returns [`ReplayError`] on the first structural violation.
    pub fn validate(&self) -> Result<(), ReplayError> {
        if self.schema_version != RECORDING_VERSION {
            return Err(ReplayError::UnsupportedVersion(self.schema_version));
        }
        if self.id.trim().is_empty() {
            return Err(ReplayError::InvalidField {
                field: "recording_id".to_string(),
                message: "must not be blank".to_string(),
            });
        }
        if self.application.trim().is_empty() {
            return Err(ReplayError::InvalidField {
                field: "application".to_string(),
                message: "must not be blank".to_string(),
            });
        }
        if self.captured_at.trim().is_empty() {
            return Err(ReplayError::InvalidField {
                field: "captured_at".to_string(),
                message: "must not be blank".to_string(),
            });
        }
        self.window.validate()?;
        if self.nodes.is_empty() {
            return Err(ReplayError::EmptyTree);
        }

        let mut by_handle: HashMap<u64, &RecordedNode> = HashMap::new();
        for node in &self.nodes {
            if node.role().trim().is_empty() {
                return Err(ReplayError::InvalidField {
                    field: format!("nodes[{}].role", node.local_handle_id()),
                    message: "must not be blank".to_string(),
                });
            }
            if node.local_handle_id() == self.window.local_handle_id() {
                return Err(ReplayError::InvalidField {
                    field: format!("nodes[{}].local_handle_id", node.local_handle_id()),
                    message: "must differ from window.local_handle_id".to_string(),
                });
            }
            if by_handle.insert(node.local_handle_id(), node).is_some() {
                return Err(ReplayError::DuplicateHandle(node.local_handle_id()));
            }
        }
        for node in &self.nodes {
            if let Some(parent) = node.parent_handle_id()
                && !by_handle.contains_key(&parent)
            {
                return Err(ReplayError::OrphanParent {
                    child: node.local_handle_id(),
                    parent,
                });
            }
        }
        for node in &self.nodes {
            let mut seen = HashSet::new();
            let mut current = Some(node.local_handle_id());
            while let Some(handle) = current {
                if !seen.insert(handle) {
                    return Err(ReplayError::ParentCycle(handle));
                }
                current = by_handle
                    .get(&handle)
                    .and_then(|node| node.parent_handle_id());
            }
        }
        let mut text_handles = HashSet::new();
        for text in &self.read_text {
            if !by_handle.contains_key(&text.element_handle_id()) {
                return Err(ReplayError::DanglingTextReference(text.element_handle_id()));
            }
            if !text_handles.insert(text.element_handle_id()) {
                return Err(ReplayError::DuplicateTextOutcome(text.element_handle_id()));
            }
        }
        Ok(())
    }

    /// Recording schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Recording id.
    #[must_use]
    pub fn recording_id(&self) -> &str {
        &self.id
    }

    /// Capture timestamp.
    #[must_use]
    pub fn captured_at(&self) -> &str {
        &self.captured_at
    }

    /// Application id.
    #[must_use]
    pub fn application(&self) -> &str {
        &self.application
    }

    /// Recorded window metadata.
    #[must_use]
    pub const fn window(&self) -> &RecordedWindow {
        &self.window
    }

    /// Recorded nodes.
    #[must_use]
    pub fn nodes(&self) -> &[RecordedNode] {
        &self.nodes
    }

    /// Recorded read-text outcomes.
    #[must_use]
    pub fn read_text(&self) -> &[RecordedReadText] {
        &self.read_text
    }
}

/// Recorded window metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct RecordedWindow {
    local_handle_id: u64,
    display_label: String,
    title: String,
    fingerprint: String,
    minimized: bool,
    foreground: bool,
    occluded: bool,
}

impl RecordedWindow {
    /// Local handle id.
    #[must_use]
    pub const fn local_handle_id(&self) -> u64 {
        self.local_handle_id
    }

    /// Display label.
    #[must_use]
    pub fn display_label(&self) -> &str {
        &self.display_label
    }

    /// Window title.
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Recorded state fingerprint.
    #[must_use]
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }

    /// Whether the window was minimized.
    #[must_use]
    pub const fn is_minimized(&self) -> bool {
        self.minimized
    }

    /// Whether the window was foreground.
    #[must_use]
    pub const fn is_foreground(&self) -> bool {
        self.foreground
    }

    /// Whether the window was occluded.
    #[must_use]
    pub const fn is_occluded(&self) -> bool {
        self.occluded
    }

    fn validate(&self) -> Result<(), ReplayError> {
        if self.local_handle_id == 0 {
            return Err(ReplayError::InvalidField {
                field: "window.local_handle_id".to_string(),
                message: "must be > 0".to_string(),
            });
        }
        if self.display_label.trim().is_empty() {
            return Err(ReplayError::InvalidField {
                field: "window.display_label".to_string(),
                message: "must not be blank".to_string(),
            });
        }
        assistant_platform_api::Fingerprint::parse(self.fingerprint.clone()).map_err(|error| {
            ReplayError::InvalidField {
                field: "window.fingerprint".to_string(),
                message: error.to_string(),
            }
        })?;
        Ok(())
    }
}

/// Recorded node metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct RecordedNode {
    local_handle_id: u64,
    parent_handle_id: Option<u64>,
    role: String,
    automation_id: Option<String>,
    class_name: Option<String>,
    name: String,
    bounds: [i64; 4],
    is_offscreen: bool,
    is_enabled: bool,
    patterns: Vec<String>,
}

impl RecordedNode {
    /// Local handle id.
    #[must_use]
    pub const fn local_handle_id(&self) -> u64 {
        self.local_handle_id
    }

    /// Parent handle id.
    #[must_use]
    pub const fn parent_handle_id(&self) -> Option<u64> {
        self.parent_handle_id
    }

    /// Accessibility role.
    #[must_use]
    pub fn role(&self) -> &str {
        &self.role
    }

    /// Automation id.
    #[must_use]
    pub fn automation_id(&self) -> Option<&str> {
        self.automation_id.as_deref()
    }

    /// Class name.
    #[must_use]
    pub fn class_name(&self) -> Option<&str> {
        self.class_name.as_deref()
    }

    /// Display name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Bounds.
    #[must_use]
    pub const fn bounds(&self) -> [i64; 4] {
        self.bounds
    }

    /// Whether the node was offscreen.
    #[must_use]
    pub const fn is_offscreen(&self) -> bool {
        self.is_offscreen
    }

    /// Whether the node was enabled.
    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        self.is_enabled
    }

    /// Pattern names.
    #[must_use]
    pub fn patterns(&self) -> &[String] {
        &self.patterns
    }
}

/// Recorded text outcome for an element.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct RecordedReadText {
    element_handle_id: u64,
    text: String,
}

impl RecordedReadText {
    /// Construct a recorded text outcome.
    #[must_use]
    pub const fn new(element_handle_id: u64, text: String) -> Self {
        Self {
            element_handle_id,
            text,
        }
    }

    /// Element handle id.
    #[must_use]
    pub const fn element_handle_id(&self) -> u64 {
        self.element_handle_id
    }

    /// Recorded text.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }
}

fn parse_window(object: &Map<String, Value>) -> Result<RecordedWindow, ReplayError> {
    Ok(RecordedWindow {
        local_handle_id: required_u64(object, "local_handle_id")?,
        display_label: required_string(object, "display_label")?,
        title: required_string(object, "title")?,
        fingerprint: required_string(object, "fingerprint")?,
        minimized: required_bool(object, "minimized")?,
        foreground: required_bool(object, "foreground")?,
        occluded: required_bool(object, "occluded")?,
    })
}

fn parse_nodes(values: &[Value]) -> Result<Vec<RecordedNode>, ReplayError> {
    values.iter().map(parse_node).collect()
}

fn parse_node(value: &Value) -> Result<RecordedNode, ReplayError> {
    let object = value.as_object().ok_or_else(|| ReplayError::InvalidField {
        field: "nodes[]".to_string(),
        message: "node must be an object".to_string(),
    })?;
    let parent = optional_u64(object, "parent_handle_id")?;
    let bounds = parse_bounds(required_array(object, "bounds")?)?;
    let patterns = required_array(object, "patterns")?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(ToString::to_string)
                .ok_or_else(|| ReplayError::InvalidField {
                    field: "nodes[].patterns[]".to_string(),
                    message: "pattern must be a string".to_string(),
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(RecordedNode {
        local_handle_id: required_u64(object, "local_handle_id")?,
        parent_handle_id: parent,
        role: required_string(object, "role")?,
        automation_id: optional_string(object, "automation_id")?,
        class_name: optional_string(object, "class_name")?,
        name: required_string(object, "name")?,
        bounds,
        is_offscreen: required_bool(object, "is_offscreen")?,
        is_enabled: required_bool(object, "is_enabled")?,
        patterns,
    })
}

fn parse_bounds(values: &[Value]) -> Result<[i64; 4], ReplayError> {
    if values.len() != 4 {
        return Err(ReplayError::InvalidField {
            field: "nodes[].bounds".to_string(),
            message: "must contain 4 integers".to_string(),
        });
    }
    let mut bounds = [0_i64; 4];
    for (index, value) in values.iter().enumerate() {
        let number = value.as_i64().ok_or_else(|| ReplayError::InvalidField {
            field: format!("nodes[].bounds[{index}]"),
            message: "must be an integer".to_string(),
        })?;
        if let Some(slot) = bounds.get_mut(index) {
            *slot = number;
        }
    }
    Ok(bounds)
}

fn parse_read_text(values: &[Value]) -> Result<Vec<RecordedReadText>, ReplayError> {
    values
        .iter()
        .map(|value| {
            let object = value.as_object().ok_or_else(|| ReplayError::InvalidField {
                field: "read_text[]".to_string(),
                message: "entry must be an object".to_string(),
            })?;
            Ok(RecordedReadText::new(
                required_u64(object, "element_handle_id")?,
                required_string(object, "text")?,
            ))
        })
        .collect()
}

fn required_object<'a>(
    object: &'a Map<String, Value>,
    key: &str,
) -> Result<&'a Map<String, Value>, ReplayError> {
    object
        .get(key)
        .and_then(Value::as_object)
        .ok_or_else(|| ReplayError::InvalidField {
            field: key.to_string(),
            message: "must be an object".to_string(),
        })
}

fn required_array<'a>(
    object: &'a Map<String, Value>,
    key: &str,
) -> Result<&'a [Value], ReplayError> {
    object
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| ReplayError::InvalidField {
            field: key.to_string(),
            message: "must be an array".to_string(),
        })
}

fn required_string(object: &Map<String, Value>, key: &str) -> Result<String, ReplayError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| ReplayError::InvalidField {
            field: key.to_string(),
            message: "must be a string".to_string(),
        })
}

fn optional_string(object: &Map<String, Value>, key: &str) -> Result<Option<String>, ReplayError> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(ReplayError::InvalidField {
            field: key.to_string(),
            message: "must be a string or null".to_string(),
        }),
    }
}

fn required_u64(object: &Map<String, Value>, key: &str) -> Result<u64, ReplayError> {
    object
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| ReplayError::InvalidField {
            field: key.to_string(),
            message: "must be an unsigned integer".to_string(),
        })
}

fn optional_u64(object: &Map<String, Value>, key: &str) -> Result<Option<u64>, ReplayError> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_u64()
            .map(Some)
            .ok_or_else(|| ReplayError::InvalidField {
                field: key.to_string(),
                message: "must be an unsigned integer or null".to_string(),
            }),
    }
}

fn required_bool(object: &Map<String, Value>, key: &str) -> Result<bool, ReplayError> {
    object
        .get(key)
        .and_then(Value::as_bool)
        .ok_or_else(|| ReplayError::InvalidField {
            field: key.to_string(),
            message: "must be a boolean".to_string(),
        })
}
