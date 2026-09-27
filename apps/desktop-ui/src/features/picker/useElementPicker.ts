import { useReducer } from "react";

import {
  createInitialPickerControllerState,
  pickerControllerReducer
} from "./pickerModel.js";

export function useElementPicker(model: unknown) {
  return useReducer(pickerControllerReducer, model, createInitialPickerControllerState);
}
