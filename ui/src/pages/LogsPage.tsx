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
  Switch,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui";
import { useI18n } from "@/lib/i18n";
import { listChannels, usageRecent, type Channel, type UsageEvent } from "../api";
import { clampStr, formatDateTime, formatDuration, terminalLabel } from "../lib";

export function LogsPage() {
  const { t } = useI18n();
  const [events, setEvents] = useState<UsageEvent[]>([]);
  const [channels, setChannels] = useState<Channel[]>([]);
  const [loading, setLoading] = useState(false);
  const [onlyFailures, setOnlyFailures] = useState(false);

  const channelNames = useMemo(() => {
    const m = new Map<string, string>();
    for (const c of channels) m.set(c.id, c.name);
    return m;
  }, [channels]);

  const filtered = useMemo(() => {
    if (!onlyFailures) return events;
    return events.filter((e) => !e.success);
  }, [events, onlyFailures]);

  async function refresh() {
    setLoading(true);
    try {
      const [cs, es] = await Promise.all([listChannels(), usageRecent(100)]);
      setChannels(cs);
      setEvents(es);
    } catch (e) {
      toast.error(t("logs.toast.loadFail"), { description: String(e) });
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    refresh();
  }, []);

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
          <div className="flex items-center justify-between">
            <div>
              <CardTitle>{t("logs.table.title")}</CardTitle>
              <CardDescription>{t("logs.table.subtitle")}</CardDescription>
            </div>
            <label className="flex items-center gap-2 text-sm">
              <Switch checked={onlyFailures} onCheckedChange={setOnlyFailures} />
              {t("logs.onlyFailures")}
            </label>
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
                  <TableHead>{t("logs.headers.error")}</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {filtered.length === 0 ? (
                  <TableRow>
                    <TableCell
                      colSpan={10}
                      className="text-center text-muted-foreground py-8"
                    >
                      {t("logs.empty")}
                    </TableCell>
                  </TableRow>
                ) : (
                  filtered.map((e) => (
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
                      <TableCell className="text-sm text-muted-foreground">
                        {e.error_kind ? clampStr(e.error_kind, 30) : "-"}
                      </TableCell>
                    </TableRow>
                  ))
                )}
              </TableBody>
            </Table>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
