import { Badge } from "@/components/ui";
import type { Protocol } from "@/types/api";

import { protocolBadgeClassName } from "../../lib";

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

type TrendChartProps = {
  days: TrendChartDay[];
  series: TrendChartSeries[];
  protocolLabel: (protocol: Protocol) => string;
};

export function TrendChart({ days, series, protocolLabel }: TrendChartProps) {
  const width = 640;
  const height = 240;
  const padLeft = 34;
  const padRight = 10;
  const padTop = 10;
  const padBottom = 24;
  const plotW = width - padLeft - padRight;
  const plotH = height - padTop - padBottom;

  const maxCount = Math.max(1, ...series.flatMap((s) => s.values));
  const yTicks = [0, Math.round(maxCount / 2), maxCount].filter(
    (value, index, all) => all.indexOf(value) === index,
  );

  const labelIndices =
    days.length <= 10
      ? new Set(days.map((_, index) => index))
      : new Set([0, Math.floor((days.length - 1) / 2), days.length - 1]);

  const xFor = (index: number) => {
    if (days.length <= 1) {
      return padLeft;
    }
    return padLeft + (index / (days.length - 1)) * plotW;
  };
  const yFor = (value: number) => padTop + plotH - (value / maxCount) * plotH;

  return (
    <div className="flex h-full flex-col space-y-2">
      <div className="min-h-[220px] w-full flex-1">
        <svg className="h-full w-full" viewBox={`0 0 ${width} ${height}`}>
          {yTicks.map((value) => {
            const y = yFor(value);
            return (
              <g key={value}>
                <line
                  x1={padLeft}
                  y1={y}
                  x2={width - padRight}
                  y2={y}
                  stroke="hsl(var(--border))"
                  strokeWidth="1"
                />
                <text
                  x={padLeft - 6}
                  y={y}
                  textAnchor="end"
                  dominantBaseline="middle"
                  fontSize="10"
                  fill="hsl(var(--muted-foreground))"
                >
                  {value}
                </text>
              </g>
            );
          })}

          {days.map((day, index) => {
            if (!labelIndices.has(index)) {
              return null;
            }
            return (
              <text
                key={day.key}
                x={xFor(index)}
                y={height - 8}
                textAnchor="middle"
                fontSize="10"
                fill="hsl(var(--muted-foreground))"
              >
                {day.label}
              </text>
            );
          })}

          {series.map((item) => {
            const path = item.values
              .map((value, index) => `${index === 0 ? "M" : "L"} ${xFor(index)} ${yFor(value)}`)
              .join(" ");
            return (
              <g key={item.channel_id}>
                <path
                  d={path}
                  fill="none"
                  stroke={item.color}
                  strokeWidth="2"
                  strokeLinejoin="round"
                  strokeLinecap="round"
                />
              </g>
            );
          })}
        </svg>
      </div>

      {series.length > 0 ? (
        <div className="flex flex-wrap gap-x-3 gap-y-1 text-[10px] text-muted-foreground">
          {series.map((item) => (
            <div key={item.channel_id} className="flex min-w-0 items-center gap-1.5">
              {item.protocol ? (
                <Badge
                  variant="outline"
                  className={`px-1 py-0 text-[10px] ${protocolBadgeClassName(item.protocol)}`}
                >
                  {protocolLabel(item.protocol)}
                </Badge>
              ) : null}
              <span
                className="inline-block h-2 w-2 shrink-0 rounded-sm"
                style={{ background: item.color }}
              />
              <span className="max-w-[160px] truncate">{item.name}</span>
            </div>
          ))}
        </div>
      ) : null}
    </div>
  );
}
