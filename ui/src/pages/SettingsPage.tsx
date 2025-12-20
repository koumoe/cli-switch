import React, { useEffect, useState } from "react";
import { Sun, Moon, Monitor, FolderOpen, Info, Database, Languages, DollarSign, RefreshCw, Shield, Power, Trash2 } from "lucide-react";
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
} from "@/components/ui";
import { useTheme, type Theme } from "@/lib/theme";
import { type Locale, useI18n } from "@/lib/i18n";
import { formatBytes, formatDateTime } from "../lib";
import { checkUpdate, clearRecords, downloadUpdate, getDbSize, getHealth, getSettings, getUpdateStatus, pricingStatus, pricingSync, updateSettings, type AppSettings, type CloseBehavior, type DbSize, type Health, type PricingStatus, type UpdateCheck, type UpdateStatus } from "../api";

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

export function SettingsPage() {
  const { theme, setTheme } = useTheme();
  const { locale, setLocale, locales, t } = useI18n();
  const [health, setHealth] = useState<Health | null>(null);
  const [dbSize, setDbSize] = useState<DbSize | null>(null);
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
  const [syncing, setSyncing] = useState(false);
  const [dbSizeLoading, setDbSizeLoading] = useState(false);
  const [clearDate, setClearDate] = useState("");
  const [clearMode, setClearMode] = useState<"date_range" | "errors" | "all" | null>(null);
  const [clearPromptOpen, setClearPromptOpen] = useState(false);
  const [clearing, setClearing] = useState(false);

  async function refreshDbSize() {
    setDbSizeLoading(true);
    try {
      const next = await getDbSize();
      setDbSize(next);
    } catch (e) {
      toast.error(t("settings.storage.dbSizeFail"), { description: String(e) });
    } finally {
      setDbSizeLoading(false);
    }
  }

  useEffect(() => {
    getHealth()
      .then(setHealth)
      .catch(() => setHealth({ status: "离线" }));

    getDbSize()
      .then(setDbSize)
      .catch(() => setDbSize(null));

    pricingStatus()
      .then(setPricing)
      .catch(() => setPricing(null));

    getSettings()
      .then(setAppSettings)
      .catch(() => setAppSettings(null));

    getUpdateStatus()
      .then(setUpdateStatus)
      .catch(() => setUpdateStatus(null));
  }, []);

  useEffect(() => {
    if (updateStatus?.stage !== "downloading") return;
    let stopped = false;

    const poll = async () => {
      try {
        const st = await getUpdateStatus();
        if (!stopped) setUpdateStatus(st);
      } catch {
        // ignore
      }
    };

    void poll();
    const id = window.setInterval(() => void poll(), 1000);
    return () => {
      stopped = true;
      window.clearInterval(id);
    };
  }, [updateStatus?.stage]);

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
    : updateStatus?.stage === "downloading"
      ? `${t("settings.update.latest")}${updateDownloadingSuffix}`
      : updateServerVersion
        ? t("settings.update.latest")
        : "-";

  const clearPromptDescKey =
    clearMode === "date_range"
      ? "settings.records.confirmDate"
      : clearMode === "errors"
        ? "settings.records.confirmErrors"
        : "settings.records.confirmAll";

  return (
    <div className="space-y-4 pb-4">
      {/* 页面标题 */}
      <div>
        <h1 className="text-lg font-semibold">{t("settings.title")}</h1>
        <p className="text-muted-foreground text-xs mt-0.5">
          {t("settings.subtitle")}
        </p>
      </div>

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

      {/* 代理配置 */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Database className="h-4 w-4" />
            {t("settings.proxy.title")}
          </CardTitle>
          <CardDescription>{t("settings.proxy.subtitle")}</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="grid grid-cols-2 gap-4">
            <div className="space-y-2">
              <label className="text-sm font-medium">{t("settings.proxy.host")}</label>
              <Input value={apiHost} disabled />
              <p className="text-xs text-muted-foreground">
                {t("settings.proxy.hostHint")}
              </p>
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">{t("settings.proxy.port")}</label>
              <Input value={apiPort} disabled />
              <p className="text-xs text-muted-foreground">
                {t("settings.proxy.portHint")}
              </p>
            </div>
          </div>
          <div className="p-3 rounded-lg bg-muted/50 text-sm text-muted-foreground">
            {t("settings.proxy.endpoint")}<code className="font-mono">{apiEndpoint}</code>
            <br />
            {t("settings.proxy.endpointHint")}
          </div>
          {health?.listen_addr && (
            <div className="text-xs text-muted-foreground">
              {t("settings.proxy.backendListen")}<code className="font-mono">{health.listen_addr}</code>
            </div>
          )}
        </CardContent>
      </Card>

      {/* 价格表与自动更新 */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <DollarSign className="h-4 w-4" />
            {t("settings.pricing.title")}
          </CardTitle>
          <CardDescription>{t("settings.pricing.subtitle")}</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex items-center justify-between gap-3">
            <div>
              <div className="font-medium text-sm">{t("settings.pricing.status")}</div>
              <div className="text-xs text-muted-foreground">
                {t("settings.pricing.count", { count: (pricing?.count ?? 0).toLocaleString() })}
                {" · "}
                {t("settings.pricing.lastSync", {
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
                  toast.success(t("settings.pricing.syncOk"));
                } catch (e) {
                  toast.error(t("settings.pricing.syncFail"), { description: String(e) });
                } finally {
                  setSyncing(false);
                }
              }}
              disabled={syncing}
              className="gap-2"
            >
              <RefreshCw className={`h-4 w-4 ${syncing ? "animate-spin" : ""}`} />
              {t("settings.pricing.sync")}
            </Button>
          </div>

          <div className="flex items-center justify-between gap-4">
            <div>
              <div className="font-medium text-sm">{t("settings.pricing.autoUpdate")}</div>
              <div className="text-xs text-muted-foreground">
                {t("settings.pricing.autoUpdateHint")}
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
              <div className="font-medium text-sm">{t("settings.pricing.intervalHours")}</div>
              <div className="text-xs text-muted-foreground">
                {t("settings.pricing.intervalHoursHint")}
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
                  toast.error(t("settings.pricing.intervalInvalid"));
                  return;
                }
                setSaving(true);
                try {
                  const next = await updateSettings({
                    pricing_auto_update_enabled: appSettings.pricing_auto_update_enabled,
                    pricing_auto_update_interval_hours: hours,
                  });
                  setAppSettings(next);
                  toast.success(t("settings.pricing.saved"));
                } catch (e) {
                  toast.error(t("settings.pricing.saveFail"), { description: String(e) });
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

      {/* 自动禁用 */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Shield className="h-4 w-4" />
            {t("settings.autoDisable.title")}
          </CardTitle>
          <CardDescription>{t("settings.autoDisable.subtitle")}</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex items-center justify-between gap-4">
            <div>
              <div className="font-medium text-sm">{t("settings.autoDisable.enable")}</div>
              <div className="text-xs text-muted-foreground">{t("settings.autoDisable.enableHint")}</div>
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
              <label className="text-sm font-medium">{t("settings.autoDisable.windowMinutes")}</label>
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
              <p className="text-xs text-muted-foreground">{t("settings.autoDisable.windowMinutesHint")}</p>
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">{t("settings.autoDisable.failureTimes")}</label>
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
              <p className="text-xs text-muted-foreground">{t("settings.autoDisable.failureTimesHint")}</p>
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">{t("settings.autoDisable.disableMinutes")}</label>
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
              <p className="text-xs text-muted-foreground">{t("settings.autoDisable.disableMinutesHint")}</p>
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
                  toast.error(t("settings.autoDisable.invalid"));
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
                  toast.success(t("settings.autoDisable.saved"));
                } catch (e) {
                  toast.error(t("settings.autoDisable.saveFail"), { description: String(e) });
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

      {/* 关闭行为 */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Monitor className="h-4 w-4" />
            {t("settings.close.title")}
          </CardTitle>
          <CardDescription>{t("settings.close.subtitle")}</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex items-center justify-between gap-4">
            <div>
              <div className="font-medium text-sm">{t("settings.close.behavior")}</div>
              <div className="text-xs text-muted-foreground">{t("settings.close.behaviorHint")}</div>
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
                  <SelectItem value="ask">{t("settings.close.ask")}</SelectItem>
                  <SelectItem value="minimize_to_tray">{t("settings.close.minimize")}</SelectItem>
                  <SelectItem value="quit">{t("settings.close.quit")}</SelectItem>
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
                  toast.success(t("settings.close.saved"));
                } catch (e) {
                  toast.error(t("settings.close.saveFail"), { description: String(e) });
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
        </CardContent>
      </Card>

      {/* 数据存储 */}
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
          <div className="space-y-2">
            <label className="text-sm font-medium">{t("settings.storage.dbSize")}</label>
            <div className="flex gap-2">
              <Input
                value={dbSize ? formatBytes(dbSize.total_bytes) : "-"}
                disabled
                className="font-mono text-sm"
              />
              <Button variant="outline" onClick={() => void refreshDbSize()} disabled={dbSizeLoading}>
                {t("common.refresh")}
              </Button>
            </div>
            <p className="text-xs text-muted-foreground">
              {dbSize
                ? t("settings.storage.dbSizeHint", {
                  db: formatBytes(dbSize.db_bytes),
                  wal: formatBytes(dbSize.wal_bytes),
                  shm: formatBytes(dbSize.shm_bytes),
                })
                : t("settings.storage.dbSizeHintEmpty")}
            </p>
          </div>
        </CardContent>
      </Card>

      {/* 记录清理 */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Trash2 className="h-4 w-4" />
            {t("settings.records.title")}
          </CardTitle>
          <CardDescription>{t("settings.records.subtitle")}</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <Dialog
            open={clearPromptOpen}
            onOpenChange={(v) => {
              if (clearing) return;
              setClearPromptOpen(v);
              if (!v) setClearMode(null);
            }}
          >
            <DialogContent className="sm:max-w-[520px]">
              <DialogHeader>
                <DialogTitle>{t("settings.records.confirmTitle")}</DialogTitle>
                <DialogDescription>
                  {t(clearPromptDescKey, {
                    date: clearDate.trim().length > 0 ? clearDate.trim() : "-",
                  })}
                </DialogDescription>
              </DialogHeader>
              <DialogFooter>
                <Button
                  variant="outline"
                  onClick={() => setClearPromptOpen(false)}
                  disabled={clearing}
                >
                  {t("common.cancel")}
                </Button>
                <Button
                  variant="destructive"
                  onClick={async () => {
                    if (!clearMode) return;
                    setClearing(true);
                    try {
                      const start_ms =
                        clearMode === "date_range" ? parseLocalDateStartMs(clearDate) : undefined;
                      const end_ms =
                        clearMode === "date_range" ? parseLocalDateEndMs(clearDate) : undefined;

                      if (clearMode === "date_range" && (start_ms === undefined || end_ms === undefined)) {
                        toast.error(t("settings.records.invalidDate"));
                        return;
                      }

                      const res = await clearRecords({
                        mode: clearMode,
                        start_ms,
                        end_ms,
                      });
                      toast.success(t("settings.records.cleared"), {
                        description: t("settings.records.clearedDetail", {
                          usage: res.usage_events_deleted.toLocaleString(),
                          failures: res.channel_failures_deleted.toLocaleString(),
                        }),
                      });
                      setClearPromptOpen(false);
                      await refreshDbSize();
                    } catch (e) {
                      toast.error(t("settings.records.clearFail"), { description: String(e) });
                    } finally {
                      setClearing(false);
                    }
                  }}
                  disabled={clearing}
                >
                  {clearing ? t("settings.records.clearing") : t("settings.records.clear")}
                </Button>
              </DialogFooter>
            </DialogContent>
          </Dialog>

          <div className="flex items-center justify-between gap-4">
            <div>
              <div className="font-medium text-sm">{t("settings.records.date")}</div>
              <div className="text-xs text-muted-foreground">{t("settings.records.dateHint")}</div>
            </div>
            <div className="flex items-center gap-2">
              <Input
                type="date"
                value={clearDate}
                onChange={(e) => setClearDate(e.target.value)}
                className="w-[160px]"
              />
              <Button
                variant="destructive"
                size="sm"
                onClick={() => {
                  const start = parseLocalDateStartMs(clearDate);
                  const end = parseLocalDateEndMs(clearDate);
                  if (start === undefined || end === undefined) {
                    toast.error(t("settings.records.invalidDate"));
                    return;
                  }
                  setClearMode("date_range");
                  setClearPromptOpen(true);
                }}
                disabled={clearing}
              >
                {t("settings.records.clear")}
              </Button>
            </div>
          </div>

          <div className="flex items-center justify-between gap-4">
            <div>
              <div className="font-medium text-sm">{t("settings.records.errors")}</div>
              <div className="text-xs text-muted-foreground">{t("settings.records.errorsHint")}</div>
            </div>
            <Button
              variant="destructive"
              size="sm"
              onClick={() => {
                setClearMode("errors");
                setClearPromptOpen(true);
              }}
              disabled={clearing}
            >
              {t("settings.records.clear")}
            </Button>
          </div>

          <div className="flex items-center justify-between gap-4">
            <div>
              <div className="font-medium text-sm">{t("settings.records.all")}</div>
              <div className="text-xs text-muted-foreground">{t("settings.records.allHint")}</div>
            </div>
            <Button
              variant="destructive"
              size="sm"
              onClick={() => {
                setClearMode("all");
                setClearPromptOpen(true);
              }}
              disabled={clearing}
            >
              {t("settings.records.clear")}
            </Button>
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

      {/* 关于 */}
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
    </div>
  );
}
