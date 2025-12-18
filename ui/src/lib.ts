import type { Protocol } from "./api";

export type Translator = (key: string, vars?: Record<string, string | number>) => string;

export function formatDateTime(ms: number | null | undefined): string {
  if (!ms) return "-";
  const d = new Date(ms);
  if (Number.isNaN(d.getTime())) return "-";
  return d.toLocaleString();
}

export function protocolLabelKey(protocol: Protocol): string {
  switch (protocol) {
    case "openai":
      return "channels.tabs.codex";
    case "anthropic":
      return "channels.tabs.claude";
    case "gemini":
      return "channels.tabs.gemini";
  }
}

export function protocolLabel(t: Translator, protocol: Protocol): string {
  return t(protocolLabelKey(protocol));
}

export function formatDuration(ms: number | null | undefined): string {
  if (ms === null || ms === undefined) return "-";
  if (ms < 1000) return `${ms}ms`;
  if (ms < 60_000) return `${(ms / 1000).toFixed(2)}s`;
  return `${(ms / 60_000).toFixed(2)}m`;
}

export function clampStr(s: string, max: number): string {
  if (s.length <= max) return s;
  return `${s.slice(0, Math.max(0, max - 1))}…`;
}
