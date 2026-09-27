import type { PickerCopy } from "../picker/pickerCopy.js";

export interface BindingWizardCopy {
  heading: string;
  stepLabels: Record<"application" | "target" | "review" | "export", string>;
  metadataHeading: string;
  appIdLabel: string;
  displayNameLabel: string;
  targetNameLabel: string;
  versionRangeLabel: string;
  candidateHeading: string;
  continueLabel: string;
  backLabel: string;
  exportLabel: string;
  validationErrorLabel: string;
  picker: PickerCopy;
  errorMessages: Record<string, string>;
}
