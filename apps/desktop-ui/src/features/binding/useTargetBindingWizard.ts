import { useReducer } from "react";

import { bindingWizardReducer, createInitialBindingWizardState } from "./bindingModel.js";
export function useTargetBindingWizard(input: unknown) {
  return useReducer(bindingWizardReducer, input, createInitialBindingWizardState);
}
