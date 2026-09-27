import { useReducer } from "react";

import { createInitialEgressPolicyState, egressPolicyControllerReducer } from "./egressPolicy.js";

export function useEgressPolicy(policy: unknown) {
  return useReducer(egressPolicyControllerReducer, policy, createInitialEgressPolicyState);
}
