import assert from "node:assert/strict";
import test from "node:test";

import {
  getCapabilityStats,
  parseCapabilityMatrix
} from "./capabilityMatrix.ts";

function createMatrix(overrides = {}) {
  return {
    probe: {
      probed_at: "2026-09-28T00:00:00Z",
      platform_os: "windows",
      session_locked: false,
      session_remote: false,
      session_headless: false
    },
    channels: {
      uia_tree: { availability: "available" },
      portal_input: { availability: "needs_consent", detail: "user approval required" }
    },
    capabilities: [
      {
        id: "tree.walk",
        resource: "read",
        side_effect: "none",
        risk: "l1",
        approval: "auto"
      },
      {
        id: "file.delete",
        resource: "destroy",
        side_effect: "disk",
        risk: "l3",
        approval: "required"
      }
    ],
    degradations: [{ id: "no_takeover_detection", impact: "confirmation frequency raised" }],
    ...overrides
  };
}

test("test_parse_capability_matrix_valid_snapshot_preserves_entries", () => {
  const result = parseCapabilityMatrix(createMatrix());
  assert.equal(result.ok, true);
  assert.equal(result.matrix.capabilities[1]?.risk, "l3");
  assert.equal(result.matrix.channels[1]?.availability, "needs_consent");
});

test("test_parse_capability_matrix_duplicate_capability_is_rejected", () => {
  const matrix = createMatrix({
    capabilities: [
      {
        id: "tree.walk",
        resource: "read",
        side_effect: "none",
        risk: "l1",
        approval: "auto"
      },
      {
        id: "tree.walk",
        resource: "read",
        side_effect: "none",
        risk: "l1",
        approval: "auto"
      }
    ]
  });
  const result = parseCapabilityMatrix(matrix);
  assert.equal(result.ok, false);
  assert.match(result.errors.join("\n"), /duplicate capability id/);
});

test("test_parse_capability_matrix_approval_mismatch_is_rejected", () => {
  const matrix = createMatrix({
    capabilities: [
      {
        id: "network.egress",
        resource: "send",
        side_effect: "network_egress",
        risk: "l4",
        approval: "auto"
      }
    ]
  });
  const result = parseCapabilityMatrix(matrix);
  assert.equal(result.ok, false);
  assert.match(result.errors.join("\n"), /approval is inconsistent/);
});

test("test_parse_capability_matrix_invalid_channel_is_rejected", () => {
  const matrix = createMatrix({
    channels: {
      uia_tree: { availability: "available" },
      portal_input: { availability: "maybe" }
    }
  });
  const result = parseCapabilityMatrix(matrix);
  assert.equal(result.ok, false);
  assert.match(result.errors.join("\n"), /unsupported availability/);
});

test("test_parse_capability_matrix_invalid_probe_timestamp_is_rejected", () => {
  const matrix = createMatrix({
    probe: {
      probed_at: "2026-02-30T00:00:00Z",
      platform_os: "windows",
      session_locked: false,
      session_remote: false,
      session_headless: false
    }
  });
  const result = parseCapabilityMatrix(matrix);
  assert.equal(result.ok, false);
  assert.match(result.errors.join("\n"), /UTC RFC 3339/);
});

test("test_parse_capability_matrix_unknown_platform_is_rejected", () => {
  const matrix = createMatrix({
    probe: {
      probed_at: "2026-09-28T00:00:00Z",
      platform_os: "plan9",
      session_locked: false,
      session_remote: false,
      session_headless: false
    }
  });
  const result = parseCapabilityMatrix(matrix);
  assert.equal(result.ok, false);
  assert.match(result.errors.join("\n"), /platform_os/);
});

test("test_parse_capability_matrix_leap_second_timestamp_is_rejected", () => {
  const matrix = createMatrix({
    probe: {
      probed_at: "2026-09-28T00:00:60Z",
      platform_os: "windows",
      session_locked: false,
      session_remote: false,
      session_headless: false
    }
  });
  const result = parseCapabilityMatrix(matrix);
  assert.equal(result.ok, false);
  assert.match(result.errors.join("\n"), /UTC RFC 3339/);
});

test("test_capability_stats_counts_approval_and_degraded_channels", () => {
  const result = parseCapabilityMatrix(createMatrix());
  assert.equal(result.ok, true);
  assert.deepEqual(getCapabilityStats(result.matrix), {
    total: 2,
    requiresApproval: 1,
    forbidden: 0,
    degradedChannels: 1
  });
});
