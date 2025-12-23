import React, { useEffect, useMemo, useState } from "react";
import { TrendingUp, Zap, DollarSign, ArrowRight } from "lucide-react";
import { toast } from "sonner";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
  Badge,
  Tabs,
  TabsList,
  TabsTrigger,
} from "@/components/ui";
import { useI18n } from "@/lib/i18n";
import { useCurrency, formatMoney, parseDecimalLike } from "@/lib/currency";
import {
  listChannels,
  statsSummary,
  statsChannels,
  statsTrend,
  type Channel,
  type Protocol,
  type StatsSummary,
  type ChannelStats,
  type TrendPoint,
} from "../api";
import { protocolLabel, protocolLabelKey } from "../lib";

type TrendDay = { key: string; label: string };
type TrendSeries = {
  channel_id: string;
  name: string;
  protocol: Protocol | null;
  color: string;
  values: number[];
};

function localDateKey(d: Date): string {
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${y}-${m}-${day}`;
}

function buildMonthDays(startMs: number, end: Date): TrendDay[] {
  const out: TrendDay[] = [];
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

function MultiLineTrendChart({
  days,
  series,
  protocolLabel,
}: {
  days: TrendDay[];
  series: TrendSeries[];
  protocolLabel: (protocol: Protocol) => string;
}) {
  const width = 640;
  const height = 240;
  const padLeft = 34;
  const padRight = 10;
  const padTop = 10;
  const padBottom = 24;
  const plotW = width - padLeft - padRight;
  const plotH = height - padTop - padBottom;

  const maxCount = Math.max(1, ...series.flatMap((s) => s.values));
  const yTicks = [0, Math.round(maxCount / 2), maxCount].filter((v, idx, arr) => arr.indexOf(v) === idx);

  const labelIndices =
    days.length <= 10
      ? new Set(days.map((_, idx) => idx))
      : new Set([0, Math.floor((days.length - 1) / 2), days.length - 1]);

  const xFor = (idx: number) => {
    if (days.length <= 1) return padLeft;
    return padLeft + (idx / (days.length - 1)) * plotW;
  };
  const yFor = (v: number) => padTop + plotH - (v / maxCount) * plotH;

  return (
    <div className="space-y-2 h-full flex flex-col">
      <div className="w-full flex-1 min-h-[220px]">
        <svg className="w-full h-full" viewBox={`0 0 ${width} ${height}`}>
          {/* grid + y axis */}
          {yTicks.map((v) => {
            const y = yFor(v);
            return (
              <g key={v}>
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
                  {v}
                </text>
              </g>
            );
          })}

          {/* x labels */}
          {days.map((d, idx) => {
            if (!labelIndices.has(idx)) return null;
            const x = xFor(idx);
            return (
              <text
                key={d.key}
                x={x}
                y={height - 8}
                textAnchor="middle"
                fontSize="10"
                fill="hsl(var(--muted-foreground))"
              >
                {d.label}
              </text>
            );
          })}

          {/* series */}
          {series.map((s) => {
            const d = s.values
              .map((v, idx) => `${idx === 0 ? "M" : "L"} ${xFor(idx)} ${yFor(v)}`)
              .join(" ");
            return (
              <g key={s.channel_id}>
                <path
                  d={d}
                  fill="none"
                  stroke={s.color}
                  strokeWidth="2"
                  strokeLinejoin="round"
                  strokeLinecap="round"
                />
              </g>
            );
          })}
        </svg>
      </div>

      {series.length > 0 && (
        <div className="flex flex-wrap gap-x-3 gap-y-1 text-[10px] text-muted-foreground">
          {series.map((s) => (
            <div key={s.channel_id} className="flex items-center gap-1.5 min-w-0">
              {s.protocol && (
                <Badge variant="outline" className="text-[10px] px-1 py-0">
                  {protocolLabel(s.protocol)}
                </Badge>
              )}
              <span
                className="inline-block h-2 w-2 rounded-sm shrink-0"
                style={{ background: s.color }}
              />
              <span className="max-w-[160px] truncate">{s.name}</span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function ChannelDistribution({
  stats,
  protocolLabel,
  view,
}: {
  stats: ChannelStats[];
  protocolLabel: (protocol: Protocol) => string;
  view: "percent" | "usage";
}) {
  const total = stats.reduce((sum, s) => sum + s.success, 0);
  if (total === 0) return null;

  const sorted = [...stats].sort((a, b) => b.success - a.success);

  return (
    <div className="space-y-2">
      {sorted.map((s) => {
        const percent = Math.round((s.success / total) * 100);
        return (
          <div key={s.channel_id} className="space-y-1">
            <div className="flex items-center justify-between text-xs">
              <div className="min-w-0 flex items-center gap-2 font-medium">
                <Badge variant="outline" className="text-[10px] px-1 py-0">
                  {protocolLabel(s.protocol)}
                </Badge>
                <span className="truncate">{s.name}</span>
              </div>
              <span className="text-muted-foreground ml-2">
                {view === "percent" ? `${percent}%` : s.success.toLocaleString()}
              </span>
            </div>
            <div className="h-2 bg-muted rounded-full overflow-hidden">
              <div
                className="h-full bg-primary rounded-full transition-all"
                style={{ width: `${percent}%` }}
              />
            </div>
          </div>
        );
      })}
    </div>
  );
}

export function OverviewPage() {
  const { t } = useI18n();
  const { currency } = useCurrency();
  const [channels, setChannels] = useState<Channel[]>([]);
  const [stats, setStats] = useState<StatsSummary | null>(null);
  const [channelStats, setChannelStats] = useState<ChannelStats[]>([]);
  const [trendItems, setTrendItems] = useState<TrendPoint[]>([]);
  const [loading, setLoading] = useState(true);
  const [distributionView, setDistributionView] = useState<"percent" | "usage">(
    "percent",
  );

  useEffect(() => {
    Promise.all([
      listChannels(),
      statsSummary("month"),
      statsChannels("month"),
      statsTrend("month"),
    ])
      .then(([cs, st, cst, tr]) => {
        setChannels(cs);
        setStats(st);
        setChannelStats(cst.items);
        setTrendItems(tr.items);
      })
      .catch((e) => {
        toast.error(t("overview.toast.loadFail"), { description: String(e) });
      })
      .finally(() => setLoading(false));
  }, []);

  const enabledByProtocol = useMemo(() => {
    const by: Record<Protocol, Channel[]> = { openai: [], anthropic: [], gemini: [] };
    for (const c of channels) {
      if (c.enabled) by[c.protocol].push(c);
    }
    for (const p of Object.keys(by) as Protocol[]) {
      by[p].sort((a, b) => (b.priority ?? 0) - (a.priority ?? 0) || a.name.localeCompare(b.name));
    }
    return by;
  }, [channels]);

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
    for (const s of channelStats) {
      const est = parseDecimalLike(s.estimated_cost_usd);
      if (!est || est <= 0) continue;
      const ch = byId.get(s.channel_id);
      const recharge = Number(ch?.recharge_multiplier ?? 1);
      const real = Number(ch?.real_multiplier ?? 1);
      if (!Number.isFinite(recharge) || recharge <= 0) continue;
      if (!Number.isFinite(real) || real <= 0) continue;
      sum += est * (real / recharge);
    }
    return sum > 0 ? sum : null;
  }, [channels, channelStats]);

  const channelStatsUsed = useMemo(
    () => channelStats.filter((s) => s.success > 0),
    [channelStats],
  );

  const monthTrend = useMemo(() => {
    const palette = [
      "hsl(var(--primary))",
      "hsl(var(--success))",
      "hsl(var(--warning))",
      "hsl(var(--destructive))",
      "hsl(190 80% 45%)",
      "hsl(260 60% 60%)",
      "hsl(330 70% 55%)",
      "hsl(30 90% 55%)",
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

    const series: TrendSeries[] = used.map(([channel_id, meta], idx) => ({
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
      {/* 页面标题 */}
      <div>
        <h1 className="text-lg font-semibold">{t("overview.title")}</h1>
        <p className="text-muted-foreground text-xs mt-0.5">
          {t("overview.subtitle")}
        </p>
      </div>

      {/* 核心指标卡片 */}
      <div className="grid gap-3 md:grid-cols-4">
        <Card>
          <CardHeader className="pb-1.5 pt-3 px-3">
            <CardDescription className="text-xs flex items-center gap-1">
              <Zap className="h-3 w-3" />
              {t("overview.cards.todayRequests")}
            </CardDescription>
          </CardHeader>
          <CardContent className="pb-3 px-3">
            <div className="text-xl font-bold">{stats?.requests ?? "-"}</div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="pb-1.5 pt-3 px-3">
            <CardDescription className="text-xs flex items-center gap-1">
              <TrendingUp className="h-3 w-3" />
              {t("overview.cards.totalTokens")}
            </CardDescription>
          </CardHeader>
          <CardContent className="pb-3 px-3">
            <div className="text-xl font-bold">
              {stats?.total_tokens?.toLocaleString() ?? "-"}
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="pb-1.5 pt-3 px-3">
            <CardDescription className="text-xs flex items-center gap-1">
              <DollarSign className="h-3 w-3" />
              {t("overview.cards.estimatedCost")}
            </CardDescription>
          </CardHeader>
          <CardContent className="pb-3 px-3">
            <div className="text-xl font-bold">
              ${stats?.estimated_cost_usd ?? "-"}
            </div>
          </CardContent>
        </Card>

        <Card>
            <CardHeader className="pb-1.5 pt-3 px-3">
              <CardDescription className="text-xs flex items-center gap-1">
              <ArrowRight className="h-3 w-3" />
              {t("overview.cards.actualSpend")}
            </CardDescription>
          </CardHeader>
          <CardContent className="pb-3 px-3">
            <div className="text-xl font-bold">{formatMoney(actualSpend, currency)}</div>
          </CardContent>
        </Card>
      </div>

      {/* 趋势图 + 渠道分布 */}
      <div className="grid gap-3 md:grid-cols-4">
        {/* 本月请求趋势 */}
        <Card className="flex flex-col md:col-span-3">
          <CardHeader className="py-3 px-3">
            <CardTitle className="text-sm">{t("overview.trend.title")}</CardTitle>
            <CardDescription className="text-xs">
              {t("overview.trend.subtitle")}
            </CardDescription>
          </CardHeader>
          <CardContent className="px-3 pb-3 flex-1 flex flex-col">
            {loading ? (
              <p className="text-muted-foreground text-xs">{t("common.loading")}</p>
            ) : monthTrend.series.length === 0 ? (
              <p className="text-muted-foreground text-xs">
                {t("overview.trend.empty")}
              </p>
            ) : (
              <MultiLineTrendChart
                days={monthTrend.days}
                series={monthTrend.series}
                protocolLabel={protocolLabelText}
              />
            )}
          </CardContent>
        </Card>

        {/* 渠道使用分布 */}
        <Card className="flex flex-col md:col-span-1">
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
            <CardDescription className="text-xs">
              {t("overview.distribution.subtitle")}
            </CardDescription>
          </CardHeader>
          <CardContent className="px-3 pb-3">
            {loading ? (
              <p className="text-muted-foreground text-xs">{t("common.loading")}</p>
            ) : channelStatsUsed.length === 0 ? (
              <p className="text-muted-foreground text-xs">
                {t("overview.distribution.empty")}
              </p>
            ) : (
              <div className="max-h-72 overflow-y-auto pr-1">
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

      {/* 活跃渠道链 */}
      <Card>
        <CardHeader className="py-3 px-3">
          <CardTitle className="text-sm">{t("overview.activeChannels.title")}</CardTitle>
          <CardDescription className="text-xs">
            {t("overview.activeChannels.subtitle")}
          </CardDescription>
        </CardHeader>
        <CardContent className="px-3 pb-3">
          {loading ? (
            <p className="text-muted-foreground text-xs">{t("common.loading")}</p>
          ) : !hasAnyEnabled ? (
            <p className="text-muted-foreground text-xs">
              {t("overview.activeChannels.empty")}
            </p>
          ) : (
            <div className="space-y-3">
              {(["openai", "anthropic", "gemini"] as Protocol[])
                .filter((p) => enabledByProtocol[p].length > 0)
                .map((p) => {
                  const list = enabledByProtocol[p];
                  return (
                    <div key={p} className="space-y-2">
                      <div className="flex items-center gap-2">
                        <Badge variant="outline" className="text-[10px] px-2 py-0.5">
                          {protocolLabel(t, p)}
                        </Badge>
                      </div>
                      <div className="flex flex-wrap items-center gap-2">
                        {list.map((c, idx) => (
                          <React.Fragment key={c.id}>
                            <div className="flex items-center gap-1.5 px-2 py-1 rounded border bg-card">
                              <Badge variant="outline" className="text-[10px] px-1 py-0">
                                {idx + 1}
                              </Badge>
                              <span className="text-xs font-medium">{c.name}</span>
                            </div>
                            {idx < list.length - 1 && (
                              <ArrowRight className="h-3 w-3 text-muted-foreground" />
                            )}
                          </React.Fragment>
                        ))}
                      </div>
                    </div>
                  );
                })}
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
