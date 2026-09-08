import React, { useEffect, useMemo, useRef, useState } from "react";
import type { ApexAxisChartSeries, ApexOptions } from "apexcharts";
import ReactApexChart from "react-apexcharts/core";
import "apexcharts/line";

export type TrendChartDay = {
  key: string;
  label: string;
};

export type TrendChartSeries = {
  channel_id: string;
  name: string;
  account_name: string;
  color: string;
  values: number[];
};

type TrendChartTooltipLabels = {
  empty: string;
  origin: string;
  name: string;
  channel: string;
  requests: string;
  omitted: (count: number) => string;
};

type TrendChartProps = {
  days: TrendChartDay[];
  series: TrendChartSeries[];
  originLabel: string;
  tooltipLabels: TrendChartTooltipLabels;
};

function resolveThemeMode(): "light" | "dark" {
  if (typeof document === "undefined") return "dark";
  return document.documentElement.classList.contains("dark") ? "dark" : "light";
}

const TREND_TOOLTIP_LIMIT = 6;

function escapeHtml(value: string): string {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");
}

export function TrendChart({
  days,
  series,
  originLabel,
  tooltipLabels,
}: TrendChartProps) {
  const chartRef = useRef<HTMLDivElement>(null);
  const [chartWidth, setChartWidth] = useState(640);
  const [themeMode, setThemeMode] = useState<"light" | "dark">(
    resolveThemeMode,
  );

  useEffect(() => {
    if (typeof document === "undefined") return;

    const updateThemeMode = () => setThemeMode(resolveThemeMode());
    updateThemeMode();

    const observer = new MutationObserver(updateThemeMode);
    observer.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["class"],
    });

    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    const element = chartRef.current;
    if (!element) return;
    const updateWidth = () => setChartWidth(element.clientWidth || 640);
    updateWidth();
    const observer = new ResizeObserver(updateWidth);
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  // The synthetic point belongs only to chart data, never to monthly statistics.
  const plotDays = useMemo(
    () => [{ key: "__origin__", label: originLabel }, ...days],
    [days, originLabel],
  );
  const integerFormat = useMemo(() => new Intl.NumberFormat(undefined, {
    maximumFractionDigits: 0,
  }), []);
  const yAxisWidth = useMemo(() => {
    const largest = series.reduce((max, item) => item.values.reduce(
      (value, next) => Math.max(value, next), max,
    ), 0);
    return Math.max(36, integerFormat.format(Math.ceil(largest * 1.2)).length * 7 + 12);
  }, [integerFormat, series]);
  const labelKeys = useMemo(() => {
    const capacity = Math.max(3, Math.floor((chartWidth - yAxisWidth) / 44));
    const step = Math.max(1, Math.ceil((plotDays.length - 1) / (capacity - 1)));
    return new Set(plotDays.filter((_, index) =>
      index === 0 || index === 1 || index === plotDays.length - 1 || index % step === 0,
    ).map((day) => day.key));
  }, [chartWidth, plotDays, yAxisWidth]);
  const dayByKey = useMemo(
    () => new Map(plotDays.map((day) => [day.key, day] as const)),
    [plotDays],
  );
  const chartSeries = useMemo<ApexAxisChartSeries>(
    () => series.map((item) => ({
      // ApexCharts also uses names as identity; equal channel labels must stay separate.
      name: item.channel_id,
      data: [0, ...item.values],
    })),
    [series],
  );

  const options = useMemo<ApexOptions>(
    () => ({
      chart: {
        type: "line",
        background: "transparent",
        fontFamily: "inherit",
        foreColor: "oklch(var(--muted-foreground))",
        toolbar: { show: false },
        zoom: { enabled: false },
        parentHeightOffset: 0,
        animations: {
          enabled: true,
          speed: 320,
          dynamicAnimation: {
            enabled: true,
            speed: 240,
          },
        },
      },
      theme: {
        mode: themeMode,
      },
      colors: series.map((item) => item.color),
      stroke: {
        curve: "straight",
        lineCap: "round",
        width: 2,
      },
      markers: {
        size: 0,
        discrete: days.length === 1 ? series.map((item, seriesIndex) => ({
          seriesIndex,
          dataPointIndex: 1,
          fillColor: item.color,
          strokeColor: "oklch(var(--card))",
          size: 4,
        })) : [],
        hover: {
          size: 4,
          sizeOffset: 2,
        },
      },
      dataLabels: {
        enabled: false,
      },
      legend: {
        show: false,
      },
      states: {
        hover: {
          filter: {
            type: "none",
          },
        },
        active: {
          filter: {
            type: "none",
          },
        },
      },
      grid: {
        borderColor: "oklch(var(--border))",
        strokeDashArray: 0,
        xaxis: {
          lines: {
            show: false,
          },
        },
        padding: {
          top: 6,
          right: 8,
          bottom: 0,
          left: 2,
        },
      },
      xaxis: {
        type: "category",
        categories: plotDays.map((day) => day.key),
        tickPlacement: "on",
        axisBorder: {
          show: false,
        },
        axisTicks: {
          show: false,
        },
        crosshairs: {
          stroke: {
            color: "oklch(var(--border))",
            width: 1,
            dashArray: 0,
          },
        },
        labels: {
          rotate: 0,
          hideOverlappingLabels: true,
          trim: false,
          style: {
            colors: "oklch(var(--muted-foreground))",
            fontSize: "10px",
            fontWeight: 500,
          },
          formatter(value) {
            const key = String(value);
            return labelKeys.has(key) ? (dayByKey.get(key)?.label ?? "") : "";
          },
        },
        tooltip: {
          enabled: false,
        },
      },
      yaxis: {
        min: 0,
        tickAmount: 2,
        forceNiceScale: true,
        labels: {
          minWidth: yAxisWidth,
          maxWidth: yAxisWidth,
          style: {
            colors: "oklch(var(--muted-foreground))",
            fontSize: "10px",
            fontWeight: 500,
          },
          formatter(value) {
            return integerFormat.format(value);
          },
        },
      },
      tooltip: {
        shared: true,
        intersect: false,
        fillSeriesColor: false,
        theme: themeMode,
        style: {
          fontSize: "11px",
          fontFamily: "inherit",
        },
        x: {
          formatter(value) {
            return dayByKey.get(String(value))?.key ?? String(value);
          },
        },
        y: {
          formatter(value) {
            return integerFormat.format(value);
          },
        },
        custom({ dataPointIndex }: { dataPointIndex: number }) {
          if (dataPointIndex === 0) {
            return `<div style="padding:8px 10px;color:oklch(var(--muted-foreground));background:oklch(var(--card));">${escapeHtml(tooltipLabels.origin)}</div>`;
          }
          const realIndex = dataPointIndex - 1;
          const hoveredDay = days[realIndex];
          if (!hoveredDay) return "";
          const rows = series.map((item) => ({
            ...item, value: item.values[realIndex] ?? 0,
          })).filter((item) => item.value > 0)
            .sort((a, b) => b.value - a.value || a.account_name.localeCompare(b.account_name));
          const visibleRows = rows.slice(0, TREND_TOOLTIP_LIMIT);
          const rowsMarkup = visibleRows.length === 0
            ? `<tr><td colspan="3" style="padding:6px 0;color:oklch(var(--muted-foreground));">${escapeHtml(tooltipLabels.empty)}</td></tr>`
            : visibleRows.map((item) => `<tr>
  <td style="padding:4px 8px 4px 0;overflow-wrap:anywhere;"><span style="display:inline-block;width:7px;height:7px;margin-right:5px;border-radius:50%;background:${item.color};"></span>${escapeHtml(item.account_name)}</td>
  <td style="padding:4px 8px 4px 0;color:oklch(var(--muted-foreground));overflow-wrap:anywhere;">${escapeHtml(item.name)}</td>
  <td style="padding:4px 0;text-align:right;font-variant-numeric:tabular-nums;white-space:nowrap;">${integerFormat.format(item.value)}</td>
</tr>`).join("");
          const omittedMarkup = rows.length > TREND_TOOLTIP_LIMIT
            ? `<div style="padding-top:5px;color:oklch(var(--muted-foreground));">${escapeHtml(tooltipLabels.omitted(rows.length - TREND_TOOLTIP_LIMIT))}</div>` : "";
          return `<div style="width:340px;max-width:min(440px,80vw);border:1px solid oklch(var(--border));border-radius:8px;background:oklch(var(--card));color:oklch(var(--foreground));padding:9px 11px;box-shadow:0 5px 18px oklch(0% 0 0 / 0.12);font-size:11px;">
  <div style="padding-bottom:5px;font-weight:500;">${escapeHtml(hoveredDay.key)}</div>
  <table style="width:100%;table-layout:fixed;border-collapse:collapse;line-height:1.4;">
    <colgroup><col style="width:28%"><col style="width:40%"><col style="width:32%"></colgroup>
    <thead><tr style="border-bottom:1px solid oklch(var(--border));color:oklch(var(--muted-foreground));">
      <th style="padding:2px 8px 5px 0;text-align:left;font-weight:400;">${escapeHtml(tooltipLabels.name)}</th>
      <th style="padding:2px 8px 5px 0;text-align:left;font-weight:400;">${escapeHtml(tooltipLabels.channel)}</th>
      <th style="padding:2px 0 5px;text-align:right;font-weight:400;">${escapeHtml(tooltipLabels.requests)}</th>
    </tr></thead><tbody>${rowsMarkup}</tbody>
  </table>${omittedMarkup}
</div>`;
        },
      },
      noData: {
        text: "",
      },
    }),
    [dayByKey, days, integerFormat, labelKeys, plotDays, series, themeMode, tooltipLabels, yAxisWidth],
  );

  return (
    <div ref={chartRef} className="flex h-full min-w-0 flex-col space-y-2">
      <div className="min-h-[220px] w-full flex-1">
        <ReactApexChart
          key={themeMode}
          type="line"
          options={options}
          series={chartSeries}
          width="100%"
          height="100%"
        />
      </div>

      {series.length > 0 ? (
        <div className="mt-1 max-h-[60px] shrink-0 overflow-y-auto border-t border-border pt-1.5">
          <div className="flex flex-wrap gap-x-3 gap-y-1.5 text-[10px] text-muted-foreground">
            {series.map((item) => (
              <div
                key={item.channel_id}
                className="flex min-w-0 max-w-full items-center gap-1.5"
                title={`${item.account_name} · ${item.name}`}
              >
                <span
                  className="inline-block h-2 w-2 shrink-0 rounded-full"
                  style={{ background: item.color }}
                />
                <span className="max-w-[260px] truncate whitespace-nowrap">
                  <span className="font-medium text-foreground">{item.account_name}</span>
                  <span className="text-muted-foreground"> · {item.name}</span>
                </span>
              </div>
            ))}
          </div>
        </div>
      ) : null}
    </div>
  );
}
