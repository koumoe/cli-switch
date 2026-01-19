import type { UpdateStatus } from "@/api";

export type CliswitchUpdateStatusEvent = CustomEvent<UpdateStatus>;
export type CliswitchUsageChangedEvent = CustomEvent<{ at_ms: number }>;

export type NpmEnvInstallProgress = {
  stage: string;
  version: string | null;
  percent: number | null;
  total_bytes: number | null;
  downloaded_bytes: number | null;
  message: string | null;
};

export type CliswitchNpmEnvInstallProgressEvent = CustomEvent<NpmEnvInstallProgress>;
