import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { Eye, RefreshCw, Search } from "lucide-react";
import type { ColumnDef } from "@tanstack/react-table";
import { parseAsInteger, parseAsString, parseAsStringLiteral, useQueryStates } from "nuqs";
import { toast } from "sonner";

import { listChannels, usageList } from "@/api";
import { PageHeader } from "@/components/PageHeader";
import { PageBody } from "@/components/layout/page-body";
import { DataTable } from "@/components/composed/data-table";
import { ProtocolBadge } from "@/components/composed/protocol-badge";
import {
  TableIconButton,
  TableMetricStack,
} from "@/components/composed/table-primitives";
import {
  Badge,
  Button,
  Card,
  CardContent,
  Dialog,
  DialogBody,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  Input,
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui";
import { useCurrency } from "@/hooks/use-currency";
import { useI18n } from "@/hooks/use-i18n";
import { useWindowEvent } from "@/hooks/use-window-event";
import { dateRangeToMs, stringsToDateRange } from "@/lib/date-utils";
import { humanizeApiError, humanizeErrorText } from "@/lib/error";
import { cn } from "@/lib/utils";
import { formatMoney, parseDecimalLike } from "@/providers/currency-provider";
import type { Channel, Protocol, UsageEvent } from "@/types/api";
import { formatDuration, formatNumber, protocolLabel } from "../../lib";

const LOG_PROTOCOL_OPTIONS = ["all", "openai", "anthropic", "gemini"] as const;
const LOG_STATUS_OPTIONS = ["all", "success", "failed"] as const;
const logsTableCellClassName = "whitespace-nowrap";

function formatLogDateParts(ms: number | null | undefined) {
  if (ms === null || ms === undefined) {
    return { date: "-", time: "-" };
  }

  const value = new Date(ms);
  if (Number.isNaN(value.getTime())) {
    return { date: "-", time: "-" };
  }

  const pad = (input: number) => String(input).padStart(2, "0");

  return {
    date: `${value.getFullYear()}-${pad(value.getMonth() + 1)}-${pad(value.getDate())}`,
    time: `${pad(value.getHours())}:${pad(value.getMinutes())}:${pad(value.getSeconds())}`,
  };
}

function DetailRow({
  label,
  children,
}: {
  label: string;
  children: ReactNode;
}) {
  return (
    <>
      <div className="py-1.5 text-[11px] font-semibold text-muted-foreground">
        {label}
      </div>
      <div className="py-1.5 text-[11px] font-mono break-all">
        {children}
      </div>
    </>
  );
}

export function LogsPage() {
  const { t } = useI18n();
  const { currency } = useCurrency();
  const [events, setEvents] = useState<UsageEvent[]>([]);
  const [channels, setChannels] = useState<Channel[]>([]);
  const [loading, setLoading] = useState(false);
  const loadingRef = useRef(false);
  const [total, setTotal] = useState(0);
  const [detailEventId, setDetailEventId] = useState<string | null>(null);
  const [search, setSearch] = useQueryStates(
    {
      start: parseAsString.withDefault(""),
      end: parseAsString.withDefault(""),
      protocol: parseAsStringLiteral(LOG_PROTOCOL_OPTIONS).withDefault("all"),
      channel: parseAsString.withDefault("all"),
      model: parseAsString.withDefault(""),
      requestId: parseAsString.withDefault(""),
      status: parseAsStringLiteral(LOG_STATUS_OPTIONS).withDefault("all"),
      page: parseAsInteger.withDefault(1),
      pageSize: parseAsInteger.withDefault(20),
    },
    {
      history: "replace",
    },
  );

  const page = Number.isFinite(search.page) && search.page > 0 ? search.page : 1;
  const pageSize = Number.isFinite(search.pageSize) && search.pageSize > 0 ? search.pageSize : 20;

  const channelNames = useMemo(() => {
    const map = new Map<string, string>();
    for (const channel of channels) {
      map.set(channel.id, channel.name);
    }
    return map;
  }, [channels]);

  const channelsById = useMemo(
    () => new Map(channels.map((channel) => [channel.id, channel] as const)),
    [channels],
  );

  const filteredChannels = useMemo(() => {
    if (search.protocol === "all") {
      return channels;
    }
    return channels.filter((channel) => channel.protocol === search.protocol);
  }, [channels, search.protocol]);

  const totalPages = useMemo(() => {
    if (total <= 0) {
      return 1;
    }
    return Math.max(1, Math.ceil(total / pageSize));
  }, [pageSize, total]);

  const pageSizeOptions = useMemo(
    () => ([10, 20, 50].includes(pageSize) ? [10, 20, 50] : [10, 20, 50, pageSize].sort((a, b) => a - b)),
    [pageSize],
  );

  const detailEvent = useMemo(
    () => events.find((event) => event.id === detailEventId) ?? null,
    [detailEventId, events],
  );
  const detailTimestamp = detailEvent ? formatLogDateParts(detailEvent.ts_ms) : null;

  function getEstimatedSpend(event: UsageEvent) {
    const estimate = parseDecimalLike(event.estimated_cost_usd);
    const channel = channelsById.get(event.channel_id);
    const multiplier = Number(channel?.real_multiplier ?? 1);

    if (estimate === null) {
      return "-";
    }
    if (!Number.isFinite(multiplier) || multiplier < 0) {
      return "-";
    }

    return formatMoney(estimate * multiplier, currency);
  }

  function formatOfficialCost(value: string | null | undefined) {
    return parseDecimalLike(value) === null ? "-" : `$${value}`;
  }

  async function refresh(
    nextPage = page,
    overrides?: Partial<{
      start: string;
      end: string;
      protocol: Protocol | "all";
      channel: string;
      model: string;
      requestId: string;
      status: "all" | "success" | "failed";
    }>,
  ) {
    setLoading(true);
    try {
      loadingRef.current = true;

      const dateRange = stringsToDateRange(
        overrides?.start ?? search.start,
        overrides?.end ?? search.end,
      );
      const protocol = overrides?.protocol ?? search.protocol;
      const channelId = overrides?.channel ?? search.channel;
      const model = overrides?.model ?? search.model;
      const requestId = overrides?.requestId ?? search.requestId;
      const status = overrides?.status ?? search.status;

      const msRange = dateRangeToMs(dateRange);
      const safePage = Number.isFinite(nextPage) && nextPage > 0 ? nextPage : 1;
      const safeOffset = Math.max(0, (safePage - 1) * pageSize);

      const result = await usageList({
        start_ms: msRange?.start_ms,
        end_ms: msRange?.end_ms,
        protocol: protocol === "all" ? undefined : protocol,
        channel_id: channelId === "all" ? undefined : channelId,
        model: model.trim() || undefined,
        request_id: requestId.trim() || undefined,
        success: status === "all" ? undefined : status === "success",
        limit: pageSize,
        offset: safeOffset,
      });

      setEvents(result.items);
      setTotal(result.total);
    } catch (error) {
      toast.error(t("logs.toast.loadFail"), {
        description: humanizeApiError(error, t),
      });
    } finally {
      setLoading(false);
      loadingRef.current = false;
    }
  }

  useEffect(() => {
    listChannels()
      .then(setChannels)
      .catch(() => setChannels([]));
  }, []);

  useEffect(() => {
    void refresh(page);
  }, [page, pageSize]);

  useEffect(() => {
    if (page > totalPages) {
      void setSearch({ page: totalPages });
    }
  }, [page, setSearch, totalPages]);

  useEffect(() => {
    if (search.channel === "all") {
      return;
    }
    if (filteredChannels.some((channel) => channel.id === search.channel)) {
      return;
    }
    void setSearch({ channel: "all" });
  }, [filteredChannels, search.channel, setSearch]);

  useEffect(() => {
    if (!detailEventId) {
      return;
    }
    if (events.some((event) => event.id === detailEventId)) {
      return;
    }
    setDetailEventId(null);
  }, [detailEventId, events]);

  useWindowEvent("cliswitch-usage-changed", () => {
    if (loadingRef.current) {
      return;
    }
    void refresh(page);
  });

  const rangeInfo = total === 0
    ? t("common.pagination.total", { total: formatNumber(total) })
    : t("logs.pagination.range", {
        start: (page - 1) * pageSize + 1,
        end: Math.min(page * pageSize, total),
        total: formatNumber(total),
      });

  const pageSizeLabel = (value: number) => t("logs.pagination.pageSize", { value });
  const columns: Array<ColumnDef<UsageEvent>> = [
    {
      id: "time",
      header: t("logs.headers.time"),
      cell: ({ row }) => {
        const { date, time } = formatLogDateParts(row.original.ts_ms);
        return (
          <TableMetricStack
            primary={date}
            secondary={time}
            primaryClassName="font-mono"
          />
        );
      },
      meta: {
        headerClassName: "w-[11%]",
        cellClassName: logsTableCellClassName,
        skeletonClassName: "mx-auto w-20",
      },
    },
    {
      id: "terminal",
      header: t("logs.headers.terminal"),
      cell: ({ row }) => (
        <ProtocolBadge protocol={row.original.protocol}>
          {protocolLabel(t, row.original.protocol)}
        </ProtocolBadge>
      ),
      meta: {
        headerClassName: "w-[11%]",
        cellClassName: logsTableCellClassName,
        skeletonClassName: "mx-auto w-16",
      },
    },
    {
      id: "channel",
      header: t("logs.headers.channel"),
      cell: ({ row }) => (
        <span className="inline-block max-w-full truncate align-middle text-xs font-semibold">
          {channelNames.get(row.original.channel_id) ?? "-"}
        </span>
      ),
      meta: {
        headerClassName: "w-[10%]",
        cellClassName: logsTableCellClassName,
        skeletonClassName: "mx-auto w-24",
      },
    },
    {
      id: "model",
      header: t("logs.headers.model"),
      cell: ({ row }) => (
        <span className="inline-block max-w-full truncate align-middle text-xs font-medium">
          {row.original.model ?? "-"}
        </span>
      ),
      meta: {
        headerClassName: "w-[15%]",
        cellClassName: logsTableCellClassName,
        skeletonClassName: "mx-auto w-28",
      },
    },
    {
      id: "timing",
      header: t("logs.headers.timing"),
      cell: ({ row }) => (
        <TableMetricStack
          primary={formatDuration(row.original.latency_ms)}
          secondary={`${t("logs.cell.ttftCompact")} ${formatDuration(row.original.ttft_ms)}`}
          primaryClassName="font-mono"
        />
      ),
      meta: {
        headerClassName: "w-[11%]",
        cellClassName: logsTableCellClassName,
        skeletonClassName: "mx-auto w-16",
      },
    },
    {
      id: "tokens",
      header: t("logs.headers.tokens"),
      cell: ({ row }) => {
        const cacheRead = row.original.cache_read_tokens ?? 0;
        const cacheWrite = row.original.cache_write_tokens ?? 0;
        const hasCache = cacheRead > 0 || cacheWrite > 0;

        return (
          <TableMetricStack
            primary={`${formatNumber(row.original.prompt_tokens)} / ${formatNumber(row.original.completion_tokens)}`}
            secondary={
              hasCache
                ? `${t("logs.cell.cacheWriteCompact")}:${formatNumber(cacheWrite)} ${t("logs.cell.cacheReadCompact")}:${formatNumber(cacheRead)}`
                : "—"
            }
            primaryClassName="font-mono"
          />
        );
      },
      meta: {
        headerClassName: "w-[17%]",
        cellClassName: logsTableCellClassName,
        skeletonClassName: "mx-auto w-32",
      },
    },
    {
      id: "cost",
      header: t("logs.headers.cost"),
      cell: ({ row }) => (
        <TableMetricStack
          primary={formatOfficialCost(row.original.estimated_cost_usd)}
          secondary={getEstimatedSpend(row.original)}
          primaryClassName="font-mono"
        />
      ),
      meta: {
        headerClassName: "w-[11%]",
        cellClassName: logsTableCellClassName,
        skeletonClassName: "mx-auto w-20",
      },
    },
    {
      id: "result",
      header: t("logs.headers.result"),
      cell: ({ row }) =>
        row.original.success ? (
          <Badge variant="success">{t("logs.filters.success")}</Badge>
        ) : (
          <Badge variant="destructive">{t("logs.filters.failed")}</Badge>
        ),
      meta: {
        headerClassName: "w-[7%]",
        cellClassName: logsTableCellClassName,
        skeletonClassName: "mx-auto w-14",
      },
    },
    {
      id: "actions",
      header: "",
      cell: ({ row }) => (
        <TableIconButton
          aria-label={t("common.details")}
          onClick={() => {
            setDetailEventId(row.original.id);
          }}
        >
          <Eye className="h-3 w-3" />
        </TableIconButton>
      ),
      meta: {
        headerClassName: "w-[7%]",
        cellClassName: logsTableCellClassName,
        skeletonClassName: "mx-auto w-7",
      },
    },
  ];

  return (
    <>
      <div className="flex h-full min-h-0 flex-col overflow-hidden">
        <PageHeader
          title={t("logs.title")}
          actions={
            <Button
              aria-label={t("common.refresh")}
              disabled={loading}
              size="icon"
              variant="outline"
              onClick={() => void refresh(page)}
            >
              <RefreshCw className={cn("h-3.5 w-3.5", loading && "animate-spin")} />
            </Button>
          }
        />

        <div className="flex-1 overflow-hidden">
          <PageBody className="flex h-full flex-col gap-3">
            <Card className="animate-fade-up shrink-0 px-4 py-3">
              <CardContent className="p-0">
                <div className="flex flex-wrap items-center gap-3">
                  <div className="flex items-center gap-1.5">
                    <Input
                      className="h-7 w-[132px] px-2 py-1 text-[11px]"
                      type="date"
                      value={search.start}
                      onChange={(event) => {
                        void setSearch({ start: event.target.value });
                      }}
                    />
                    <span className="text-[11px] text-muted-foreground">~</span>
                    <Input
                      className="h-7 w-[132px] px-2 py-1 text-[11px]"
                      type="date"
                      value={search.end}
                      onChange={(event) => {
                        void setSearch({ end: event.target.value });
                      }}
                    />
                  </div>

                  <Select
                    value={search.protocol}
                    onValueChange={(value) => {
                      void setSearch({
                        protocol: value as Protocol | "all",
                        channel: "all",
                      });
                    }}
                  >
                    <SelectTrigger className="h-7 w-[106px] px-2 py-1 text-[11px]">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="all">{t("logs.filters.all")}</SelectItem>
                      <SelectItem value="openai">{protocolLabel(t, "openai")}</SelectItem>
                      <SelectItem value="anthropic">{protocolLabel(t, "anthropic")}</SelectItem>
                      <SelectItem value="gemini">{protocolLabel(t, "gemini")}</SelectItem>
                    </SelectContent>
                  </Select>

                  <Select
                    value={search.channel}
                    onValueChange={(value) => {
                      void setSearch({ channel: value });
                    }}
                  >
                    <SelectTrigger className="h-7 w-[154px] px-2 py-1 text-[11px]">
                      <SelectValue placeholder={t("logs.filters.channel")} />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="all">{t("logs.filters.all")}</SelectItem>
                      {filteredChannels.map((channel) => (
                        <SelectItem key={channel.id} value={channel.id}>
                          {channel.name}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>

                  <Input
                    className="h-7 w-[120px] px-2 py-1 text-[11px]"
                    placeholder={t("logs.filters.model")}
                    value={search.model}
                    onChange={(event) => {
                      void setSearch({ model: event.target.value });
                    }}
                  />

                  <Input
                    className="h-7 w-[140px] px-2 py-1 text-[11px]"
                    placeholder={t("logs.filters.dimension")}
                    value={search.requestId}
                    onChange={(event) => {
                      void setSearch({ requestId: event.target.value });
                    }}
                  />

                  <Select
                    value={search.status}
                    onValueChange={(value) => {
                      void setSearch({
                        status: value as "all" | "success" | "failed",
                      });
                    }}
                  >
                    <SelectTrigger className="h-7 w-[98px] px-2 py-1 text-[11px]">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="all">{t("logs.filters.all")}</SelectItem>
                      <SelectItem value="success">{t("logs.filters.success")}</SelectItem>
                      <SelectItem value="failed">{t("logs.filters.failed")}</SelectItem>
                    </SelectContent>
                  </Select>

                  <Button
                    className="h-7 rounded-md px-2 py-1 text-[11px]"
                    disabled={loading}
                    size="sm"
                    onClick={() => {
                      if (page !== 1) {
                        void setSearch({ page: 1 });
                        return;
                      }
                      void refresh(1);
                    }}
                  >
                    <Search className="h-3 w-3" />
                    {t("logs.filters.search")}
                  </Button>

                  <Button
                    className="h-7 rounded-md px-2 py-1 text-[11px]"
                    disabled={loading}
                    size="sm"
                    variant="outline"
                    onClick={() => {
                      const next = {
                        start: "",
                        end: "",
                        protocol: "all" as const,
                        channel: "all",
                        model: "",
                        requestId: "",
                        status: "all" as const,
                      };

                      void setSearch({
                        ...next,
                        page: 1,
                        pageSize,
                      });

                      if (page === 1) {
                        void refresh(1, next);
                      }
                    }}
                  >
                    {t("logs.filters.reset")}
                  </Button>
                </div>
              </CardContent>
            </Card>

            <Card className="animate-fade-up [animation-delay:60ms] flex min-h-0 flex-1 flex-col overflow-hidden">
              <DataTable
                columns={columns}
                data={events}
                loading={loading}
                loadingRowCount={pageSize}
                getRowId={(row) => row.id}
                className="min-h-0"
                tableClassName="table-fixed text-[11px]"
                containerClassName="h-full overflow-y-auto"
                emptyState={t("logs.empty")}
                rowClassName={() => "hover:!bg-secondary/35"}
                pagination={{
                  page,
                  total,
                  pageSize,
                  pageSizeOptions,
                  manual: true,
                  summary: <span>{rangeInfo}</span>,
                  pageSizeOptionLabel: pageSizeLabel,
                  pageSizeSuffix: null,
                  disabled: loading,
                  onPageChange: (nextPage) => {
                    void setSearch({ page: nextPage });
                  },
                  onPageSizeChange: (nextPageSize) => {
                    void setSearch({ pageSize: nextPageSize, page: 1 });
                  },
                }}
              />
            </Card>
          </PageBody>
        </div>
      </div>

      <Dialog
        open={Boolean(detailEvent)}
        onOpenChange={(open) => {
          if (!open) {
            setDetailEventId(null);
          }
        }}
      >
        <DialogContent className="max-h-[85vh] max-w-[560px] p-0 [&>button:last-child]:hidden">
          <DialogHeader>
            <DialogTitle>{t("common.details")}</DialogTitle>
            <DialogDescription>
              {detailTimestamp ? `${detailTimestamp.date} ${detailTimestamp.time}` : "-"}
            </DialogDescription>
          </DialogHeader>

          <DialogBody className="min-h-0 flex-1 overflow-y-auto">
            {detailEvent ? (
              <div className="grid grid-cols-[110px_1fr]">
                <DetailRow label={t("logs.details.id")}>{detailEvent.id}</DetailRow>
                <DetailRow label={t("logs.details.requestId")}>
                  {detailEvent.request_id ?? "-"}
                </DetailRow>
                <DetailRow label={t("logs.headers.terminal")}>
                  <ProtocolBadge className="w-fit" protocol={detailEvent.protocol}>
                    {protocolLabel(t, detailEvent.protocol)}
                  </ProtocolBadge>
                </DetailRow>
                <DetailRow label={t("logs.headers.channel")}>
                  {channelNames.get(detailEvent.channel_id) ?? detailEvent.channel_id}
                </DetailRow>
                <DetailRow label={t("logs.headers.model")}>
                  {detailEvent.model ?? "-"}
                </DetailRow>
                <DetailRow label={t("logs.headers.result")}>
                  {detailEvent.success ? (
                    <Badge variant="success">
                      {detailEvent.http_status ?? 200} {t("logs.filters.success")}
                    </Badge>
                  ) : (
                    <Badge variant="destructive">
                      {detailEvent.http_status ?? "ERR"} {t("logs.filters.failed")}
                    </Badge>
                  )}
                </DetailRow>
                <DetailRow label={t("logs.cell.duration")}>
                  {formatDuration(detailEvent.latency_ms)}
                </DetailRow>
                <DetailRow label={t("logs.cell.ttft")}>
                  {formatDuration(detailEvent.ttft_ms)}
                </DetailRow>
                <DetailRow label={t("logs.cell.input")}>
                  {formatNumber(detailEvent.prompt_tokens)}
                </DetailRow>
                <DetailRow label={t("logs.cell.output")}>
                  {formatNumber(detailEvent.completion_tokens)}
                </DetailRow>
                <DetailRow label={t("logs.cell.total")}>
                  {formatNumber(detailEvent.total_tokens)}
                </DetailRow>
                <DetailRow label={t("logs.cell.cacheRead")}>
                  {formatNumber(detailEvent.cache_read_tokens)}
                </DetailRow>
                <DetailRow label={t("logs.cell.cacheWrite")}>
                  {formatNumber(detailEvent.cache_write_tokens)}
                </DetailRow>
                <DetailRow label={t("logs.headers.cost")}>
                  {formatOfficialCost(detailEvent.estimated_cost_usd)}
                </DetailRow>
                <DetailRow label={t("logs.details.estimatedSpend")}>
                  {getEstimatedSpend(detailEvent)}
                </DetailRow>
                <DetailRow label={t("logs.details.errorKind")}>
                  {detailEvent.error_kind ? (
                    <Badge variant="destructive">{detailEvent.error_kind}</Badge>
                  ) : (
                    "-"
                  )}
                </DetailRow>
                <DetailRow label={t("logs.details.errorDetail")}>
                  {detailEvent.error_detail ? (
                    <pre className="m-0 whitespace-pre-wrap text-[10px] leading-relaxed text-destructive">
                      {humanizeErrorText(detailEvent.error_detail)}
                    </pre>
                  ) : (
                    "-"
                  )}
                </DetailRow>
              </div>
            ) : null}
          </DialogBody>

          <DialogFooter>
            <DialogClose asChild>
              <Button size="sm" variant="outline">
                {t("common.close")}
              </Button>
            </DialogClose>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );
}
