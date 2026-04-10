import React, { useEffect, useMemo, useState } from "react";
import { TrendingUp, Zap, DollarSign, ArrowRight } from "lucide-react";
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
import { StatCard } from "@/components/composed/stat-card";
import {
  TrendChart,
  type TrendChartDay,
  type TrendChartSeries,
} from "@/components/composed/trend-chart";
import { PageHeader } from "@/components/PageHeader";
import { useCurrency } from "@/hooks/use-currency";
import { useI18n } from "@/hooks/use-i18n";
import { humanizeApiError } from "@/lib/error";
import { formatDecimal, formatMoney, parseDecimalLike } from "@/providers/currency-provider";
import { getSettings, listChannels, statsChannels, statsSummary, statsTrend } from "@/api";
import type { AppSettings, Channel, ChannelStats, Protocol, StatsSummary, TrendPoint } from "@/types/api";
import {
  formatNumber,
  protocolColor,
  protocolLabel,
} from "../../lib";
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
        toast.error(t("overview.toast.loadFail"), { description: humanizeApiError(e, t) });
      })
      .finally(() => setLoading(false));
  }, []);

  useEffect(() => {
    const timer = window.setInterval(() => setNowMs(Date.now()), 30_000);
    return () => window.clearInterval(timer);
  }, []);

  const enabledByProtocol = useMemo(() => {
    const by: Record<Protocol, Channel[]> = { openai: [], anthropic: [], gemini: [] };
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
      by[p].sort((a, b) => (b.priority ?? 0) - (a.priority ?? 0) || a.name.localeCompare(b.name));
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
    const palette = [
      protocolColor("openai"),
      protocolColor("anthropic"),
      protocolColor("gemini"),
      "hsl(var(--chart-4))",
      "hsl(var(--chart-5))",
      "hsl(var(--success))",
      "hsl(var(--warning))",
      "hsl(var(--destructive))",
    ];

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
      .sort((a, b) => b[1].total - a[1].total || a[1].name.localeCompare(b[1].name));

    const series: TrendChartSeries[] = used.map(([channel_id, meta], idx) => ({
      channel_id,
      name: meta.name,
      protocol: protocolById.get(channel_id) ?? null,
      color: palette[idx % palette.length]!,
      values: days.map((d) => byDayChannel.get(`${d.key}|${channel_id}`) ?? 0),
    }));

    return { days, series };
  }, [trendItems, stats?.start_ms, channels]);

  const protocolLabelText = (protocol: Protocol) => protocolLabel(t, protocol);

  return (
    <div className="space-y-4 pb-4">
      <PageHeader title={t("overview.title")} />

      {/* 核心指标卡片 */}
      <div className="grid gap-3 md:grid-cols-4">
        <StatCard
          icon={Zap}
          label={t("overview.cards.todayRequests")}
          value={stats?.requests ?? "-"}
          loading={loading}
          className="animate-fade-up"
        />
        <StatCard
          icon={TrendingUp}
          label={t("overview.cards.totalTokens")}
          value={formatNumber(stats?.total_tokens)}
          loading={loading}
          className="animate-fade-up anim-d1"
        />
        <StatCard
          icon={DollarSign}
          label={t("overview.cards.estimatedCost")}
          value={estimatedOfficialCost === null ? "-" : `$${formatDecimal(estimatedOfficialCost)}`}
          loading={loading}
          className="animate-fade-up anim-d2"
        />
        <StatCard
          icon={ArrowRight}
          label={t("overview.cards.actualSpend")}
          value={formatMoney(actualSpend, currency)}
          loading={loading}
          className="animate-fade-up anim-d3"
        />
      </div>

      {/* 趋势图 + 渠道分布 */}
      <div className="grid gap-3 md:grid-cols-4">
        {/* 本月请求趋势 */}
        <Card className="animate-fade-up anim-d2 flex flex-col md:col-span-3">
          <CardHeader className="py-3 px-3">
            <CardTitle className="text-sm">{t("overview.trend.title")}</CardTitle>
          </CardHeader>
          <CardContent className="px-3 pb-3 flex-1 flex flex-col">
            {loading ? (
              <div className="space-y-3">
                <Skeleton className="h-6 w-28" />
                <Skeleton className="h-[220px] w-full" />
              </div>
            ) : monthTrend.series.length === 0 ? (
              <p className="text-muted-foreground text-xs">
                {t("overview.trend.empty")}
              </p>
            ) : (
              <TrendChart
                days={monthTrend.days}
                series={monthTrend.series}
                protocolLabel={protocolLabelText}
              />
            )}
          </CardContent>
        </Card>

        {/* 渠道使用分布 */}
        <div className="relative md:col-span-1">
          <Card className="animate-fade-up anim-d3 md:absolute md:inset-0 flex flex-col overflow-hidden">
            <CardHeader className="py-3 px-3">
              <div className="flex items-center justify-between gap-2">
                <CardTitle className="text-sm">
                  {t("overview.distribution.title")}
                </CardTitle>
                <Tabs
                  value={distributionView}
                  onValueChange={(v) =>
                    setDistributionView(v === "usage" ? "usage" : "percent")
                  }
                >
                  <TabsList className="h-7 p-0.5">
                    <TabsTrigger value="percent" className="px-2 py-0.5 text-xs">
                      {t("overview.distribution.view.percent")}
                    </TabsTrigger>
                    <TabsTrigger value="usage" className="px-2 py-0.5 text-xs">
                      {t("overview.distribution.view.usage")}
                    </TabsTrigger>
                  </TabsList>
                </Tabs>
              </div>
            </CardHeader>
            <CardContent className="px-3 pb-3 flex-1 min-h-0 flex flex-col">
              {loading ? (
                <div className="space-y-3">
                  <Skeleton className="h-5 w-full" />
                  <Skeleton className="h-5 w-full" />
                  <Skeleton className="h-5 w-4/5" />
                </div>
              ) : channelStatsUsed.length === 0 ? (
                <p className="text-muted-foreground text-xs">
                  {t("overview.distribution.empty")}
                </p>
              ) : (
                <div className="flex-1 min-h-0 overflow-y-auto pr-1">
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
      </div>

      {/* 活跃渠道链 */}
      <Card className="animate-fade-up anim-d4">
        <CardHeader className="py-3 px-3">
          <CardTitle className="text-sm">{t("overview.activeChannels.title")}</CardTitle>
        </CardHeader>
        <CardContent className="px-3 pb-3">
          {loading ? (
            <div className="space-y-3">
              <Skeleton className="h-8 w-48" />
              <Skeleton className="h-8 w-60" />
              <Skeleton className="h-8 w-40" />
            </div>
          ) : !hasAnyEnabled ? (
            <p className="text-muted-foreground text-xs">
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
    </div>
  );
}
