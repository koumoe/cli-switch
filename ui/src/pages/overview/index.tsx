import React, { useEffect, useMemo, useRef, useState } from "react";
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
import { TrendChart } from "@/components/composed/trend-chart";
import { PageHeader } from "@/components/PageHeader";
import { PageBody } from "@/components/layout/page-body";
import {
  getSettings,
  listChannels,
  listRemoteAccounts,
  statsChannels,
  statsSummary,
  statsTrend,
} from "@/api";
import { useCurrency } from "@/hooks/use-currency";
import { useI18n } from "@/hooks/use-i18n";
import { useWindowEvent } from "@/hooks/use-window-event";
import { humanizeApiError } from "@/lib/error";
import {
  calculateEstimatedSpend,
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
import { buildMonthTrend, localDateKey } from "./trend-data";

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
  const { currency, usdToCnyRate } = useCurrency();
  const [appSettings, setAppSettings] = useState<AppSettings | null>(null);
  const [channels, setChannels] = useState<Channel[]>([]);
  const [accountNames, setAccountNames] = useState<Record<string, string>>({});
  const refreshId = useRef(0);
  const [stats, setStats] = useState<StatsSummary | null>(null);
  const [channelStats, setChannelStats] = useState<ChannelStats[]>([]);
  const [trendItems, setTrendItems] = useState<TrendPoint[]>([]);
  const [loading, setLoading] = useState(true);
  const [nowMs, setNowMs] = useState(() => Date.now());
  const [distributionView, setDistributionView] = useState<"percent" | "usage">(
    "percent",
  );

  async function refresh(showLoading = false) {
    const requestId = ++refreshId.current;
    if (showLoading) setLoading(true);
    try {
      const [cs, settings, st, cst, tr, accounts] = await Promise.all([
        listChannels(),
        getSettings().catch(() => null),
        statsSummary({ range: "month" }),
        statsChannels({ range: "month" }),
        statsTrend("month"),
        listRemoteAccounts().catch(() => []),
      ]);
      if (requestId !== refreshId.current) return;
      setChannels(cs);
      setAccountNames(
        Object.fromEntries(accounts.map((account) => [account.id, account.name])),
      );
      if (settings) setAppSettings(settings);
      setStats(st);
      setChannelStats(cst.items);
      setTrendItems(tr.items);
    } catch (error) {
      if (requestId !== refreshId.current) return;
      toast.error(t("overview.toast.loadFail"), {
        description: humanizeApiError(error, t),
      });
    } finally {
      if (requestId === refreshId.current) setLoading(false);
    }
  }

  const todayKey = localDateKey(new Date(nowMs));
  useEffect(() => {
    void refresh(true);
    return () => { refreshId.current += 1; };
  }, [todayKey]);

  useWindowEvent("cliswitch-accounts-changed", () => { void refresh(); });
  useWindowEvent("cliswitch-channels-changed", () => { void refresh(); });
  useWindowEvent("cliswitch-usage-changed", () => { void refresh(); });
  useWindowEvent("focus", () => {
    setNowMs(Date.now());
    void refresh();
  });

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
      if (!ch) continue;
      const real = Number(ch.real_multiplier ?? 1);
      if (!Number.isFinite(real) || real < 0) continue;
      const converted = calculateEstimatedSpend(
        est,
        real,
        ch.recharge_currency,
        currency,
        usdToCnyRate,
      );
      if (converted === null) return null;
      hasAny = true;
      sum += converted;
    }
    return hasAny ? sum : null;
  }, [channels, channelStats, currency, usdToCnyRate]);

  const estimatedOfficialCost = useMemo(
    () => parseDecimalLike(stats?.estimated_cost_usd),
    [stats?.estimated_cost_usd],
  );

  const channelStatsUsed = useMemo(
    () => channelStats.filter((s) => s.success > 0),
    [channelStats],
  );

  const monthTrend = useMemo(() => {
    const now = new Date(nowMs);
    return buildMonthTrend({
      startMs: stats?.start_ms ?? new Date(now.getFullYear(), now.getMonth(), 1).getTime(),
      now,
      items: trendItems,
      channels,
      accountNames,
      colorForChannel: pickTrendColor,
    });
  }, [trendItems, stats?.start_ms, channels, accountNames, todayKey]);

  const channelsById = useMemo(
    () => new Map(channels.map((channel) => [channel.id, channel])),
    [channels],
  );

  const protocolLabelText = (protocol: Protocol) => protocolLabel(t, protocol);
  const trendTooltipLabels = useMemo(
    () => ({
      empty: t("overview.trend.tooltip.empty"),
      origin: t("overview.trend.tooltip.origin"),
      name: t("overview.trend.tooltip.name"),
      channel: t("overview.trend.tooltip.channel"),
      requests: t("overview.trend.tooltip.requests"),
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
                    originLabel={t("overview.trend.origin")}
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
                      channelsById={channelsById}
                      accountNames={accountNames}
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
                  accountNames={accountNames}
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
