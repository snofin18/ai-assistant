import assert from "node:assert/strict";
import test from "node:test";

import {
  approvalControllerReducer,
  createInitialApprovalControllerState,
  getApprovalBlockReason,
  parseApprovalCardModel,
} from "./approvalModel.ts";

function createModel(overrides = {}) {
  return {
    requestId: "approval-1",
    action: "notepad.replace_text",
    targetLabel: "Quarterly report",
    summary: "Replace every occurrence",
    impact: "12 changes in one document",
    diff: {
      kind: "text",
      entries: [
        {
          kind: "removed",
          beforeLine: 1,
          afterLine: null,
          text: "quarterly report",
        },
      ],
    },
    instructionOrigin: "user_request",
    toolSelectionReason: "The adapter exposes an L1 text API",
    riskLevel: "medium",
    reversibility: "L1ContentSnapshot",
    undo: {
      isAvailable: true,
      methodKey: "approval.undo.content_snapshot",
      anchorId: "anchor-55",
      disabledReasonKey: null,
    },
    scopeOptions: ["once", "this_task"],
    evidence: [
      {
        id: "evidence-1",
        kind: "tree_snapshot",
        labelKey: "approval.evidence.before",
        href: "evidence://tree-1",
      },
    ],
    isExpired: false,
    ...overrides,
  };
}

test("test_parse_approval_card_valid_model_preserves_fields", () => {
  const result = parseApprovalCardModel(createModel());
  assert.equal(result.ok, true);
  assert.equal(result.model.action, "notepad.replace_text");
  assert.deepEqual(result.model.scopeOptions, ["once", "this_task"]);
});

test("test_approval_app_content_requires_explicit_override_before_approval", () => {
  const result = parseApprovalCardModel(createModel({ instructionOrigin: "app_content" }));
  assert.equal(result.ok, true);
  const initial = createInitialApprovalControllerState(result.model);
  assert.equal(
    getApprovalBlockReason(result.model, initial),
    "approval.error.app_content_override_required"
  );
  const selected = approvalControllerReducer(initial, { type: "select_scope", scope: "once" });
  assert.equal(
    getApprovalBlockReason(result.model, selected),
    "approval.error.app_content_override_required"
  );
  const overridden = approvalControllerReducer(selected, {
    type: "set_app_content_override",
    isEnabled: true,
  });
  assert.equal(getApprovalBlockReason(result.model, overridden), null);
  const approved = approvalControllerReducer(overridden, {
    type: "approve",
    requiredIrreversibleConfirmation: "CONFIRM",
  });
  assert.equal(approved.decision?.kind, "approved");
});

test("test_parse_approval_card_high_risk_with_broad_scope_is_rejected", () => {
  const result = parseApprovalCardModel(
    createModel({ riskLevel: "high", scopeOptions: ["once", "this_task"] })
  );
  assert.equal(result.ok, false);
  assert.match(result.errors.join("\n"), /only the once scope/);
});

test("test_parse_approval_card_irreversible_with_undo_is_rejected", () => {
  const result = parseApprovalCardModel(
    createModel({
      riskLevel: "critical",
      reversibility: "L3Irreversible",
      diff: { kind: "irreversible", warningKey: "approval.warning.point_of_no_return" },
      scopeOptions: ["once"],
    })
  );
  assert.equal(result.ok, false);
  assert.match(result.errors.join("\n"), /must not advertise an undo path/);
});

test("test_approval_irreversible_requires_exact_secondary_confirmation", () => {
  const result = parseApprovalCardModel(
    createModel({
      riskLevel: "critical",
      reversibility: "L3Irreversible",
      diff: { kind: "irreversible", warningKey: "approval.warning.point_of_no_return" },
      undo: {
        isAvailable: false,
        methodKey: "approval.undo.none",
        anchorId: null,
        disabledReasonKey: "approval.undo.irreversible",
      },
      scopeOptions: ["once"],
    })
  );
  assert.equal(result.ok, true);
  const initial = createInitialApprovalControllerState(result.model);
  assert.equal(
    getApprovalBlockReason(result.model, initial, "CONFIRM"),
    "approval.error.irreversible_confirmation_required"
  );
  const typed = approvalControllerReducer(initial, {
    type: "set_irreversible_confirmation",
    confirmation: "CONFIRM",
  });
  const approved = approvalControllerReducer(typed, {
    type: "approve",
    requiredIrreversibleConfirmation: "CONFIRM",
  });
  assert.equal(approved.decision?.kind, "approved");
});

test("test_parse_approval_card_malformed_diff_is_rejected", () => {
  const result = parseApprovalCardModel(createModel({ diff: { kind: "text", entries: [{}] } }));
  assert.equal(result.ok, false);
  assert.match(result.errors.join("\n"), /text diff entries are malformed/);
});
