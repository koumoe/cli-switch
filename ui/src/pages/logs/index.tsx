import React, { useEffect, useMemo, useRef, useState } from "react";
import type { ColumnDef } from "@tanstack/react-table";
import { RefreshCw } from "lucide-react";
import { parseAsInteger, parseAsString, parseAsStringLiteral, useQueryStates } from "nuqs";
import { toast } from "sonner";
import type { DateRange } from "react-day-picker";
import {
  Button,
  Card,
  CardContent,
  CardHeader,
  Badge,
  Input,
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui";
import { DataTable } from "@/components/composed/data-table";
import { DateRangePicker } from "@/components/composed/date-range-picker";
import { PageHeader } from "@/components/PageHeader";
import { useCurrency } from "@/hooks/use-currency";
import { useI18n } from "@/hooks/use-i18n";
import { dateRangeToMs, dateRangeToStrings, stringsToDateRange } from "@/lib/date-utils";
import { useWindowEvent } from "@/hooks/use-window-event";
import { humanizeApiError, humanizeErrorText } from "@/lib/error";
import { formatMoney, parseDecimalLike } from "@/providers/currency-provider";
import { listChannels, usageList } from "@/api";
import type { Channel, Protocol, UsageEvent } from "@/types/api";
import {
  clampStr,
  formatDateTime,
  formatDuration,
  formatNumber,
  protocolBadgeClassName,
  protocolLabel,
} from "../../lib";

const LOG_PROTOCOL_OPTIONS = ["all", "openai", "anthropic", "gemini"] as const;
const LOG_STATUS_OPTIONS = ["all", "success", "failed"] as const;

export function LogsPage() {
  const { locale, t } = useI18n();
  const { currency } = useCurrency();
  const [events, setEvents] = useState<UsageEvent[]>([]);
  const [channels, setChannels] = useState<Channel[]>([]);
  const [loading, setLoading] = useState(false);
  const loadingRef = useRef(false);
  const [total, setTotal] = useState(0);
  const [expandedEventId, setExpandedEventId] = useState<string | null>(null);
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
    }
  );
  const page = Number.isFinite(search.page) && search.page > 0 ? search.page : 1;
  const pageSize = Number.isFinite(search.pageSize) && search.pageSize > 0 ? search.pageSize : 20;
  const dateRange = useMemo<DateRange | undefined>(
    () => stringsToDateRange(search.start, search.end),
    [search.end, search.start]
  );

  const channelNames = useMemo(() => {
    const m = new Map<string, string>();
    for (const c of channels) m.set(c.id, c.name);
    return m;
  }, [channels]);

  const channelsById = useMemo(() => new Map(channels.map((c) => [c.id, c] as const)), [channels]);
  const filteredChannels = useMemo(() => {
    if (search.protocol === "all") return channels;
    return channels.filter((channel) => channel.protocol === search.protocol);
  }, [channels, search.protocol]);
  const selectedChannel = useMemo(
    () => filteredChannels.find((channel) => channel.id === search.channel) ?? null,
    [filteredChannels, search.channel]
  );

  const totalPages = useMemo(() => {
    if (total <= 0) return 1;
    return Math.max(1, Math.ceil(total / pageSize));
  }, [total, pageSize]);

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
    }>
  ) {
    setLoading(true);
    try {
      loadingRef.current = true;
      const sDateRange = stringsToDateRange(
        overrides?.start ?? search.start,
        overrides?.end ?? search.end
      );
      const sProtocol = overrides?.protocol ?? search.protocol;
      const sChannelId = overrides?.channel ?? search.channel;
      const sModel = overrides?.model ?? search.model;
      const sRequestId = overrides?.requestId ?? search.requestId;
      const sStatus = overrides?.status ?? search.status;

      const msRange = dateRangeToMs(sDateRange);
      const safePageSize = pageSize;
      const safePage = Number.isFinite(nextPage) && nextPage > 0 ? nextPage : 1;
      const rawOffset = (safePage - 1) * safePageSize;
      const safeOffset = Number.isFinite(rawOffset) && rawOffset >= 0 ? rawOffset : 0;

      const res = await usageList({
        start_ms: msRange?.start_ms,
        end_ms: msRange?.end_ms,
        protocol: sProtocol === "all" ? undefined : sProtocol,
        channel_id: sChannelId === "all" ? undefined : sChannelId,
        model: sModel.trim().length > 0 ? sModel.trim() : undefined,
        request_id: sRequestId.trim().length > 0 ? sRequestId.trim() : undefined,
        success: sStatus === "all" ? undefined : sStatus === "success",
        limit: safePageSize,
        offset: safeOffset,
      });

      setEvents(res.items);
      setTotal(res.total);
    } catch (e) {
      toast.error(t("logs.toast.loadFail"), { description: humanizeApiError(e, t) });
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
    if (search.channel === "all") return;
    if (filteredChannels.some((channel) => channel.id === search.channel)) return;
    void setSearch({ channel: "all" });
  }, [filteredChannels, search.channel, setSearch]);

  useEffect(() => {
    if (!expandedEventId) return;
    if (events.some((event) => event.id === expandedEventId)) return;
    setExpandedEventId(null);
  }, [events, expandedEventId]);

  useWindowEvent("cliswitch-usage-changed", () => {
    if (loadingRef.current) return;
    void refresh(page);
  });

  const columns = useMemo<Array<ColumnDef<UsageEvent>>>(
    () => [
      {
        id: "time",
        header: t("logs.headers.time"),
        cell: ({ row }) => <div className="leading-4">{formatDateTime(row.original.ts_ms)}</div>,
        meta: {
          headerClassName: "w-28",
          skeletonClassName: "w-20 mx-auto",
        },
      },
      {
        id: "protocol",
        header: t("logs.headers.terminal"),
        cell: ({ row }) => (
          <Badge
            variant="outline"
            className={protocolBadgeClassName(row.original.protocol)}
          >
            {protocolLabel(t, row.original.protocol)}
          </Badge>
        ),
        meta: {
          headerClassName: "w-16",
          skeletonClassName: "w-12 mx-auto",
        },
      },
      {
        id: "channel",
        header: t("logs.headers.channel"),
        cell: ({ row }) => (
          <div className="max-w-22.5 truncate">
            {channelNames.get(row.original.channel_id) ?? "-"}
          </div>
        ),
        meta: {
          headerClassName: "w-24",
          skeletonClassName: "w-16 mx-auto",
        },
      },
      {
        id: "model",
        header: t("logs.headers.model"),
        cell: ({ row }) => (
          <div className="max-w-25 truncate text-muted-foreground">{row.original.model ?? "-"}</div>
        ),
        meta: {
          headerClassName: "w-28",
          skeletonClassName: "w-24 mx-auto",
        },
      },
      {
        id: "timing",
        header: t("logs.headers.timing"),
        cell: ({ row }) => (
          <div className="flex flex-col items-center gap-0.5 whitespace-nowrap text-xs text-muted-foreground">
            <div>{t("logs.cell.duration")}: {formatDuration(row.original.latency_ms)}</div>
            <div>{t("logs.cell.ttft")}: {formatDuration(row.original.ttft_ms)}</div>
          </div>
        ),
        meta: {
          headerClassName: "w-24",
          skeletonClassName: "w-20 mx-auto",
        },
      },
      {
        id: "tokens",
        header: t("logs.headers.tokens"),
        cell: ({ row }) => (
          <div className="flex flex-col items-center gap-0.5 text-xs text-muted-foreground">
            <div>{t("logs.cell.input")}: {formatNumber(row.original.prompt_tokens)}</div>
            <div>{t("logs.cell.output")}: {formatNumber(row.original.completion_tokens)}</div>
          </div>
        ),
        meta: {
          headerClassName: "w-24",
          skeletonClassName: "w-18 mx-auto",
        },
      },
      {
        id: "cost",
        header: t("logs.headers.cost"),
        cell: ({ row }) => (
          <div className="truncate whitespace-nowrap font-mono text-xs text-muted-foreground">
            {row.original.estimated_cost_usd ? `$${row.original.estimated_cost_usd}` : "-"}
          </div>
        ),
        meta: {
          headerClassName: "w-20",
          skeletonClassName: "w-16 ml-auto",
        },
      },
      {
        id: "result",
        header: t("logs.headers.result"),
        cell: ({ row }) => (
          <div className="flex flex-col items-center gap-1">
            <div>
              {row.original.success ? (
                <Badge variant="success">{row.original.http_status ?? 200}</Badge>
              ) : (
                <Badge variant="destructive">{row.original.http_status ?? "ERR"}</Badge>
              )}
            </div>
            {(row.original.error_detail || row.original.error_kind) ? (
              <Tooltip>
                <TooltipTrigger asChild>
                  <div className="max-w-20 cursor-default truncate text-xs text-destructive/80">
                    {clampStr(
                      row.original.error_detail
                        ? humanizeErrorText(row.original.error_detail)
                        : row.original.error_kind || "-",
                      30
                    )}
                  </div>
                </TooltipTrigger>
                <TooltipContent className="max-w-130 whitespace-pre-wrap wrap-break-word">
                  {row.original.error_detail
                    ? humanizeErrorText(row.original.error_detail)
                    : row.original.error_kind}
                </TooltipContent>
              </Tooltip>
            ) : null}
          </div>
        ),
        meta: {
          headerClassName: "w-20",
          skeletonClassName: "w-12 mx-auto",
        },
      },
      {
        id: "details",
        header: t("common.details"),
        cell: ({ row }) => (
          <Button
            size="sm"
            variant="ghost"
            className="h-7 px-2 text-xs"
            onClick={() => {
              setExpandedEventId((current) =>
                current === row.original.id ? null : row.original.id
              );
            }}
          >
            {expandedEventId === row.original.id ? t("common.close") : t("common.details")}
          </Button>
        ),
        meta: {
          headerClassName: "w-14",
          skeletonClassName: "w-10 mx-auto",
        },
      },
    ],
    [channelNames, expandedEventId, t]
  );

  return (
    <div className="flex h-full min-h-0 flex-col gap-4 overflow-hidden">
      <PageHeader
        title={t("logs.title")}
        actions={
          <Button size="sm" variant="outline" onClick={() => void refresh(page)} disabled={loading}>
            <RefreshCw className={`h-4 w-4 mr-2 ${loading ? "animate-spin" : ""}`} />
            {t("common.refresh")}
          </Button>
        }
      />

      <Card className="flex-1 min-h-0 flex flex-col">
        <CardHeader>
          <div className="flex flex-wrap items-end gap-2 pt-2">
            <div className="space-y-1">
              <div className="text-xs text-muted-foreground">{t("logs.filters.dateRange")}</div>
              <DateRangePicker
                value={dateRange}
                onChange={(nextRange) => {
                  const next = dateRangeToStrings(nextRange);
                  void setSearch({
                    start: next?.start ?? "",
                    end: next?.end ?? "",
                  });
                }}
                placeholder={t("logs.filters.selectDateRange")}
                className="h-8"
                locale={locale}
              />
            </div>
            <div className="space-y-1">
              <div className="text-xs text-muted-foreground">{t("logs.filters.terminal")}</div>
              <Select
                value={search.protocol}
                onValueChange={(value) => {
                  void setSearch({
                    protocol: value as Protocol | "all",
                    channel: "all",
                  });
                }}
              >
                <SelectTrigger className="h-8 w-[140px]">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="all">{t("logs.filters.all")}</SelectItem>
                  <SelectItem value="openai">{protocolLabel(t, "openai")}</SelectItem>
                  <SelectItem value="anthropic">{protocolLabel(t, "anthropic")}</SelectItem>
                  <SelectItem value="gemini">{protocolLabel(t, "gemini")}</SelectItem>
                </SelectContent>
              </Select>
            </div>
            <div className="space-y-1">
              <div className="text-xs text-muted-foreground">{t("logs.filters.channel")}</div>
              <Select
                value={search.channel}
                onValueChange={(value) => {
                  void setSearch({ channel: value });
                }}
              >
                <SelectTrigger className="h-8 w-[190px]">
                  <div className="flex min-w-0 flex-1 items-center gap-2 overflow-hidden text-left">
                    {selectedChannel ? (
                      <>
                        <span className="inline-flex shrink-0 items-center whitespace-nowrap rounded-md border px-1 py-0.5 text-[10px] font-medium leading-none">
                          {protocolLabel(t, selectedChannel.protocol)}
                        </span>
                        <span className="truncate">{selectedChannel.name}</span>
                      </>
                    ) : (
                      <span className="truncate">{t("logs.filters.all")}</span>
                    )}
                  </div>
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="all">{t("logs.filters.all")}</SelectItem>
                  {filteredChannels.map((c) => {
                    const endpoint = protocolLabel(t, c.protocol);
                    return (
                      <SelectItem
                        key={c.id}
                        value={c.id}
                        textValue={`${c.name} ${endpoint}`}
                        className="py-2"
                      >
                        <span className="flex min-w-0 items-center gap-2">
                          <span className="inline-flex shrink-0 items-center whitespace-nowrap rounded-md border px-1 py-0.5 text-[10px] font-medium leading-none">
                            {endpoint}
                          </span>
                          <span className="truncate">{c.name}</span>
                        </span>
                      </SelectItem>
                    );
                  })}
                </SelectContent>
              </Select>
            </div>
            <div className="space-y-1">
              <div className="text-xs text-muted-foreground">{t("logs.filters.model")}</div>
              <Input
                value={search.model}
                onChange={(e) => {
                  void setSearch({ model: e.target.value });
                }}
                placeholder={t("logs.filters.modelPlaceholder")}
                className="h-8 w-[170px]"
              />
            </div>
            <div className="space-y-1">
              <div className="text-xs text-muted-foreground">{t("logs.filters.dimension")}</div>
              <Input
                value={search.requestId}
                onChange={(e) => {
                  void setSearch({ requestId: e.target.value });
                }}
                placeholder={t("logs.filters.dimensionPlaceholder")}
                className="h-8 w-[170px]"
              />
            </div>
            <div className="space-y-1">
              <div className="text-xs text-muted-foreground">{t("logs.filters.status")}</div>
              <Select
                value={search.status}
                onValueChange={(value) => {
                  void setSearch({
                    status: value as "all" | "success" | "failed",
                  });
                }}
              >
                <SelectTrigger className="h-8 w-[120px]">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="all">{t("logs.filters.all")}</SelectItem>
                  <SelectItem value="success">{t("logs.filters.success")}</SelectItem>
                  <SelectItem value="failed">{t("logs.filters.failed")}</SelectItem>
                </SelectContent>
              </Select>
            </div>

            <div className="flex items-center gap-2 ml-auto">
              <Button
                size="sm"
                variant="outline"
                onClick={() => {
                  if (page !== 1) {
                    void setSearch({ page: 1 });
                    return;
                  }
                  void refresh(1);
                }}
                disabled={loading}
              >
                {t("logs.filters.search")}
              </Button>
              <Button
                size="sm"
                variant="ghost"
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
                disabled={loading}
              >
                {t("logs.filters.reset")}
              </Button>
            </div>
          </div>
        </CardHeader>
        <CardContent className="p-0 flex flex-col min-h-0 flex-1">
          <DataTable
            columns={columns}
            data={events}
            loading={loading}
            getRowId={(row) => row.id}
            emptyState={t("logs.empty")}
            containerClassName="h-full overflow-y-auto"
            tableClassName="w-full table-fixed text-xs [&_th]:whitespace-nowrap [&_th]:text-center [&_td]:text-center"
            rowClassName={() => "h-[72px]"}
            isRowExpanded={(row) => row.original.id === expandedEventId}
            expandedRowCellClassName="p-0"
            renderExpandedRow={(row) => {
              const detailEvent = row.original;
              return (
                <div className="grid gap-3 rounded-lg border bg-muted/20 p-4 text-sm">
                  <div className="grid grid-cols-[120px_1fr] gap-2">
                    <div className="text-muted-foreground">{t("logs.details.id")}</div>
                    <div className="font-mono break-all">{detailEvent.id}</div>
                  </div>
                  <div className="grid grid-cols-[120px_1fr] gap-2">
                    <div className="text-muted-foreground">{t("logs.details.requestId")}</div>
                    <div className="font-mono break-all">{detailEvent.request_id ?? "-"}</div>
                  </div>
                  <div className="grid grid-cols-[120px_1fr] gap-2">
                    <div className="text-muted-foreground">{t("logs.headers.terminal")}</div>
                    <div>{protocolLabel(t, detailEvent.protocol)}</div>
                  </div>
                  <div className="grid grid-cols-[120px_1fr] gap-2">
                    <div className="text-muted-foreground">{t("logs.headers.channel")}</div>
                    <div className="break-all">
                      {channelNames.get(detailEvent.channel_id) ?? detailEvent.channel_id}
                    </div>
                  </div>
                  <div className="grid grid-cols-[120px_1fr] gap-2">
                    <div className="text-muted-foreground">{t("logs.headers.model")}</div>
                    <div className="break-all">{detailEvent.model ?? "-"}</div>
                  </div>
                  <div className="grid grid-cols-[120px_1fr] gap-2">
                    <div className="text-muted-foreground">{t("logs.headers.result")}</div>
                    <div>
                      {detailEvent.success ? (
                        <Badge variant="success">{detailEvent.http_status ?? 200}</Badge>
                      ) : (
                        <Badge variant="destructive">{detailEvent.http_status ?? "ERR"}</Badge>
                      )}
                    </div>
                  </div>
                  <div className="grid grid-cols-[120px_1fr] gap-2">
                    <div className="text-muted-foreground">{t("logs.headers.timing")}</div>
                    <div className="flex flex-wrap gap-x-4 gap-y-1">
                      <div>{t("logs.cell.duration")}: {formatDuration(detailEvent.latency_ms)}</div>
                      <div>{t("logs.cell.ttft")}: {formatDuration(detailEvent.ttft_ms)}</div>
                    </div>
                  </div>
                  <div className="grid grid-cols-[120px_1fr] gap-2">
                    <div className="text-muted-foreground">{t("logs.headers.tokens")}</div>
                    <div className="flex flex-wrap gap-x-4 gap-y-1">
                      <div>{t("logs.cell.input")}: {formatNumber(detailEvent.prompt_tokens)}</div>
                      <div>{t("logs.cell.output")}: {formatNumber(detailEvent.completion_tokens)}</div>
                      <div>{t("logs.cell.total")}: {formatNumber(detailEvent.total_tokens)}</div>
                      {detailEvent.cache_read_tokens != null ? (
                        <div>{t("logs.cell.cacheRead")}: {formatNumber(detailEvent.cache_read_tokens)}</div>
                      ) : null}
                      {detailEvent.cache_write_tokens != null ? (
                        <div>{t("logs.cell.cacheWrite")}: {formatNumber(detailEvent.cache_write_tokens)}</div>
                      ) : null}
                    </div>
                  </div>
                  <div className="grid grid-cols-[120px_1fr] gap-2">
                    <div className="text-muted-foreground">{t("logs.headers.cost")}</div>
                    <div className="font-mono">
                      {detailEvent.estimated_cost_usd ? `$${detailEvent.estimated_cost_usd}` : "-"}
                    </div>
                  </div>
                  <div className="grid grid-cols-[120px_1fr] gap-2">
                    <div className="text-muted-foreground">{t("logs.details.estimatedSpend")}</div>
                    <div className="font-mono">
                      {(() => {
                        const est = parseDecimalLike(detailEvent.estimated_cost_usd);
                        const ch = channelsById.get(detailEvent.channel_id);
                        const real = Number(ch?.real_multiplier ?? 1);
                        if (!est || est <= 0) return "-";
                        if (!Number.isFinite(real) || real < 0) return "-";
                        return formatMoney(est * real, currency);
                      })()}
                    </div>
                  </div>
                  <div className="grid grid-cols-[120px_1fr] gap-2">
                    <div className="text-muted-foreground">{t("logs.details.errorKind")}</div>
                    <div className="font-mono break-all">{detailEvent.error_kind ?? "-"}</div>
                  </div>
                  <div className="grid grid-cols-[120px_1fr] gap-2">
                    <div className="text-muted-foreground">{t("logs.details.errorDetail")}</div>
                    <pre className="rounded border bg-background/70 p-2 text-xs whitespace-pre-wrap break-words">
                      {detailEvent.error_detail ? humanizeErrorText(detailEvent.error_detail) : "-"}
                    </pre>
                  </div>
                </div>
              );
            }}
            pagination={{
              page,
              pageSize,
              total,
              disabled: loading,
              manual: true,
              onPageChange: (next) => {
                void setSearch({ page: next });
              },
              onPageSizeChange: (next) => {
                void setSearch({ pageSize: next, page: 1 });
              },
            }}
          />
        </CardContent>
      </Card>
    </div>
  );
}
