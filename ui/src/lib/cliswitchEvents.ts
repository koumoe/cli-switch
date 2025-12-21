import type { UpdateStatus } from "@/api";

export type CliswitchUpdateStatusEvent = CustomEvent<UpdateStatus>;
export type CliswitchUsageChangedEvent = CustomEvent<{ at_ms: number }>;

