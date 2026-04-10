import type { Protocol } from "@/types/api";
export { formatDateTime, formatNumber } from "@/lib/format";

export type Translator = (key: string, vars?: Record<string, string | number>) => string;

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

export function protocolColor(protocol: Protocol): string {
  switch (protocol) {
    case "openai":
      return "hsl(var(--codex))";
    case "anthropic":
      return "hsl(var(--claude))";
    case "gemini":
      return "hsl(var(--gemini))";
  }
}

export function protocolBadgeClassName(protocol: Protocol): string {
  switch (protocol) {
    case "openai":
      return "border-codex/20 bg-codex/10 text-codex";
    case "anthropic":
      return "border-claude/20 bg-claude/10 text-claude";
    case "gemini":
      return "border-gemini/20 bg-gemini/10 text-gemini";
  }
}

export function protocolProgressClassName(protocol: Protocol): string {
  switch (protocol) {
    case "openai":
      return "bg-[linear-gradient(90deg,hsl(var(--codex)),hsl(var(--primary)))]";
    case "anthropic":
      return "bg-[linear-gradient(90deg,hsl(var(--claude)),hsl(var(--warning)))]";
    case "gemini":
      return "bg-[linear-gradient(90deg,hsl(var(--gemini)),hsl(var(--primary)))]";
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
