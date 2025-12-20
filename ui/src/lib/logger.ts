export type LogLevel = "none" | "debug" | "info" | "warning" | "error";

type LogEntry = {
  ts_ms: number;
  level: Exclude<LogLevel, "none">;
  message: string;
  event?: string;
  href?: string;
  fields?: unknown;
};

const LEVEL_RANK: Record<LogLevel, number> = {
  none: 99,
  debug: 10,
  info: 20,
  warning: 30,
  error: 40
};

let currentLevel: LogLevel = "warning";

export function setLogLevel(level: LogLevel) {
  currentLevel = level;
}

export function getLogLevel(): LogLevel {
  return currentLevel;
}

function shouldEmit(level: Exclude<LogLevel, "none">): boolean {
  if (currentLevel === "none") return false;
  return LEVEL_RANK[level] >= LEVEL_RANK[currentLevel];
}

function consoleMethod(level: Exclude<LogLevel, "none">): (...args: unknown[]) => void {
  if (level === "debug") return console.debug.bind(console);
  if (level === "info") return console.info.bind(console);
  if (level === "warning") return console.warn.bind(console);
  return console.error.bind(console);
}

function emit(level: Exclude<LogLevel, "none">, message: string, opts?: { event?: string; fields?: unknown }) {
  if (!shouldEmit(level)) return;

  const entry: LogEntry = {
    ts_ms: Date.now(),
    level,
    message,
    event: opts?.event,
    href: typeof window !== "undefined" ? window.location.href : undefined,
    fields: opts?.fields
  };

  try {
    const prefix = `[${level}] ${message}`;
    if (opts?.event || opts?.fields !== undefined) {
      consoleMethod(level)(prefix, { event: opts?.event, fields: opts?.fields });
    } else {
      consoleMethod(level)(prefix);
    }
  } catch {
    // ignore
  }

  void fetch("/api/logs/ingest", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(entry),
    keepalive: true
  }).catch(() => {
    // ignore
  });
}

export const logger = {
  debug(message: string, fields?: unknown, event?: string) {
    emit("debug", message, { fields, event });
  },
  info(message: string, fields?: unknown, event?: string) {
    emit("info", message, { fields, event });
  },
  warn(message: string, fields?: unknown, event?: string) {
    emit("warning", message, { fields, event });
  },
  error(message: string, fields?: unknown, event?: string) {
    emit("error", message, { fields, event });
  }
};
