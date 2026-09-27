import type {
  CapabilityApproval,
  CapabilityRiskLevel,
  CapabilitySideEffect,
  CapabilityResource,
  ChannelAvailability
} from "./capabilityMatrix.js";

/**
 * All user-visible Capability Matrix strings are injected here.
 */
export interface CapabilityMatrixCopy {
  heading: string;
  probeLabel: string;
  platformLabel: string;
  sessionLabel: string;
  totalLabel: string;
  approvalRequiredLabel: string;
  forbiddenLabel: string;
  degradedChannelsLabel: string;
  channelsHeading: string;
  capabilitiesHeading: string;
  degradationsHeading: string;
  capabilityIdColumnLabel: string;
  resourceColumnLabel: string;
  riskColumnLabel: string;
  approvalColumnLabel: string;
  noDegradations: string;
  validationErrorLabel: string;
  riskLabels: Record<CapabilityRiskLevel, string>;
  approvalLabels: Record<CapabilityApproval, string>;
  resourceLabels: Record<CapabilityResource, string>;
  sideEffectLabels: Record<CapabilitySideEffect, string>;
  channelAvailabilityLabels: Record<ChannelAvailability, string>;
}
