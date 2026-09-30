//! File observations used by the save-as handler.

use std::path::Path;
use std::time::UNIX_EPOCH;

use assistant_tool_bus::ToolBusError;

/// Captures the before/after file fact needed by `file_changed`.
///
/// # Errors
///
/// Returns an explicit `ToolBus` failure when metadata cannot be read or its
/// modification time cannot be represented as Unix milliseconds.
pub(crate) fn snapshot_file(
    path: &Path,
    tool: &str,
) -> Result<assistant_verify::FileSnapshot, ToolBusError> {
    match std::fs::metadata(path) {
        Ok(metadata) => {
            let modified = metadata
                .modified()
                .map_err(|error| ToolBusError::Mcp {
                    code: -32_000,
                    message: format!("{tool}: cannot read modification time: {error}"),
                })?
                .duration_since(UNIX_EPOCH)
                .map_err(|error| ToolBusError::Mcp {
                    code: -32_000,
                    message: format!("{tool}: modification time predates Unix epoch: {error}"),
                })?;
            let mtime_ms = i64::try_from(modified.as_millis()).map_err(|_| ToolBusError::Mcp {
                code: -32_000,
                message: format!("{tool}: modification time does not fit i64 milliseconds"),
            })?;
            Ok(assistant_verify::FileSnapshot::present(
                metadata.len(),
                mtime_ms,
                None,
            ))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(assistant_verify::FileSnapshot::absent())
        }
        Err(error) => Err(ToolBusError::Mcp {
            code: -32_000,
            message: format!("{tool}: cannot inspect `{}`: {error}", path.display()),
        }),
    }
}
