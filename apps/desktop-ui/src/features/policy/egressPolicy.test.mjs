import assert from "node:assert/strict";
import test from "node:test";

import {
  createInitialEgressPolicyState,
  egressPolicyControllerReducer,
  getEffectiveEgressLevel,
  parseEgressPolicy,
} from "./egressPolicy.ts";

function createPolicy(overrides = {}) {
  return {
    defaultLevel: "redacted",
    appOverrides: { "com.microsoft.notepad": "full" },
    hasLocalModel: true,
    ...overrides,
  };
}

test("test_parse_egress_policy_valid_input_preserves_overrides", () => {
  const result = parseEgressPolicy(createPolicy());
  assert.equal(result.ok, true);
  assert.equal(result.policy.appOverrides["com.microsoft.notepad"], "full");
});

test("test_parse_egress_policy_local_only_without_local_model_is_rejected", () => {
  const result = parseEgressPolicy(
    createPolicy({ defaultLevel: "local_only", hasLocalModel: false }),
  );
  assert.equal(result.ok, false);
  assert.match(result.errors.join("\n"), /local_only requires/);
});

test("test_egress_policy_upgrade_requires_explicit_confirmation", () => {
  const state = createInitialEgressPolicyState(createPolicy());
  const next = egressPolicyControllerReducer(state, {
    type: "request_change",
    request: { kind: "default", level: "full" },
  });
  assert.equal(next.errorKey, "policy.error.upgrade_confirmation_required");
  assert.deepEqual(next.pendingChange, { kind: "default", level: "full" });
  assert.equal(next.change, null);
});

test("test_egress_policy_downgrade_applies_immediately_and_emits_change", () => {
  const state = createInitialEgressPolicyState(createPolicy());
  const next = egressPolicyControllerReducer(state, {
    type: "request_change",
    request: { kind: "default", level: "local_only" },
  });
  assert.equal(next.errorKey, null);
  assert.equal(next.change?.changedLevel, "local_only");
  assert.equal(next.parseResult.ok && next.parseResult.policy.defaultLevel, "local_only");
});

test("test_egress_policy_setting_local_only_without_local_model_is_blocked", () => {
  const state = createInitialEgressPolicyState(
    createPolicy({ hasLocalModel: false, appOverrides: {} }),
  );
  const next = egressPolicyControllerReducer(state, {
    type: "request_change",
    request: {
      kind: "app_override",
      appId: "com.microsoft.notepad",
      level: "local_only",
    },
  });
  assert.equal(next.errorKey, "policy.error.local_model_unavailable");
  assert.equal(next.change, null);
});

test("test_egress_policy_clear_app_override_uses_default_effective_level", () => {
  const state = createInitialEgressPolicyState(createPolicy());
  const next = egressPolicyControllerReducer(state, {
    type: "request_change",
    request: { kind: "clear_app_override", appId: "com.microsoft.notepad" },
  });
  assert.equal(next.change?.appId, "com.microsoft.notepad");
  assert.equal(next.change?.changedLevel, "redacted");
  assert.equal(
    next.parseResult.ok && "com.microsoft.notepad" in next.parseResult.policy.appOverrides,
    false,
  );
});

test("test_egress_policy_invalid_app_id_returns_explicit_error", () => {
  const state = createInitialEgressPolicyState(createPolicy());
  const next = egressPolicyControllerReducer(state, {
    type: "request_change",
    request: { kind: "app_override", appId: "Bad App", level: "full" },
  });
  assert.equal(next.errorKey, "policy.error.invalid_app_id");
});

test("test_egress_policy_upgrade_confirmation_is_consumed_by_one_change", () => {
  const initial = createInitialEgressPolicyState(createPolicy());
  const pending = egressPolicyControllerReducer(initial, {
    type: "request_change",
    request: { kind: "default", level: "full" },
  });
  const confirmed = egressPolicyControllerReducer(pending, { type: "confirm_pending_change" });
  assert.equal(confirmed.pendingChange, null);
  assert.equal(confirmed.parseResult.ok && confirmed.parseResult.policy.defaultLevel, "full");
  const downgraded = egressPolicyControllerReducer(confirmed, {
    type: "request_change",
    request: { kind: "default", level: "redacted" },
  });
  const secondUpgrade = egressPolicyControllerReducer(downgraded, {
    type: "request_change",
    request: {
      kind: "app_override",
      appId: "browser.edge",
      level: "full",
    },
  });
  assert.equal(secondUpgrade.change, null);
  assert.equal(secondUpgrade.errorKey, "policy.error.upgrade_confirmation_required");
});

test("test_effective_egress_level_uses_override_before_default", () => {
  const parsed = parseEgressPolicy(createPolicy());
  assert.equal(parsed.ok, true);
  assert.equal(getEffectiveEgressLevel(parsed.policy, "com.microsoft.notepad"), "full");
  assert.equal(getEffectiveEgressLevel(parsed.policy, "browser.edge"), "redacted");
});
