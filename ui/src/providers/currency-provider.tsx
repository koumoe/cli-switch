import {
  createContext,
  useCallback,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";

import { getUsdCnyExchangeRate } from "@/api";
import { formatNumber } from "@/lib/format";
import { useI18n } from "@/hooks/use-i18n";
import type { ExchangeRate } from "@/types/api";
import type { Locale } from "@/types/locale";

export type Currency = "USD" | "CNY";
export type CurrencyMode = "auto" | Currency;

const STORAGE_KEY = "cliswitch-currency-mode";
const EXCHANGE_RATE_POLL_INTERVAL_MS = 30 * 60 * 1_000;

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

export function convertCurrency(
  amount: number | null | undefined,
  sourceCurrency: Currency,
  targetCurrency: Currency,
  usdToCnyRate: number | null | undefined,
): number | null {
  if (amount === null || amount === undefined || !Number.isFinite(amount)) return null;
  if (sourceCurrency === targetCurrency) return amount;
  if (
    usdToCnyRate === null ||
    usdToCnyRate === undefined ||
    !Number.isFinite(usdToCnyRate) ||
    usdToCnyRate <= 0
  ) {
    return null;
  }

  return sourceCurrency === "USD"
    ? amount * usdToCnyRate
    : amount / usdToCnyRate;
}

export function calculateEstimatedSpend(
  officialCostUsd: number | null | undefined,
  realMultiplier: number | null | undefined,
  rechargeCurrency: Currency,
  displayCurrency: Currency,
  usdToCnyRate: number | null | undefined,
): number | null {
  if (
    officialCostUsd === null ||
    officialCostUsd === undefined ||
    !Number.isFinite(officialCostUsd) ||
    officialCostUsd < 0 ||
    realMultiplier === null ||
    realMultiplier === undefined ||
    !Number.isFinite(realMultiplier) ||
    realMultiplier < 0
  ) {
    return null;
  }

  return convertCurrency(
    officialCostUsd * realMultiplier,
    rechargeCurrency,
    displayCurrency,
    usdToCnyRate,
  );
}

export type CurrencyContextValue = {
  currencyMode: CurrencyMode;
  setCurrencyMode: (next: CurrencyMode) => void;
  currency: Currency;
  exchangeRate: ExchangeRate | null;
  exchangeRateLoading: boolean;
  usdToCnyRate: number | null;
};

export const CurrencyContext = createContext<CurrencyContextValue | null>(null);

export function CurrencyProvider({ children }: { children: ReactNode }) {
  const { locale } = useI18n();
  const [currencyMode, setCurrencyModeState] = useState<CurrencyMode>(() => getInitialCurrencyMode());
  const [exchangeRate, setExchangeRate] = useState<ExchangeRate | null>(null);
  const [exchangeRateLoading, setExchangeRateLoading] = useState(false);

  useEffect(() => {
    let active = true;
    const refresh = async () => {
      setExchangeRateLoading(true);
      try {
        const next = await getUsdCnyExchangeRate();
        if (
          active &&
          next.base_currency === "USD" &&
          next.quote_currency === "CNY" &&
          Number.isFinite(next.rate) &&
          next.rate > 0
        ) {
          setExchangeRate(next);
        }
      } catch {
        // Keep the last cached rate in memory. The backend returns a persisted
        // stale rate whenever refreshing the external source fails.
      } finally {
        if (active) setExchangeRateLoading(false);
      }
    };

    void refresh();
    const timer = window.setInterval(refresh, EXCHANGE_RATE_POLL_INTERVAL_MS);
    return () => {
      active = false;
      window.clearInterval(timer);
    };
  }, []);

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

  const usdToCnyRate = exchangeRate?.rate ?? null;

  const value = useMemo(
    () => ({
      currencyMode,
      setCurrencyMode,
      currency,
      exchangeRate,
      exchangeRateLoading,
      usdToCnyRate,
    }),
    [
      currencyMode,
      setCurrencyMode,
      currency,
      exchangeRate,
      exchangeRateLoading,
      usdToCnyRate,
    ],
  );

  return <CurrencyContext.Provider value={value}>{children}</CurrencyContext.Provider>;
}
