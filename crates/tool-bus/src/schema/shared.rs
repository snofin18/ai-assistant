//! Small shared helpers for schema registration and instance validation.

/// Maximum recursion depth for schema and instance walks.
pub(super) const MAX_NESTING_DEPTH: usize = 64;

/// JSON pointer 的一段（RFC 6901 转义：`~` → `~0`，`/` → `~1`）。
pub(super) fn child_pointer(pointer: &str, name: &str) -> String {
    let escaped = name.replace('~', "~0").replace('/', "~1");
    format!("{pointer}/{escaped}")
}

/// JSON pointer 的数组下标段。
pub(super) fn child_pointer_index(pointer: &str, index: usize) -> String {
    format!("{pointer}/{index}")
}

/// 空 pointer 的可读替身。
pub(super) const fn at(pointer: &str) -> &str {
    if pointer.is_empty() {
        "<root>"
    } else {
        pointer
    }
}
