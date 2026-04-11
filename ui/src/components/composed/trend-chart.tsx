import React, { useEffect, useMemo, useState } from "react";
import type { ApexAxisChartSeries, ApexOptions } from "apexcharts";
import ReactApexChart from "react-apexcharts/core";
import "apexcharts/line";

import type { Protocol } from "@/types/api";

export type TrendChartDay = {
  key: string;
  label: string;
};

export type TrendChartSeries = {
  channel_id: string;
  name: string;
  protocol: Protocol | null;
  color: string;
  values: number[];
};

type TrendChartTooltipLabels = {
  empty: string;
  omitted: (count: number) => string;
};

type TrendChartProps = {
  days: TrendChartDay[];
  series: TrendChartSeries[];
  protocolLabel: (protocol: Protocol) => string;
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
  protocolLabel,
  tooltipLabels,
}: TrendChartProps) {
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

  const labelIndices = useMemo(
    () =>
      days.length <= 10
        ? new Set(days.map((_, index) => index))
        : new Set([0, Math.floor((days.length - 1) / 2), days.length - 1]),
    [days],
  );

  const dayByKey = useMemo(
    () => new Map(days.map((day) => [day.key, day] as const)),
    [days],
  );

  const chartSeries = useMemo<ApexAxisChartSeries>(
    () =>
      series.map((item) => ({
        name: item.name,
        data: item.values,
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
        categories: days.map((day) => day.key),
        tickPlacement: "between",
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
          hideOverlappingLabels: false,
          trim: false,
          style: {
            colors: "oklch(var(--muted-foreground))",
            fontSize: "10px",
            fontWeight: 500,
          },
          formatter(value, _timestamp, opts) {
            const index = opts?.dataPointIndex ?? -1;
            if (!labelIndices.has(index)) return "";
            return dayByKey.get(String(value))?.label ?? "";
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
          minWidth: 28,
          maxWidth: 28,
          style: {
            colors: "oklch(var(--muted-foreground))",
            fontSize: "10px",
            fontWeight: 500,
          },
          formatter(value) {
            return String(Math.round(value));
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
            return String(Math.round(value));
          },
        },
        custom({ dataPointIndex }: { dataPointIndex: number }) {
          const hoveredDay = days[dataPointIndex];
          if (!hoveredDay) return "";

          const rankedRows = series
            .map((item) => ({
              color: item.color,
              name: item.name,
              protocol: item.protocol ? protocolLabel(item.protocol) : "",
              value: item.values[dataPointIndex] ?? 0,
            }))
            .sort((a, b) => b.value - a.value || a.name.localeCompare(b.name));

          const nonZeroRows = rankedRows.filter((item) => item.value > 0);
          const sourceRows = nonZeroRows.length > 0 ? nonZeroRows : rankedRows;
          const visibleRows = sourceRows.slice(0, TREND_TOOLTIP_LIMIT);
          const omittedCount = sourceRows.length - visibleRows.length;

          const rowsMarkup =
            nonZeroRows.length === 0
              ? `<div style="padding: 4px 0 2px; color: oklch(var(--muted-foreground));">${escapeHtml(tooltipLabels.empty)}</div>`
              : visibleRows
                  .map((item) => {
                    const protocolMarkup = item.protocol
                      ? `<span style="margin-left: 6px; color: oklch(var(--muted-foreground));">${escapeHtml(item.protocol)}</span>`
                      : "";
                    return `<div style="display:flex; align-items:center; justify-content:space-between; gap:12px; padding: 3px 0;">
  <div style="display:flex; min-width:0; align-items:center; gap:8px;">
    <span style="display:inline-block; width:8px; height:8px; flex:none; border-radius:9999px; background:${item.color};"></span>
    <span style="min-width:0; overflow:hidden; text-overflow:ellipsis; white-space:nowrap;">
      <span style="color: oklch(var(--foreground)); font-weight:600;">${escapeHtml(item.name)}</span>${protocolMarkup}
    </span>
  </div>
  <span style="flex:none; color: oklch(var(--foreground)); font-weight:600;">${Math.round(item.value)}</span>
</div>`;
                  })
                  .join("");

          const omittedMarkup =
            nonZeroRows.length > 0 && omittedCount > 0
              ? `<div style="padding-top: 6px; color: oklch(var(--muted-foreground)); border-top: 1px solid oklch(var(--border));">${escapeHtml(tooltipLabels.omitted(omittedCount))}</div>`
              : "";

          return `<div style="min-width: 220px; max-width: 280px; border: 1px solid oklch(var(--border)); border-radius: 10px; background: oklch(var(--card)); padding: 10px 12px; box-shadow: 0 10px 30px oklch(0% 0 0 / 0.12);">
  <div style="padding-bottom: 8px; margin-bottom: 6px; border-bottom: 1px solid oklch(var(--border)); color: oklch(var(--foreground)); font-size: 12px; font-weight: 700;">${escapeHtml(hoveredDay.key)}</div>
  <div style="display:flex; flex-direction:column; gap:0; font-size: 11px; line-height: 1.35;">${rowsMarkup}</div>
  ${omittedMarkup}
</div>`;
        },
      },
      noData: {
        text: "",
      },
    }),
    [dayByKey, days, labelIndices, protocolLabel, series, themeMode, tooltipLabels],
  );

  return (
    <div className="flex h-full flex-col space-y-2">
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
                className="flex min-w-0 items-center gap-1.5"
              >
                <span
                  className="inline-block h-2 w-2 shrink-0 rounded-full"
                  style={{ background: item.color }}
                />
                <span className="max-w-[140px] truncate font-medium text-foreground">
                  {item.name}
                </span>
                {item.protocol ? (
                  <span className="truncate text-muted-foreground">
                    {protocolLabel(item.protocol)}
                  </span>
                ) : null}
              </div>
            ))}
          </div>
        </div>
      ) : null}
    </div>
  );
}
