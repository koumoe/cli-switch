import React, { useEffect, useRef, useState } from "react";
import { RefreshCw } from "lucide-react";
import { toast } from "sonner";
import type { DateRange } from "react-day-picker";
import {
  Button,
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  Badge,
  DateRangePicker,
  dateRangeToMs,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui";
import { PageHeader } from "@/components/PageHeader";
import { PaginationBar } from "@/components/PaginationBar";
import { useI18n } from "@/lib/i18n";
import { humanizeApiError } from "@/lib/error";
import { useWindowEvent } from "@/lib/useWindowEvent";
import { useCurrency, formatMoney, parseDecimalLike } from "@/lib/currency";
import {
  listChannels,
  statsSummary,
  statsChannels,
  type Channel,
  type StatsSummary,
  type ChannelStats,
} from "../api";
import { protocolLabel } from "../lib";

export function MonitorPage() {
  const { locale, t } = useI18n();
  const { currency } = useCurrency();
  const colClass = {
    channel: "w-28",
    terminal: "w-20",
    requests: "w-16",
    success: "w-16",
    failed: "w-16",
    estimatedCost: "w-28",
    actualSpend: "w-28",
    avgLatency: "w-24",
  } as const;
  const [stats, setStats] = useState<StatsSummary | null>(null);
  const [channelStats, setChannelStats] = useState<ChannelStats[]>([]);
  const [channels, setChannels] = useState<Channel[]>([]);
  const [loading, setLoading] = useState(false);
  const loadingRef = useRef(false);
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState(20);

  const [dateRange, setDateRange] = useState<DateRange | undefined>(() => {
    const now = new Date();
    return { from: now, to: now };
  });

  async function refresh() {
    setLoading(true);
    try {
      loadingRef.current = true;
      const cs = await listChannels();
      setChannels(cs);

      const msRange = dateRangeToMs(dateRange);
      if (!msRange) {
        setStats(null);
        setChannelStats([]);
        return;
      }

      const q = { start_ms: msRange.start_ms, end_ms: msRange.end_ms };
      const [st, cst] = await Promise.all([statsSummary(q), statsChannels(q)]);
      setStats(st);
      setChannelStats(
        [...cst.items].sort((a, b) => {
          if (b.success !== a.success) return b.success - a.success;
          if (b.requests !== a.requests) return b.requests - a.requests;
          return a.name.localeCompare(b.name);
        })
      );
    } catch (e) {
      toast.error(t("monitor.toast.loadFail"), { description: humanizeApiError(e, t) });
    } finally {
      setLoading(false);
      loadingRef.current = false;
    }
  }

  useEffect(() => {
    refresh();
  }, [dateRange?.from, dateRange?.to]);

  useWindowEvent("cliswitch-usage-changed", () => {
    if (loadingRef.current) return;
    void refresh();
  });

  const successRate =
    stats && stats.requests > 0
      ? Math.round((stats.success / stats.requests) * 100)
      : 0;

  const channelsById = React.useMemo(() => new Map(channels.map((c) => [c.id, c] as const)), [channels]);

  const totalActualSpend = React.useMemo(() => {
    if (!channelStats.length || !channels.length) return null;
    let sum = 0;
    let hasAny = false;
    for (const s of channelStats) {
      const est = parseDecimalLike(s.estimated_cost_usd);
      if (!est || est <= 0) continue;
      const ch = channelsById.get(s.channel_id);
      const real = Number(ch?.real_multiplier ?? 1);
      if (!Number.isFinite(real) || real < 0) continue;
      hasAny = true;
      sum += est * real;
    }
    return hasAny ? sum : null;
  }, [channelStats, channels.length, channelsById]);

  const totalPages = React.useMemo(
    () => Math.max(1, Math.ceil(channelStats.length / pageSize)),
    [channelStats.length, pageSize]
  );

  const currentPage = Math.min(page, totalPages);
  const pagedChannelStats = React.useMemo(() => {
    const start = (currentPage - 1) * pageSize;
    return channelStats.slice(start, start + pageSize);
  }, [channelStats, currentPage, pageSize]);

  useEffect(() => {
    setPage((current) => Math.min(current, totalPages));
  }, [totalPages]);

  return (
    <div className="flex h-full min-h-0 flex-col gap-4 overflow-hidden">
      <PageHeader
        title={t("monitor.title")}
        actions={
          <>
            <DateRangePicker
              value={dateRange}
              onChange={setDateRange}
              placeholder={t("monitor.range.selectRange")}
              className="h-8 w-[260px]"
              disabled={loading}
              locale={locale}
            />
            <Button
              size="sm"
              variant="outline"
              onClick={refresh}
              disabled={loading || !dateRange?.from}
            >
              <RefreshCw className={`h-4 w-4 mr-2 ${loading ? "animate-spin" : ""}`} />
              {t("common.refresh")}
            </Button>
          </>
        }
      />

      {/* 统计卡片 */}
      <div className="grid gap-3 md:grid-cols-5">
        <Card>
          <CardHeader className="pb-2">
            <CardDescription>{t("monitor.cards.totalRequests")}</CardDescription>
          </CardHeader>
          <CardContent>
            <div className="text-xl font-semibold">{stats?.requests ?? "-"}</div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="pb-2">
            <CardDescription>{t("monitor.cards.successRate")}</CardDescription>
          </CardHeader>
          <CardContent>
            <div className="text-xl font-semibold">{successRate}%</div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="pb-2">
            <CardDescription>{t("monitor.cards.failed")}</CardDescription>
          </CardHeader>
          <CardContent>
            <div className="text-xl font-semibold text-destructive">
              {stats?.failed ?? "-"}
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="pb-2">
            <CardDescription>{t("monitor.cards.estimatedCost")}</CardDescription>
          </CardHeader>
          <CardContent>
            <div className="text-xl font-semibold">
              ${stats?.estimated_cost_usd ?? "-"}
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="pb-2">
            <CardDescription>{t("monitor.cards.actualSpend")}</CardDescription>
          </CardHeader>
          <CardContent>
            <div className="text-xl font-semibold">
              {formatMoney(totalActualSpend, currency)}
            </div>
          </CardContent>
        </Card>
      </div>

      {/* 渠道统计 */}
      {channelStats.length > 0 && (
        <Card className="flex flex-1 min-h-0 flex-col">
          <CardContent className="flex flex-1 min-h-0 flex-col p-0">
            <div className="flex-1 min-h-0 overflow-hidden">
              <Table containerClassName="h-full overflow-y-auto">
                <TableHeader className="sticky top-0 z-10 bg-background">
                  <TableRow>
                    <TableHead className={colClass.channel}>
                      {t("monitor.channelStats.headers.channel")}
                    </TableHead>
                    <TableHead className={colClass.terminal}>
                      {t("monitor.channelStats.headers.terminal")}
                    </TableHead>
                    <TableHead className={colClass.requests}>
                      {t("monitor.channelStats.headers.requests")}
                    </TableHead>
                    <TableHead className={colClass.success}>
                      {t("monitor.channelStats.headers.success")}
                    </TableHead>
                    <TableHead className={colClass.failed}>
                      {t("monitor.channelStats.headers.failed")}
                    </TableHead>
                    <TableHead className={colClass.estimatedCost}>
                      {t("monitor.channelStats.headers.estimatedCost")}
                    </TableHead>
                    <TableHead className={colClass.actualSpend}>
                      {t("monitor.channelStats.headers.actualSpend")}
                    </TableHead>
                    <TableHead className={colClass.avgLatency}>
                      {t("monitor.channelStats.headers.avgLatency")}
                    </TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {pagedChannelStats.map((cs) => (
                    <TableRow key={cs.channel_id}>
                      <TableCell className="font-medium">{cs.name}</TableCell>
                      <TableCell>
                        <Badge variant="outline">{protocolLabel(t, cs.protocol)}</Badge>
                      </TableCell>
                      <TableCell>{cs.requests}</TableCell>
                      <TableCell className="text-success">
                        {cs.success}
                      </TableCell>
                      <TableCell className="text-destructive">
                        {cs.failed}
                      </TableCell>
                      <TableCell className="text-muted-foreground font-mono">
                        {cs.estimated_cost_usd ? `$${cs.estimated_cost_usd}` : "-"}
                      </TableCell>
                      <TableCell className="text-muted-foreground font-mono">
                        {(() => {
                          const est = parseDecimalLike(cs.estimated_cost_usd);
                          const ch = channelsById.get(cs.channel_id);
                          const real = Number(ch?.real_multiplier ?? 1);
                          if (!est || est <= 0) return "-";
                          if (!Number.isFinite(real) || real < 0) return "-";
                          return formatMoney(est * real, currency);
                        })()}
                      </TableCell>
                      <TableCell className="text-muted-foreground">
                        {cs.avg_latency_ms
                          ? `${Math.round(cs.avg_latency_ms)}ms`
                          : "-"}
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </div>
            <PaginationBar
              page={currentPage}
              total={channelStats.length}
              totalPages={totalPages}
              pageSize={pageSize}
              disabled={loading}
              onPageChange={setPage}
              onPageSizeChange={(next) => {
                setPageSize(next);
                setPage(1);
              }}
            />
          </CardContent>
        </Card>
      )}
    </div>
  );
}
