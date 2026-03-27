import type { NewApiManagedChannelMissingPrompt, UpdateStatus } from "@/api";

export type CliswitchUpdateStatusEvent = CustomEvent<UpdateStatus>;
export type CliswitchUsageChangedEvent = CustomEvent<{ at_ms: number }>;
export type CliswitchNewApiManagedChannelMissingEvent =
  CustomEvent<NewApiManagedChannelMissingPrompt>;
export type CliswitchChannelsChangedEvent = CustomEvent<{ at_ms: number }>;
