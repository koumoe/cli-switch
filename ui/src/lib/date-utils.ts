import { format, isValid, parse } from "date-fns"
import { enUS, zhCN } from "date-fns/locale"
import type { Locale as DateFnsLocale } from "date-fns"
import type { DateRange } from "react-day-picker"

const localeMap: Record<string, DateFnsLocale> = {
  "zh-CN": zhCN,
  "en-US": enUS,
}

export function resolveDateFnsLocale(locale?: string): DateFnsLocale {
  if (!locale) return zhCN
  return localeMap[locale] ?? zhCN
}

export function dateRangeToStrings(
  range: DateRange | undefined
): { start: string; end: string } | null {
  if (!range?.from) return null
  const start = format(range.from, "yyyy-MM-dd")
  const end = range.to ? format(range.to, "yyyy-MM-dd") : start
  return { start, end }
}

export function dateRangeToMs(
  range: DateRange | undefined
): { start_ms: number; end_ms: number } | null {
  if (!range?.from) return null

  const startDate = new Date(range.from)
  startDate.setHours(0, 0, 0, 0)

  const endDate = range.to ? new Date(range.to) : new Date(range.from)
  endDate.setHours(23, 59, 59, 999)

  return { start_ms: startDate.getTime(), end_ms: endDate.getTime() }
}

export function stringsToDateRange(
  start: string,
  end: string
): DateRange | undefined {
  if (!start && !end) return undefined

  const fromDate = start ? parse(start, "yyyy-MM-dd", new Date()) : null
  const toDate = end ? parse(end, "yyyy-MM-dd", new Date()) : null

  if (!fromDate || !isValid(fromDate)) return undefined

  return {
    from: fromDate,
    to: toDate && isValid(toDate) ? toDate : undefined,
  }
}
