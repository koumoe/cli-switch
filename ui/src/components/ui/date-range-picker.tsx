import * as React from "react"
import { addMonths, format, isSameMonth, isValid, parse, startOfMonth, startOfWeek, subDays } from "date-fns"
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

interface DateRangePickerProps {
  value?: DateRange
  onChange?: (range: DateRange | undefined) => void
  placeholder?: string
  className?: string
  disabled?: boolean
  locale?: string // "zh-CN" | "en-US"
}

export function DateRangePicker({
  value,
  onChange,
  placeholder = "Select date range",
  className,
  disabled,
  locale = "zh-CN",
}: DateRangePickerProps) {
  const [open, setOpen] = React.useState(false)
  const dateFnsLocale = localeMap[locale] ?? zhCN
  const [startMonth, setStartMonth] = React.useState<Date>(() => startOfMonth(value?.from ?? new Date()))
  const [endMonth, setEndMonth] = React.useState<Date>(() => {
    const base = value?.to ?? value?.from ?? new Date()
    return startOfMonth(base)
  })
  const [phase, setPhase] = React.useState<"start" | "end">("start")

  const displayText = React.useMemo(() => {
    if (!value?.from) return placeholder
    if (!value.to) {
      return format(value.from, "yyyy-MM-dd")
    }
    return `${format(value.from, "yyyy-MM-dd")} ~ ${format(value.to, "yyyy-MM-dd")}`
  }, [value, placeholder])

  const applyPreset = React.useCallback((preset: "today" | "yesterday" | "week" | "month") => {
    const now = new Date()
    const range: DateRange =
      preset === "today"
        ? { from: now, to: now }
        : preset === "yesterday"
          ? (() => {
              const d = subDays(now, 1)
              return { from: d, to: d }
            })()
          : preset === "week"
            ? { from: startOfWeek(now, { weekStartsOn: 1 }), to: now }
            : { from: startOfMonth(now), to: now }

    onChange?.(range)
    setOpen(false)
    setPhase("start")
  }, [onChange])

  const presetLabels = React.useMemo(() => {
    if (locale === "zh-CN") {
      return { today: "今日", yesterday: "昨日", week: "本周", month: "本月" } as const
    }
    return { today: "Today", yesterday: "Yesterday", week: "This week", month: "This month" } as const
  }, [locale])

  return (
    <Popover
      open={open}
      onOpenChange={(v) => {
        setOpen(v)
        if (v) {
          const from = value?.from ?? new Date()
          const fromMonth = startOfMonth(from)
          const rawTo = value?.to ?? null
          const toMonth = rawTo ? startOfMonth(rawTo) : addMonths(fromMonth, 1)

          setStartMonth(fromMonth)
          setEndMonth(isSameMonth(fromMonth, toMonth) ? addMonths(fromMonth, 1) : toMonth)
          setPhase(value?.from && !value?.to ? "end" : "start")
        }
      }}
    >
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
        <div className="p-3 space-y-3">
          <div className="flex flex-col sm:flex-row gap-3">
            <Calendar
              className="p-0"
              initialFocus
              mode="range"
              month={startMonth}
              onMonthChange={(m) => {
                setStartMonth(startOfMonth(m))
              }}
              selected={value}
              onSelect={(range, selectedDay) => {
                if (!selectedDay) {
                  onChange?.(range)
                  return
                }
                if (phase === "start") {
                  const next: DateRange = { from: selectedDay, to: undefined }
                  onChange?.(next)
                  setPhase("end")
                  return
                }

                if (!range?.from) {
                  onChange?.({ from: selectedDay, to: undefined })
                  setPhase("end")
                  return
                }
                onChange?.(range)
                if (range?.from && range?.to) {
                  setOpen(false)
                  setPhase("start")
                } else {
                  setPhase("end")
                }
              }}
              locale={dateFnsLocale}
            />
            <Calendar
              className="p-0"
              mode="range"
              month={endMonth}
              onMonthChange={(m) => {
                setEndMonth(startOfMonth(m))
              }}
              selected={value}
              onSelect={(range, selectedDay) => {
                if (!selectedDay) {
                  onChange?.(range)
                  return
                }
                if (phase === "start") {
                  const next: DateRange = { from: selectedDay, to: undefined }
                  onChange?.(next)
                  setPhase("end")
                  return
                }

                if (!range?.from) {
                  onChange?.({ from: selectedDay, to: undefined })
                  setPhase("end")
                  return
                }
                onChange?.(range)
                if (range?.from && range?.to) {
                  setOpen(false)
                  setPhase("start")
                } else {
                  setPhase("end")
                }
              }}
              locale={dateFnsLocale}
            />
          </div>

          <div className="border-t pt-3 flex flex-wrap gap-2">
            <Button type="button" size="sm" variant="secondary" onClick={() => applyPreset("today")} disabled={disabled}>
              {presetLabels.today}
            </Button>
            <Button type="button" size="sm" variant="secondary" onClick={() => applyPreset("yesterday")} disabled={disabled}>
              {presetLabels.yesterday}
            </Button>
            <Button type="button" size="sm" variant="secondary" onClick={() => applyPreset("week")} disabled={disabled}>
              {presetLabels.week}
            </Button>
            <Button type="button" size="sm" variant="secondary" onClick={() => applyPreset("month")} disabled={disabled}>
              {presetLabels.month}
            </Button>
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
