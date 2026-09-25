//! Pure lexical path validation and resolution containment checks.

use crate::{PolicyError, PolicyResult};

const MAX_PATH_BYTES: usize = 4096;

/// Inputs required to validate a path without performing IO.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PathValidationRequest<'a> {
    /// Path supplied by model or caller.
    pub raw_path: &'a str,
    /// Allowed root directory.
    pub root: &'a str,
    /// Platform-resolved path used to detect symlink escape.
    pub resolved_path: &'a str,
}

/// A path that passed lexical and containment checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedPath {
    normalized: String,
}

impl ValidatedPath {
    /// Returns the normalized absolute path with `/` separators.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.normalized
    }
}

/// Validates a path and requires both its lexical form and resolved form to
/// remain inside `root`.
///
/// The function is pure. The caller is responsible for supplying the
/// platform-resolved path; this validator never follows links itself.
///
/// # Errors
///
/// Returns [`PolicyError::PathRejected`] for malformed paths, traversal,
/// reserved device names, or containment escapes.
pub fn validate_path(request: &PathValidationRequest<'_>) -> PolicyResult<ValidatedPath> {
    let root = parse_path(request.root, "root", false)?;
    if !root.is_absolute {
        return Err(reject("root must be an absolute path"));
    }
    let raw = parse_path(request.raw_path, "raw path", true)?;
    let resolved = parse_path(request.resolved_path, "resolved path", false)?;
    if !resolved.is_absolute {
        return Err(reject("resolved path must be absolute"));
    }
    if !same_prefix(&root, &resolved) || !starts_with(&resolved.components, &root.components) {
        return Err(reject("resolved path escapes the allowed root"));
    }

    let components = if raw.is_absolute {
        if !same_prefix(&root, &raw) || !starts_with(&raw.components, &root.components) {
            return Err(reject("absolute raw path is outside the allowed root"));
        }
        raw.components
    } else {
        let mut combined = root.components.clone();
        combined.extend(raw.components);
        combined
    };
    if components.is_empty() {
        return Err(reject("path must identify a file below the root"));
    }
    Ok(ValidatedPath {
        normalized: render_path(root.prefix.as_deref(), &components),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedPath {
    prefix: Option<String>,
    components: Vec<String>,
    is_absolute: bool,
}

fn parse_path(value: &str, label: &str, allow_relative: bool) -> PolicyResult<ParsedPath> {
    if value.is_empty() {
        return Err(reject(&format!("{label} is empty")));
    }
    if value.len() > MAX_PATH_BYTES {
        return Err(reject(&format!("{label} exceeds {MAX_PATH_BYTES} bytes")));
    }
    if value.contains('\0') || value.chars().any(char::is_control) {
        return Err(reject(&format!("{label} contains a control character")));
    }
    if value.trim() != value {
        return Err(reject(&format!(
            "{label} has leading or trailing whitespace"
        )));
    }
    if value.contains('%') {
        return Err(reject(&format!(
            "{label} contains percent-encoding or an encoded separator"
        )));
    }

    let normalized = value.replace('\\', "/");
    if normalized.starts_with("//") {
        return Err(reject(&format!("{label} is a UNC or device path")));
    }
    let (prefix, remainder, is_absolute) = split_prefix(&normalized, label, allow_relative)?;
    let mut components = Vec::new();
    for component in remainder.split('/') {
        if component.is_empty() || component == "." {
            continue;
        }
        if component == ".." {
            return Err(reject(&format!(
                "{label} contains a parent-directory segment"
            )));
        }
        validate_component(component, label)?;
        components.push(component.to_owned());
    }
    Ok(ParsedPath {
        prefix,
        components,
        is_absolute,
    })
}

fn split_prefix<'a>(
    value: &'a str,
    label: &str,
    allow_relative: bool,
) -> PolicyResult<(Option<String>, &'a str, bool)> {
    let first = value.as_bytes().first().copied();
    let second = value.as_bytes().get(1).copied();
    if first.is_some_and(|character| character.is_ascii_alphabetic()) && second == Some(b':') {
        let remainder = value
            .get(2..)
            .ok_or_else(|| reject(&format!("{label} has malformed drive prefix")))?;
        if !remainder.starts_with('/') {
            return Err(reject(&format!(
                "{label} is drive-relative; only absolute drive paths are accepted"
            )));
        }
        let prefix = format!(
            "{}:",
            value
                .get(..1)
                .ok_or_else(|| reject(&format!("{label} has malformed drive prefix")))?
                .to_ascii_uppercase()
        );
        Ok((Some(prefix), remainder, true))
    } else if value.starts_with('/') {
        Ok((None, value, true))
    } else if allow_relative {
        Ok((None, value, false))
    } else {
        Err(reject(&format!("{label} must be absolute")))
    }
}

fn validate_component(component: &str, label: &str) -> PolicyResult<()> {
    if component.contains(':') {
        return Err(reject(&format!(
            "{label} contains ':' inside a path segment"
        )));
    }
    if component.ends_with('.') || component.ends_with(' ') {
        return Err(reject(&format!(
            "{label} contains a segment ending in dot or space"
        )));
    }
    let base = component.split('.').next().unwrap_or(component);
    if is_reserved_device_name(base) {
        return Err(reject(&format!("{label} contains reserved device name")));
    }
    Ok(())
}

fn is_reserved_device_name(value: &str) -> bool {
    let upper = value.to_ascii_uppercase();
    matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (upper.len() == 4
            && (upper.starts_with("COM") || upper.starts_with("LPT"))
            && upper
                .as_bytes()
                .get(3)
                .is_some_and(|digit| (b'1'..=b'9').contains(digit)))
}

fn starts_with(candidate: &[String], root: &[String]) -> bool {
    candidate.len() >= root.len()
        && candidate
            .iter()
            .zip(root)
            .all(|(left, right)| left.eq_ignore_ascii_case(right))
}

fn same_prefix(left: &ParsedPath, right: &ParsedPath) -> bool {
    match (&left.prefix, &right.prefix) {
        (Some(left), Some(right)) => left.eq_ignore_ascii_case(right),
        (None, None) => true,
        _ => false,
    }
}

fn render_path(prefix: Option<&str>, components: &[String]) -> String {
    let body = components.join("/");
    prefix.map_or_else(|| format!("/{body}"), |prefix| format!("{prefix}/{body}"))
}

fn reject(reason: &str) -> PolicyError {
    PolicyError::PathRejected {
        reason: reason.to_owned(),
    }
}
