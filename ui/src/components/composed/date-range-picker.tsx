import * as React from "react"
import { addMonths, format, isSameMonth, startOfMonth, startOfWeek, subDays } from "date-fns"
import { Calendar as CalendarIcon } from "lucide-react"
import type { DateRange } from "react-day-picker"
import { Button } from "@/components/ui/button"
import { Calendar } from "@/components/ui/calendar"
import { useI18n } from "@/hooks/use-i18n"
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover"
import { cn } from "@/lib/utils"
import { resolveDateFnsLocale } from "@/lib/date-utils"
import { normalizeLocale, translateForLocale } from "@/providers/i18n-provider"

interface DateRangePickerProps {
  value?: DateRange
  onChange?: (range: DateRange | undefined) => void
  placeholder?: string
  className?: string
  disabled?: boolean
  locale?: string
}

export function DateRangePicker({
  value,
  onChange,
  placeholder,
  className,
  disabled,
  locale,
}: DateRangePickerProps) {
  const { locale: currentLocale } = useI18n()
  const effectiveLocale = normalizeLocale(locale ?? currentLocale) ?? currentLocale
  const [open, setOpen] = React.useState(false)
  const dateFnsLocale = resolveDateFnsLocale(effectiveLocale)
  const [startMonth, setStartMonth] = React.useState<Date>(() =>
    startOfMonth(value?.from ?? new Date())
  )
  const [endMonth, setEndMonth] = React.useState<Date>(() => {
    const base = value?.to ?? value?.from ?? new Date()
    return startOfMonth(base)
  })
  const [phase, setPhase] = React.useState<"start" | "end">("start")
  const resolvedPlaceholder =
    placeholder ?? translateForLocale(effectiveLocale, "common.dateRangePicker.placeholder")

  const displayText = React.useMemo(() => {
    if (!value?.from) return resolvedPlaceholder
    if (!value.to) {
      return format(value.from, "yyyy-MM-dd")
    }
    return `${format(value.from, "yyyy-MM-dd")} ~ ${format(value.to, "yyyy-MM-dd")}`
  }, [resolvedPlaceholder, value])

  const presetLabels = React.useMemo(() => {
    return {
      today: translateForLocale(effectiveLocale, "common.dateRangePicker.presets.today"),
      yesterday: translateForLocale(effectiveLocale, "common.dateRangePicker.presets.yesterday"),
      week: translateForLocale(effectiveLocale, "common.dateRangePicker.presets.week"),
      month: translateForLocale(effectiveLocale, "common.dateRangePicker.presets.month"),
    } as const
  }, [effectiveLocale])

  const applyPreset = React.useCallback(
    (preset: "today" | "yesterday" | "week" | "month") => {
      const now = new Date()
      const range: DateRange =
        preset === "today"
          ? { from: now, to: now }
          : preset === "yesterday"
            ? (() => {
                const date = subDays(now, 1)
                return { from: date, to: date }
              })()
            : preset === "week"
              ? { from: startOfWeek(now, { weekStartsOn: 1 }), to: now }
              : { from: startOfMonth(now), to: now }

      onChange?.(range)
      setOpen(false)
      setPhase("start")
    },
    [onChange]
  )

  const handleSelect = React.useCallback(
    (range: DateRange | undefined, selectedDay?: Date) => {
      if (!selectedDay) {
        onChange?.(range)
        return
      }

      if (phase === "start") {
        onChange?.({ from: selectedDay, to: undefined })
        setPhase("end")
        return
      }

      if (!range?.from) {
        onChange?.({ from: selectedDay, to: undefined })
        setPhase("end")
        return
      }

      onChange?.(range)
      if (range.to) {
        setOpen(false)
        setPhase("start")
      } else {
        setPhase("end")
      }
    },
    [onChange, phase]
  )

  return (
    <Popover
      open={open}
      onOpenChange={(nextOpen) => {
        setOpen(nextOpen)
        if (!nextOpen) return

        const from = value?.from ?? new Date()
        const fromMonth = startOfMonth(from)
        const rawTo = value?.to ?? null
        const toMonth = rawTo ? startOfMonth(rawTo) : addMonths(fromMonth, 1)

        setStartMonth(fromMonth)
        setEndMonth(
          isSameMonth(fromMonth, toMonth) ? addMonths(fromMonth, 1) : toMonth
        )
        setPhase(value?.from && !value?.to ? "end" : "start")
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
        <div className="space-y-3 p-3">
          <div className="flex flex-col gap-3 sm:flex-row">
            <Calendar
              className="p-0"
              initialFocus
              mode="range"
              month={startMonth}
              onMonthChange={(month) => {
                setStartMonth(startOfMonth(month))
              }}
              selected={value}
              onSelect={handleSelect}
              locale={dateFnsLocale}
            />
            <Calendar
              className="p-0"
              mode="range"
              month={endMonth}
              onMonthChange={(month) => {
                setEndMonth(startOfMonth(month))
              }}
              selected={value}
              onSelect={handleSelect}
              locale={dateFnsLocale}
            />
          </div>

          <div className="flex flex-wrap gap-2 border-t pt-3">
            <Button
              type="button"
              size="sm"
              variant="secondary"
              onClick={() => applyPreset("today")}
              disabled={disabled}
            >
              {presetLabels.today}
            </Button>
            <Button
              type="button"
              size="sm"
              variant="secondary"
              onClick={() => applyPreset("yesterday")}
              disabled={disabled}
            >
              {presetLabels.yesterday}
            </Button>
            <Button
              type="button"
              size="sm"
              variant="secondary"
              onClick={() => applyPreset("week")}
              disabled={disabled}
            >
              {presetLabels.week}
            </Button>
            <Button
              type="button"
              size="sm"
              variant="secondary"
              onClick={() => applyPreset("month")}
              disabled={disabled}
            >
              {presetLabels.month}
            </Button>
          </div>
        </div>
      </PopoverContent>
    </Popover>
  )
}
