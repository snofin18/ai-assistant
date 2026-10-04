# UI tree recording fixtures

These fixtures are deterministic UI tree snapshots for offline replay. They do
not capture pixels, input events, or live platform handles.

## Version 1 shape (legacy dry-run)

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

## Version 2 shape (tree snapshot sequence)

`*.sequence.json` files are the `xtask replay --suite core` inputs. They record a
baseline snapshot plus an ordered list of settled UIA snapshots and the changes
expected between adjacent snapshots:

```json
{
  "schema_version": 2,
  "suite": "core",
  "baseline": {"window": {}, "nodes": [], "read_text": []},
  "steps": [
    {
      "step_id": "replace-text",
      "expected_changes": [
        {"kind": "text_changed", "node_handle_id": 3, "field": "read_text", "before": "before", "after": "after"}
      ],
      "after": {"window": {}, "nodes": [], "read_text": []}
    }
  ]
}
```

`expected_changes` covers `node_added`, `node_removed`, `property_changed`, and
`text_changed`. Replay computes the actual diff from the snapshots and fails
with exit code 1 if it differs from the recorded expectation.

Each file is capped at 1 MiB; a sequence is capped at 128 steps, 4096 nodes per
snapshot, and 4096 expected changes per step. Unknown suites, missing fixtures,
unsupported versions, malformed trees, and dangling text references fail closed.
