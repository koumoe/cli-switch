import React, { createContext, useCallback, useContext, useMemo, useState } from "react";

import enUS from "@/locales/en-US.json";
import zhCN from "@/locales/zh-CN.json";

export type Locale = "zh-CN" | "en-US";

const STORAGE_KEY = "cliswitch-locale";

const MESSAGES: Record<Locale, unknown> = {
  "zh-CN": zhCN,
  "en-US": enUS,
};

function normalizeLocale(input: string): Locale | null {
  const v = input.trim();
  if (!v) return null;
  const lower = v.toLowerCase();
  if (lower === "zh" || lower.startsWith("zh-")) return "zh-CN";
  if (lower === "en" || lower.startsWith("en-")) return "en-US";
  return null;
}

function detectSystemLocale(): Locale {
  if (typeof window === "undefined") return "zh-CN";
  const candidates = [navigator.language, ...(navigator.languages ?? [])].filter(Boolean);
  for (const c of candidates) {
    const n = normalizeLocale(String(c));
    if (n) return n;
  }
  return "zh-CN";
}

function getInitialLocale(): Locale {
  if (typeof window === "undefined") return "zh-CN";
  const stored = localStorage.getItem(STORAGE_KEY);
  const normalized = stored ? normalizeLocale(stored) : null;
  return normalized ?? detectSystemLocale();
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
  const [locale, setLocaleState] = useState<Locale>(() => getInitialLocale());

  const setLocale = useCallback((next: Locale) => {
    setLocaleState(next);
    if (typeof window !== "undefined") {
      localStorage.setItem(STORAGE_KEY, next);
    }
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

