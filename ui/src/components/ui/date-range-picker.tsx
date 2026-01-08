import * as React from "react"
import { format, isValid, parse, startOfDay, startOfMonth, startOfWeek, subDays } from "date-fns"
import { zhCN, enUS } from "date-fns/locale"
import { Calendar as CalendarIcon } from "lucide-react"
import type { DateRange } from "react-day-picker"
import type { Locale as DateFnsLocale } from "date-fns"
import { cn } from "@/lib/utils"
import { Button } from "@/components/ui/button"
import { Calendar } from "@/components/ui/calendar"
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover"

const localeMap: Record<string, DateFnsLocale> = {
  "zh-CN": zhCN,
  "en-US": enUS,
}

export type DateRangePresetKey = "today" | "yesterday" | "week" | "month"

export function resolveDateFnsLocale(locale: string | undefined): DateFnsLocale {
  return localeMap[locale ?? ""] ?? zhCN
}

export function computePresetDateRange(preset: DateRangePresetKey, locale: string | undefined, now = new Date()): DateRange {
  const dateFnsLocale = resolveDateFnsLocale(locale)
  const today = startOfDay(now)
  switch (preset) {
    case "today":
      return { from: today, to: today }
    case "yesterday": {
      const yesterday = startOfDay(subDays(today, 1))
      return { from: yesterday, to: yesterday }
    }
    case "week": {
      const weekStart = startOfDay(startOfWeek(now, { locale: dateFnsLocale }))
      return { from: weekStart, to: today }
    }
    case "month": {
      const monthStart = startOfDay(startOfMonth(now))
      return { from: monthStart, to: today }
    }
  }
}

interface DateRangePickerProps {
  value?: DateRange
  onChange?: (range: DateRange | undefined) => void
  onPresetChange?: (preset: DateRangePresetKey | null) => void
  placeholder?: string
  className?: string
  disabled?: boolean
  locale?: string // "zh-CN" | "en-US"
}

export function DateRangePicker({
  value,
  onChange,
  onPresetChange,
  placeholder = "Select date range",
  className,
  disabled,
  locale = "zh-CN",
}: DateRangePickerProps) {
  const [open, setOpen] = React.useState(false)
  const dateFnsLocale = resolveDateFnsLocale(locale)
  const selectStepRef = React.useRef<0 | 1>(0)

  React.useEffect(() => {
    if (!open) selectStepRef.current = 0
  }, [open])

  const presetLabels = React.useMemo(() => {
    if (locale.startsWith("zh")) {
      return { today: "今日", yesterday: "昨日", week: "本周", month: "本月" } as const
    }
    return { today: "Today", yesterday: "Yesterday", week: "This week", month: "This month" } as const
  }, [locale])
  const presets = [
    { key: "today", label: presetLabels.today },
    { key: "yesterday", label: presetLabels.yesterday },
    { key: "week", label: presetLabels.week },
    { key: "month", label: presetLabels.month },
  ] as const

  const displayText = React.useMemo(() => {
    if (!value?.from) return placeholder
    if (!value.to) {
      return format(value.from, "yyyy-MM-dd")
    }
    return `${format(value.from, "yyyy-MM-dd")} ~ ${format(value.to, "yyyy-MM-dd")}`
  }, [value, placeholder])

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button
          variant="outline"
          className={cn(
            "justify-start text-left font-normal",
            !value?.from && "text-muted-foreground",
            className
          )}
          disabled={disabled}
        >
          <CalendarIcon className="mr-2 h-4 w-4" />
          {displayText}
        </Button>
      </PopoverTrigger>
      <PopoverContent className="w-auto p-0" align="start">
        <div className="p-0">
          <Calendar
            initialFocus
            mode="range"
            defaultMonth={value?.from}
            selected={value}
            onSelect={(range, selectedDay) => {
              const clickedDay = selectedDay instanceof Date ? selectedDay : null
              if (clickedDay) onPresetChange?.(null)

              if (value?.from && value?.to && selectStepRef.current === 0 && clickedDay) {
                selectStepRef.current = 1
                onChange?.({ from: clickedDay, to: undefined })
                return
              }

              if (!range?.from) {
                selectStepRef.current = 0
                onChange?.(range)
                return
              }

              if (!range.to) {
                selectStepRef.current = 1
                onChange?.(range)
                return
              }

              if (selectStepRef.current !== 1) {
                selectStepRef.current = 1
                onChange?.({ from: clickedDay ?? range.from, to: undefined })
                return
              }

              onChange?.(range)
              setOpen(false)
            }}
            numberOfMonths={2}
            locale={dateFnsLocale}
          />
          <div className="border-t px-3 py-2">
            <div className="flex flex-wrap gap-2">
              {presets.map((p) => (
                <Button
                  key={p.key}
                  size="sm"
                  variant="outline"
                  className="h-7 px-2 text-xs"
                  onClick={() => {
                    const range = computePresetDateRange(p.key, locale)
                    if (onPresetChange) {
                      onPresetChange(p.key)
                    } else {
                      onChange?.(range)
                    }
                    setOpen(false)
                  }}
                >
                  {p.label}
                </Button>
              ))}
            </div>
          </div>
        </div>
      </PopoverContent>
    </Popover>
  )
}

// ============ Helper functions ============

/**
 * Convert DateRange to API format strings (YYYY-MM-DD)
 * Returns null if range is invalid
 */
export function dateRangeToStrings(range: DateRange | undefined): { start: string; end: string } | null {
  if (!range?.from) return null
  const start = format(range.from, "yyyy-MM-dd")
  const end = range.to ? format(range.to, "yyyy-MM-dd") : start
  return { start, end }
}

/**
 * Convert DateRange to millisecond timestamps (local timezone)
 * start_ms: beginning of start day (00:00:00.000)
 * end_ms: end of end day (23:59:59.999)
 */
export function dateRangeToMs(range: DateRange | undefined): { start_ms: number; end_ms: number } | null {
  if (!range?.from) return null
  const startDate = new Date(range.from)
  startDate.setHours(0, 0, 0, 0)
  const endDate = range.to ? new Date(range.to) : new Date(range.from)
  endDate.setHours(23, 59, 59, 999)
  return { start_ms: startDate.getTime(), end_ms: endDate.getTime() }
}

/**
 * Parse YYYY-MM-DD strings to DateRange
 */
export function stringsToDateRange(start: string, end: string): DateRange | undefined {
  if (!start && !end) return undefined
  const fromDate = start ? parse(start, "yyyy-MM-dd", new Date()) : null
  const toDate = end ? parse(end, "yyyy-MM-dd", new Date()) : null

  if (!fromDate || !isValid(fromDate)) return undefined

  return {
    from: fromDate,
    to: toDate && isValid(toDate) ? toDate : undefined,
  }
}
