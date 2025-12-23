import React, { createContext, useCallback, useContext, useMemo, useState } from "react";

import { useI18n, type Locale } from "@/lib/i18n";

export type Currency = "USD" | "CNY";
export type CurrencyMode = "auto" | Currency;

const STORAGE_KEY = "cliswitch-currency-mode";

function detectCurrencyFromLocale(locale: Locale): Currency {
  return locale.startsWith("zh") ? "CNY" : "USD";
}

function normalizeCurrencyMode(input: string): CurrencyMode | null {
  const v = input.trim().toUpperCase();
  if (!v) return null;
  if (v === "AUTO") return "auto";
  if (v === "USD") return "USD";
  if (v === "CNY") return "CNY";
  return null;
}

function getInitialCurrencyMode(): CurrencyMode {
  if (typeof window === "undefined") return "auto";
  const stored = localStorage.getItem(STORAGE_KEY);
  const normalized = stored ? normalizeCurrencyMode(stored) : null;
  return normalized ?? "auto";
}

export function currencySymbol(currency: Currency): string {
  switch (currency) {
    case "USD":
      return "$";
    case "CNY":
      return "¥";
  }
}

export function formatDecimal(n: number, maxDecimals = 6): string {
  if (!Number.isFinite(n)) return "-";
  const s = n.toFixed(Math.max(0, Math.min(12, Math.floor(maxDecimals))));
  return s.replace(/\.?0+$/, "");
}

export function parseDecimalLike(v: string | number | null | undefined): number | null {
  if (v === null || v === undefined) return null;
  if (typeof v === "number") return Number.isFinite(v) ? v : null;
  const s = v.trim();
  if (!s) return null;
  const n = Number(s);
  return Number.isFinite(n) ? n : null;
}

export function formatMoney(
  amount: number | null | undefined,
  currency: Currency,
  maxDecimals = 6,
): string {
  if (amount === null || amount === undefined) return "-";
  if (!Number.isFinite(amount)) return "-";
  return `${currencySymbol(currency)}${formatDecimal(amount, maxDecimals)}`;
}

type CurrencyContextValue = {
  currencyMode: CurrencyMode;
  setCurrencyMode: (next: CurrencyMode) => void;
  currency: Currency;
};

const CurrencyContext = createContext<CurrencyContextValue | null>(null);

export function CurrencyProvider({ children }: { children: React.ReactNode }) {
  const { locale } = useI18n();
  const [currencyMode, setCurrencyModeState] = useState<CurrencyMode>(() => getInitialCurrencyMode());

  const setCurrencyMode = useCallback((next: CurrencyMode) => {
    setCurrencyModeState(next);
    if (typeof window !== "undefined") {
      localStorage.setItem(STORAGE_KEY, next);
    }
  }, []);

  const currency = useMemo(() => {
    if (currencyMode === "auto") return detectCurrencyFromLocale(locale);
    return currencyMode;
  }, [currencyMode, locale]);

  const value = useMemo(
    () => ({ currencyMode, setCurrencyMode, currency }),
    [currencyMode, setCurrencyMode, currency],
  );
  return <CurrencyContext.Provider value={value}>{children}</CurrencyContext.Provider>;
}

export function useCurrency() {
  const ctx = useContext(CurrencyContext);
  if (!ctx) throw new Error("useCurrency must be used within CurrencyProvider");
  return ctx;
}

