/**
 * All user-visible intent-launcher strings are injected here.
 */
export interface IntentLauncherCopy {
  heading: string;
  goalLabel: string;
  goalPlaceholder: string;
  submit: string;
  submitting: string;
  accepted: string;
  rejectedPrefix: string;
}

export const defaultIntentLauncherCopy: IntentLauncherCopy = {
  heading: "Start a task",
  goalLabel: "What should the assistant do?",
  goalPlaceholder: "Replace every occurrence of the quarterly report and save",
  submit: "Submit",
  submitting: "Submitting…",
  accepted: "Intent accepted by Core",
  rejectedPrefix: "Core rejected the intent",
};
