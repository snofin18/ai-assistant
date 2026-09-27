import { useReducer } from "react";

import {
  bindingWizardReducer,
  createInitialBindingWizardState
} from "./bindingModel.js";
import type { BindingWizardInput } from "./bindingModel.js";

export function useTargetBindingWizard(input: BindingWizardInput) {
  return useReducer(bindingWizardReducer, input, createInitialBindingWizardState);
}
