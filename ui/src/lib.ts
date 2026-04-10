import type { Protocol } from "@/types/api";
export { formatDateTime, formatNumber } from "@/lib/format";

export type Translator = (
  key: string,
  vars?: Record<string, string | number>,
) => string;

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

function protocolThemeName(protocol: Protocol): "codex" | "claude" | "gemini" {
  switch (protocol) {
    case "openai":
      return "codex";
    case "anthropic":
      return "claude";
    case "gemini":
      return "gemini";
  }
}

export function protocolColor(protocol: Protocol): string {
  return `oklch(var(--${protocolThemeName(protocol)}))`;
}

export function protocolDotClassName(protocol: Protocol): string {
  switch (protocol) {
    case "openai":
      return "bg-codex";
    case "anthropic":
      return "bg-claude";
    case "gemini":
      return "bg-gemini";
  }
}

export function protocolBadgeClassName(_protocol: Protocol): string {
  return "border-border/80 bg-secondary/55 text-foreground";
}

export function protocolProgressClassName(protocol: Protocol): string {
  switch (protocol) {
    case "openai":
      return "bg-codex";
    case "anthropic":
      return "bg-claude";
    case "gemini":
      return "bg-gemini";
  }
}

export function formatDuration(ms: number | null | undefined): string {
  if (ms === null || ms === undefined) return "-";
  if (ms < 1000) return `${ms}ms`;
  if (ms < 60_000) return `${(ms / 1000).toFixed(2)}s`;
  return `${(ms / 60_000).toFixed(2)}m`;
}

export function formatBytes(bytes: number | null | undefined): string {
  if (bytes === null || bytes === undefined) return "-";
  if (!Number.isFinite(bytes)) return "-";
  const b = Math.max(0, bytes);
  const units = ["B", "KB", "MB", "GB", "TB"] as const;
  let v = b;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i += 1;
  }
  const n = i === 0 ? String(Math.round(v)) : v.toFixed(v >= 10 ? 1 : 2);
  return `${n}${units[i]}`;
}

export function clampStr(s: string, max: number): string {
  if (s.length <= max) return s;
  return `${s.slice(0, Math.max(0, max - 1))}…`;
}
