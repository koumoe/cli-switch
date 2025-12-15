import React, { useEffect, useState } from "react";
import { Sun, Moon, Monitor, FolderOpen, Info, Database } from "lucide-react";
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
} from "@/components/ui";
import { useTheme, type Theme } from "@/lib/theme";
import { getHealth, type Health } from "../api";

export function SettingsPage() {
  const { theme, setTheme } = useTheme();
  const [health, setHealth] = useState<Health | null>(null);

  useEffect(() => {
    getHealth()
      .then(setHealth)
      .catch(() => setHealth({ status: "离线" }));
  }, []);

  const apiEndpoint = (() => {
    const env = (import.meta.env.VITE_BACKEND_URL as string | undefined)?.trim();
    if (env) return env.replace(/\/+$/, "");
    if (import.meta.env.DEV) return "http://127.0.0.1:3210";
    return window.location.origin;
  })();

  let apiHost = "-";
  let apiPort = "-";
  try {
    const u = new URL(apiEndpoint);
    apiHost = u.hostname;
    apiPort = u.port || (u.protocol === "https:" ? "443" : "80");
  } catch {
    // ignore
  }

  const themeOptions: { value: Theme; label: string; icon: React.ElementType }[] = [
    { value: "light", label: "浅色", icon: Sun },
    { value: "dark", label: "深色", icon: Moon },
    { value: "system", label: "跟随系统", icon: Monitor },
  ];

  return (
    <div className="space-y-6">
      {/* 页面标题 */}
      <div>
        <h1 className="text-2xl font-semibold tracking-tight">设置</h1>
        <p className="text-muted-foreground text-sm mt-1">
          应用配置和系统信息
        </p>
      </div>

      {/* 外观设置 */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Sun className="h-4 w-4" />
            外观
          </CardTitle>
          <CardDescription>自定义应用的视觉风格</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex items-center justify-between">
            <div>
              <div className="font-medium text-sm">主题</div>
              <div className="text-xs text-muted-foreground">
                选择浅色、深色或跟随系统设置
              </div>
            </div>
            <div className="flex gap-2">
              {themeOptions.map((opt) => {
                const Icon = opt.icon;
                const isActive = theme === opt.value;
                return (
                  <Button
                    key={opt.value}
                    variant={isActive ? "default" : "outline"}
                    size="sm"
                    onClick={() => setTheme(opt.value)}
                    className="gap-2"
                  >
                    <Icon className="h-4 w-4" />
                    {opt.label}
                  </Button>
                );
              })}
            </div>
          </div>
        </CardContent>
      </Card>

      {/* 代理配置 */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Database className="h-4 w-4" />
            代理配置
          </CardTitle>
          <CardDescription>配置 CLI 的 API 端点地址</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="grid grid-cols-2 gap-4">
            <div className="space-y-2">
              <label className="text-sm font-medium">服务地址</label>
              <Input value={apiHost} disabled />
              <p className="text-xs text-muted-foreground">
                CLI 应连接到的主机名 / IP
              </p>
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">服务端口</label>
              <Input value={apiPort} disabled />
              <p className="text-xs text-muted-foreground">
                CLI 应连接到的端口号
              </p>
            </div>
          </div>
          <div className="p-3 rounded-lg bg-muted/50 text-sm text-muted-foreground">
            API 端点：<code className="font-mono">{apiEndpoint}</code>
            <br />
            将此地址配置为你的 AI 工具的 API 端点即可使用。
          </div>
          {health?.listen_addr && (
            <div className="text-xs text-muted-foreground">
              后端监听：<code className="font-mono">{health.listen_addr}</code>
            </div>
          )}
        </CardContent>
      </Card>

      {/* 数据存储 */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <FolderOpen className="h-4 w-4" />
            数据存储
          </CardTitle>
          <CardDescription>应用数据和配置文件位置</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="space-y-2">
            <label className="text-sm font-medium">数据目录</label>
            <div className="flex gap-2">
              <Input
                value={health?.data_dir ?? "-"}
                disabled
                className="font-mono text-sm"
              />
              <Button
                variant="outline"
                onClick={() => {
                  toast.info("功能开发中", {
                    description: "打开文件夹功能将在后续版本中提供",
                  });
                }}
              >
                打开
              </Button>
            </div>
            <p className="text-xs text-muted-foreground">
              存放数据库、日志和配置文件
            </p>
          </div>
          <div className="space-y-2">
            <label className="text-sm font-medium">数据库文件</label>
            <Input value={health?.db_path ?? "-"} disabled className="font-mono text-sm" />
          </div>
        </CardContent>
      </Card>

      {/* 关于 */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Info className="h-4 w-4" />
            关于
          </CardTitle>
          <CardDescription>应用版本和系统信息</CardDescription>
        </CardHeader>
        <CardContent>
          <div className="space-y-3">
            <div className="flex items-center justify-between py-2 border-b">
              <span className="text-sm text-muted-foreground">应用名称</span>
              <span className="text-sm font-medium">CliSwitch</span>
            </div>
            <div className="flex items-center justify-between py-2 border-b">
              <span className="text-sm text-muted-foreground">版本</span>
              <span className="text-sm font-mono">
                {health?.version ? `v${health.version}` : "-"}
              </span>
            </div>
            <div className="flex items-center justify-between py-2 border-b">
              <span className="text-sm text-muted-foreground">后端状态</span>
              <Badge variant={health?.status === "ok" ? "success" : "destructive"}>
                {health?.status === "ok" ? "运行中" : health?.status ?? "检测中..."}
              </Badge>
            </div>
            <div className="flex items-center justify-between py-2">
              <span className="text-sm text-muted-foreground">描述</span>
              <span className="text-sm text-right max-w-[300px]">
                通用 AI CLI 配置管理平台
              </span>
            </div>
          </div>

          <div className="mt-6 p-4 rounded-lg bg-muted/50">
            <p className="text-sm text-muted-foreground">
              CliSwitch 是一个本地多渠道 CLI 代理工具，用于管理和切换多个 AI API
              服务。支持 OpenAI、Anthropic、Gemini 等协议，提供高可用性和自动故障转移。
            </p>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
