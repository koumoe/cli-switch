import type {
  RemoteGroupAddedAlert,
  RemoteManagedChannelMissingPrompt,
  RemoteManagedChannelMultiplierPrompt,
  UpdateStatus,
} from "@/api";

export type CliswitchUpdateStatusEvent = CustomEvent<UpdateStatus>;
export type CliswitchUsageChangedEvent = CustomEvent<{ at_ms: number }>;
export type CliswitchRemoteManagedChannelMissingEvent =
  CustomEvent<RemoteManagedChannelMissingPrompt>;
export type CliswitchRemoteManagedChannelMultiplierEvent =
  CustomEvent<RemoteManagedChannelMultiplierPrompt>;
export type CliswitchRemoteGroupAddedEvent = CustomEvent<RemoteGroupAddedAlert>;
export type CliswitchChannelsChangedEvent = CustomEvent<{ at_ms: number }>;
