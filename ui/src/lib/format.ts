import { getCurrentLocale, type Locale } from "@/lib/i18n";

type LocaleNumberOptions = Intl.NumberFormatOptions & {
  locale?: Locale;
};

type LocaleDateTimeOptions = {
  locale?: Locale;
};

export function formatNumber(
  value: number | null | undefined,
  options?: LocaleNumberOptions,
): string {
  if (value === null || value === undefined) return "-";
  if (!Number.isFinite(value)) return "-";

  const { locale = getCurrentLocale(), ...intlOptions } = options ?? {};
  return new Intl.NumberFormat(locale, intlOptions).format(value);
}

export function formatDateTime(
  ms: number | null | undefined,
  options?: LocaleDateTimeOptions,
): string {
  if (ms === null || ms === undefined) return "-";
  const d = new Date(ms);
  if (Number.isNaN(d.getTime())) return "-";

  const locale = options?.locale ?? getCurrentLocale();
  return new Intl.DateTimeFormat(locale, {
    dateStyle: "short",
    timeStyle: "medium",
  }).format(d);
}
