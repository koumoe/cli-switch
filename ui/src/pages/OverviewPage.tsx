import React, { useEffect, useState } from "react";
import { Activity, Radio, CheckCircle, XCircle, Clock } from "lucide-react";
import { toast } from "sonner";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
  Badge,
} from "@/components/ui";
import {
  listChannels,
  usageRecent,
  statsSummary,
  type Channel,
  type UsageEvent,
  type StatsSummary,
} from "../api";
import { formatDateTime, formatDuration } from "../lib";

export function OverviewPage() {
  const [channels, setChannels] = useState<Channel[]>([]);
  const [recent, setRecent] = useState<UsageEvent[]>([]);
  const [stats, setStats] = useState<StatsSummary | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    Promise.all([listChannels(), usageRecent(10), statsSummary("today")])
      .then(([cs, rs, st]) => {
        setChannels(cs);
        setRecent(rs);
        setStats(st);
      })
      .catch((e) => {
        toast.error("加载失败", { description: String(e) });
      })
      .finally(() => setLoading(false));
  }, []);

  const enabledChannels = channels.filter((c) => c.enabled);
  const successRate =
    stats && stats.requests > 0
      ? Math.round((stats.success / stats.requests) * 100)
      : 0;

  return (
    <div className="space-y-6">
      {/* 页面标题 */}
      <div>
        <h1 className="text-2xl font-semibold tracking-tight">概览</h1>
        <p className="text-muted-foreground text-sm mt-1">
          服务状态、渠道健康与最近请求
        </p>
      </div>

      {/* 统计卡片 */}
      <div className="grid gap-4 md:grid-cols-4">
        <Card>
          <CardHeader className="pb-2">
            <CardDescription>活跃渠道</CardDescription>
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">
              {enabledChannels.length}
              <span className="text-muted-foreground text-sm font-normal ml-1">
                / {channels.length}
              </span>
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="pb-2">
            <CardDescription>今日请求</CardDescription>
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
            <CardDescription>总 Token</CardDescription>
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">
              {stats?.total_tokens?.toLocaleString() ?? "-"}
            </div>
          </CardContent>
        </Card>
      </div>

      {/* 渠道健康状态 */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Radio className="h-4 w-4" />
            渠道状态
          </CardTitle>
          <CardDescription>已配置渠道的启用状态</CardDescription>
        </CardHeader>
        <CardContent>
          {loading ? (
            <p className="text-muted-foreground text-sm">加载中...</p>
          ) : channels.length === 0 ? (
            <p className="text-muted-foreground text-sm">暂无渠道</p>
          ) : (
            <div className="grid gap-2 sm:grid-cols-2 lg:grid-cols-3">
              {channels.map((c) => (
                <div
                  key={c.id}
                  className="flex items-center justify-between p-3 rounded-lg border bg-card"
                >
                  <div className="flex items-center gap-3">
                    {c.enabled ? (
                      <CheckCircle className="h-4 w-4 text-success" />
                    ) : (
                      <XCircle className="h-4 w-4 text-muted-foreground" />
                    )}
                    <div>
                      <div className="font-medium text-sm">{c.name}</div>
                      <div className="text-xs text-muted-foreground">
                        {c.protocol}
                      </div>
                    </div>
                  </div>
                  <Badge variant={c.enabled ? "success" : "secondary"}>
                    {c.enabled ? "ON" : "OFF"}
                  </Badge>
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>

      {/* 最近请求 */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Activity className="h-4 w-4" />
            最近请求
          </CardTitle>
          <CardDescription>最近 10 条请求记录</CardDescription>
        </CardHeader>
        <CardContent>
          {loading ? (
            <p className="text-muted-foreground text-sm">加载中...</p>
          ) : recent.length === 0 ? (
            <p className="text-muted-foreground text-sm">暂无请求</p>
          ) : (
            <div className="space-y-2">
              {recent.map((e) => (
                <div
                  key={e.id}
                  className="flex items-center justify-between py-2 border-b last:border-0"
                >
                  <div className="flex items-center gap-3">
                    {e.success ? (
                      <CheckCircle className="h-4 w-4 text-success" />
                    ) : (
                      <XCircle className="h-4 w-4 text-destructive" />
                    )}
                    <div>
                      <div className="text-sm font-medium">
                        {e.model ?? e.protocol}
                      </div>
                      <div className="text-xs text-muted-foreground">
                        {formatDateTime(e.ts_ms)}
                      </div>
                    </div>
                  </div>
                  <div className="flex items-center gap-4">
                    <Badge variant="outline">{e.protocol}</Badge>
                    <div className="flex items-center gap-1 text-xs text-muted-foreground">
                      <Clock className="h-3 w-3" />
                      {formatDuration(e.latency_ms)}
                    </div>
                  </div>
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
