import { useEffect, useReducer } from "react";

import {
  approvalControllerReducer,
  createInitialApprovalControllerState,
} from "./approvalModel.js";
import type { ApprovalControllerAction } from "./approvalModel.js";

/**
 * Owns approval-card interaction state without performing authorization.
 */
export function useApprovalCard(model: unknown): {
  state: ReturnType<typeof createInitialApprovalControllerState>;
  dispatch: (action: ApprovalControllerAction) => void;
} {
  const [state, dispatch] = useReducer(
    approvalControllerReducer,
    model,
    createInitialApprovalControllerState,
  );
  useEffect(() => {
    dispatch({ type: "replace_model", model });
  }, [model]);
  return { state, dispatch };
}
