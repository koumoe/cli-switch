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
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
  Switch,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui";
import {
  listChannels,
  usageRecent,
  statsSummary,
  statsChannels,
  type Channel,
  type UsageEvent,
  type StatsSummary,
  type ChannelStats,
} from "../api";
import { formatDateTime, formatDuration, clampStr, terminalLabel } from "../lib";

export function MonitorPage() {
  const [events, setEvents] = useState<UsageEvent[]>([]);
  const [channels, setChannels] = useState<Channel[]>([]);
  const [stats, setStats] = useState<StatsSummary | null>(null);
  const [channelStats, setChannelStats] = useState<ChannelStats[]>([]);
  const [loading, setLoading] = useState(false);

  const [onlyFailures, setOnlyFailures] = useState(false);
  const [range, setRange] = useState<"today" | "month">("today");

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
      const [cs, es, st, cst] = await Promise.all([
        listChannels(),
        usageRecent(100),
        statsSummary(range),
        statsChannels(range),
      ]);
      setChannels(cs);
      setEvents(es);
      setStats(st);
      setChannelStats(cst.items);
    } catch (e) {
      toast.error("加载失败", { description: String(e) });
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    refresh();
  }, [range]);

  const successRate =
    stats && stats.requests > 0
      ? Math.round((stats.success / stats.requests) * 100)
      : 0;

  return (
    <div className="space-y-6">
      {/* 页面标题 */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-semibold tracking-tight">监控</h1>
          <p className="text-muted-foreground text-sm mt-1">
            请求日志、统计和渠道性能
          </p>
        </div>
        <div className="flex items-center gap-2">
          <Select
            value={range}
            onValueChange={(v) => setRange(v as "today" | "month")}
          >
            <SelectTrigger className="w-[100px]">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="today">今天</SelectItem>
              <SelectItem value="month">本月</SelectItem>
            </SelectContent>
          </Select>
          <Button variant="outline" onClick={refresh} disabled={loading}>
            <RefreshCw className={`h-4 w-4 mr-2 ${loading ? "animate-spin" : ""}`} />
            刷新
          </Button>
        </div>
      </div>

      {/* 统计卡片 */}
      <div className="grid gap-4 md:grid-cols-4">
        <Card>
          <CardHeader className="pb-2">
            <CardDescription>总请求</CardDescription>
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">{stats?.requests ?? "-"}</div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="pb-2">
            <CardDescription>成功率</CardDescription>
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">{successRate}%</div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="pb-2">
            <CardDescription>失败数</CardDescription>
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold text-destructive">
              {stats?.failed ?? "-"}
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="pb-2">
            <CardDescription>预估成本</CardDescription>
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">
              ${stats?.estimated_cost_usd ?? "-"}
            </div>
          </CardContent>
        </Card>
      </div>

      {/* 渠道统计 */}
      {channelStats.length > 0 && (
        <Card>
          <CardHeader>
            <CardTitle>渠道统计</CardTitle>
            <CardDescription>各渠道的请求量和性能</CardDescription>
          </CardHeader>
          <CardContent className="p-0">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>渠道</TableHead>
                  <TableHead>终端</TableHead>
                  <TableHead className="text-right">请求</TableHead>
                  <TableHead className="text-right">成功</TableHead>
                  <TableHead className="text-right">失败</TableHead>
                  <TableHead className="text-right">平均延迟</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {channelStats.map((cs) => (
                  <TableRow key={cs.channel_id}>
                    <TableCell className="font-medium">{cs.name}</TableCell>
                    <TableCell>
                      <Badge variant="outline">{terminalLabel(cs.protocol)}</Badge>
                    </TableCell>
                    <TableCell className="text-right">{cs.requests}</TableCell>
                    <TableCell className="text-right text-success">
                      {cs.success}
                    </TableCell>
                    <TableCell className="text-right text-destructive">
                      {cs.failed}
                    </TableCell>
                    <TableCell className="text-right text-muted-foreground">
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

      {/* 请求日志 */}
      <Card>
        <CardHeader>
          <div className="flex items-center justify-between">
            <div>
              <CardTitle>请求日志</CardTitle>
              <CardDescription>最近 100 条请求记录</CardDescription>
            </div>
            <div className="flex items-center gap-2">
              <label className="flex items-center gap-2 text-sm">
                <Switch checked={onlyFailures} onCheckedChange={setOnlyFailures} />
                仅失败
              </label>
            </div>
          </div>
        </CardHeader>
        <CardContent className="p-0">
          <Table>
            <TableHeader>
                <TableRow>
                  <TableHead className="w-[160px]">时间</TableHead>
                  <TableHead>终端</TableHead>
                  <TableHead>渠道</TableHead>
                  <TableHead>模型</TableHead>
                  <TableHead>状态</TableHead>
                  <TableHead className="text-right">延迟</TableHead>
                <TableHead>错误</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {filtered.length === 0 ? (
                <TableRow>
                  <TableCell
                    colSpan={7}
                    className="text-center text-muted-foreground py-8"
                  >
                    暂无请求记录
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
                        <Badge variant="success">
                          {e.http_status ?? 200}
                        </Badge>
                      ) : (
                        <Badge variant="destructive">
                          {e.http_status ?? "ERR"}
                        </Badge>
                      )}
                    </TableCell>
                    <TableCell className="text-right text-muted-foreground">
                      {formatDuration(e.latency_ms)}
                    </TableCell>
                    <TableCell className="text-sm text-muted-foreground">
                      {e.error_kind ? clampStr(e.error_kind, 30) : "-"}
                    </TableCell>
                  </TableRow>
                ))
              )}
            </TableBody>
          </Table>
        </CardContent>
      </Card>
    </div>
  );
}
