import { createContext, useCallback, useEffect, useMemo, useState, type ReactNode } from "react";

import sharedEnUS from "@shared-locales/en-US.json";
import sharedZhCN from "@shared-locales/zh-CN.json";
import uiEnUS from "@/locales/ui/en-US.json";
import uiZhCN from "@/locales/ui/zh-CN.json";
import type { Locale } from "@/types/locale";

export type { Locale } from "@/types/locale";

const STORAGE_KEY = "cliswitch-locale";
let currentLocale: Locale = "zh-CN";
let currentLocaleRevision = 0;

function isMergeableRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === "object" && !Array.isArray(value);
}

function deepMergeMessages(base: unknown, overlay: unknown): unknown {
  if (!isMergeableRecord(base) || !isMergeableRecord(overlay)) {
    return overlay ?? base;
  }

  const out: Record<string, unknown> = { ...base };
  for (const [key, value] of Object.entries(overlay)) {
    const prev = out[key];
    out[key] =
      isMergeableRecord(prev) && isMergeableRecord(value) ? deepMergeMessages(prev, value) : value;
  }
  return out;
}

const MESSAGES: Record<Locale, unknown> = {
  "zh-CN": deepMergeMessages(sharedZhCN, uiZhCN),
  "en-US": deepMergeMessages(sharedEnUS, uiEnUS),
};

export function normalizeLocale(input: string): Locale | null {
  const value = input.trim();
  if (!value) return null;

  const lower = value.toLowerCase();
  if (lower === "zh" || lower.startsWith("zh-")) return "zh-CN";
  if (lower === "en" || lower.startsWith("en-")) return "en-US";
  return null;
}

export function detectSystemLocale(): Locale {
  if (typeof window === "undefined") return "zh-CN";

  const candidates = [navigator.language, ...(navigator.languages ?? [])].filter(Boolean);
  for (const candidate of candidates) {
    const locale = normalizeLocale(String(candidate));
    if (locale) return locale;
  }

  return "zh-CN";
}

export function getStoredLocale(): Locale | null {
  if (typeof window === "undefined") return null;
  const stored = window.localStorage.getItem(STORAGE_KEY);
  return stored ? normalizeLocale(stored) : null;
}

export function getInitialLocale(): Locale {
  return getStoredLocale() ?? detectSystemLocale();
}

currentLocale = getInitialLocale();

export function getCurrentLocale(): Locale {
  return currentLocale;
}

export function persistLocale(next: Locale) {
  if (typeof window !== "undefined") {
    window.localStorage.setItem(STORAGE_KEY, next);
  }
}

function applyRuntimeLocale(
  next: Locale,
  options?: {
    persist?: boolean;
    bumpRevision?: boolean;
  },
) {
  currentLocale = next;
  if (options?.bumpRevision ?? true) {
    currentLocaleRevision += 1;
  }
  if (options?.persist ?? true) {
    persistLocale(next);
  }
}

function getPathValue(obj: unknown, path: string): unknown {
  if (!obj || !path) return undefined;

  const parts = path.split(".").filter(Boolean);
  let current: unknown = obj;
  for (const part of parts) {
    if (
      current &&
      typeof current === "object" &&
      !Array.isArray(current) &&
      part in current
    ) {
      current = (current as Record<string, unknown>)[part];
    } else {
      return undefined;
    }
  }
  return current;
}

function interpolate(template: string, vars?: Record<string, string | number>): string {
  if (!vars) return template;

  return template.replace(/\{\{\s*([a-zA-Z0-9_]+)\s*\}\}/g, (match, key) => {
    const value = vars[key];
    return value === undefined || value === null ? match : String(value);
  });
}

function translate(locale: Locale, key: string, vars?: Record<string, string | number>): string {
  const message = getPathValue(MESSAGES[locale], key);
  if (typeof message === "string") return interpolate(message, vars);

  const fallback = getPathValue(MESSAGES["zh-CN"], key);
  if (typeof fallback === "string") return interpolate(fallback, vars);

  return key;
}

export function translateForLocale(
  locale: Locale,
  key: string,
  vars?: Record<string, string | number>,
): string {
  return translate(locale, key, vars);
}

export type I18nContextValue = {
  locale: Locale;
  setLocale: (next: Locale) => void;
  t: (key: string, vars?: Record<string, string | number>) => string;
  locales: { value: Locale; label: string }[];
};

export const I18nContext = createContext<I18nContextValue | null>(null);

export function I18nProvider({ children }: { children: ReactNode }) {
  const [locale, setLocaleState] = useState<Locale>(() => currentLocale);

  const setLocale = useCallback((next: Locale) => {
    applyRuntimeLocale(next);
    setLocaleState(next);
  }, []);

  useEffect(() => {
    currentLocale = locale;
  }, [locale]);

  useEffect(() => {
    let cancelled = false;

    const bootstrapLocale = async () => {
      const bootstrapRevision = currentLocaleRevision;
      try {
        const requestLocale = getCurrentLocale();
        const response = await fetch("/api/settings", {
          method: "GET",
          headers: {
            "X-CliSwitch-Locale": requestLocale,
          },
        });
        if (!response.ok) return;

        const payload = (await response.json()) as { ui_locale?: string };
        const next = payload.ui_locale ? normalizeLocale(payload.ui_locale) : null;
        if (!next || cancelled || currentLocaleRevision !== bootstrapRevision) return;

        applyRuntimeLocale(next, { bumpRevision: false });
        setLocaleState(next);
      } catch {
        // Ignore startup fallback failures.
      }
    };

    void bootstrapLocale();
    return () => {
      cancelled = true;
    };
  }, []);

  const t = useCallback(
    (key: string, vars?: Record<string, string | number>) => translate(locale, key, vars),
    [locale],
  );

  const locales = useMemo(
    () => [
      { value: "zh-CN" as const, label: translate(locale, "language.zhCN") },
      { value: "en-US" as const, label: translate(locale, "language.enUS") },
    ],
    [locale],
  );

  const value = useMemo(() => ({ locale, setLocale, t, locales }), [locale, setLocale, t, locales]);

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}
