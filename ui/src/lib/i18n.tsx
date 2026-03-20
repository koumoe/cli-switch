import React, { createContext, useCallback, useContext, useEffect, useMemo, useState } from "react";

import sharedEnUS from "@shared-locales/en-US.json";
import sharedZhCN from "@shared-locales/zh-CN.json";
import uiEnUS from "@/locales/ui/en-US.json";
import uiZhCN from "@/locales/ui/zh-CN.json";

export type Locale = "zh-CN" | "en-US";

const STORAGE_KEY = "cliswitch-locale";
let currentLocale: Locale = "zh-CN";

function deepMergeMessages(base: unknown, overlay: unknown): unknown {
  if (!base || typeof base !== "object" || Array.isArray(base)) {
    return overlay ?? base;
  }
  if (!overlay || typeof overlay !== "object" || Array.isArray(overlay)) {
    return overlay ?? base;
  }

  const out: Record<string, unknown> = { ...(base as Record<string, unknown>) };
  for (const [key, value] of Object.entries(overlay as Record<string, unknown>)) {
    const prev = out[key];
    if (
      prev &&
      value &&
      typeof prev === "object" &&
      typeof value === "object" &&
      !Array.isArray(prev) &&
      !Array.isArray(value)
    ) {
      out[key] = deepMergeMessages(prev, value);
    } else {
      out[key] = value;
    }
  }
  return out;
}

const MESSAGES: Record<Locale, unknown> = {
  "zh-CN": deepMergeMessages(sharedZhCN, uiZhCN),
  "en-US": deepMergeMessages(sharedEnUS, uiEnUS),
};

export function normalizeLocale(input: string): Locale | null {
  const v = input.trim();
  if (!v) return null;
  const lower = v.toLowerCase();
  if (lower === "zh" || lower.startsWith("zh-")) return "zh-CN";
  if (lower === "en" || lower.startsWith("en-")) return "en-US";
  return null;
}

export function detectSystemLocale(): Locale {
  if (typeof window === "undefined") return "zh-CN";
  const candidates = [navigator.language, ...(navigator.languages ?? [])].filter(Boolean);
  for (const c of candidates) {
    const n = normalizeLocale(String(c));
    if (n) return n;
  }
  return "zh-CN";
}

export function getStoredLocale(): Locale | null {
  if (typeof window === "undefined") return "zh-CN";
  const stored = localStorage.getItem(STORAGE_KEY);
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
    localStorage.setItem(STORAGE_KEY, next);
  }
}

function getPathValue(obj: unknown, path: string): unknown {
  if (!obj) return undefined;
  if (!path) return undefined;
  const parts = path.split(".").filter(Boolean);
  let cur: any = obj;
  for (const p of parts) {
    if (cur && typeof cur === "object" && p in cur) {
      cur = cur[p];
    } else {
      return undefined;
    }
  }
  return cur;
}

function interpolate(template: string, vars?: Record<string, string | number>): string {
  if (!vars) return template;
  return template.replace(/\{\{\s*([a-zA-Z0-9_]+)\s*\}\}/g, (m, k) => {
    const v = vars[k];
    return v === undefined || v === null ? m : String(v);
  });
}

function translate(locale: Locale, key: string, vars?: Record<string, string | number>): string {
  const msg = getPathValue(MESSAGES[locale], key);
  if (typeof msg === "string") return interpolate(msg, vars);
  const fallback = getPathValue(MESSAGES["zh-CN"], key);
  if (typeof fallback === "string") return interpolate(fallback, vars);
  return key;
}

type I18nContextValue = {
  locale: Locale;
  setLocale: (next: Locale) => void;
  t: (key: string, vars?: Record<string, string | number>) => string;
  locales: { value: Locale; label: string }[];
};

const I18nContext = createContext<I18nContextValue | null>(null);

export function I18nProvider({ children }: { children: React.ReactNode }) {
  const [locale, setLocaleState] = useState<Locale>(() => currentLocale);

  const setLocale = useCallback((next: Locale) => {
    currentLocale = next;
    setLocaleState(next);
    persistLocale(next);
  }, []);

  useEffect(() => {
    currentLocale = locale;
  }, [locale]);

  useEffect(() => {
    let cancelled = false;
    const bootstrapLocale = async () => {
      try {
        const requestLocale = getCurrentLocale();
        const res = await fetch("/api/settings", {
          method: "GET",
          headers: {
            "X-CliSwitch-Locale": requestLocale,
          },
        });
        if (!res.ok) return;
        const payload = (await res.json()) as { ui_locale?: string };
        const next = payload.ui_locale ? normalizeLocale(payload.ui_locale) : null;
        if (!next || cancelled) return;
        currentLocale = next;
        setLocaleState(next);
        persistLocale(next);
      } catch {
        // ignore: startup fallback locale is already available
      }
    };
    void bootstrapLocale();
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const t = useCallback(
    (key: string, vars?: Record<string, string | number>) => translate(locale, key, vars),
    [locale]
  );

  const locales = useMemo(
    () => [
      { value: "zh-CN" as const, label: translate(locale, "language.zhCN") },
      { value: "en-US" as const, label: translate(locale, "language.enUS") },
    ],
    [locale]
  );

  const value = useMemo(() => ({ locale, setLocale, t, locales }), [locale, setLocale, t, locales]);

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export function useI18n() {
  const ctx = useContext(I18nContext);
  if (!ctx) throw new Error("useI18n must be used within I18nProvider");
  return ctx;
}
