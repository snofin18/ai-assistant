# UI tree recording fixtures

These fixtures are deterministic UI tree snapshots for offline replay. They do
not capture pixels, input events, or live platform handles.

## Version 1 shape

- `schema_version` must be `1`.
- `window.local_handle_id` must be non-zero and distinct from every node handle.
- `nodes[].local_handle_id` values must be unique.
- Every non-null `nodes[].parent_handle_id` must name another node.
- Parent links must form a DAG with no cycles.
- Every `read_text[].element_handle_id` must name an existing node.

Recordings are captured after the target UI has settled. Replay compares stable
AutomationId paths and recorded read outcomes; it does not compare the full tree
node-for-node because transient teaching tips and download states may add nodes
(ADR-0022 D7).
