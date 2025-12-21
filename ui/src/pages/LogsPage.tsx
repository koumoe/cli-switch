import React, { useEffect, useMemo, useRef, useState } from "react";
import { RefreshCw } from "lucide-react";
import { toast } from "sonner";
import type { DateRange } from "react-day-picker";
import {
  Button,
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
  Badge,
  DateRangePicker,
  dateRangeToMs,
  Input,
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
  Tooltip,
  TooltipContent,
  TooltipTrigger,
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui";
import { humanizeErrorText } from "@/lib/error";
import { useI18n } from "@/lib/i18n";
import { useWindowEvent } from "@/lib/useWindowEvent";
import {
  listChannels,
  usageList,
  type Channel,
  type Protocol,
  type UsageEvent,
} from "../api";
import { clampStr, formatDateTime, formatDuration, protocolLabel, protocolLabelKey } from "../lib";

export function LogsPage() {
  const { locale, t } = useI18n();
  const [events, setEvents] = useState<UsageEvent[]>([]);
  const [channels, setChannels] = useState<Channel[]>([]);
  const [loading, setLoading] = useState(false);
  const loadingRef = useRef(false);
  const [total, setTotal] = useState(0);
  const [detailOpen, setDetailOpen] = useState(false);
  const [detailEvent, setDetailEvent] = useState<UsageEvent | null>(null);

  const [dateRange, setDateRange] = useState<DateRange | undefined>(undefined);
  const [protocol, setProtocol] = useState<Protocol | "all">("all");
  const [channelId, setChannelId] = useState<string>("all");
  const [model, setModel] = useState("");
  const [requestId, setRequestId] = useState("");
  const [status, setStatus] = useState<"all" | "success" | "failed">("all");

  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState(20);

  const channelNames = useMemo(() => {
    const m = new Map<string, string>();
    for (const c of channels) m.set(c.id, c.name);
    return m;
  }, [channels]);

  const totalPages = useMemo(() => {
    if (total <= 0) return 1;
    return Math.max(1, Math.ceil(total / pageSize));
  }, [total, pageSize]);

  async function refresh(
    nextPage = page,
    overrides?: Partial<{
      dateRange: DateRange | undefined;
      protocol: Protocol | "all";
      channelId: string;
      model: string;
      requestId: string;
      status: "all" | "success" | "failed";
    }>
  ) {
    setLoading(true);
    try {
      loadingRef.current = true;
      const sDateRange = overrides?.dateRange !== undefined ? overrides.dateRange : dateRange;
      const sProtocol = overrides?.protocol ?? protocol;
      const sChannelId = overrides?.channelId ?? channelId;
      const sModel = overrides?.model ?? model;
      const sRequestId = overrides?.requestId ?? requestId;
      const sStatus = overrides?.status ?? status;

      const msRange = dateRangeToMs(sDateRange);
      const safePageSize = Number.isFinite(pageSize) && pageSize > 0 ? pageSize : 50;
      const rawOffset = (nextPage - 1) * safePageSize;
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
      setPage(nextPage);
    } catch (e) {
      toast.error(t("logs.toast.loadFail"), { description: String(e) });
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
    refresh(1);
  }, [pageSize]);

  useWindowEvent("cliswitch-usage-changed", () => {
    if (loadingRef.current) return;
    void refresh(page);
  });

  return (
    <div className="flex flex-col gap-4 h-full min-h-0">
      <Dialog
        open={detailOpen}
        onOpenChange={(v) => {
          setDetailOpen(v);
          if (!v) setDetailEvent(null);
        }}
      >
        <DialogContent className="sm:max-w-[760px]">
          <DialogHeader>
            <DialogTitle>{t("logs.title")} - {t("common.details")}</DialogTitle>
            <DialogDescription>
              {detailEvent ? formatDateTime(detailEvent.ts_ms) : "-"}
            </DialogDescription>
          </DialogHeader>

          {detailEvent && (
            <div className="grid gap-3 text-sm">
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
                <div className="break-all">{channelNames.get(detailEvent.channel_id) ?? detailEvent.channel_id}</div>
              </div>
              <div className="grid grid-cols-[120px_1fr] gap-2">
                <div className="text-muted-foreground">{t("logs.headers.model")}</div>
                <div className="break-all">{detailEvent.model ?? "-"}</div>
              </div>
              <div className="grid grid-cols-[120px_1fr] gap-2">
                <div className="text-muted-foreground">{t("logs.details.routeId")}</div>
                <div className="font-mono break-all">{detailEvent.route_id ?? "-"}</div>
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
                  <div>{t("logs.cell.input")}: {detailEvent.prompt_tokens?.toLocaleString() ?? "-"}</div>
                  <div>{t("logs.cell.output")}: {detailEvent.completion_tokens?.toLocaleString() ?? "-"}</div>
                  <div>{t("logs.cell.total")}: {detailEvent.total_tokens?.toLocaleString() ?? "-"}</div>
                  {detailEvent.cache_read_tokens != null && (
                    <div>{t("logs.cell.cacheRead")}: {detailEvent.cache_read_tokens.toLocaleString()}</div>
                  )}
                  {detailEvent.cache_write_tokens != null && (
                    <div>{t("logs.cell.cacheWrite")}: {detailEvent.cache_write_tokens.toLocaleString()}</div>
                  )}
                </div>
              </div>
              <div className="grid grid-cols-[120px_1fr] gap-2">
                <div className="text-muted-foreground">{t("logs.headers.cost")}</div>
                <div className="font-mono">
                  {detailEvent.estimated_cost_usd ? `$${detailEvent.estimated_cost_usd}` : "-"}
                </div>
              </div>
              <div className="grid grid-cols-[120px_1fr] gap-2">
                <div className="text-muted-foreground">{t("logs.details.errorKind")}</div>
                <div className="font-mono break-all">{detailEvent.error_kind ?? "-"}</div>
              </div>
              <div className="grid grid-cols-[120px_1fr] gap-2">
                <div className="text-muted-foreground">{t("logs.details.errorDetail")}</div>
                <pre className="text-xs whitespace-pre-wrap break-words rounded border bg-muted/30 p-2">{detailEvent.error_detail ? humanizeErrorText(detailEvent.error_detail) : "-"}</pre>
              </div>
            </div>
          )}
        </DialogContent>
      </Dialog>

      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-lg font-semibold">{t("logs.title")}</h1>
          <p className="text-muted-foreground text-xs mt-0.5">{t("logs.subtitle")}</p>
        </div>
        <div className="flex flex-col items-end gap-1">
          <Button size="sm" variant="outline" onClick={() => refresh(page)} disabled={loading}>
            <RefreshCw className={`h-4 w-4 mr-2 ${loading ? "animate-spin" : ""}`} />
            {t("common.refresh")}
          </Button>
          <div className="text-xs text-muted-foreground">{t("common.autoRefresh1m")}</div>
        </div>
      </div>

      <Card className="flex-1 min-h-0 flex flex-col">
        <CardHeader>
          <div className="flex items-start justify-between gap-4">
            <div>
              <CardTitle>{t("logs.table.title")}</CardTitle>
              <CardDescription>{t("logs.table.subtitle")}</CardDescription>
            </div>
            <div className="text-xs text-muted-foreground">
              {t("logs.pagination.total", { total: total.toLocaleString() })}
            </div>
          </div>

          <div className="flex flex-wrap items-end gap-2 pt-2">
            <div className="space-y-1">
              <div className="text-xs text-muted-foreground">{t("logs.filters.dateRange")}</div>
              <DateRangePicker
                value={dateRange}
                onChange={setDateRange}
                placeholder={t("logs.filters.selectDateRange")}
                className="h-8"
                locale={locale}
              />
            </div>
            <div className="space-y-1">
              <div className="text-xs text-muted-foreground">{t("logs.filters.terminal")}</div>
              <Select value={protocol} onValueChange={(v) => setProtocol(v as Protocol | "all")}>
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
              <Select value={channelId} onValueChange={setChannelId}>
                <SelectTrigger className="h-8 w-[170px]">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="all">{t("logs.filters.all")}</SelectItem>
                  {channels.map((c) => {
                    const endpoint = protocolLabel(t, c.protocol);
                    return (
                      <SelectItem
                        key={c.id}
                        value={c.id}
                        textValue={`${c.name} ${endpoint}`}
                      >
                        <div className="flex items-center gap-2 min-w-0">
                          <Badge variant="outline" className="text-[10px] px-1 py-0">
                            {endpoint}
                          </Badge>
                          <span className="truncate">{c.name}</span>
                        </div>
                      </SelectItem>
                    );
                  })}
                </SelectContent>
              </Select>
            </div>
            <div className="space-y-1">
              <div className="text-xs text-muted-foreground">{t("logs.filters.model")}</div>
              <Input
                value={model}
                onChange={(e) => setModel(e.target.value)}
                placeholder={t("logs.filters.modelPlaceholder")}
                className="h-8 w-[170px]"
              />
            </div>
            <div className="space-y-1">
              <div className="text-xs text-muted-foreground">{t("logs.filters.dimension")}</div>
              <Input
                value={requestId}
                onChange={(e) => setRequestId(e.target.value)}
                placeholder={t("logs.filters.dimensionPlaceholder")}
                className="h-8 w-[170px]"
              />
            </div>
            <div className="space-y-1">
              <div className="text-xs text-muted-foreground">{t("logs.filters.status")}</div>
              <Select value={status} onValueChange={(v) => setStatus(v as typeof status)}>
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
              <Button size="sm" variant="outline" onClick={() => refresh(1)} disabled={loading}>
                {t("logs.filters.search")}
              </Button>
              <Button
                size="sm"
                variant="ghost"
                onClick={() => {
                  const next = {
                    dateRange: undefined as DateRange | undefined,
                    protocol: "all" as const,
                    channelId: "all",
                    model: "",
                    requestId: "",
                    status: "all" as const,
                  };
                  setDateRange(next.dateRange);
                  setProtocol(next.protocol);
                  setChannelId(next.channelId);
                  setModel(next.model);
                  setRequestId(next.requestId);
                  setStatus(next.status);
                  refresh(1, next);
                }}
                disabled={loading}
              >
                {t("logs.filters.reset")}
              </Button>
            </div>
          </div>
        </CardHeader>
        <CardContent className="p-0 flex flex-col min-h-0 flex-1">
          <div className="flex-1 min-h-0 overflow-hidden">
            <Table
              containerClassName="h-full overflow-y-auto"
              className="w-full table-fixed text-xs [&_th]:text-center [&_td]:text-center"
            >
              <TableHeader className="sticky top-0 bg-background z-10 [&_th]:whitespace-nowrap">
                <TableRow>
                  <TableHead className="w-28">{t("logs.headers.time")}</TableHead>
                  <TableHead className="w-16">{t("logs.headers.terminal")}</TableHead>
                  <TableHead className="w-24">{t("logs.headers.channel")}</TableHead>
                  <TableHead className="w-28">{t("logs.headers.model")}</TableHead>
                  <TableHead className="w-24">{t("logs.headers.timing")}</TableHead>
                  <TableHead className="w-24">{t("logs.headers.tokens")}</TableHead>
                  <TableHead className="w-20">{t("logs.headers.cost")}</TableHead>
                  <TableHead className="w-20">{t("logs.headers.result")}</TableHead>
                  <TableHead className="w-14">{t("common.details")}</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {events.length === 0 ? (
                  <TableRow>
                    <TableCell
                      colSpan={9}
                      className="text-center text-muted-foreground py-8"
                    >
                      {t("logs.empty")}
                    </TableCell>
                  </TableRow>
                ) : (
                  events.map((e) => (
                    <TableRow key={e.id} className="h-[72px]">
                      <TableCell className="py-3">
                        <div className="leading-4">{formatDateTime(e.ts_ms)}</div>
                      </TableCell>
                      <TableCell className="py-3">
                        <Badge variant="outline">{protocolLabel(t, e.protocol)}</Badge>
                      </TableCell>
                      <TableCell className="py-3">
                        <div className="truncate max-w-22.5">
                          {channelNames.get(e.channel_id) ?? "-"}
                        </div>
                      </TableCell>
                      <TableCell className="py-3">
                        <div className="text-muted-foreground truncate max-w-25">
                          {e.model ?? "-"}
                        </div>
                      </TableCell>
                      <TableCell className="py-3">
                        <div className="flex flex-col items-center gap-0.5 text-xs text-muted-foreground whitespace-nowrap">
                          <div>{t("logs.cell.duration")}: {formatDuration(e.latency_ms)}</div>
                          <div>{t("logs.cell.ttft")}: {formatDuration(e.ttft_ms)}</div>
                        </div>
                      </TableCell>
                      <TableCell className="py-3">
                        <div className="flex flex-col items-center gap-0.5 text-xs text-muted-foreground">
                          <div>{t("logs.cell.input")}: {e.prompt_tokens?.toLocaleString() ?? "-"}</div>
                          <div>{t("logs.cell.output")}: {e.completion_tokens?.toLocaleString() ?? "-"}</div>
                        </div>
                      </TableCell>
                      <TableCell className="py-3">
                        <div className="text-xs font-mono text-muted-foreground whitespace-nowrap truncate">
                          {e.estimated_cost_usd ? `$${e.estimated_cost_usd}` : "-"}
                        </div>
                      </TableCell>
                      <TableCell className="py-3">
                        <div className="flex flex-col items-center gap-1">
                          <div>
                            {e.success ? (
                              <Badge variant="success">{e.http_status ?? 200}</Badge>
                            ) : (
                              <Badge variant="destructive">
                                {e.http_status ?? "ERR"}
                              </Badge>
                            )}
                          </div>
                          {(e.error_detail || e.error_kind) && (
                            <Tooltip>
                              <TooltipTrigger asChild>
                                <div className="text-xs text-destructive/80 truncate max-w-20 cursor-default">
                                  {clampStr(
                                    e.error_detail
                                      ? humanizeErrorText(e.error_detail)
                                      : e.error_kind || "-",
                                    30
                                  )}
                                </div>
                              </TooltipTrigger>
                              <TooltipContent className="max-w-130 whitespace-pre-wrap wrap-break-word">
                                {e.error_detail
                                  ? humanizeErrorText(e.error_detail)
                                  : e.error_kind}
                              </TooltipContent>
                            </Tooltip>
                          )}
                        </div>
                      </TableCell>
                      <TableCell className="py-3">
                        <Button
                          size="sm"
                          variant="ghost"
                          className="h-7 px-2 text-xs"
                          onClick={() => {
                            setDetailEvent(e);
                            setDetailOpen(true);
                          }}
                        >
                          {t("common.open")}
                        </Button>
                      </TableCell>
                    </TableRow>
                  ))
                )}
              </TableBody>
            </Table>
          </div>

          <div className="flex items-center justify-between px-4 py-3 border-t bg-background rounded-b-lg">
            <div className="flex items-center gap-2 text-sm text-muted-foreground">
              {t("logs.pagination.page", { page, totalPages })}
              <Select
                value={String(pageSize)}
                onValueChange={(v) => {
                  const n = Number(v);
                  if (Number.isFinite(n) && n > 0) setPageSize(n);
                }}
              >
                <SelectTrigger className="h-8 w-[100px]">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="20">20</SelectItem>
                  <SelectItem value="50">50</SelectItem>
                  <SelectItem value="100">100</SelectItem>
                  <SelectItem value="200">200</SelectItem>
                </SelectContent>
              </Select>
              <span>{t("logs.pagination.perPage")}</span>
            </div>
            <div className="flex items-center gap-2">
              <Button
                size="sm"
                variant="outline"
                onClick={() => refresh(Math.max(1, page - 1))}
                disabled={loading || page <= 1}
              >
                {t("logs.pagination.prev")}
              </Button>
              <Button
                size="sm"
                variant="outline"
                onClick={() => refresh(Math.min(totalPages, page + 1))}
                disabled={loading || page >= totalPages}
              >
                {t("logs.pagination.next")}
              </Button>
            </div>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
