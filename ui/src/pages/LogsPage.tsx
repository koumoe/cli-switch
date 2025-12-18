import React, { useEffect, useMemo, useState } from "react";
import { RefreshCw } from "lucide-react";
import { toast } from "sonner";
import {
  Button,
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
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
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui";
import { useI18n } from "@/lib/i18n";
import {
  listChannels,
  usageList,
  type Channel,
  type Protocol,
  type UsageEvent,
} from "../api";
import { clampStr, formatDateTime, formatDuration, terminalLabel } from "../lib";

function parseLocalDateStartMs(s: string): number | undefined {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(s.trim());
  if (!m) return undefined;
  const y = Number(m[1]);
  const mo = Number(m[2]) - 1;
  const d = Number(m[3]);
  const dt = new Date(y, mo, d, 0, 0, 0, 0);
  const ms = dt.getTime();
  return Number.isFinite(ms) ? ms : undefined;
}

function parseLocalDateEndMs(s: string): number | undefined {
  const start = parseLocalDateStartMs(s);
  if (start === undefined) return undefined;
  return start + 86_399_999;
}

export function LogsPage() {
  const { t } = useI18n();
  const [events, setEvents] = useState<UsageEvent[]>([]);
  const [channels, setChannels] = useState<Channel[]>([]);
  const [loading, setLoading] = useState(false);
  const [total, setTotal] = useState(0);

  const [startDate, setStartDate] = useState("");
  const [endDate, setEndDate] = useState("");
  const [protocol, setProtocol] = useState<Protocol | "all">("all");
  const [channelId, setChannelId] = useState<string>("all");
  const [model, setModel] = useState("");
  const [requestId, setRequestId] = useState("");
  const [status, setStatus] = useState<"all" | "success" | "failed">("all");

  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState(50);

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
      startDate: string;
      endDate: string;
      protocol: Protocol | "all";
      channelId: string;
      model: string;
      requestId: string;
      status: "all" | "success" | "failed";
    }>
  ) {
    setLoading(true);
    try {
      const sStartDate = overrides?.startDate ?? startDate;
      const sEndDate = overrides?.endDate ?? endDate;
      const sProtocol = overrides?.protocol ?? protocol;
      const sChannelId = overrides?.channelId ?? channelId;
      const sModel = overrides?.model ?? model;
      const sRequestId = overrides?.requestId ?? requestId;
      const sStatus = overrides?.status ?? status;

      const start_ms = parseLocalDateStartMs(sStartDate);
      const end_ms = parseLocalDateEndMs(sEndDate);
      const safePageSize = Number.isFinite(pageSize) && pageSize > 0 ? pageSize : 50;
      const rawOffset = (nextPage - 1) * safePageSize;
      const safeOffset = Number.isFinite(rawOffset) && rawOffset >= 0 ? rawOffset : 0;

      const res = await usageList({
        start_ms,
        end_ms,
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

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-lg font-semibold">{t("logs.title")}</h1>
          <p className="text-muted-foreground text-xs mt-0.5">{t("logs.subtitle")}</p>
        </div>
        <Button size="sm" variant="outline" onClick={refresh} disabled={loading}>
          <RefreshCw className={`h-4 w-4 mr-2 ${loading ? "animate-spin" : ""}`} />
          {t("common.refresh")}
        </Button>
      </div>

      <Card>
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
              <div className="text-xs text-muted-foreground">{t("logs.filters.startDate")}</div>
              <Input
                type="date"
                value={startDate}
                onChange={(e) => setStartDate(e.target.value)}
                className="h-8 w-[150px]"
              />
            </div>
            <div className="space-y-1">
              <div className="text-xs text-muted-foreground">{t("logs.filters.endDate")}</div>
              <Input
                type="date"
                value={endDate}
                onChange={(e) => setEndDate(e.target.value)}
                className="h-8 w-[150px]"
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
                  <SelectItem value="openai">{terminalLabel("openai")}</SelectItem>
                  <SelectItem value="anthropic">{terminalLabel("anthropic")}</SelectItem>
                  <SelectItem value="gemini">{terminalLabel("gemini")}</SelectItem>
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
                  {channels.map((c) => (
                    <SelectItem key={c.id} value={c.id}>
                      {c.name}
                    </SelectItem>
                  ))}
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
                    startDate: "",
                    endDate: "",
                    protocol: "all" as const,
                    channelId: "all",
                    model: "",
                    requestId: "",
                    status: "all" as const,
                  };
                  setStartDate(next.startDate);
                  setEndDate(next.endDate);
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
        <CardContent className="p-0">
          <div className="overflow-x-auto">
            <Table className="min-w-[1100px]">
              <TableHeader>
                <TableRow>
                  <TableHead className="w-[160px]">{t("logs.headers.time")}</TableHead>
                  <TableHead>{t("logs.headers.terminal")}</TableHead>
                  <TableHead>{t("logs.headers.channel")}</TableHead>
                  <TableHead>{t("logs.headers.model")}</TableHead>
                  <TableHead>{t("logs.headers.status")}</TableHead>
                  <TableHead className="text-right">{t("logs.headers.ttft")}</TableHead>
                  <TableHead className="text-right">{t("logs.headers.duration")}</TableHead>
                  <TableHead className="text-right">{t("logs.headers.inputTokens")}</TableHead>
                  <TableHead className="text-right">{t("logs.headers.outputTokens")}</TableHead>
                  <TableHead className="text-right">{t("logs.headers.cost")}</TableHead>
                  <TableHead>{t("logs.headers.error")}</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {events.length === 0 ? (
                  <TableRow>
                    <TableCell
                      colSpan={11}
                      className="text-center text-muted-foreground py-8"
                    >
                      {t("logs.empty")}
                    </TableCell>
                  </TableRow>
                ) : (
                  events.map((e) => (
                    <TableRow key={e.id}>
                      <TableCell className="text-muted-foreground text-sm">
                        {formatDateTime(e.ts_ms)}
                      </TableCell>
                      <TableCell>
                        <Badge variant="outline">{terminalLabel(e.protocol)}</Badge>
                      </TableCell>
                      <TableCell className="text-sm">
                        {channelNames.get(e.channel_id) ?? "-"}
                      </TableCell>
                      <TableCell className="text-sm text-muted-foreground">
                        {e.model ?? "-"}
                      </TableCell>
                      <TableCell>
                        {e.success ? (
                          <Badge variant="success">{e.http_status ?? 200}</Badge>
                        ) : (
                          <Badge variant="destructive">
                            {e.http_status ?? "ERR"}
                          </Badge>
                        )}
                      </TableCell>
                      <TableCell className="text-right text-muted-foreground">
                        {formatDuration(e.ttft_ms)}
                      </TableCell>
                      <TableCell className="text-right text-muted-foreground">
                        {formatDuration(e.latency_ms)}
                      </TableCell>
                      <TableCell className="text-right text-muted-foreground">
                        {e.prompt_tokens?.toLocaleString() ?? "-"}
                      </TableCell>
                      <TableCell className="text-right text-muted-foreground">
                        {e.completion_tokens?.toLocaleString() ?? "-"}
                      </TableCell>
                      <TableCell className="text-right font-mono text-xs text-muted-foreground">
                        {e.estimated_cost_usd ? `$${e.estimated_cost_usd}` : "-"}
                      </TableCell>
                      <TableCell className="text-sm text-muted-foreground">
                        {e.error_detail || e.error_kind ? (
                          <Tooltip>
                            <TooltipTrigger asChild>
                              <span className="cursor-default">
                                {clampStr(e.error_detail || e.error_kind || "-", 60)}
                              </span>
                            </TooltipTrigger>
                            <TooltipContent className="max-w-[520px] whitespace-pre-wrap break-words">
                              {e.error_detail || e.error_kind}
                            </TooltipContent>
                          </Tooltip>
                        ) : (
                          "-"
                        )}
                      </TableCell>
                    </TableRow>
                  ))
                )}
              </TableBody>
            </Table>
          </div>

          <div className="flex items-center justify-between px-4 py-3 border-t bg-background">
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
