import { createContext, useCallback, useMemo, useState, type ReactNode } from "react";

import { formatNumber } from "@/lib/format";
import { useI18n } from "@/hooks/use-i18n";
import type { Locale } from "@/types/locale";

export type Currency = "USD" | "CNY";
export type CurrencyMode = "auto" | Currency;

const STORAGE_KEY = "cliswitch-currency-mode";

function detectCurrencyFromLocale(locale: Locale): Currency {
  return locale.startsWith("zh") ? "CNY" : "USD";
}

function normalizeCurrencyMode(input: string): CurrencyMode | null {
  const value = input.trim().toUpperCase();
  if (!value) return null;
  if (value === "AUTO") return "auto";
  if (value === "USD") return "USD";
  if (value === "CNY") return "CNY";
  return null;
}

function getInitialCurrencyMode(): CurrencyMode {
  if (typeof window === "undefined") return "auto";
  const stored = window.localStorage.getItem(STORAGE_KEY);
  const normalized = stored ? normalizeCurrencyMode(stored) : null;
  return normalized ?? "auto";
}

export function formatDecimal(n: number, maxDecimals = 6, locale?: Locale): string {
  if (!Number.isFinite(n)) return "-";
  return formatNumber(n, {
    locale,
    minimumFractionDigits: 0,
    maximumFractionDigits: Math.max(0, Math.min(12, Math.floor(maxDecimals))),
  });
}

export function parseDecimalLike(v: string | number | null | undefined): number | null {
  if (v === null || v === undefined) return null;
  if (typeof v === "number") return Number.isFinite(v) ? v : null;

  const value = v.trim();
  if (!value) return null;

  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : null;
}

export function formatMoney(
  amount: number | null | undefined,
  currency: Currency,
  maxDecimals = 6,
  locale?: Locale,
): string {
  if (amount === null || amount === undefined || !Number.isFinite(amount)) return "-";

  return formatNumber(amount, {
    locale,
    style: "currency",
    currency,
    minimumFractionDigits: 0,
    maximumFractionDigits: Math.max(0, Math.min(12, Math.floor(maxDecimals))),
  });
}

export type CurrencyContextValue = {
  currencyMode: CurrencyMode;
  setCurrencyMode: (next: CurrencyMode) => void;
  currency: Currency;
};

export const CurrencyContext = createContext<CurrencyContextValue | null>(null);

export function CurrencyProvider({ children }: { children: ReactNode }) {
  const { locale } = useI18n();
  const [currencyMode, setCurrencyModeState] = useState<CurrencyMode>(() => getInitialCurrencyMode());

  const setCurrencyMode = useCallback((next: CurrencyMode) => {
    setCurrencyModeState(next);
    if (typeof window !== "undefined") {
      window.localStorage.setItem(STORAGE_KEY, next);
    }
  }, []);

  const currency = useMemo(
    () => (currencyMode === "auto" ? detectCurrencyFromLocale(locale) : currencyMode),
    [currencyMode, locale],
  );

  const value = useMemo(
    () => ({ currencyMode, setCurrencyMode, currency }),
    [currencyMode, setCurrencyMode, currency],
  );

  return <CurrencyContext.Provider value={value}>{children}</CurrencyContext.Provider>;
}
