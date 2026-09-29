/**
 * DOM-level tests for the first real UI → Core call site.
 *
 * The Tauri `invoke` boundary is mocked so the test exercises the real
 * component, the real `sendUiCommand` path, and the real zod validation —
 * including the fail-closed case where Core answers with something that is not
 * a contract outcome.
 */

import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

import { IntentLauncher } from "./IntentLauncher.js";

function renderLauncher() {
  return render(<IntentLauncher now={() => 1_700_000_000_000} />);
}

describe("IntentLauncher", () => {
  test("a valid goal is sent and Core's outcome is shown", async () => {
    invokeMock.mockResolvedValueOnce({
      status: "intent_accepted",
      intent_id: "i_ui_1",
    });
    renderLauncher();

    fireEvent.change(screen.getByLabelText("What should the assistant do?"), {
      target: { value: "Replace the quarterly report and save" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Submit" }));

    await waitFor(() => {
      expect(screen.getByText("Intent accepted by Core")).toBeDefined();
    });
    const [, args] = invokeMock.mock.calls[0] as [string, { envelope: unknown }];
    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(args.envelope).toEqual({
      version: "1.0",
      command: {
        kind: "submit_intent",
        intent_id: "i_ui_1700000000000_1",
        goal: "Replace the quarterly report and save",
      },
    });
  });

  test("an outcome outside the contract is rejected, not rendered as success", async () => {
    invokeMock.mockResolvedValueOnce({ status: "silently_succeeded" });
    renderLauncher();

    fireEvent.change(screen.getByLabelText("What should the assistant do?"), {
      target: { value: "Replace the quarterly report and save" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Submit" }));

    await waitFor(() => {
      expect(screen.getByText(/ui_command_outcome_invalid/)).toBeDefined();
    });
    expect(screen.queryByText("Intent accepted by Core")).toBeNull();
  });

  test("an empty goal cannot be submitted", () => {
    renderLauncher();
    const submit = screen.getByRole("button", { name: "Submit" });
    expect(submit instanceof HTMLButtonElement && submit.disabled).toBe(true);
    expect(invokeMock).not.toHaveBeenCalled();
  });
});
