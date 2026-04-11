import React, { useEffect, useMemo, useState } from "react";
import { toast } from "sonner";

import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  Skeleton,
  Tabs,
  TabsList,
  TabsTrigger,
} from "@/components/ui";
import { MetricCard } from "@/components/composed/metric-card";
import {
  TrendChart,
  type TrendChartDay,
  type TrendChartSeries,
} from "@/components/composed/trend-chart";
import { PageHeader } from "@/components/PageHeader";
import { PageBody } from "@/components/layout/page-body";
import {
  getSettings,
  listChannels,
  statsChannels,
  statsSummary,
  statsTrend,
} from "@/api";
import { useCurrency } from "@/hooks/use-currency";
import { useI18n } from "@/hooks/use-i18n";
import { humanizeApiError } from "@/lib/error";
import {
  formatDecimal,
  formatMoney,
  parseDecimalLike,
} from "@/providers/currency-provider";
import type {
  AppSettings,
  Channel,
  ChannelStats,
  Protocol,
  StatsSummary,
  TrendPoint,
} from "@/types/api";
import { formatNumber, protocolLabel } from "../../lib";
import { ActiveChannelChain } from "./active-channel-chain";
import { ChannelDistribution } from "./channel-distribution";

function localDateKey(d: Date): string {
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${y}-${m}-${day}`;
}

function buildMonthDays(startMs: number, end: Date): TrendChartDay[] {
  const out: TrendChartDay[] = [];
  const cur = new Date(startMs);
  cur.setHours(0, 0, 0, 0);
  const endLocal = new Date(end);
  endLocal.setHours(0, 0, 0, 0);

  while (cur.getTime() <= endLocal.getTime()) {
    out.push({ key: localDateKey(cur), label: String(cur.getDate()) });
    cur.setDate(cur.getDate() + 1);
  }
  return out;
}

const trendPalette = [
  "oklch(var(--chart-1))",
  "oklch(var(--chart-2))",
  "oklch(var(--chart-3))",
  "oklch(var(--chart-4))",
  "oklch(var(--chart-5))",
  "oklch(var(--chart-6))",
  "oklch(var(--chart-7))",
  "oklch(var(--chart-8))",
  "oklch(var(--chart-9))",
  "oklch(var(--chart-10))",
] as const;

function pickTrendColor(channelId: string): string {
  let hash = 0;
  for (let index = 0; index < channelId.length; index += 1) {
    hash = (hash * 33 + channelId.charCodeAt(index)) >>> 0;
  }
  return trendPalette[hash % trendPalette.length]!;
}

export function OverviewPage() {
  const { t } = useI18n();
  const { currency } = useCurrency();
  const [appSettings, setAppSettings] = useState<AppSettings | null>(null);
  const [channels, setChannels] = useState<Channel[]>([]);
  const [stats, setStats] = useState<StatsSummary | null>(null);
  const [channelStats, setChannelStats] = useState<ChannelStats[]>([]);
  const [trendItems, setTrendItems] = useState<TrendPoint[]>([]);
  const [loading, setLoading] = useState(true);
  const [nowMs, setNowMs] = useState(() => Date.now());
  const [distributionView, setDistributionView] = useState<"percent" | "usage">(
    "percent",
  );

  useEffect(() => {
    Promise.all([
      listChannels(),
      getSettings().catch(() => null),
      statsSummary({ range: "month" }),
      statsChannels({ range: "month" }),
      statsTrend("month"),
    ])
      .then(([cs, settings, st, cst, tr]) => {
        setChannels(cs);
        if (settings) {
          setAppSettings(settings);
        }
        setStats(st);
        setChannelStats(cst.items);
        setTrendItems(tr.items);
      })
      .catch((e) => {
        toast.error(t("overview.toast.loadFail"), {
          description: humanizeApiError(e, t),
        });
      })
      .finally(() => setLoading(false));
  }, []);

  useEffect(() => {
    const timer = window.setInterval(() => setNowMs(Date.now()), 30_000);
    return () => window.clearInterval(timer);
  }, []);

  const enabledByProtocol = useMemo(() => {
    const by: Record<Protocol, Channel[]> = {
      openai: [],
      anthropic: [],
      gemini: [],
    };
    for (const c of channels) {
      if (!c.enabled) continue;
      const blockedByProtection =
        (appSettings?.auto_disable_enabled ?? false) &&
        !c.ignore_channel_protection &&
        (c.auto_disabled_until_ms ?? 0) > nowMs;
      if (!blockedByProtection) {
        by[c.protocol].push(c);
      }
    }
    for (const p of Object.keys(by) as Protocol[]) {
      by[p].sort(
        (a, b) =>
          (b.priority ?? 0) - (a.priority ?? 0) || a.name.localeCompare(b.name),
      );
    }
    return by;
  }, [appSettings?.auto_disable_enabled, channels, nowMs]);

  const hasAnyEnabled = useMemo(
    () =>
      enabledByProtocol.openai.length > 0 ||
      enabledByProtocol.anthropic.length > 0 ||
      enabledByProtocol.gemini.length > 0,
    [enabledByProtocol],
  );

  const actualSpend = useMemo(() => {
    if (!channelStats.length || !channels.length) return null;
    const byId = new Map(channels.map((c) => [c.id, c] as const));
    let sum = 0;
    let hasAny = false;
    for (const s of channelStats) {
      const est = parseDecimalLike(s.estimated_cost_usd);
      if (!est || est <= 0) continue;
      const ch = byId.get(s.channel_id);
      const real = Number(ch?.real_multiplier ?? 1);
      if (!Number.isFinite(real) || real < 0) continue;
      hasAny = true;
      sum += est * real;
    }
    return hasAny ? sum : null;
  }, [channels, channelStats]);

  const estimatedOfficialCost = useMemo(
    () => parseDecimalLike(stats?.estimated_cost_usd),
    [stats?.estimated_cost_usd],
  );

  const channelStatsUsed = useMemo(
    () => channelStats.filter((s) => s.success > 0),
    [channelStats],
  );

  const monthTrend = useMemo(() => {
    const startMs = stats?.start_ms ?? Date.now();
    const days = buildMonthDays(startMs, new Date());
    const byDayChannel = new Map<string, number>();
    for (const it of trendItems) {
      const k = `${localDateKey(new Date(it.bucket_start_ms))}|${it.channel_id}`;
      byDayChannel.set(k, (byDayChannel.get(k) ?? 0) + it.success);
    }

    const totals = new Map<string, { name: string; total: number }>();
    for (const it of trendItems) {
      const cur = totals.get(it.channel_id);
      totals.set(it.channel_id, {
        name: it.name,
        total: (cur?.total ?? 0) + it.success,
      });
    }

    const protocolById = new Map<string, Protocol>();
    for (const c of channels) protocolById.set(c.id, c.protocol);

    const used = [...totals.entries()]
      .filter(([, v]) => v.total > 0)
      .sort(
        (a, b) => b[1].total - a[1].total || a[1].name.localeCompare(b[1].name),
      );

    const series: TrendChartSeries[] = used.map(([channel_id, meta]) => ({
      channel_id,
      name: meta.name,
      protocol: protocolById.get(channel_id) ?? null,
      color: pickTrendColor(channel_id),
      values: days.map((d) => byDayChannel.get(`${d.key}|${channel_id}`) ?? 0),
    }));

    return { days, series };
  }, [trendItems, stats?.start_ms, channels]);

  const protocolLabelText = (protocol: Protocol) => protocolLabel(t, protocol);
  const trendTooltipLabels = useMemo(
    () => ({
      empty: t("overview.trend.tooltip.empty"),
      omitted: (count: number) => t("overview.trend.tooltip.omitted", { count }),
    }),
    [t],
  );

  return (
    <div className="flex h-full min-h-0 flex-col">
      <PageHeader title={t("overview.title")} />
      <div className="flex-1 overflow-y-auto">
        <PageBody className="space-y-4">
          <div className="grid gap-4 md:grid-cols-4">
            <MetricCard
              label={t("overview.cards.todayRequests")}
              value={stats?.requests ?? "-"}
              barColor="bg-primary"
              loading={loading}
              className="animate-fade-up"
            />
            <MetricCard
              label={t("overview.cards.totalTokens")}
              value={formatNumber(stats?.total_tokens)}
              barColor="bg-muted-foreground/45"
              loading={loading}
              className="animate-fade-up [animation-delay:60ms]"
            />
            <MetricCard
              label={t("overview.cards.estimatedCost")}
              value={
                estimatedOfficialCost === null
                  ? "-"
                  : `$${formatDecimal(estimatedOfficialCost)}`
              }
              barColor="bg-warning"
              loading={loading}
              className="animate-fade-up [animation-delay:120ms]"
            />
            <MetricCard
              label={t("overview.cards.actualSpend")}
              value={formatMoney(actualSpend, currency)}
              barColor="bg-success"
              loading={loading}
              className="animate-fade-up [animation-delay:180ms]"
            />
          </div>

          <div className="grid gap-4 md:h-[360px] md:grid-cols-4">
            <Card className="animate-fade-up flex flex-col md:col-span-3">
              <CardHeader className="px-4 pt-3.5 pb-2.5">
                <CardTitle>{t("overview.trend.title")}</CardTitle>
              </CardHeader>
              <CardContent className="flex min-h-0 flex-1 flex-col px-4 pb-2.5">
                {loading ? (
                  <div className="space-y-3">
                    <Skeleton className="h-6 w-28" />
                    <Skeleton className="h-[220px] w-full" />
                  </div>
                ) : monthTrend.series.length === 0 ? (
                  <p className="text-xs text-muted-foreground">
                    {t("overview.trend.empty")}
                  </p>
                ) : (
                  <TrendChart
                    days={monthTrend.days}
                    series={monthTrend.series}
                    protocolLabel={protocolLabelText}
                    tooltipLabels={trendTooltipLabels}
                  />
                )}
              </CardContent>
            </Card>

            <Card className="animate-fade-up flex flex-col overflow-hidden [animation-delay:60ms]">
              <CardHeader className="px-4 pt-3.5 pb-2.5">
                <div className="flex items-center justify-between gap-3">
                  <CardTitle className="shrink-0 self-center whitespace-nowrap">
                    {t("overview.distribution.title")}
                  </CardTitle>
                  <Tabs
                    value={distributionView}
                    onValueChange={(value) =>
                      setDistributionView(
                        value === "usage" ? "usage" : "percent",
                      )
                    }
                  >
                    <TabsList className="shrink-0 self-center">
                      <TabsTrigger
                        value="percent"
                        className="text-[10px]"
                      >
                        {t("overview.distribution.view.percent")}
                      </TabsTrigger>
                      <TabsTrigger
                        value="usage"
                        className="text-[10px]"
                      >
                        {t("overview.distribution.view.usage")}
                      </TabsTrigger>
                    </TabsList>
                  </Tabs>
                </div>
              </CardHeader>
              <CardContent className="flex min-h-0 flex-1 flex-col px-4 pb-2.5">
                {loading ? (
                  <div className="space-y-3">
                    <Skeleton className="h-5 w-full" />
                    <Skeleton className="h-5 w-full" />
                    <Skeleton className="h-5 w-4/5" />
                  </div>
                ) : channelStatsUsed.length === 0 ? (
                  <p className="text-xs text-muted-foreground">
                    {t("overview.distribution.empty")}
                  </p>
                ) : (
                  <div className="min-h-0 flex-1 overflow-y-auto">
                    <ChannelDistribution
                      stats={channelStatsUsed}
                      protocolLabel={protocolLabelText}
                      view={distributionView}
                    />
                  </div>
                )}
              </CardContent>
            </Card>
          </div>

          <Card className="animate-fade-up px-4 py-3.5 [animation-delay:120ms]">
            <CardHeader className="mb-2.5 p-0">
              <CardTitle>{t("overview.activeChannels.title")}</CardTitle>
            </CardHeader>
            <CardContent className="p-0">
              {loading ? (
                <div className="space-y-3">
                  <Skeleton className="h-8 w-48" />
                  <Skeleton className="h-8 w-60" />
                  <Skeleton className="h-8 w-40" />
                </div>
              ) : !hasAnyEnabled ? (
                <p className="text-xs text-muted-foreground">
                  {t("overview.activeChannels.empty")}
                </p>
              ) : (
                <ActiveChannelChain
                  enabledByProtocol={enabledByProtocol}
                  settings={appSettings}
                  protocolLabel={protocolLabelText}
                />
              )}
            </CardContent>
          </Card>
        </PageBody>
      </div>
    </div>
  );
}
