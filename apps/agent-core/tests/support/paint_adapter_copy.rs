//! Adapter-copy helper for Paint integration tests.
//!
//! The copied fixture contains only declarations needed by parser and assembly
//! tests; it deliberately excludes runtime code and any external resources.

use std::path::Path;

pub fn copy_paint_adapter(destination: &Path) -> Result<(), std::io::Error> {
    let source = super::workspace_root().join("adapters/com.microsoft.paint");
    std::fs::create_dir_all(destination.join("selectors"))?;
    std::fs::create_dir_all(destination.join("tools"))?;
    std::fs::copy(
        source.join("selectors").join("targets.json"),
        destination.join("selectors").join("targets.json"),
    )?;
    std::fs::copy(
        source.join("tools").join("tools.json"),
        destination.join("tools").join("tools.json"),
    )?;
    Ok(())
}
