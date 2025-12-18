import React, { useEffect, useRef, useState } from "react";
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
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui";
import { useI18n } from "@/lib/i18n";
import {
  statsSummary,
  statsChannels,
  type StatsSummary,
  type ChannelStats,
} from "../api";
import { protocolLabel } from "../lib";

export function MonitorPage() {
  const { t } = useI18n();
  const colClass = {
    channel: "w-28",
    terminal: "w-20",
    requests: "w-16",
    success: "w-16",
    failed: "w-16",
    cost: "w-28",
    avgLatency: "w-24",
  } as const;
  const [stats, setStats] = useState<StatsSummary | null>(null);
  const [channelStats, setChannelStats] = useState<ChannelStats[]>([]);
  const [loading, setLoading] = useState(false);
  const loadingRef = useRef(false);

  const [range, setRange] = useState<"today" | "month">("today");

  async function refresh() {
    setLoading(true);
    try {
      loadingRef.current = true;
      const [st, cst] = await Promise.all([statsSummary(range), statsChannels(range)]);
      setStats(st);
      setChannelStats(
        [...cst.items].sort((a, b) => {
          if (b.success !== a.success) return b.success - a.success;
          if (b.requests !== a.requests) return b.requests - a.requests;
          return a.name.localeCompare(b.name);
        })
      );
    } catch (e) {
      toast.error(t("monitor.toast.loadFail"), { description: String(e) });
    } finally {
      setLoading(false);
      loadingRef.current = false;
    }
  }

  useEffect(() => {
    refresh();
  }, [range]);

  // 自动刷新：每分钟一次（保留手动刷新按钮）
  useEffect(() => {
    const id = window.setInterval(() => {
      if (loadingRef.current) return;
      void refresh();
    }, 60_000);
    return () => window.clearInterval(id);
  }, [range]);

  const successRate =
    stats && stats.requests > 0
      ? Math.round((stats.success / stats.requests) * 100)
      : 0;

  return (
    <div className="space-y-4 pb-4">
      {/* 页面标题 */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-lg font-semibold">{t("monitor.title")}</h1>
          <p className="text-muted-foreground text-xs mt-0.5">
            {t("monitor.subtitle")}
          </p>
        </div>
        <div className="flex flex-col items-end gap-1">
          <div className="flex items-center gap-2">
            <Select
              value={range}
              onValueChange={(v) => setRange(v as "today" | "month")}
            >
              <SelectTrigger className="w-[110px] h-8">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="today">{t("monitor.range.today")}</SelectItem>
                <SelectItem value="month">{t("monitor.range.month")}</SelectItem>
              </SelectContent>
            </Select>
            <Button size="sm" variant="outline" onClick={refresh} disabled={loading}>
              <RefreshCw className={`h-4 w-4 mr-2 ${loading ? "animate-spin" : ""}`} />
              {t("common.refresh")}
            </Button>
          </div>
          <div className="text-xs text-muted-foreground">{t("common.autoRefresh1m")}</div>
        </div>
      </div>

      {/* 统计卡片 */}
      <div className="grid gap-3 md:grid-cols-4">
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
      </div>

      {/* 渠道统计 */}
      {channelStats.length > 0 && (
        <Card>
          <CardHeader>
            <CardTitle>{t("monitor.channelStats.title")}</CardTitle>
            <CardDescription>{t("monitor.channelStats.subtitle")}</CardDescription>
          </CardHeader>
          <CardContent className="p-0">
	            <Table>
	              <TableHeader>
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
	                  <TableHead className={colClass.cost}>
	                    {t("monitor.channelStats.headers.cost")}
	                  </TableHead>
	                  <TableHead className={colClass.avgLatency}>
	                    {t("monitor.channelStats.headers.avgLatency")}
	                  </TableHead>
	                </TableRow>
	              </TableHeader>
              <TableBody>
                {channelStats.map((cs) => (
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
                    <TableCell className="text-muted-foreground">
                      {cs.avg_latency_ms
                        ? `${Math.round(cs.avg_latency_ms)}ms`
                        : "-"}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </CardContent>
        </Card>
      )}
    </div>
  );
}
