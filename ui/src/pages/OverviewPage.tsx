import React, { useEffect, useMemo, useState } from "react";
import { TrendingUp, Zap, DollarSign, Clock, ArrowRight } from "lucide-react";
import { toast } from "sonner";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
  Badge,
} from "@/components/ui";
import { useI18n } from "@/lib/i18n";
import {
  listChannels,
  usageRecent,
  statsSummary,
  statsChannels,
  type Channel,
  type Protocol,
  type UsageEvent,
  type StatsSummary,
  type ChannelStats,
} from "../api";
import { terminalLabel } from "../lib";

type HourlyData = { hour: number; count: number };

function getTodayStartMs(now = new Date()): number {
  return new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime();
}

function aggregateByHour(events: UsageEvent[], startMs: number): HourlyData[] {
  const counts: HourlyData[] = Array.from({ length: 24 }, (_, hour) => ({ hour, count: 0 }));

  for (const e of events) {
    if (e.ts_ms >= startMs) {
      const hour = new Date(e.ts_ms).getHours();
      counts[hour] = { hour, count: counts[hour]!.count + 1 };
    }
  }

  return counts;
}

function TrendChart({
  data,
  getTitle,
}: {
  data: HourlyData[];
  getTitle: (hour: number, count: number) => string;
}) {
  const maxCount = Math.max(...data.map((d) => d.count), 1);
  const currentHour = new Date().getHours();

  return (
    <div className="flex items-end gap-[2px] h-16">
      {data.map((d) => {
        const height = (d.count / maxCount) * 100;
        const isCurrent = d.hour === currentHour;
        return (
          <div
            key={d.hour}
            className="flex-1 group relative"
            title={getTitle(d.hour, d.count)}
          >
            <div
              className={`w-full rounded-sm transition-all ${
                isCurrent
                  ? "bg-primary"
                  : d.count > 0
                  ? "bg-primary/40"
                  : "bg-muted"
              }`}
              style={{ height: `${Math.max(height, 4)}%` }}
            />
          </div>
        );
      })}
    </div>
  );
}

function ChannelDistribution({ stats }: { stats: ChannelStats[] }) {
  const total = stats.reduce((sum, s) => sum + s.requests, 0);
  if (total === 0) return null;

  const sorted = [...stats].sort((a, b) => b.requests - a.requests);

  return (
    <div className="space-y-2">
      {sorted.map((s) => {
        const percent = Math.round((s.requests / total) * 100);
        return (
          <div key={s.channel_id} className="space-y-1">
            <div className="flex items-center justify-between text-xs">
              <span className="font-medium truncate">{s.name}</span>
              <span className="text-muted-foreground ml-2">
                {percent}% ({s.requests})
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
  const [channels, setChannels] = useState<Channel[]>([]);
  const [recent, setRecent] = useState<UsageEvent[]>([]);
  const [stats, setStats] = useState<StatsSummary | null>(null);
  const [channelStats, setChannelStats] = useState<ChannelStats[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    Promise.all([
      listChannels(),
      usageRecent(200),
      statsSummary("today"),
      statsChannels("today"),
    ])
      .then(([cs, rs, st, cst]) => {
        setChannels(cs);
        setRecent(rs);
        setStats(st);
        setChannelStats(cst.items);
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

  const rangeStartMs = stats?.start_ms ?? getTodayStartMs();
  const todayEvents = useMemo(
    () => recent.filter((e) => e.ts_ms >= rangeStartMs),
    [recent, rangeStartMs]
  );
  const hourlyData = useMemo(() => aggregateByHour(todayEvents, rangeStartMs), [todayEvents, rangeStartMs]);

  const avgLatency = useMemo(() => {
    const validEvents = todayEvents.filter((e) => e.latency_ms > 0);
    if (validEvents.length === 0) return null;
    const sum = validEvents.reduce((acc, e) => acc + e.latency_ms, 0);
    return Math.round(sum / validEvents.length);
  }, [todayEvents]);

  const formatLatency = (ms: number | null) => {
    if (ms === null) return "-";
    if (ms < 1000) return `${ms}ms`;
    return `${(ms / 1000).toFixed(1)}s`;
  };

  return (
    <div className="space-y-4">
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
              <Clock className="h-3 w-3" />
              {t("overview.cards.avgLatency")}
            </CardDescription>
          </CardHeader>
          <CardContent className="pb-3 px-3">
            <div className="text-xl font-bold">{formatLatency(avgLatency)}</div>
          </CardContent>
        </Card>
      </div>

      {/* 趋势图 + 渠道分布 */}
      <div className="grid gap-3 md:grid-cols-2">
        {/* 今日请求趋势 */}
        <Card>
          <CardHeader className="py-3 px-3">
            <CardTitle className="text-sm">{t("overview.trend.title")}</CardTitle>
            <CardDescription className="text-xs">
              {t("overview.trend.subtitle")}
            </CardDescription>
          </CardHeader>
          <CardContent className="px-3 pb-3">
            {loading ? (
              <p className="text-muted-foreground text-xs">{t("common.loading")}</p>
            ) : (
              <div className="space-y-2">
                <TrendChart
                  data={hourlyData}
                  getTitle={(hour, count) =>
                    t("overview.trend.barTooltip", {
                      hour: String(hour).padStart(2, "0"),
                      count,
                    })
                  }
                />
                <div className="flex justify-between text-[10px] text-muted-foreground">
                  <span>00:00</span>
                  <span>06:00</span>
                  <span>12:00</span>
                  <span>18:00</span>
                  <span>24:00</span>
                </div>
              </div>
            )}
          </CardContent>
        </Card>

        {/* 渠道使用分布 */}
        <Card>
          <CardHeader className="py-3 px-3">
            <CardTitle className="text-sm">
              {t("overview.distribution.title")}
            </CardTitle>
            <CardDescription className="text-xs">
              {t("overview.distribution.subtitle")}
            </CardDescription>
          </CardHeader>
          <CardContent className="px-3 pb-3">
            {loading ? (
              <p className="text-muted-foreground text-xs">{t("common.loading")}</p>
            ) : channelStats.length === 0 ? (
              <p className="text-muted-foreground text-xs">
                {t("overview.distribution.empty")}
              </p>
            ) : (
              <ChannelDistribution stats={channelStats} />
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
                          {terminalLabel(p)}
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
