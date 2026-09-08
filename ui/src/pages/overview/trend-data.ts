import type { TrendChartDay, TrendChartSeries } from "@/components/composed/trend-chart";
import { resolveChannelIdentity } from "@/lib/channel-display";
import type { Channel, TrendPoint } from "@/types/api";

export function localDateKey(date: Date): string {
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
}

export function buildMonthDays(startMs: number, end: Date): TrendChartDay[] {
  const current = new Date(startMs);
  current.setHours(0, 0, 0, 0);
  const lastDay = new Date(end);
  lastDay.setHours(0, 0, 0, 0);
  const year = current.getFullYear();
  const month = current.getMonth();
  const days: TrendChartDay[] = [];
  while (
    current.getTime() <= lastDay.getTime() &&
    current.getFullYear() === year && current.getMonth() === month
  ) {
    days.push({ key: localDateKey(current), label: String(current.getDate()) });
    current.setDate(current.getDate() + 1);
  }
  return days;
}

export function buildMonthTrend({
  startMs,
  now,
  items,
  channels,
  accountNames,
  colorForChannel,
}: {
  startMs: number;
  now: Date;
  items: TrendPoint[];
  channels: Channel[];
  accountNames: Readonly<Record<string, string>>;
  colorForChannel: (channelId: string) => string;
}): { days: TrendChartDay[]; series: TrendChartSeries[] } {
  const days = buildMonthDays(startMs, now);
  const dayKeys = new Set(days.map((day) => day.key));
  const byChannel = new Map<string, { name: string; total: number; days: Map<string, number> }>();
  for (const item of items) {
    const dayKey = localDateKey(new Date(item.bucket_start_ms));
    if (!dayKeys.has(dayKey)) continue;
    const channel = byChannel.get(item.channel_id) ?? {
      name: item.name, total: 0, days: new Map<string, number>(),
    };
    channel.total += item.success;
    channel.days.set(dayKey, (channel.days.get(dayKey) ?? 0) + item.success);
    byChannel.set(item.channel_id, channel);
  }
  const channelsById = new Map(channels.map((channel) => [channel.id, channel]));
  const series = [...byChannel.entries()]
    .filter(([, channel]) => channel.total > 0)
    .sort((a, b) => b[1].total - a[1].total || a[1].name.localeCompare(b[1].name))
    .map(([channel_id, item]): TrendChartSeries => {
      const identity = resolveChannelIdentity(channelsById.get(channel_id), accountNames, item.name);
      return {
        channel_id,
        name: identity.channelName,
        account_name: identity.accountName,
        color: colorForChannel(channel_id),
        values: days.map((day) => item.days.get(day.key) ?? 0),
      };
    });
  return { days, series };
}
