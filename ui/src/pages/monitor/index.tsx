import React, { useEffect, useRef, useState } from "react";
import type { ColumnDef } from "@tanstack/react-table";
import { RefreshCw } from "lucide-react";
import { toast } from "sonner";
import type { DateRange } from "react-day-picker";
import { Button, Card, CardContent } from "@/components/ui";
import { DataTable } from "@/components/composed/data-table";
import { DateRangePicker } from "@/components/composed/date-range-picker";
import { MetricCard } from "@/components/composed/metric-card";
import { ProtocolBadge } from "@/components/composed/protocol-badge";
import { PageHeader } from "@/components/PageHeader";
import { PageBody } from "@/components/layout/page-body";
import { useCurrency } from "@/hooks/use-currency";
import { useI18n } from "@/hooks/use-i18n";
import { dateRangeToMs } from "@/lib/date-utils";
import { useWindowEvent } from "@/hooks/use-window-event";
import { humanizeApiError } from "@/lib/error";
import { formatMoney, parseDecimalLike } from "@/providers/currency-provider";
import { listChannels, statsChannels, statsSummary } from "@/api";
import type { Channel, ChannelStats, StatsSummary } from "@/types/api";
import { protocolLabel } from "../../lib";

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
        }),
      );
    } catch (e) {
      toast.error(t("monitor.toast.loadFail"), {
        description: humanizeApiError(e, t),
      });
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

  const channelsById = React.useMemo(
    () => new Map(channels.map((c) => [c.id, c] as const)),
    [channels],
  );

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
  const columns = React.useMemo<Array<ColumnDef<ChannelStats>>>(
    () => [
      {
        accessorKey: "name",
        header: t("monitor.channelStats.headers.channel"),
        enableSorting: true,
        cell: ({ row }) => (
          <span
            className="mx-auto block max-w-[220px] truncate text-center font-medium"
            title={row.original.name}
          >
            {row.original.name}
          </span>
        ),
        meta: {
          headerClassName: colClass.channel,
          skeletonClassName: "w-28 mx-auto",
        },
      },
      {
        id: "protocol",
        header: t("monitor.channelStats.headers.terminal"),
        accessorFn: (row) => row.protocol,
        enableSorting: true,
        cell: ({ row }) => (
          <ProtocolBadge protocol={row.original.protocol}>
            {protocolLabel(t, row.original.protocol)}
          </ProtocolBadge>
        ),
        meta: {
          headerClassName: colClass.terminal,
          skeletonClassName: "w-14 mx-auto",
        },
      },
      {
        accessorKey: "requests",
        header: t("monitor.channelStats.headers.requests"),
        enableSorting: true,
        meta: {
          headerClassName: colClass.requests,
          skeletonClassName: "w-10 mx-auto",
        },
      },
      {
        accessorKey: "success",
        header: t("monitor.channelStats.headers.success"),
        enableSorting: true,
        cell: ({ row }) => (
          <span className="text-success">{row.original.success}</span>
        ),
        meta: {
          headerClassName: colClass.success,
          skeletonClassName: "w-10 mx-auto",
        },
      },
      {
        accessorKey: "failed",
        header: t("monitor.channelStats.headers.failed"),
        enableSorting: true,
        cell: ({ row }) => (
          <span className="text-destructive">{row.original.failed}</span>
        ),
        meta: {
          headerClassName: colClass.failed,
          skeletonClassName: "w-10 mx-auto",
        },
      },
      {
        id: "estimated_cost_usd",
        header: t("monitor.channelStats.headers.estimatedCost"),
        accessorFn: (row) => parseDecimalLike(row.estimated_cost_usd) ?? -1,
        enableSorting: true,
        cell: ({ row }) => (
          <span className="font-mono text-muted-foreground">
            {row.original.estimated_cost_usd
              ? `$${row.original.estimated_cost_usd}`
              : "-"}
          </span>
        ),
        meta: {
          headerClassName: colClass.estimatedCost,
          skeletonClassName: "w-18 mx-auto",
        },
      },
      {
        id: "actual_spend",
        header: t("monitor.channelStats.headers.actualSpend"),
        accessorFn: (row) => {
          const est = parseDecimalLike(row.estimated_cost_usd);
          const channel = channelsById.get(row.channel_id);
          const real = Number(channel?.real_multiplier ?? 1);
          if (!est || est <= 0) return -1;
          if (!Number.isFinite(real) || real < 0) return -1;
          return est * real;
        },
        enableSorting: true,
        cell: ({ row }) => {
          const est = parseDecimalLike(row.original.estimated_cost_usd);
          const channel = channelsById.get(row.original.channel_id);
          const real = Number(channel?.real_multiplier ?? 1);
          if (!est || est <= 0)
            return <span className="font-mono text-muted-foreground">-</span>;
          if (!Number.isFinite(real) || real < 0) {
            return <span className="font-mono text-muted-foreground">-</span>;
          }
          return (
            <span className="font-mono text-muted-foreground">
              {formatMoney(est * real, currency)}
            </span>
          );
        },
        meta: {
          headerClassName: colClass.actualSpend,
          skeletonClassName: "w-18 mx-auto",
        },
      },
      {
        accessorKey: "avg_latency_ms",
        header: t("monitor.channelStats.headers.avgLatency"),
        enableSorting: true,
        cell: ({ row }) => (
          <span className="text-muted-foreground">
            {row.original.avg_latency_ms
              ? `${Math.round(row.original.avg_latency_ms)}ms`
              : "-"}
          </span>
        ),
        meta: {
          headerClassName: colClass.avgLatency,
          skeletonClassName: "w-16 mx-auto",
        },
      },
    ],
    [
      channelsById,
      colClass.actualSpend,
      colClass.avgLatency,
      colClass.channel,
      colClass.estimatedCost,
      colClass.failed,
      colClass.requests,
      colClass.success,
      colClass.terminal,
      currency,
      t,
    ],
  );

  return (
    <div className="flex h-full min-h-0 flex-col overflow-hidden">
      <PageHeader
        title={t("monitor.title")}
        actions={
          <>
            <DateRangePicker
              value={dateRange}
              onChange={setDateRange}
              placeholder={t("monitor.range.selectRange")}
              className="h-7 w-[260px]"
              disabled={loading}
              locale={locale}
            />
            <Button
              size="sm"
              variant="outline"
              onClick={refresh}
              disabled={loading || !dateRange?.from}
            >
              <RefreshCw
                className={`h-3.5 w-3.5 ${loading ? "animate-spin" : ""}`}
              />
              {t("common.refresh")}
            </Button>
          </>
        }
      />
      <div className="flex-1 overflow-y-auto">
        <PageBody className="space-y-3">
          <div className="grid gap-3 md:grid-cols-5">
            <MetricCard
              label={t("monitor.cards.totalRequests")}
              value={stats?.requests ?? "-"}
              barColor="bg-primary"
              className="animate-fade-up"
            />
            <MetricCard
              label={t("monitor.cards.successRate")}
              value={`${successRate}%`}
              barColor="bg-success"
              className="animate-fade-up [animation-delay:60ms]"
            />
            <MetricCard
              label={t("monitor.cards.failed")}
              value={stats?.failed ?? "-"}
              barColor="bg-destructive"
              className="animate-fade-up [animation-delay:120ms]"
            />
            <MetricCard
              label={t("monitor.cards.estimatedCost")}
              value={`$${stats?.estimated_cost_usd ?? "-"}`}
              barColor="bg-warning"
              className="animate-fade-up [animation-delay:180ms]"
            />
            <MetricCard
              label={t("monitor.cards.actualSpend")}
              value={formatMoney(totalActualSpend, currency)}
              barColor="bg-muted-foreground/45"
              className="animate-fade-up [animation-delay:240ms]"
            />
          </div>

          <Card className="flex min-h-0 flex-col overflow-hidden">
            <CardContent className="flex min-h-0 flex-col p-0">
              <DataTable
                columns={columns}
                data={channelStats}
                loading={loading}
                getRowId={(row) => row.channel_id}
                containerClassName="h-full overflow-y-auto"
                emptyState={t("monitor.channelStats.empty")}
                pagination={{
                  page,
                  pageSize,
                  disabled: loading,
                  onPageChange: setPage,
                  onPageSizeChange: (next) => {
                    setPageSize(next);
                    setPage(1);
                  },
                }}
              />
            </CardContent>
          </Card>
        </PageBody>
      </div>
    </div>
  );
}
