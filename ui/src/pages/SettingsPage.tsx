import React, { useEffect, useState } from "react";
import { Sun, Moon, Monitor, FolderOpen, Info, Database, Languages, DollarSign, RefreshCw, Shield, Power, ScrollText, Trash2, Palette, Settings2, HardDrive, Cpu } from "lucide-react";
import { toast } from "sonner";
import {
  Button,
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
  Badge,
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  Input,
  Switch,
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
} from "@/components/ui";
import { useTheme, type Theme } from "@/lib/theme";
import { type Locale, useI18n } from "@/lib/i18n";
import { setLogLevel } from "@/lib/logger";
import { formatDateTime } from "../lib";
import { checkUpdate, downloadUpdate, getHealth, getSettings, getUpdateStatus, pricingStatus, pricingSync, updateSettings, type AppSettings, type AutoStartLaunchMode, type CloseBehavior, type Health, type PricingStatus, type UpdateCheck, type UpdateStatus } from "../api";
import type { CliswitchUpdateStatusEvent } from "@/lib/cliswitchEvents";
import { SettingsMaintenancePage } from "./SettingsMaintenancePage";

function joinPath(base: string, sub: string): string {
  const sep = base.includes("\\") ? "\\" : "/";
  if (base.endsWith(sep)) return `${base}${sub}`;
  return `${base}${sep}${sub}`;
}

function navigate(to: string) {
  if (window.location.pathname === to) return;
  window.history.pushState({}, "", to);
  window.dispatchEvent(new PopStateEvent("popstate"));
}

export function SettingsPage({ pathname }: { pathname?: string }) {
  const { theme, setTheme } = useTheme();
  const { locale, setLocale, locales, t } = useI18n();
  const [health, setHealth] = useState<Health | null>(null);
  const [pricing, setPricing] = useState<PricingStatus | null>(null);
  const [appSettings, setAppSettings] = useState<AppSettings | null>(null);
  const [updateStatus, setUpdateStatus] = useState<UpdateStatus | null>(null);
  const [updateChecking, setUpdateChecking] = useState(false);
  const [updatePromptOpen, setUpdatePromptOpen] = useState(false);
  const [updateCheckResult, setUpdateCheckResult] = useState<UpdateCheck | null>(null);
  const [updateDownloading, setUpdateDownloading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [autoDisableSaving, setAutoDisableSaving] = useState(false);
  const [closeSaving, setCloseSaving] = useState(false);
  const [autoStartSaving, setAutoStartSaving] = useState(false);
  const [autoStartLaunchSaving, setAutoStartLaunchSaving] = useState(false);
  const [syncing, setSyncing] = useState(false);
  const [logSaving, setLogSaving] = useState(false);
  const [logRetentionDraft, setLogRetentionDraft] = useState<string>("");

  useEffect(() => {
    getHealth()
      .then(setHealth)
      .catch(() => setHealth({ status: "离线" }));

    pricingStatus()
      .then(setPricing)
      .catch(() => setPricing(null));

    getSettings()
      .then((s) => {
        setAppSettings(s);
        setLogRetentionDraft(String(s.log_retention_days ?? ""));
        setLogLevel(s.log_level);
      })
      .catch(() => setAppSettings(null));

    getUpdateStatus()
      .then(setUpdateStatus)
      .catch(() => setUpdateStatus(null));
  }, []);

  useEffect(() => {
    const onUpdateStatus = (e: Event) => {
      const st = (e as CliswitchUpdateStatusEvent).detail;
      if (!st) return;
      setUpdateStatus(st);
    };
    window.addEventListener("cliswitch-update-status", onUpdateStatus as EventListener);
    return () => {
      window.removeEventListener("cliswitch-update-status", onUpdateStatus as EventListener);
    };
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
    { value: "light", label: t("theme.light"), icon: Sun },
    { value: "dark", label: t("theme.dark"), icon: Moon },
    { value: "system", label: t("theme.system"), icon: Monitor },
  ];

  const backendStatusLabel =
    health?.status === "ok"
      ? t("status.running")
      : health?.status === "离线"
        ? t("status.offline")
        : health?.status ?? t("status.checking");

  const updateServerVersion = updateStatus?.latest_version ?? updateCheckResult?.latest_version ?? null;
  const updateDownloadingSuffix =
    updateStatus && updateStatus.stage === "downloading"
      ? updateStatus.download_percent !== null
        ? t("settings.update.downloadingSuffix", { percent: updateStatus.download_percent })
        : t("settings.update.downloadingSuffixUnknown")
      : "";
  const updateStatusText = updateStatus?.pending_version
    ? t("settings.update.ready", { version: updateStatus.pending_version })
    : updateStatus?.stage === "staging"
      ? t("settings.update.staging")
    : updateStatus?.stage === "downloading"
      ? `${t("settings.update.latest")}${updateDownloadingSuffix}`
      : updateServerVersion
        ? t("settings.update.latest")
        : "-";

  if (pathname?.startsWith("/settings/maintenance")) {
    return <SettingsMaintenancePage onBack={() => navigate("/settings")} />;
  }

  return (
    <div className="space-y-4 pb-4">
      {/* 页面标题 */}
      <div>
        <h1 className="text-lg font-semibold">{t("settings.title")}</h1>
        <p className="text-muted-foreground text-xs mt-0.5">
          {t("settings.subtitle")}
        </p>
      </div>

      {/* 标签页 */}
      <Tabs defaultValue="appearance" className="w-full">
        <TabsList className="w-full justify-start">
          <TabsTrigger value="appearance" className="gap-1.5">
            <Palette className="h-3.5 w-3.5" />
            {t("settings.tabs.appearance")}
          </TabsTrigger>
          <TabsTrigger value="channel" className="gap-1.5">
            <Shield className="h-3.5 w-3.5" />
            {t("settings.tabs.channel")}
          </TabsTrigger>
          <TabsTrigger value="application" className="gap-1.5">
            <Settings2 className="h-3.5 w-3.5" />
            {t("settings.tabs.application")}
          </TabsTrigger>
          <TabsTrigger value="data" className="gap-1.5">
            <HardDrive className="h-3.5 w-3.5" />
            {t("settings.tabs.data")}
          </TabsTrigger>
          <TabsTrigger value="system" className="gap-1.5">
            <Cpu className="h-3.5 w-3.5" />
            {t("settings.tabs.system")}
          </TabsTrigger>
        </TabsList>

        {/* 界面标签页 */}
        <TabsContent value="appearance" className="space-y-4 mt-4">
          {/* 外观设置 */}
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <Sun className="h-4 w-4" />
                {t("settings.appearance.title")}
              </CardTitle>
              <CardDescription>{t("settings.appearance.subtitle")}</CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="flex items-center justify-between">
                <div>
                  <div className="font-medium text-sm">{t("settings.appearance.theme")}</div>
                  <div className="text-xs text-muted-foreground">
                    {t("settings.appearance.themeHint")}
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

          {/* 语言设置 */}
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <Languages className="h-4 w-4" />
                {t("settings.language.title")}
              </CardTitle>
              <CardDescription>{t("settings.language.subtitle")}</CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="flex items-center justify-between gap-4">
                <div className="font-medium text-sm">{t("settings.language.label")}</div>
                <div className="w-[220px]">
                  <Select value={locale} onValueChange={(v) => setLocale(v as Locale)}>
                    <SelectTrigger>
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      {locales.map((l) => (
                        <SelectItem key={l.value} value={l.value}>
                          {l.label}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </div>
              </div>
            </CardContent>
          </Card>
        </TabsContent>

        {/* 渠道标签页 */}
        <TabsContent value="channel" className="space-y-4 mt-4">
          {/* 渠道保护（原自动禁用） */}
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <Shield className="h-4 w-4" />
                {t("settings.channelProtection.title")}
              </CardTitle>
              <CardDescription>{t("settings.channelProtection.subtitle")}</CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="flex items-center justify-between gap-4">
                <div>
                  <div className="font-medium text-sm">{t("settings.channelProtection.enable")}</div>
                  <div className="text-xs text-muted-foreground">{t("settings.channelProtection.enableHint")}</div>
                </div>
                <Switch
                  checked={appSettings?.auto_disable_enabled ?? false}
                  onCheckedChange={(v) => {
                    setAppSettings((prev) => (prev ? { ...prev, auto_disable_enabled: v } : prev));
                  }}
                  disabled={!appSettings}
                />
              </div>

              <div className="grid grid-cols-3 gap-3">
                <div className="space-y-2">
                  <label className="text-sm font-medium">{t("settings.channelProtection.windowMinutes")}</label>
                  <Input
                    type="number"
                    min={1}
                    value={appSettings?.auto_disable_window_minutes ?? 3}
                    onChange={(e) => {
                      const n = Number(e.target.value);
                      setAppSettings((prev) =>
                        prev
                          ? {
                              ...prev,
                              auto_disable_window_minutes: Number.isFinite(n) ? Math.floor(n) : 3,
                            }
                          : prev
                      );
                    }}
                    className="h-8"
                    disabled={!appSettings || !(appSettings?.auto_disable_enabled ?? false)}
                  />
                  <p className="text-xs text-muted-foreground">{t("settings.channelProtection.windowMinutesHint")}</p>
                </div>
                <div className="space-y-2">
                  <label className="text-sm font-medium">{t("settings.channelProtection.failureTimes")}</label>
                  <Input
                    type="number"
                    min={1}
                    value={appSettings?.auto_disable_failure_times ?? 5}
                    onChange={(e) => {
                      const n = Number(e.target.value);
                      setAppSettings((prev) =>
                        prev
                          ? {
                              ...prev,
                              auto_disable_failure_times: Number.isFinite(n) ? Math.floor(n) : 5,
                            }
                          : prev
                      );
                    }}
                    className="h-8"
                    disabled={!appSettings || !(appSettings?.auto_disable_enabled ?? false)}
                  />
                  <p className="text-xs text-muted-foreground">{t("settings.channelProtection.failureTimesHint")}</p>
                </div>
                <div className="space-y-2">
                  <label className="text-sm font-medium">{t("settings.channelProtection.pauseMinutes")}</label>
                  <Input
                    type="number"
                    min={1}
                    value={appSettings?.auto_disable_disable_minutes ?? 30}
                    onChange={(e) => {
                      const n = Number(e.target.value);
                      setAppSettings((prev) =>
                        prev
                          ? {
                              ...prev,
                              auto_disable_disable_minutes: Number.isFinite(n) ? Math.floor(n) : 30,
                            }
                          : prev
                      );
                    }}
                    className="h-8"
                    disabled={!appSettings || !(appSettings?.auto_disable_enabled ?? false)}
                  />
                  <p className="text-xs text-muted-foreground">{t("settings.channelProtection.pauseMinutesHint")}</p>
                </div>
              </div>

              <div className="flex justify-end">
                <Button
                  size="sm"
                  onClick={async () => {
                    if (!appSettings) return;
                    const win = appSettings.auto_disable_window_minutes;
                    const times = appSettings.auto_disable_failure_times;
                    const mins = appSettings.auto_disable_disable_minutes;
                    if (
                      !Number.isFinite(win) ||
                      !Number.isFinite(times) ||
                      !Number.isFinite(mins) ||
                      win < 1 ||
                      times < 1 ||
                      mins < 1
                    ) {
                      toast.error(t("settings.channelProtection.invalid"));
                      return;
                    }
                    setAutoDisableSaving(true);
                    try {
                      const next = await updateSettings({
                        auto_disable_enabled: appSettings.auto_disable_enabled,
                        auto_disable_window_minutes: win,
                        auto_disable_failure_times: times,
                        auto_disable_disable_minutes: mins,
                      });
                      setAppSettings(next);
                      toast.success(t("settings.channelProtection.saved"));
                    } catch (e) {
                      toast.error(t("settings.channelProtection.saveFail"), { description: String(e) });
                    } finally {
                      setAutoDisableSaving(false);
                    }
                  }}
                  disabled={!appSettings || autoDisableSaving}
                >
                  {t("common.save")}
                </Button>
              </div>
            </CardContent>
          </Card>

          {/* 定价数据（原价格表） */}
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <DollarSign className="h-4 w-4" />
                {t("settings.pricingData.title")}
              </CardTitle>
              <CardDescription>{t("settings.pricingData.subtitle")}</CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="flex items-center justify-between gap-3">
                <div>
                  <div className="font-medium text-sm">{t("settings.pricingData.status")}</div>
                  <div className="text-xs text-muted-foreground">
                    {t("settings.pricingData.count", { count: (pricing?.count ?? 0).toLocaleString() })}
                    {" · "}
                    {t("settings.pricingData.lastSync", {
                      time: pricing?.last_sync_ms ? formatDateTime(pricing.last_sync_ms) : "-",
                    })}
                  </div>
                </div>
                <Button
                  size="sm"
                  variant="outline"
                  onClick={async () => {
                    setSyncing(true);
                    try {
                      await pricingSync();
                      const st = await pricingStatus();
                      setPricing(st);
                      toast.success(t("settings.pricingData.syncOk"));
                    } catch (e) {
                      toast.error(t("settings.pricingData.syncFail"), { description: String(e) });
                    } finally {
                      setSyncing(false);
                    }
                  }}
                  disabled={syncing}
                  className="gap-2"
                >
                  <RefreshCw className={`h-4 w-4 ${syncing ? "animate-spin" : ""}`} />
                  {t("settings.pricingData.sync")}
                </Button>
              </div>

              <div className="flex items-center justify-between gap-4">
                <div>
                  <div className="font-medium text-sm">{t("settings.pricingData.autoUpdate")}</div>
                  <div className="text-xs text-muted-foreground">
                    {t("settings.pricingData.autoUpdateHint")}
                  </div>
                </div>
                <Switch
                  checked={appSettings?.pricing_auto_update_enabled ?? false}
                  onCheckedChange={(v) => {
                    setAppSettings((prev) => (prev ? { ...prev, pricing_auto_update_enabled: v } : prev));
                  }}
                  disabled={!appSettings}
                />
              </div>

              <div className="flex items-center justify-between gap-4">
                <div>
                  <div className="font-medium text-sm">{t("settings.pricingData.intervalHours")}</div>
                  <div className="text-xs text-muted-foreground">
                    {t("settings.pricingData.intervalHoursHint")}
                  </div>
                </div>
                <Input
                  type="number"
                  min={1}
                  max={8760}
                  value={appSettings?.pricing_auto_update_interval_hours ?? 24}
                  onChange={(e) => {
                    const n = Number(e.target.value);
                    setAppSettings((prev) =>
                      prev
                        ? {
                            ...prev,
                            pricing_auto_update_interval_hours: Number.isFinite(n) ? Math.floor(n) : 24,
                          }
                        : prev
                    );
                  }}
                  className="w-[140px] h-8"
                  disabled={!appSettings || !(appSettings?.pricing_auto_update_enabled ?? false)}
                />
              </div>

              <div className="flex justify-end">
                <Button
                  size="sm"
                  onClick={async () => {
                    if (!appSettings) return;
                    const hours = appSettings.pricing_auto_update_interval_hours;
                    if (!Number.isFinite(hours) || hours < 1 || hours > 8760) {
                      toast.error(t("settings.pricingData.intervalInvalid"));
                      return;
                    }
                    setSaving(true);
                    try {
                      const next = await updateSettings({
                        pricing_auto_update_enabled: appSettings.pricing_auto_update_enabled,
                        pricing_auto_update_interval_hours: hours,
                      });
                      setAppSettings(next);
                      toast.success(t("settings.pricingData.saved"));
                    } catch (e) {
                      toast.error(t("settings.pricingData.saveFail"), { description: String(e) });
                    } finally {
                      setSaving(false);
                    }
                  }}
                  disabled={!appSettings || saving}
                >
                  {t("common.save")}
                </Button>
              </div>
            </CardContent>
          </Card>
        </TabsContent>

        {/* 应用标签页 */}
        <TabsContent value="application" className="space-y-4 mt-4">
          {/* 窗口关闭（原关闭行为） */}
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <Monitor className="h-4 w-4" />
                {t("settings.windowClose.title")}
              </CardTitle>
              <CardDescription>{t("settings.windowClose.subtitle")}</CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="flex items-center justify-between gap-4">
                <div>
                  <div className="font-medium text-sm">{t("settings.windowClose.behavior")}</div>
                  <div className="text-xs text-muted-foreground">{t("settings.windowClose.behaviorHint")}</div>
                </div>
                <div className="w-[220px]">
                  <Select
                    value={(appSettings?.close_behavior ?? "ask") as CloseBehavior}
                    onValueChange={(v) => {
                      setAppSettings((prev) => (prev ? { ...prev, close_behavior: v as CloseBehavior } : prev));
                    }}
                    disabled={!appSettings}
                  >
                    <SelectTrigger>
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="ask">{t("settings.windowClose.ask")}</SelectItem>
                      <SelectItem value="minimize_to_tray">{t("settings.windowClose.minimize")}</SelectItem>
                      <SelectItem value="quit">{t("settings.windowClose.quit")}</SelectItem>
                    </SelectContent>
                  </Select>
                </div>
              </div>

              <div className="flex justify-end">
                <Button
                  size="sm"
                  onClick={async () => {
                    if (!appSettings) return;
                    setCloseSaving(true);
                    try {
                      const next = await updateSettings({ close_behavior: appSettings.close_behavior });
                      setAppSettings(next);
                      toast.success(t("settings.windowClose.saved"));
                    } catch (e) {
                      toast.error(t("settings.windowClose.saveFail"), { description: String(e) });
                    } finally {
                      setCloseSaving(false);
                    }
                  }}
                  disabled={!appSettings || closeSaving}
                >
                  {t("common.save")}
                </Button>
              </div>
            </CardContent>
          </Card>

          {/* 开机自启动 */}
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <Power className="h-4 w-4" />
                {t("settings.startup.title")}
              </CardTitle>
              <CardDescription>{t("settings.startup.subtitle")}</CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="flex items-center justify-between gap-4">
                <div>
                  <div className="font-medium text-sm">{t("settings.startup.enable")}</div>
                  <div className="text-xs text-muted-foreground">{t("settings.startup.enableHint")}</div>
                </div>
                <Switch
                  checked={appSettings?.auto_start_enabled ?? false}
                  onCheckedChange={async (v) => {
                    if (!appSettings) return;
                    const prev = appSettings.auto_start_enabled;
                    setAppSettings({ ...appSettings, auto_start_enabled: v });
                    setAutoStartSaving(true);
                    try {
                      const next = await updateSettings({ auto_start_enabled: v });
                      setAppSettings(next);
                      toast.success(t("settings.startup.saved"));
                    } catch (e) {
                      setAppSettings({ ...appSettings, auto_start_enabled: prev });
                      toast.error(t("settings.startup.saveFail"), { description: String(e) });
                    } finally {
                      setAutoStartSaving(false);
                    }
                  }}
                  disabled={!appSettings || autoStartSaving}
                />
              </div>

              <div className="flex items-center justify-between gap-4">
                <div>
                  <div className="font-medium text-sm">{t("settings.startup.launchMode")}</div>
                  <div className="text-xs text-muted-foreground">{t("settings.startup.launchModeHint")}</div>
                </div>
                <div className="w-[220px]">
                  <Select
                    value={(appSettings?.auto_start_launch_mode ?? "show_window") as AutoStartLaunchMode}
                    onValueChange={async (v) => {
                      if (!appSettings) return;
                      const prev = appSettings.auto_start_launch_mode;
                      setAppSettings({ ...appSettings, auto_start_launch_mode: v as AutoStartLaunchMode });
                      setAutoStartLaunchSaving(true);
                      try {
                        const next = await updateSettings({ auto_start_launch_mode: v as AutoStartLaunchMode });
                        setAppSettings(next);
                        toast.success(t("settings.startup.launchSaved"));
                      } catch (e) {
                        setAppSettings({ ...appSettings, auto_start_launch_mode: prev });
                        toast.error(t("settings.startup.saveFail"), { description: String(e) });
                      } finally {
                        setAutoStartLaunchSaving(false);
                      }
                    }}
                    disabled={!appSettings || autoStartSaving || autoStartLaunchSaving}
                  >
                    <SelectTrigger>
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="show_window">{t("settings.startup.launchShow")}</SelectItem>
                      <SelectItem value="minimize_to_tray">{t("settings.startup.launchMinimize")}</SelectItem>
                    </SelectContent>
                  </Select>
                </div>
              </div>
            </CardContent>
          </Card>

          {/* 自动更新 */}
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <RefreshCw className="h-4 w-4" />
                {t("settings.update.title")}
              </CardTitle>
              <CardDescription>{t("settings.update.subtitle")}</CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <Dialog open={updatePromptOpen} onOpenChange={setUpdatePromptOpen}>
                <DialogContent className="sm:max-w-[520px]">
                  <DialogHeader>
                    <DialogTitle>{t("settings.update.promptTitle")}</DialogTitle>
                    <DialogDescription>
                      {t("settings.update.promptDesc", {
                        version: updateCheckResult?.latest_version ?? "-",
                      })}
                    </DialogDescription>
                  </DialogHeader>
                  <DialogFooter>
                    <Button
                      variant="outline"
                      onClick={() => setUpdatePromptOpen(false)}
                      disabled={updateDownloading}
                    >
                      {t("common.cancel")}
                    </Button>
                    <Button
                      onClick={async () => {
                        setUpdateDownloading(true);
                        try {
                          const dl = await downloadUpdate();
                          setUpdateStatus(dl.status);
                          toast.success(t("settings.update.downloading"));
                          setUpdatePromptOpen(false);
                        } catch (e) {
                          toast.error(t("settings.update.downloadFail"), { description: String(e) });
                        } finally {
                          setUpdateDownloading(false);
                        }
                      }}
                      disabled={updateDownloading}
                    >
                      {t("settings.update.updateNow")}
                    </Button>
                  </DialogFooter>
                </DialogContent>
              </Dialog>

              <div className="flex items-center justify-between gap-4">
                <div>
                  <div className="font-medium text-sm">{t("settings.update.autoEnable")}</div>
                  <div className="text-xs text-muted-foreground">{t("settings.update.autoEnableHint")}</div>
                </div>
                <Switch
                  checked={appSettings?.app_auto_update_enabled ?? false}
                  onCheckedChange={async (v) => {
                    if (!appSettings) return;
                    const prev = appSettings.app_auto_update_enabled;
                    setAppSettings({ ...appSettings, app_auto_update_enabled: v });
                    try {
                      const next = await updateSettings({ app_auto_update_enabled: v });
                      setAppSettings(next);
                      toast.success(t("settings.update.saved"));
                      if (v) {
                        const dl = await downloadUpdate();
                        setUpdateStatus(dl.status);
                        if (dl.started) toast.success(t("settings.update.autoStarted"));
                      }
                    } catch (e) {
                      setAppSettings({ ...appSettings, app_auto_update_enabled: prev });
                      toast.error(t("settings.update.saveFail"), { description: String(e) });
                    }
                  }}
                  disabled={!appSettings}
                />
              </div>

              <div className="flex items-center justify-between gap-4">
                <div>
                  <div className="font-medium text-sm">{t("settings.update.status")}</div>
                  <div className="text-xs text-muted-foreground space-y-0.5">
                    <div>{updateStatusText}</div>
                    {updateServerVersion ? (
                      <div>{t("settings.update.serverVersion", { version: updateServerVersion })}</div>
                    ) : null}
                  </div>
                </div>
                <div className="flex items-center gap-2">
                  <Button
                    size="sm"
                    variant="outline"
                    onClick={async () => {
                      setUpdateChecking(true);
                      try {
                        const res = await checkUpdate();
                        setUpdateCheckResult(res);
                        const st = await getUpdateStatus().catch(() => null);
                        if (st) setUpdateStatus(st);

                        if (!res.update_available) {
                          toast.success(t("settings.update.uptodate"));
                          return;
                        }

                        toast.success(
                          t("settings.update.found", { version: res.latest_version ?? "-" })
                        );

                        if (!appSettings?.app_auto_update_enabled) {
                          setUpdatePromptOpen(true);
                        } else {
                          const dl = await downloadUpdate();
                          setUpdateStatus(dl.status);
                          if (dl.started) toast.success(t("settings.update.downloading"));
                        }
                      } catch (e) {
                        toast.error(t("settings.update.checkFail"), { description: String(e) });
                      } finally {
                        setUpdateChecking(false);
                      }
                    }}
                    disabled={updateChecking}
                  >
                    {t("settings.update.check")}
                  </Button>
                </div>
              </div>
            </CardContent>
          </Card>
        </TabsContent>

        {/* 数据标签页 */}
        <TabsContent value="data" className="space-y-4 mt-4">
          {/* 数据目录（原数据存储） */}
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <FolderOpen className="h-4 w-4" />
                {t("settings.storage.title")}
              </CardTitle>
              <CardDescription>{t("settings.storage.subtitle")}</CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="space-y-2">
                <label className="text-sm font-medium">{t("settings.storage.dataDir")}</label>
                <div className="flex gap-2">
                  <Input
                    value={health?.data_dir ?? "-"}
                    disabled
                    className="font-mono text-sm"
                  />
                  <Button
                    variant="outline"
                    onClick={() => {
                      toast.info(t("settings.storage.openInDevTitle"), {
                        description: t("settings.storage.openInDevDesc"),
                      });
                    }}
                  >
                    {t("common.open")}
                  </Button>
                </div>
                <p className="text-xs text-muted-foreground">
                  {t("settings.storage.dataDirHint")}
                </p>
              </div>
              <div className="space-y-2">
                <label className="text-sm font-medium">{t("settings.storage.dbFile")}</label>
                <Input value={health?.db_path ?? "-"} disabled className="font-mono text-sm" />
              </div>
            </CardContent>
          </Card>

          {/* 数据维护 */}
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <HardDrive className="h-4 w-4" />
                {t("settings.maintenance.title")}
              </CardTitle>
              <CardDescription>{t("settings.maintenance.subtitle")}</CardDescription>
            </CardHeader>
            <CardContent className="flex items-center justify-between gap-4">
              <div className="text-xs text-muted-foreground">{t("settings.maintenance.hint")}</div>
              <Button variant="outline" onClick={() => navigate("/settings/maintenance")}>
                {t("common.open")}
              </Button>
            </CardContent>
          </Card>

          {/* 应用日志 */}
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <ScrollText className="h-4 w-4" />
                {t("settings.logging.title")}
              </CardTitle>
              <CardDescription>{t("settings.logging.subtitle")}</CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="flex items-center justify-between gap-4">
                <div>
                  <div className="font-medium text-sm">{t("settings.logging.level")}</div>
                  <div className="text-xs text-muted-foreground">{t("settings.logging.levelHint")}</div>
                </div>
                <Select
                  value={appSettings?.log_level ?? "warning"}
                  onValueChange={async (v) => {
                    if (!appSettings) return;
                    const prev = appSettings.log_level;
                    setAppSettings({ ...appSettings, log_level: v as any });
                    setLogSaving(true);
                    try {
                      const next = await updateSettings({ log_level: v as any });
                      setAppSettings(next);
                      setLogLevel(next.log_level);
                      toast.success(t("settings.logging.saved"));
                    } catch (e) {
                      setAppSettings({ ...appSettings, log_level: prev });
                      toast.error(t("settings.logging.saveFail"), { description: String(e) });
                    } finally {
                      setLogSaving(false);
                    }
                  }}
                  disabled={!appSettings || logSaving}
                >
                  <SelectTrigger className="w-[180px]">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="none">{t("settings.logging.levelNone")}</SelectItem>
                    <SelectItem value="debug">{t("settings.logging.levelDebug")}</SelectItem>
                    <SelectItem value="info">{t("settings.logging.levelInfo")}</SelectItem>
                    <SelectItem value="warning">{t("settings.logging.levelWarning")}</SelectItem>
                    <SelectItem value="error">{t("settings.logging.levelError")}</SelectItem>
                  </SelectContent>
                </Select>
              </div>

              <div className="flex items-center justify-between gap-4">
                <div>
                  <div className="font-medium text-sm">{t("settings.logging.retentionDays")}</div>
                  <div className="text-xs text-muted-foreground">{t("settings.logging.retentionHint")}</div>
                </div>
                <div className="flex items-center gap-2">
                  <Input
                    value={logRetentionDraft}
                    type="number"
                    min={1}
                    max={3650}
                    onChange={(e) => setLogRetentionDraft(e.target.value)}
                    className="w-[120px] font-mono text-sm"
                    disabled={!appSettings || logSaving}
                  />
                  <Button
                    variant="outline"
                    onClick={async () => {
                      if (!appSettings) return;
                      const raw = logRetentionDraft.trim();
                      const n = Number.parseInt(raw, 10);
                      if (!Number.isFinite(n) || n < 1 || n > 3650) {
                        toast.error(t("settings.logging.retentionInvalid"));
                        return;
                      }

                      const prev = appSettings.log_retention_days;
                      setLogSaving(true);
                      try {
                        const next = await updateSettings({ log_retention_days: n });
                        setAppSettings(next);
                        setLogRetentionDraft(String(next.log_retention_days));
                        toast.success(t("settings.logging.saved"));
                      } catch (e) {
                        setLogRetentionDraft(String(prev));
                        toast.error(t("settings.logging.saveFail"), { description: String(e) });
                      } finally {
                        setLogSaving(false);
                      }
                    }}
                    disabled={!appSettings || logSaving}
                  >
                    {t("common.save")}
                  </Button>
                </div>
              </div>

              <div className="space-y-2">
                <label className="text-sm font-medium">{t("settings.logging.dir")}</label>
                <Input
                  value={health?.data_dir ? joinPath(health.data_dir, "logs") : "-"}
                  disabled
                  className="font-mono text-sm"
                />
              </div>
            </CardContent>
          </Card>

        </TabsContent>

        {/* 系统标签页 */}
        <TabsContent value="system" className="space-y-4 mt-4">
          {/* 服务信息（原代理配置） */}
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <Database className="h-4 w-4" />
                {t("settings.serviceInfo.title")}
              </CardTitle>
              <CardDescription>{t("settings.serviceInfo.subtitle")}</CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="grid grid-cols-2 gap-4">
                <div className="space-y-2">
                  <label className="text-sm font-medium">{t("settings.serviceInfo.host")}</label>
                  <Input value={apiHost} disabled />
                  <p className="text-xs text-muted-foreground">
                    {t("settings.serviceInfo.hostHint")}
                  </p>
                </div>
                <div className="space-y-2">
                  <label className="text-sm font-medium">{t("settings.serviceInfo.port")}</label>
                  <Input value={apiPort} disabled />
                  <p className="text-xs text-muted-foreground">
                    {t("settings.serviceInfo.portHint")}
                  </p>
                </div>
              </div>
              <div className="p-3 rounded-lg bg-muted/50 text-sm text-muted-foreground">
                {t("settings.serviceInfo.endpoint")}<code className="font-mono">{apiEndpoint}</code>
                <br />
                {t("settings.serviceInfo.endpointHint")}
              </div>
              {health?.listen_addr && (
                <div className="text-xs text-muted-foreground">
                  {t("settings.serviceInfo.backendListen")}<code className="font-mono">{health.listen_addr}</code>
                </div>
              )}
            </CardContent>
          </Card>

          {/* 版本信息（原关于） */}
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <Info className="h-4 w-4" />
                {t("settings.about.title")}
              </CardTitle>
              <CardDescription>{t("settings.about.subtitle")}</CardDescription>
            </CardHeader>
            <CardContent>
              <div className="space-y-3">
                <div className="flex items-center justify-between py-2 border-b">
                  <span className="text-sm text-muted-foreground">{t("settings.about.appName")}</span>
                  <span className="text-sm font-medium">CliSwitch</span>
                </div>
                <div className="flex items-center justify-between py-2 border-b">
                  <span className="text-sm text-muted-foreground">{t("settings.about.version")}</span>
                  <span className="text-sm font-mono">
                    {health?.version ? `v${health.version}` : "-"}
                  </span>
                </div>
                <div className="flex items-center justify-between py-2 border-b">
                  <span className="text-sm text-muted-foreground">{t("settings.about.backendStatus")}</span>
                  <Badge variant={health?.status === "ok" ? "success" : "destructive"}>
                    {backendStatusLabel}
                  </Badge>
                </div>
                <div className="flex items-center justify-between py-2">
                  <span className="text-sm text-muted-foreground">{t("settings.about.description")}</span>
                  <span className="text-sm text-right max-w-[300px]">
                    {t("settings.about.descText")}
                  </span>
                </div>
              </div>

              <div className="mt-6 p-4 rounded-lg bg-muted/50">
                <p className="text-sm text-muted-foreground">
                  {t("settings.about.intro")}
                </p>
              </div>
            </CardContent>
          </Card>
        </TabsContent>
      </Tabs>
    </div>
  );
}
