import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";

import fixture from "../../agent-core/tests/fixtures/ui_ipc/step_approval_requested.json";
import { App } from "./App.js";
import { HostIpcError } from "./ipc/client.js";
import type { UiEvent } from "./ipc/contract.js";

describe("App approval event bridge", () => {
  test("a browser approval event becomes a once-only card and approval command", async () => {
    let onEvent: ((event: UiEvent) => void) | null = null;
    const cleanup = vi.fn();
    const connectEvents = vi.fn(async (handlers: { onEvent: (event: UiEvent) => void }) => {
      onEvent = handlers.onEvent;
      return cleanup;
    });
    const sendCommand = vi.fn(async () => ({
      status: "approval_granted" as const,
      request_id: "runtime:browser.t5_2.profile:submit_form",
      scope: "once" as const,
    }));
    const view = render(
      <App connectEvents={connectEvents} now={() => 1_000} sendCommand={sendCommand} />,
    );

    await waitFor(() => expect(onEvent).not.toBeNull());
    act(() => onEvent?.(fixture as UiEvent));
    expect(await screen.findByText("Human approval required")).toBeDefined();

    fireEvent.change(screen.getByLabelText("Type once to confirm"), {
      target: { value: "once" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Approve once" }));

    await waitFor(() => expect(sendCommand).toHaveBeenCalledTimes(1));
    expect(sendCommand).toHaveBeenCalledWith({
      kind: "approve_request",
      request_id: "runtime:browser.t5_2.profile:submit_form",
      scope: "once",
    });
    view.unmount();
    expect(cleanup).toHaveBeenCalledTimes(1);
  });

  test("bridge failures are visible and never rendered as an approval", async () => {
    const connectEvents = vi.fn(async (handlers: { onFailure: (error: HostIpcError) => void }) => {
      handlers.onFailure(new HostIpcError("core_transport_unavailable", ["Core disconnected"]));
      return vi.fn();
    });
    render(<App connectEvents={connectEvents} sendCommand={vi.fn()} />);
    expect(
      await screen.findByText(
        /Event stream unavailable: core_transport_unavailable: Core disconnected/,
      ),
    ).toBeDefined();
    expect(screen.queryByText("Human approval required")).toBeNull();
  });
});
