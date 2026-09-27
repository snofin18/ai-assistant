import { useEffect, useReducer } from "react";

import { createInitialTimelineControllerState, timelineControllerReducer } from "./timelineModel.js";
import type { TimelineControllerAction } from "./timelineModel.js";

/**
 * Owns timeline selection, undo, and replay intent state.
 */
export function useTimeline(model: unknown): {
  state: ReturnType<typeof createInitialTimelineControllerState>;
  dispatch: (action: TimelineControllerAction) => void;
} {
  const [state, dispatch] = useReducer(
    timelineControllerReducer,
    model,
    createInitialTimelineControllerState
  );
  useEffect(() => {
    dispatch({ type: "replace_model", model });
  }, [model]);
  return { state, dispatch };
}
