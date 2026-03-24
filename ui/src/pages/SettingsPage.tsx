import React, { useEffect, useState } from "react";
import { Sun, Moon, Monitor, FolderOpen, Info, Database, Languages, DollarSign, RefreshCw, Shield, Power, ScrollText, Palette, Settings2, Cpu, Bot, Trash2 } from "lucide-react";
import { toast } from "sonner";
import { format } from "date-fns";
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
  dateRangeToStrings,
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
import { PageHeader } from "@/components/PageHeader";
import { useTheme, type Theme } from "@/lib/theme";
import { type Locale, useI18n } from "@/lib/i18n";
import { humanizeApiError, humanizeIssue } from "@/lib/error";
import { installCliToolWithToast } from "@/lib/cliToolInstaller";
import { useCurrency, type CurrencyMode } from "@/lib/currency";
import { setLogLevel } from "@/lib/logger";
import { UpdatePromptDialog } from "@/components/UpdatePromptDialog";
import { postIpc } from "@/lib/ipc";
import { formatBytes, formatDateTime } from "../lib";
import { applyCliToolsProxyConfig, checkUpdate, clearLogs, clearRecords, createChatBridgePairingToken, deactivateChatBridgeBinding, downloadUpdate, getChatBridgeWeixinStatus, getChatBridgeWhatsAppStatus, getCliToolsProxyConfigStatus, getCliToolsStatus, getDbSize, getHealth, getLogsSize, getSettings, getUpdateChangelog, getUpdateStatus, ignoreUpdate, listChatBridgeBindings, logoutChatBridgeWeixin, logoutChatBridgeWhatsApp, openDataDir, pricingStatus, pricingSync, startChatBridgeWeixinLogin, startChatBridgeWhatsAppLogin, updateSettings, type AppSettings, type AutoStartLaunchMode, type ChatBridgeBinding, type ChatBridgePairingToken, type ChatBridgeWeixinStatus, type ChatBridgeWhatsAppStatus, type ChatPlatform, type CloseBehavior, type DbSize, type Health, type LogsSize, type PricingStatus, type RecordsClearMode, type UpdateCheck, type UpdateStatus, type ChangelogSection, type CliToolId, type CliToolsStatus, type CliToolStatus, type CliToolProxyConfigStatus, type CliToolProxyConfigToolStatus } from "../api";
import type { CliswitchUpdateStatusEvent } from "@/lib/cliswitchEvents";
import { clearUpdateReadyShown } from "@/lib/updateReadyPrompt";

function joinPath(base: string, sub: string): string {
  const sep = base.includes("\\") ? "\\" : "/";
  if (base.endsWith(sep)) return `${base}${sub}`;
  return `${base}${sep}${sub}`;
}

function normalizeQrImageSrc(src: string | null | undefined): string | null {
  if (typeof src !== "string") return null;
  const trimmed = src.trim();
  if (!trimmed) return null;
  if (trimmed.startsWith("data:image/") || trimmed.startsWith("blob:")) {
    return trimmed;
  }
  try {
    const parsed = new URL(trimmed);
    if (parsed.protocol === "http:" || parsed.protocol === "https:") {
      return trimmed;
    }
  } catch {
    return null;
  }
  return null;
}

export function SettingsPage() {
  const { theme, setTheme } = useTheme();
  const { locale, setLocale, locales, t } = useI18n();
  const { currencyMode, setCurrencyMode } = useCurrency();
  const [health, setHealth] = useState<Health | null>(null);
  const [pricing, setPricing] = useState<PricingStatus | null>(null);
  const [appSettings, setAppSettings] = useState<AppSettings | null>(null);
  const [updateStatus, setUpdateStatus] = useState<UpdateStatus | null>(null);
  const [updateChecking, setUpdateChecking] = useState(false);
  const [updatePromptOpen, setUpdatePromptOpen] = useState(false);
  const [updatePromptVersion, setUpdatePromptVersion] = useState<string | null>(null);
  const [updateCheckResult, setUpdateCheckResult] = useState<UpdateCheck | null>(null);
  const [updateChangelogSections, setUpdateChangelogSections] = useState<ChangelogSection[] | null>(null);
  const [updateChangelogLoading, setUpdateChangelogLoading] = useState(false);
  const [updateChangelogError, setUpdateChangelogError] = useState<string | null>(null);
  const [updateDownloading, setUpdateDownloading] = useState(false);
  const [updateIgnoring, setUpdateIgnoring] = useState(false);
  const [cliToolsStatus, setCliToolsStatus] = useState<CliToolsStatus | null>(null);
  const [cliToolsLoading, setCliToolsLoading] = useState(false);
  const [cliToolsProxyConfig, setCliToolsProxyConfig] = useState<CliToolProxyConfigStatus | null>(null);
  const [cliToolsProxyConfigLoading, setCliToolsProxyConfigLoading] = useState(false);
  const [cliProxyConfigBusy, setCliProxyConfigBusy] = useState<Record<CliToolId, boolean>>({
    gemini: false,
    claude: false,
    codex: false,
  });
  const [cliToolBusy, setCliToolBusy] = useState<Record<CliToolId, boolean>>({
    gemini: false,
    claude: false,
    codex: false,
  });
  const [saving, setSaving] = useState(false);
  const [autoDisableSaving, setAutoDisableSaving] = useState(false);
  const [closeSaving, setCloseSaving] = useState(false);
  const [autoStartSaving, setAutoStartSaving] = useState(false);
  const [autoStartLaunchSaving, setAutoStartLaunchSaving] = useState(false);
  const [serverLanSaving, setServerLanSaving] = useState(false);
  const [countTokensMockSaving, setCountTokensMockSaving] = useState(false);
  const [syncing, setSyncing] = useState(false);
  const [logSaving, setLogSaving] = useState(false);
  const [logRetentionDraft, setLogRetentionDraft] = useState<string>("");
  const [chatBridgeSaving, setChatBridgeSaving] = useState(false);
  const [chatBridgeBindings, setChatBridgeBindings] = useState<ChatBridgeBinding[]>([]);
  const [chatBridgeBindingsLoading, setChatBridgeBindingsLoading] = useState(false);
  const [chatBridgePairingToken, setChatBridgePairingToken] = useState<ChatBridgePairingToken | null>(null);
  const [chatBridgePairingCreating, setChatBridgePairingCreating] = useState(false);
  const [chatBridgePairingPlatform, setChatBridgePairingPlatform] = useState<ChatPlatform>("weixin");
  const [chatBridgeUnbindTarget, setChatBridgeUnbindTarget] = useState<ChatBridgeBinding | null>(null);
  const [chatBridgeUnbinding, setChatBridgeUnbinding] = useState(false);
  const [chatBridgeTelegramTokenDraft, setChatBridgeTelegramTokenDraft] = useState("");
  const [chatBridgeDiscordTokenDraft, setChatBridgeDiscordTokenDraft] = useState("");
  const [chatBridgePlatformTab, setChatBridgePlatformTab] = useState<ChatPlatform>("weixin");
  const [chatBridgePairingDialogOpen, setChatBridgePairingDialogOpen] = useState(false);
  const [chatBridgeBindingsDialogPlatform, setChatBridgeBindingsDialogPlatform] = useState<ChatPlatform | null>(null);
  const [chatBridgeWhatsAppStatus, setChatBridgeWhatsAppStatus] = useState<ChatBridgeWhatsAppStatus | null>(null);
  const [chatBridgeWhatsAppStatusLoading, setChatBridgeWhatsAppStatusLoading] = useState(false);
  const [chatBridgeWhatsAppActionBusy, setChatBridgeWhatsAppActionBusy] = useState(false);
  const [chatBridgeWeixinStatus, setChatBridgeWeixinStatus] = useState<ChatBridgeWeixinStatus | null>(null);
  const [chatBridgeWeixinStatusLoading, setChatBridgeWeixinStatusLoading] = useState(false);
  const [chatBridgeWeixinActionBusy, setChatBridgeWeixinActionBusy] = useState(false);

  // 数据库相关 state
  const [dbSize, setDbSize] = useState<DbSize | null>(null);
  const [dbSizeLoading, setDbSizeLoading] = useState(false);
  const [recordsType, setRecordsType] = useState<Exclude<RecordsClearMode, "date_range">>("all");
  const [recordsTimeScope, setRecordsTimeScope] = useState<"all" | "date_range">("all");
  const [recordsDateRange, setRecordsDateRange] = useState<DateRange | undefined>(undefined);
  const [recordsPromptOpen, setRecordsPromptOpen] = useState(false);
  const [recordsClearing, setRecordsClearing] = useState(false);

  // 日志清理相关 state
  const [logsSize, setLogsSize] = useState<LogsSize | null>(null);
  const [logsSizeLoading, setLogsSizeLoading] = useState(false);
  const [logsScope, setLogsScope] = useState<"all" | "date_range">("all");
  const [logsDateRange, setLogsDateRange] = useState<DateRange | undefined>(undefined);
  const [logsPromptOpen, setLogsPromptOpen] = useState(false);
  const [logsClearing, setLogsClearing] = useState(false);

  async function refreshDbSize() {
    setDbSizeLoading(true);
    try {
      const next = await getDbSize();
      setDbSize(next);
    } catch (e) {
      toast.error(t("settings.storage.dbSizeFail"), { description: humanizeApiError(e, t) });
    } finally {
      setDbSizeLoading(false);
    }
  }

  async function refreshLogsSize() {
    setLogsSizeLoading(true);
    try {
      const next = await getLogsSize();
      setLogsSize(next);
    } catch (e) {
      toast.error(t("settings.maintenance.logsSizeFail"), { description: humanizeApiError(e, t) });
    } finally {
      setLogsSizeLoading(false);
    }
  }

  async function refreshCliToolsStatus() {
    setCliToolsLoading(true);
    try {
      const next = await getCliToolsStatus();
      setCliToolsStatus(next);
    } catch (e) {
      toast.error(t("settings.cliTools.loadFail"), { description: humanizeApiError(e, t) });
    } finally {
      setCliToolsLoading(false);
    }
  }

  async function refreshCliToolsProxyConfigStatus() {
    setCliToolsProxyConfigLoading(true);
    try {
      const next = await getCliToolsProxyConfigStatus();
      setCliToolsProxyConfig(next);
    } catch (e) {
      toast.error(t("settings.cliProxyConfig.loadFail"), { description: humanizeApiError(e, t) });
    } finally {
      setCliToolsProxyConfigLoading(false);
    }
  }

  async function refreshChatBridgeBindings() {
    setChatBridgeBindingsLoading(true);
    try {
      const next = await listChatBridgeBindings();
      setChatBridgeBindings(next);
    } catch (e) {
      toast.error(t("settings.chatBridge.bindings.loadFail"), { description: humanizeApiError(e, t) });
    } finally {
      setChatBridgeBindingsLoading(false);
    }
  }

  async function confirmUnbind() {
    if (!chatBridgeUnbindTarget) return;
    setChatBridgeUnbinding(true);
    try {
      await deactivateChatBridgeBinding(chatBridgeUnbindTarget.id);
      toast.success(t("settings.chatBridge.bindings.unbindOk"));
      setChatBridgeUnbindTarget(null);
      await refreshChatBridgeBindings();
    } catch (e) {
      toast.error(t("settings.chatBridge.bindings.unbindFail"), { description: humanizeApiError(e, t) });
    } finally {
      setChatBridgeUnbinding(false);
    }
  }

  async function saveChatBridgeBaseConfig() {
    if (!appSettings) return;
    setChatBridgeSaving(true);
    try {
      const next = await updateSettings({
        chat_bridge_enabled: appSettings.chat_bridge_enabled,
        chat_bridge_allow_new_projects: appSettings.chat_bridge_allow_new_projects,
      });
      setAppSettings(next);
      toast.success(t("settings.chatBridge.saved"));
    } catch (e) {
      toast.error(t("settings.chatBridge.saveFail"), { description: humanizeApiError(e, t) });
    } finally {
      setChatBridgeSaving(false);
    }
  }

  async function saveTelegramConfig() {
    if (!appSettings) return;
    setChatBridgeSaving(true);
    try {
      const patch: Partial<AppSettings> = {
        chat_bridge_telegram_enabled: appSettings.chat_bridge_telegram_enabled,
      };
      if (chatBridgeTelegramTokenDraft.trim().length > 0) {
        patch.chat_bridge_telegram_bot_token = chatBridgeTelegramTokenDraft;
      }
      const next = await updateSettings(patch);
      setAppSettings(next);
      setChatBridgeTelegramTokenDraft("");
      toast.success(t("settings.chatBridge.saved"));
    } catch (e) {
      toast.error(t("settings.chatBridge.saveFail"), { description: humanizeApiError(e, t) });
    } finally {
      setChatBridgeSaving(false);
    }
  }

  async function saveDiscordConfig() {
    if (!appSettings) return;
    setChatBridgeSaving(true);
    try {
      const patch: Partial<AppSettings> = {
        chat_bridge_discord_enabled: appSettings.chat_bridge_discord_enabled,
      };
      if (chatBridgeDiscordTokenDraft.trim().length > 0) {
        patch.chat_bridge_discord_bot_token = chatBridgeDiscordTokenDraft;
      }
      const next = await updateSettings(patch);
      setAppSettings(next);
      setChatBridgeDiscordTokenDraft("");
      toast.success(t("settings.chatBridge.saved"));
    } catch (e) {
      toast.error(t("settings.chatBridge.saveFail"), { description: humanizeApiError(e, t) });
    } finally {
      setChatBridgeSaving(false);
    }
  }

  async function saveWhatsAppConfig(options?: { silent?: boolean }): Promise<AppSettings | null> {
    if (!appSettings) return null;
    setChatBridgeSaving(true);
    try {
      const next = await updateSettings({
        chat_bridge_whatsapp_enabled: appSettings.chat_bridge_whatsapp_enabled,
      });
      setAppSettings(next);
      if (!options?.silent) {
        toast.success(t("settings.chatBridge.saved"));
      }
      return next;
    } catch (e) {
      toast.error(t("settings.chatBridge.saveFail"), { description: humanizeApiError(e, t) });
      return null;
    } finally {
      setChatBridgeSaving(false);
    }
  }

  async function saveWeixinConfig(options?: { silent?: boolean }): Promise<AppSettings | null> {
    if (!appSettings) return null;
    setChatBridgeSaving(true);
    try {
      const next = await updateSettings({
        chat_bridge_weixin_enabled: appSettings.chat_bridge_weixin_enabled,
      });
      setAppSettings(next);
      if (!options?.silent) {
        toast.success(t("settings.chatBridge.saved"));
      }
      return next;
    } catch (e) {
      toast.error(t("settings.chatBridge.saveFail"), { description: humanizeApiError(e, t) });
      return null;
    } finally {
      setChatBridgeSaving(false);
    }
  }

  async function refreshChatBridgeWhatsAppStatus(options?: { silent?: boolean }) {
    const shouldShowSpinner = !options?.silent;
    if (shouldShowSpinner) {
      setChatBridgeWhatsAppStatusLoading(true);
    }
    try {
      const next = await getChatBridgeWhatsAppStatus();
      setChatBridgeWhatsAppStatus(next);
      return next;
    } catch (e) {
      if (!options?.silent) {
        toast.error(t("settings.chatBridge.whatsapp.statusLoadFail"), {
          description: humanizeApiError(e, t),
        });
      }
      return null;
    } finally {
      if (shouldShowSpinner) {
        setChatBridgeWhatsAppStatusLoading(false);
      }
    }
  }

  async function refreshChatBridgeWeixinStatus(options?: { silent?: boolean }) {
    const shouldShowSpinner = !options?.silent;
    if (shouldShowSpinner) {
      setChatBridgeWeixinStatusLoading(true);
    }
    try {
      const next = await getChatBridgeWeixinStatus();
      setChatBridgeWeixinStatus(next);
      return next;
    } catch (e) {
      if (!options?.silent) {
        toast.error(t("settings.chatBridge.weixin.statusLoadFail"), {
          description: humanizeApiError(e, t),
        });
      }
      return null;
    } finally {
      if (shouldShowSpinner) {
        setChatBridgeWeixinStatusLoading(false);
      }
    }
  }

  async function beginWhatsAppLogin() {
    if (!appSettings) return;
    setChatBridgeSaving(true);
    let saved: AppSettings | null = null;
    try {
      saved = await updateSettings({
        chat_bridge_enabled: appSettings.chat_bridge_enabled,
        chat_bridge_whatsapp_enabled: appSettings.chat_bridge_whatsapp_enabled,
      });
      setAppSettings(saved);
    } catch (e) {
      toast.error(t("settings.chatBridge.saveFail"), { description: humanizeApiError(e, t) });
      saved = null;
    } finally {
      setChatBridgeSaving(false);
    }

    if (!saved || !saved.chat_bridge_enabled || !saved.chat_bridge_whatsapp_enabled) {
      toast.error(t("settings.chatBridge.whatsapp.enableRequired"));
      return;
    }

    setChatBridgeWhatsAppActionBusy(true);
    try {
      await startChatBridgeWhatsAppLogin();
      toast.success(t("settings.chatBridge.whatsapp.loginStarted"));
      await refreshChatBridgeWhatsAppStatus({ silent: true });
    } catch (e) {
      toast.error(t("settings.chatBridge.whatsapp.loginFail"), {
        description: humanizeApiError(e, t),
      });
    } finally {
      setChatBridgeWhatsAppActionBusy(false);
    }
  }

  async function disconnectWhatsApp() {
    setChatBridgeWhatsAppActionBusy(true);
    try {
      await logoutChatBridgeWhatsApp();
      toast.success(t("settings.chatBridge.whatsapp.logoutOk"));
      await refreshChatBridgeWhatsAppStatus({ silent: true });
    } catch (e) {
      toast.error(t("settings.chatBridge.whatsapp.logoutFail"), {
        description: humanizeApiError(e, t),
      });
    } finally {
      setChatBridgeWhatsAppActionBusy(false);
    }
  }

  async function beginWeixinLogin() {
    if (!appSettings) return;
    setChatBridgeSaving(true);
    let saved: AppSettings | null = null;
    try {
      saved = await updateSettings({
        chat_bridge_enabled: appSettings.chat_bridge_enabled,
        chat_bridge_weixin_enabled: appSettings.chat_bridge_weixin_enabled,
      });
      setAppSettings(saved);
    } catch (e) {
      toast.error(t("settings.chatBridge.saveFail"), { description: humanizeApiError(e, t) });
      saved = null;
    } finally {
      setChatBridgeSaving(false);
    }

    if (!saved || !saved.chat_bridge_enabled || !saved.chat_bridge_weixin_enabled) {
      toast.error(t("settings.chatBridge.weixin.enableRequired"));
      return;
    }

    setChatBridgeWeixinActionBusy(true);
    try {
      await startChatBridgeWeixinLogin();
      toast.success(t("settings.chatBridge.weixin.loginStarted"));
      await refreshChatBridgeWeixinStatus({ silent: true });
    } catch (e) {
      toast.error(t("settings.chatBridge.weixin.loginFail"), {
        description: humanizeApiError(e, t),
      });
    } finally {
      setChatBridgeWeixinActionBusy(false);
    }
  }

  async function disconnectWeixin() {
    setChatBridgeWeixinActionBusy(true);
    try {
      await logoutChatBridgeWeixin();
      toast.success(t("settings.chatBridge.weixin.logoutOk"));
      await refreshChatBridgeWeixinStatus({ silent: true });
    } catch (e) {
      toast.error(t("settings.chatBridge.weixin.logoutFail"), {
        description: humanizeApiError(e, t),
      });
    } finally {
      setChatBridgeWeixinActionBusy(false);
    }
  }

  async function generateChatBridgePairingToken() {
    setChatBridgePairingCreating(true);
    try {
      const next = await createChatBridgePairingToken({
        platform: chatBridgePairingPlatform,
      });
      setChatBridgePairingToken(next);
      toast.success(t("settings.chatBridge.pairing.created"));
    } catch (e) {
      toast.error(t("settings.chatBridge.pairing.createFail"), { description: humanizeApiError(e, t) });
    } finally {
      setChatBridgePairingCreating(false);
    }
  }

  function openChatBridgePairing(platform: ChatPlatform) {
    setChatBridgePairingPlatform(platform);
    setChatBridgePairingToken(null);
    setChatBridgePairingDialogOpen(true);
  }

  function openChatBridgeBindings(platform: ChatPlatform) {
    setChatBridgeBindingsDialogPlatform(platform);
    void refreshChatBridgeBindings();
  }

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
        setChatBridgeTelegramTokenDraft("");
        setChatBridgeDiscordTokenDraft("");
        setLogRetentionDraft(String(s.log_retention_days ?? ""));
        setLogLevel(s.log_level);
      })
      .catch(() => setAppSettings(null));

    getUpdateStatus()
      .then(setUpdateStatus)
      .catch(() => setUpdateStatus(null));

    void refreshDbSize();
    void refreshLogsSize();
    void refreshCliToolsStatus();
    void refreshCliToolsProxyConfigStatus();
    void refreshChatBridgeBindings();
    void refreshChatBridgeWhatsAppStatus({ silent: true });
    void refreshChatBridgeWeixinStatus({ silent: true });
    // eslint-disable-next-line react-hooks/exhaustive-deps
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

  const chatBridgeBindingsByPlatform: Record<ChatPlatform, ChatBridgeBinding[]> = {
    telegram: chatBridgeBindings.filter((binding) => binding.platform === "telegram"),
    whatsapp: chatBridgeBindings.filter((binding) => binding.platform === "whatsapp"),
    weixin: chatBridgeBindings.filter((binding) => binding.platform === "weixin"),
    discord: chatBridgeBindings.filter((binding) => binding.platform === "discord"),
  };

  const chatBridgeBindingsDialogItems = chatBridgeBindingsDialogPlatform
    ? chatBridgeBindingsByPlatform[chatBridgeBindingsDialogPlatform]
    : [];

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
  const updateIgnored =
    (updateStatus?.latest_ignored ?? updateCheckResult?.latest_ignored ?? false) && !!updateServerVersion;
  const updateDownloadingSuffix =
    updateStatus && updateStatus.stage === "downloading"
      ? updateStatus.download_percent !== null
        ? t("settings.update.downloadingSuffix", { percent: updateStatus.download_percent })
        : t("settings.update.downloadingSuffixUnknown")
      : "";
  const updateStatusText = updateStatus?.pending_version
    ? t("settings.update.ready", { version: updateStatus.pending_version })
    : updateStatus?.stage === "error"
      ? humanizeIssue(updateStatus.issue, t) ?? t("settings.update.checkFail")
    : updateStatus?.stage === "staging"
      ? t("settings.update.staging")
    : updateStatus?.stage === "downloading"
      ? `${t("settings.update.latest")}${updateDownloadingSuffix}`
      : updateServerVersion
        ? (updateStatus?.update_available ?? updateCheckResult?.update_available)
          ? updateIgnored
            ? `${t("settings.update.available", { version: updateServerVersion })}${t("settings.update.ignoredSuffix")}`
            : t("settings.update.available", { version: updateServerVersion })
          : t("settings.update.latest")
        : "-";

  const recordsDateStr = recordsDateRange?.from
    ? `${format(recordsDateRange.from, "yyyy-MM-dd")}${recordsDateRange.to ? ` ~ ${format(recordsDateRange.to, "yyyy-MM-dd")}` : ""}`
    : "-";

  const logsDateStr = logsDateRange?.from
    ? `${format(logsDateRange.from, "yyyy-MM-dd")}${logsDateRange.to ? ` ~ ${format(logsDateRange.to, "yyyy-MM-dd")}` : ""}`
    : "-";

  function reopenUpdateReadyPrompt(status: UpdateStatus) {
    const version = status.pending_version;
    if (!version) return;
    clearUpdateReadyShown(version);
    window.dispatchEvent(new CustomEvent<UpdateStatus>("cliswitch-update-status", { detail: status }));
  }

  function openUpdatePrompt(version: string) {
    setUpdatePromptVersion(version);
    setUpdateChangelogSections(null);
    setUpdateChangelogError(null);
    setUpdatePromptOpen(true);
  }

  useEffect(() => {
    if (!updatePromptOpen || !updatePromptVersion) return;
    let cancelled = false;
    setUpdateChangelogLoading(true);
    setUpdateChangelogError(null);
    getUpdateChangelog(updatePromptVersion, locale)
      .then((res) => {
        if (cancelled) return;
        setUpdateChangelogSections(res.sections);
      })
      .catch((e) => {
        if (cancelled) return;
        setUpdateChangelogError(humanizeApiError(e, t));
        setUpdateChangelogSections(null);
      })
      .finally(() => {
        if (cancelled) return;
        setUpdateChangelogLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [updatePromptOpen, updatePromptVersion, locale, t]);

  useEffect(() => {
    const whatsappEnabled =
      (appSettings?.chat_bridge_enabled ?? false) &&
      (appSettings?.chat_bridge_whatsapp_enabled ?? false);
    if (chatBridgePlatformTab !== "whatsapp" || !whatsappEnabled) {
      return;
    }

    let cancelled = false;
    let timer: number | null = null;
    const nextPollDelay = (status: ChatBridgeWhatsAppStatus | null) => {
      if (!status) return 5_000;
      if (status.connected) return 15_000;
      if (status.state === "awaiting_qr") return 2_000;
      if (status.state === "error") return 5_000;
      return 3_000;
    };
    const refresh = async () => {
      const next = await refreshChatBridgeWhatsAppStatus({ silent: true });
      if (cancelled) return;
      timer = window.setTimeout(() => {
        void refresh();
      }, nextPollDelay(next));
    };

    void refresh();

    return () => {
      cancelled = true;
      if (timer !== null) {
        window.clearTimeout(timer);
      }
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [
    appSettings?.chat_bridge_enabled,
    appSettings?.chat_bridge_whatsapp_enabled,
    chatBridgePlatformTab,
  ]);

  useEffect(() => {
    const weixinEnabled =
      (appSettings?.chat_bridge_enabled ?? false) &&
      (appSettings?.chat_bridge_weixin_enabled ?? false);
    if (chatBridgePlatformTab !== "weixin" || !weixinEnabled) {
      return;
    }

    let cancelled = false;
    let timer: number | null = null;
    const nextPollDelay = (status: ChatBridgeWeixinStatus | null) => {
      if (!status) return 5_000;
      if (status.connected) return 15_000;
      if (status.state === "awaiting_qr") return 2_000;
      if (status.state === "error") return 5_000;
      return 3_000;
    };
    const refresh = async () => {
      const next = await refreshChatBridgeWeixinStatus({ silent: true });
      if (cancelled) return;
      timer = window.setTimeout(() => {
        void refresh();
      }, nextPollDelay(next));
    };

    void refresh();

    return () => {
      cancelled = true;
      if (timer !== null) {
        window.clearTimeout(timer);
      }
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [
    appSettings?.chat_bridge_enabled,
    appSettings?.chat_bridge_weixin_enabled,
    chatBridgePlatformTab,
  ]);

  const whatsappStatusKey = chatBridgeWhatsAppStatus?.state ?? "disabled";
  const whatsappConnected = chatBridgeWhatsAppStatus?.connected ?? false;
  const whatsappQrImageSrc = normalizeQrImageSrc(chatBridgeWhatsAppStatus?.qr_image);
  const whatsappStatusLabel = t(`settings.chatBridge.whatsapp.state.${whatsappStatusKey}`);
  const whatsappStatusTone = whatsappStatusKey === "error"
    ? "destructive"
    : whatsappConnected
      ? "success"
      : "secondary";
  const weixinStatusKey = chatBridgeWeixinStatus?.state ?? "disabled";
  const weixinConnected = chatBridgeWeixinStatus?.connected ?? false;
  const weixinQrImageSrc = normalizeQrImageSrc(chatBridgeWeixinStatus?.qr_image);
  const weixinStatusLabel = t(`settings.chatBridge.weixin.state.${weixinStatusKey}`);
  const weixinStatusTone = weixinStatusKey === "error"
    ? "destructive"
    : weixinConnected
      ? "success"
      : "secondary";

  const chatBridgeTab = (
    <>
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Bot className="h-4 w-4" />
            {t("settings.chatBridge.configTitle")}
          </CardTitle>
          <CardDescription>{t("settings.chatBridge.configHint")}</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex items-center justify-between gap-4">
            <div>
              <div className="font-medium text-sm">{t("settings.chatBridge.enable")}</div>
              <div className="text-xs text-muted-foreground">{t("settings.chatBridge.enableHint")}</div>
            </div>
            <Switch
              checked={appSettings?.chat_bridge_enabled ?? false}
              onCheckedChange={(v) => {
                setAppSettings((prev) => (prev ? { ...prev, chat_bridge_enabled: v } : prev));
              }}
              disabled={!appSettings || chatBridgeSaving}
            />
          </div>

          <div className="flex items-center justify-between gap-4">
            <div>
              <div className="font-medium text-sm">{t("settings.chatBridge.allowNewProjects")}</div>
              <div className="text-xs text-muted-foreground">{t("settings.chatBridge.allowNewProjectsHint")}</div>
            </div>
            <Switch
              checked={appSettings?.chat_bridge_allow_new_projects ?? false}
              onCheckedChange={(v) => {
                setAppSettings((prev) => (prev ? { ...prev, chat_bridge_allow_new_projects: v } : prev));
              }}
              disabled={!appSettings || chatBridgeSaving}
            />
          </div>

          <div className="flex justify-end">
            <Button
              size="sm"
              disabled={!appSettings || chatBridgeSaving}
              onClick={() => void saveChatBridgeBaseConfig()}
            >
              {t("common.save")}
            </Button>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Bot className="h-4 w-4" />
            {t("settings.chatBridge.platformConfigTitle")}
          </CardTitle>
          <CardDescription>{t("settings.chatBridge.platformConfigHint")}</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <Tabs value={chatBridgePlatformTab} onValueChange={(value) => setChatBridgePlatformTab(value as ChatPlatform)} className="w-full">
            <TabsList className="w-full justify-start">
              <TabsTrigger value="weixin">{t("settings.chatBridge.platform.weixin")}</TabsTrigger>
              <TabsTrigger value="telegram">{t("settings.chatBridge.platform.telegram")}</TabsTrigger>
              <TabsTrigger value="whatsapp">{t("settings.chatBridge.platform.whatsapp")}</TabsTrigger>
              <TabsTrigger value="discord">{t("settings.chatBridge.platform.discord")}</TabsTrigger>
            </TabsList>

            <TabsContent value="telegram" className="mt-4">
              <div className="rounded-lg border p-4 space-y-4">
                <div className="flex items-start justify-between gap-3">
                  <div className="space-y-1">
                    <div className="font-medium text-sm">{t("settings.chatBridge.telegramConfigTitle")}</div>
                    <div className="text-xs text-muted-foreground">{t("settings.chatBridge.telegramConfigHint")}</div>
                  </div>
                  <Badge variant="secondary">{t("settings.chatBridge.supportReady")}</Badge>
                </div>

                <div className="flex items-center justify-between gap-4">
                  <div>
                    <div className="font-medium text-sm">{t("settings.chatBridge.telegramEnable")}</div>
                    <div className="text-xs text-muted-foreground">{t("settings.chatBridge.telegramEnableHint")}</div>
                  </div>
                  <Switch
                    checked={appSettings?.chat_bridge_telegram_enabled ?? false}
                    onCheckedChange={(v) => {
                      setAppSettings((prev) => (prev ? { ...prev, chat_bridge_telegram_enabled: v } : prev));
                    }}
                    disabled={!appSettings || chatBridgeSaving}
                  />
                </div>

                <div className="space-y-2">
                  <label className="text-sm font-medium">{t("settings.chatBridge.telegramToken")}</label>
                  <Input
                    type="password"
                    autoComplete="new-password"
                    value={chatBridgeTelegramTokenDraft}
                    onChange={(e) => {
                      const value = e.target.value;
                      setChatBridgeTelegramTokenDraft(value);
                    }}
                    placeholder={appSettings?.chat_bridge_telegram_bot_token_configured
                      ? t("settings.chatBridge.telegramTokenPlaceholderConfigured")
                      : t("settings.chatBridge.telegramTokenPlaceholder")}
                    disabled={!appSettings || chatBridgeSaving}
                  />
                  <p className="text-xs text-muted-foreground">
                    {appSettings?.chat_bridge_telegram_bot_token_configured
                      ? t("settings.chatBridge.telegramTokenHintConfigured")
                      : t("settings.chatBridge.telegramTokenHint")}
                  </p>
                </div>

                <div className="flex items-center justify-between gap-4 rounded-lg border bg-muted/30 px-3 py-3">
                  <div className="space-y-1">
                    <div className="font-medium text-sm">{t("settings.chatBridge.bindings.title")}</div>
                    <div className="text-xs text-muted-foreground">
                      {t("settings.chatBridge.bindings.summary", { count: chatBridgeBindingsByPlatform.telegram.length })}
                    </div>
                  </div>
                  <div className="flex items-center gap-2">
                    <Button
                      size="sm"
                      variant="outline"
                      onClick={() => openChatBridgePairing("telegram")}
                    >
                      {t("settings.chatBridge.actions.pairing")}
                    </Button>
                    <Button
                      size="sm"
                      variant="outline"
                      onClick={() => openChatBridgeBindings("telegram")}
                    >
                      {t("settings.chatBridge.actions.bindings")}
                    </Button>
                  </div>
                </div>

                <div className="flex justify-end">
                  <Button
                    size="sm"
                    disabled={!appSettings || chatBridgeSaving}
                    onClick={() => void saveTelegramConfig()}
                  >
                    {t("common.save")}
                  </Button>
                </div>
              </div>
            </TabsContent>

            <TabsContent value="discord" className="mt-4">
              <div className="rounded-lg border p-4 space-y-4">
                <div className="flex items-start justify-between gap-3">
                  <div className="space-y-1">
                    <div className="font-medium text-sm">{t("settings.chatBridge.discordConfigTitle")}</div>
                    <div className="text-xs text-muted-foreground">{t("settings.chatBridge.discordConfigHint")}</div>
                  </div>
                  <Badge variant="secondary">{t("settings.chatBridge.supportReady")}</Badge>
                </div>

                <div className="flex items-center justify-between gap-4">
                  <div>
                    <div className="font-medium text-sm">{t("settings.chatBridge.discordEnable")}</div>
                    <div className="text-xs text-muted-foreground">{t("settings.chatBridge.discordEnableHint")}</div>
                  </div>
                  <Switch
                    checked={appSettings?.chat_bridge_discord_enabled ?? false}
                    onCheckedChange={(v) => {
                      setAppSettings((prev) => (prev ? { ...prev, chat_bridge_discord_enabled: v } : prev));
                    }}
                    disabled={!appSettings || chatBridgeSaving}
                  />
                </div>

                <div className="space-y-2">
                  <label className="text-sm font-medium">{t("settings.chatBridge.discordToken")}</label>
                  <Input
                    type="password"
                    autoComplete="new-password"
                    value={chatBridgeDiscordTokenDraft}
                    onChange={(e) => {
                      const value = e.target.value;
                      setChatBridgeDiscordTokenDraft(value);
                    }}
                    placeholder={appSettings?.chat_bridge_discord_bot_token_configured
                      ? t("settings.chatBridge.discordTokenPlaceholderConfigured")
                      : t("settings.chatBridge.discordTokenPlaceholder")}
                    disabled={!appSettings || chatBridgeSaving}
                  />
                  <p className="text-xs text-muted-foreground">
                    {appSettings?.chat_bridge_discord_bot_token_configured
                      ? t("settings.chatBridge.discordTokenHintConfigured")
                      : t("settings.chatBridge.discordTokenHint")}
                  </p>
                </div>

                <div className="flex items-center justify-between gap-4 rounded-lg border bg-muted/30 px-3 py-3">
                  <div className="space-y-1">
                    <div className="font-medium text-sm">{t("settings.chatBridge.bindings.title")}</div>
                    <div className="text-xs text-muted-foreground">
                      {t("settings.chatBridge.bindings.summary", { count: chatBridgeBindingsByPlatform.discord.length })}
                    </div>
                  </div>
                  <div className="flex items-center gap-2">
                    <Button
                      size="sm"
                      variant="outline"
                      onClick={() => openChatBridgePairing("discord")}
                    >
                      {t("settings.chatBridge.actions.pairing")}
                    </Button>
                    <Button
                      size="sm"
                      variant="outline"
                      onClick={() => openChatBridgeBindings("discord")}
                    >
                      {t("settings.chatBridge.actions.bindings")}
                    </Button>
                  </div>
                </div>

                <div className="flex justify-end">
                  <Button
                    size="sm"
                    disabled={!appSettings || chatBridgeSaving}
                    onClick={() => void saveDiscordConfig()}
                  >
                    {t("common.save")}
                  </Button>
                </div>
              </div>
            </TabsContent>

            <TabsContent value="whatsapp" className="mt-4">
              <div className="rounded-lg border p-4 space-y-4">
                <div className="flex items-start justify-between gap-3">
                  <div className="space-y-1">
                    <div className="font-medium text-sm">{t("settings.chatBridge.whatsappConfigTitle")}</div>
                    <div className="text-xs text-muted-foreground">{t("settings.chatBridge.whatsappConfigHint")}</div>
                  </div>
                  <Badge variant="secondary">{t("settings.chatBridge.supportReady")}</Badge>
                </div>

                <div className="flex items-center justify-between gap-4">
                  <div>
                    <div className="font-medium text-sm">{t("settings.chatBridge.whatsappEnable")}</div>
                    <div className="text-xs text-muted-foreground">{t("settings.chatBridge.whatsappEnableHint")}</div>
                  </div>
                  <Switch
                    checked={appSettings?.chat_bridge_whatsapp_enabled ?? false}
                    onCheckedChange={(v) => {
                      setAppSettings((prev) => (prev ? { ...prev, chat_bridge_whatsapp_enabled: v } : prev));
                    }}
                    disabled={!appSettings || chatBridgeSaving}
                  />
                </div>

                <div className="rounded-lg border bg-muted/20 p-4 space-y-3">
                  <div className="flex items-start justify-between gap-3">
                    <div className="space-y-1">
                      <div className="font-medium text-sm">{t("settings.chatBridge.whatsapp.runtimeTitle")}</div>
                      <div className="text-xs text-muted-foreground">{t("settings.chatBridge.whatsapp.runtimeHint")}</div>
                    </div>
                    <Badge variant={whatsappStatusTone}>{whatsappStatusLabel}</Badge>
                  </div>

                  <div className="grid gap-3 md:grid-cols-2">
                    <div className="rounded-lg border bg-background px-3 py-3 space-y-1">
                      <div className="text-xs text-muted-foreground">{t("settings.chatBridge.whatsapp.connectionLabel")}</div>
                      <div className="text-sm font-medium">
                        {whatsappConnected
                          ? t("settings.chatBridge.whatsapp.connectionConnected")
                          : t("settings.chatBridge.whatsapp.connectionDisconnected")}
                      </div>
                    </div>
                    <div className="rounded-lg border bg-background px-3 py-3 space-y-1">
                      <div className="text-xs text-muted-foreground">{t("settings.chatBridge.whatsapp.accountLabel")}</div>
                      <div className="text-sm font-medium break-all">
                        {chatBridgeWhatsAppStatus?.me || t("settings.chatBridge.whatsapp.accountEmpty")}
                      </div>
                    </div>
                  </div>

                  {chatBridgeWhatsAppStatus?.last_error ? (
                    <div className="rounded-lg border border-destructive/30 bg-destructive/5 px-3 py-3 space-y-1">
                      <div className="text-xs text-muted-foreground">{t("settings.chatBridge.whatsapp.lastErrorLabel")}</div>
                      <div className="text-sm break-words">{chatBridgeWhatsAppStatus.last_error}</div>
                    </div>
                  ) : null}

                  <div className="rounded-lg border bg-background px-3 py-3 space-y-3">
                    <div className="space-y-1">
                      <div className="font-medium text-sm">{t("settings.chatBridge.whatsapp.qrTitle")}</div>
                      <div className="text-xs text-muted-foreground">{t("settings.chatBridge.whatsapp.qrHint")}</div>
                    </div>
                    {whatsappQrImageSrc ? (
                      <div className="flex justify-center">
                        <img
                          src={whatsappQrImageSrc}
                          alt={t("settings.chatBridge.whatsapp.qrAlt")}
                          className="h-56 w-56 rounded-lg border bg-white p-3"
                        />
                      </div>
                    ) : (
                      <div className="rounded-lg border border-dashed px-3 py-6 text-center text-sm text-muted-foreground">
                        {chatBridgeWhatsAppStatusLoading
                          ? t("common.loading")
                          : t("settings.chatBridge.whatsapp.qrEmpty")}
                      </div>
                    )}
                  </div>

                  <div className="flex flex-wrap gap-2">
                    <Button
                      size="sm"
                      onClick={() => void beginWhatsAppLogin()}
                      disabled={!appSettings || chatBridgeSaving || chatBridgeWhatsAppActionBusy}
                    >
                      {t("settings.chatBridge.whatsapp.loginAction")}
                    </Button>
                    <Button
                      size="sm"
                      variant="outline"
                      onClick={() => void disconnectWhatsApp()}
                      disabled={!appSettings || chatBridgeSaving || chatBridgeWhatsAppActionBusy}
                    >
                      {t("settings.chatBridge.whatsapp.logoutAction")}
                    </Button>
                    <Button
                      size="sm"
                      variant="outline"
                      onClick={() => void refreshChatBridgeWhatsAppStatus()}
                      disabled={chatBridgeWhatsAppStatusLoading || chatBridgeWhatsAppActionBusy}
                    >
                      {t("common.refresh")}
                    </Button>
                  </div>
                </div>

                <div className="flex items-center justify-between gap-4 rounded-lg border bg-muted/30 px-3 py-3">
                  <div className="space-y-1">
                    <div className="font-medium text-sm">{t("settings.chatBridge.bindings.title")}</div>
                    <div className="text-xs text-muted-foreground">
                      {t("settings.chatBridge.bindings.summary", { count: chatBridgeBindingsByPlatform.whatsapp.length })}
                    </div>
                  </div>
                  <div className="flex items-center gap-2">
                    <Button
                      size="sm"
                      variant="outline"
                      onClick={() => openChatBridgePairing("whatsapp")}
                    >
                      {t("settings.chatBridge.actions.pairing")}
                    </Button>
                    <Button
                      size="sm"
                      variant="outline"
                      onClick={() => openChatBridgeBindings("whatsapp")}
                    >
                      {t("settings.chatBridge.actions.bindings")}
                    </Button>
                  </div>
                </div>

                <div className="flex justify-end">
                  <Button
                    size="sm"
                    disabled={!appSettings || chatBridgeSaving}
                    onClick={() => void saveWhatsAppConfig()}
                  >
                    {t("common.save")}
                  </Button>
                </div>
              </div>
            </TabsContent>

            <TabsContent value="weixin" className="mt-4">
              <div className="rounded-lg border p-4 space-y-4">
                <div className="flex items-start justify-between gap-3">
                  <div className="space-y-1">
                    <div className="font-medium text-sm">{t("settings.chatBridge.weixinConfigTitle")}</div>
                    <div className="text-xs text-muted-foreground">{t("settings.chatBridge.weixinConfigHint")}</div>
                  </div>
                  <Badge variant="secondary">{t("settings.chatBridge.supportReady")}</Badge>
                </div>

                <div className="flex items-center justify-between gap-4">
                  <div>
                    <div className="font-medium text-sm">{t("settings.chatBridge.weixinEnable")}</div>
                    <div className="text-xs text-muted-foreground">{t("settings.chatBridge.weixinEnableHint")}</div>
                  </div>
                  <Switch
                    checked={appSettings?.chat_bridge_weixin_enabled ?? false}
                    onCheckedChange={(v) => {
                      setAppSettings((prev) => (prev ? { ...prev, chat_bridge_weixin_enabled: v } : prev));
                    }}
                    disabled={!appSettings || chatBridgeSaving}
                  />
                </div>

                <div className="rounded-lg border bg-muted/20 p-4 space-y-3">
                  <div className="flex items-start justify-between gap-3">
                    <div className="space-y-1">
                      <div className="font-medium text-sm">{t("settings.chatBridge.weixin.runtimeTitle")}</div>
                      <div className="text-xs text-muted-foreground">{t("settings.chatBridge.weixin.runtimeHint")}</div>
                    </div>
                    <Badge variant={weixinStatusTone}>{weixinStatusLabel}</Badge>
                  </div>

                  <div className="grid gap-3 md:grid-cols-2">
                    <div className="rounded-lg border bg-background px-3 py-3 space-y-1">
                      <div className="text-xs text-muted-foreground">{t("settings.chatBridge.weixin.connectionLabel")}</div>
                      <div className="text-sm font-medium">
                        {weixinConnected
                          ? t("settings.chatBridge.weixin.connectionConnected")
                          : t("settings.chatBridge.weixin.connectionDisconnected")}
                      </div>
                    </div>
                    <div className="rounded-lg border bg-background px-3 py-3 space-y-1">
                      <div className="text-xs text-muted-foreground">{t("settings.chatBridge.weixin.accountLabel")}</div>
                      <div className="text-sm font-medium break-all">
                        {chatBridgeWeixinStatus?.me || t("settings.chatBridge.weixin.accountEmpty")}
                      </div>
                    </div>
                  </div>

                  {chatBridgeWeixinStatus?.last_error ? (
                    <div className="rounded-lg border border-destructive/30 bg-destructive/5 px-3 py-3 space-y-1">
                      <div className="text-xs text-muted-foreground">{t("settings.chatBridge.weixin.lastErrorLabel")}</div>
                      <div className="text-sm break-words">{chatBridgeWeixinStatus.last_error}</div>
                    </div>
                  ) : null}

                  <div className="rounded-lg border bg-background px-3 py-3 space-y-3">
                    <div className="space-y-1">
                      <div className="font-medium text-sm">{t("settings.chatBridge.weixin.qrTitle")}</div>
                      <div className="text-xs text-muted-foreground">{t("settings.chatBridge.weixin.qrHint")}</div>
                    </div>
                    {weixinQrImageSrc ? (
                      <div className="flex justify-center">
                        <img
                          src={weixinQrImageSrc}
                          alt={t("settings.chatBridge.weixin.qrAlt")}
                          className="h-56 w-56 rounded-lg border bg-white p-3"
                        />
                      </div>
                    ) : (
                      <div className="rounded-lg border border-dashed px-3 py-6 text-center text-sm text-muted-foreground">
                        {chatBridgeWeixinStatusLoading
                          ? t("common.loading")
                          : t("settings.chatBridge.weixin.qrEmpty")}
                      </div>
                    )}
                  </div>

                  <div className="flex flex-wrap gap-2">
                    <Button
                      size="sm"
                      onClick={() => void beginWeixinLogin()}
                      disabled={!appSettings || chatBridgeSaving || chatBridgeWeixinActionBusy}
                    >
                      {t("settings.chatBridge.weixin.loginAction")}
                    </Button>
                    <Button
                      size="sm"
                      variant="outline"
                      onClick={() => void disconnectWeixin()}
                      disabled={!appSettings || chatBridgeSaving || chatBridgeWeixinActionBusy}
                    >
                      {t("settings.chatBridge.weixin.logoutAction")}
                    </Button>
                    <Button
                      size="sm"
                      variant="outline"
                      onClick={() => void refreshChatBridgeWeixinStatus()}
                      disabled={chatBridgeWeixinStatusLoading || chatBridgeWeixinActionBusy}
                    >
                      {t("common.refresh")}
                    </Button>
                  </div>
                </div>

                <div className="flex items-center justify-between gap-4 rounded-lg border bg-muted/30 px-3 py-3">
                  <div className="space-y-1">
                    <div className="font-medium text-sm">{t("settings.chatBridge.bindings.title")}</div>
                    <div className="text-xs text-muted-foreground">
                      {t("settings.chatBridge.bindings.summary", { count: chatBridgeBindingsByPlatform.weixin.length })}
                    </div>
                  </div>
                  <div className="flex items-center gap-2">
                    <Button
                      size="sm"
                      variant="outline"
                      onClick={() => openChatBridgePairing("weixin")}
                    >
                      {t("settings.chatBridge.actions.pairing")}
                    </Button>
                    <Button
                      size="sm"
                      variant="outline"
                      onClick={() => openChatBridgeBindings("weixin")}
                    >
                      {t("settings.chatBridge.actions.bindings")}
                    </Button>
                  </div>
                </div>

                <div className="flex justify-end">
                  <Button
                    size="sm"
                    disabled={!appSettings || chatBridgeSaving}
                    onClick={() => void saveWeixinConfig()}
                  >
                    {t("common.save")}
                  </Button>
                </div>
              </div>
            </TabsContent>
          </Tabs>
        </CardContent>
      </Card>

      <Dialog
        open={chatBridgePairingDialogOpen}
        onOpenChange={(open) => {
          if (!chatBridgePairingCreating) {
            setChatBridgePairingDialogOpen(open);
          }
        }}
      >
        <DialogContent className="sm:max-w-[560px]">
          <DialogHeader>
            <DialogTitle>{t("settings.chatBridge.pairing.title")}</DialogTitle>
            <DialogDescription>
              {t("settings.chatBridge.pairing.subtitle", {
                platform: t(`settings.chatBridge.platform.${chatBridgePairingPlatform}`),
              })}
            </DialogDescription>
          </DialogHeader>

          <div className="space-y-4">
            <div className="space-y-2">
              <label className="text-sm font-medium">{t("settings.chatBridge.pairing.platform")}</label>
              <Input
                value={t(`settings.chatBridge.platform.${chatBridgePairingPlatform}`)}
                disabled
              />
              <p className="text-xs text-muted-foreground">
                {t("settings.chatBridge.pairing.platformHint", {
                  platform: t(`settings.chatBridge.platform.${chatBridgePairingPlatform}`),
                })}
              </p>
            </div>

            <div className="space-y-2">
              <label className="text-sm font-medium">{t("settings.chatBridge.pairing.token")}</label>
              <Input value={chatBridgePairingToken?.token ?? ""} disabled placeholder={t("settings.chatBridge.pairing.empty")} className="font-mono text-sm" />
              <p className="text-xs text-muted-foreground">{t("settings.chatBridge.pairing.tokenHint")}</p>
            </div>

            <div className="space-y-2">
              <label className="text-sm font-medium">{t("settings.chatBridge.pairing.expiresAt")}</label>
              <Input
                value={chatBridgePairingToken?.expires_at_ms ? formatDateTime(chatBridgePairingToken.expires_at_ms) : "-"}
                disabled
              />
              <p className="text-xs text-muted-foreground">{t("settings.chatBridge.pairing.expiresHint")}</p>
            </div>
          </div>

          <DialogFooter>
            <Button
              variant="outline"
              onClick={() => setChatBridgePairingDialogOpen(false)}
              disabled={chatBridgePairingCreating}
            >
              {t("common.cancel")}
            </Button>
            <Button onClick={() => void generateChatBridgePairingToken()} disabled={chatBridgePairingCreating}>
              {t("settings.chatBridge.pairing.generate")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog
        open={!!chatBridgeBindingsDialogPlatform}
        onOpenChange={(open) => {
          if (!open && !chatBridgeUnbinding) {
            setChatBridgeBindingsDialogPlatform(null);
          }
        }}
      >
        <DialogContent className="sm:max-w-[560px]">
          <DialogHeader>
            <DialogTitle>
              {t("settings.chatBridge.bindings.dialogTitle", {
                platform: t(`settings.chatBridge.platform.${chatBridgeBindingsDialogPlatform ?? "telegram"}`),
              })}
            </DialogTitle>
            <DialogDescription>
              {t("settings.chatBridge.bindings.dialogSubtitle", {
                platform: t(`settings.chatBridge.platform.${chatBridgeBindingsDialogPlatform ?? "telegram"}`),
              })}
            </DialogDescription>
          </DialogHeader>

          {chatBridgeBindingsLoading && chatBridgeBindingsDialogItems.length === 0 ? (
            <div className="text-sm text-muted-foreground">{t("common.loading")}</div>
          ) : chatBridgeBindingsDialogItems.length === 0 ? (
            <div className="text-sm text-muted-foreground">
              {t("settings.chatBridge.bindings.emptyForPlatform", {
                platform: t(`settings.chatBridge.platform.${chatBridgeBindingsDialogPlatform ?? "telegram"}`),
              })}
            </div>
          ) : (
            <div className="space-y-2 max-h-[360px] overflow-y-auto pr-1">
              {chatBridgeBindingsDialogItems.map((binding) => (
                <div key={binding.id} className="flex items-start justify-between gap-4 rounded-lg border bg-background px-3 py-2">
                  <div className="min-w-0">
                    <div className="font-medium text-sm truncate">
                      {binding.display_name ?? binding.platform_user_id}
                    </div>
                    <div className="text-xs text-muted-foreground truncate">
                      {binding.platform_user_id}
                    </div>
                  </div>
                  <div className="flex items-center gap-2 flex-shrink-0">
                    <div className="text-xs text-muted-foreground whitespace-nowrap">
                      {formatDateTime(binding.bound_at_ms)}
                    </div>
                    <Button
                      variant="ghost"
                      size="icon"
                      className="h-7 w-7 text-destructive hover:text-destructive"
                      onClick={() => setChatBridgeUnbindTarget(binding)}
                    >
                      <Trash2 className="h-4 w-4" />
                    </Button>
                  </div>
                </div>
              ))}
            </div>
          )}

          <DialogFooter>
            <Button variant="outline" onClick={() => setChatBridgeBindingsDialogPlatform(null)} disabled={chatBridgeUnbinding}>
              {t("common.ok")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog open={!!chatBridgeUnbindTarget} onOpenChange={(v) => { if (!chatBridgeUnbinding) { if (!v) setChatBridgeUnbindTarget(null); } }}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t("settings.chatBridge.bindings.unbindTitle")}</DialogTitle>
            <DialogDescription>
              {t("settings.chatBridge.bindings.unbindConfirm", {
                name: chatBridgeUnbindTarget?.display_name ?? chatBridgeUnbindTarget?.platform_user_id ?? "",
                platform: t(`settings.chatBridge.platform.${chatBridgeUnbindTarget?.platform ?? "telegram"}`)
              })}
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" onClick={() => setChatBridgeUnbindTarget(null)} disabled={chatBridgeUnbinding}>
              {t("common.cancel")}
            </Button>
            <Button variant="destructive" onClick={confirmUnbind} disabled={chatBridgeUnbinding}>
              {t("settings.chatBridge.bindings.unbind")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );

  return (
    <div className="space-y-4 pb-4">
      <PageHeader title={t("settings.title")} />

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
          <TabsTrigger value="update" className="gap-1.5">
            <RefreshCw className="h-3.5 w-3.5" />
            {t("settings.tabs.update")}
          </TabsTrigger>
          <TabsTrigger value="data" className="gap-1.5">
            <Database className="h-3.5 w-3.5" />
            {t("settings.tabs.data")}
          </TabsTrigger>
          <TabsTrigger value="chatBridge" className="gap-1.5">
            <Bot className="h-3.5 w-3.5" />
            {t("settings.tabs.chatBridge")}
          </TabsTrigger>
          <TabsTrigger value="system" className="gap-1.5">
            <Cpu className="h-3.5 w-3.5" />
            {t("settings.tabs.system")}
          </TabsTrigger>
        </TabsList>

        {/* 界面标签页 */}
        <TabsContent value="appearance" className="mt-2 space-y-4">
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
                  <Select
                    value={locale}
                    onValueChange={async (v) => {
                      const nextLocale = v as Locale;
                      try {
                        const next = await updateSettings({ ui_locale: nextLocale });
                        setAppSettings(next);
                        setLocale(next.ui_locale);
                      } catch (e) {
                        toast.error(t("settings.language.saveFail"), {
                          description: humanizeApiError(e, t),
                        });
                      }
                    }}
                  >
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

          {/* 货币单位 */}
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <DollarSign className="h-4 w-4" />
                {t("settings.currency.title")}
              </CardTitle>
              <CardDescription>{t("settings.currency.subtitle")}</CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="flex items-center justify-between gap-4">
                <div className="font-medium text-sm">{t("settings.currency.label")}</div>
                <div className="w-[220px]">
                  <Select
                    value={currencyMode}
                    onValueChange={(v) => setCurrencyMode(v as CurrencyMode)}
                  >
                    <SelectTrigger>
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="auto">{t("settings.currency.options.auto")}</SelectItem>
                      <SelectItem value="CNY">{t("settings.currency.options.cny")}</SelectItem>
                      <SelectItem value="USD">{t("settings.currency.options.usd")}</SelectItem>
                    </SelectContent>
                  </Select>
                </div>
              </div>
            </CardContent>
          </Card>
        </TabsContent>

        {/* 渠道标签页 */}
        <TabsContent value="channel" className="mt-2 space-y-4">
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
                      toast.error(t("settings.channelProtection.saveFail"), { description: humanizeApiError(e, t) });
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
                      toast.error(t("settings.pricingData.syncFail"), { description: humanizeApiError(e, t) });
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
                      toast.error(t("settings.pricingData.saveFail"), { description: humanizeApiError(e, t) });
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
        <TabsContent value="application" className="mt-2 space-y-4">
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
                      toast.error(t("settings.windowClose.saveFail"), { description: humanizeApiError(e, t) });
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
                      toast.error(t("settings.startup.saveFail"), { description: humanizeApiError(e, t) });
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
                        toast.error(t("settings.startup.saveFail"), { description: humanizeApiError(e, t) });
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
        </TabsContent>

        {/* CLI 标签页 */}
        <TabsContent value="update" className="mt-2 space-y-4">
          {/* CLI 代理配置（Claude / Codex / Gemini -> CliSwitch） */}
          <Card>
            <CardHeader>
              <div className="flex items-start justify-between gap-3">
                <div className="space-y-1">
                  <CardTitle className="flex items-center gap-2">
                    <Power className="h-4 w-4" />
                    {t("settings.cliProxyConfig.title")}
                  </CardTitle>
                  <CardDescription>{t("settings.cliProxyConfig.subtitle")}</CardDescription>
                </div>
                <Button
                  size="sm"
                  variant="outline"
                  onClick={refreshCliToolsProxyConfigStatus}
                  disabled={cliToolsProxyConfigLoading}
                  className="gap-2"
                >
                  <RefreshCw className={`h-4 w-4 ${cliToolsProxyConfigLoading ? "animate-spin" : ""}`} />
                  {t("settings.cliProxyConfig.refresh")}
                </Button>
              </div>
            </CardHeader>
            <CardContent className="space-y-3">
              {!cliToolsProxyConfig ? (
                <div className="text-sm text-muted-foreground">
                  {cliToolsProxyConfigLoading ? t("common.loading") : "-"}
                </div>
              ) : null}

              {(cliToolsProxyConfig?.tools ?? []).map((tool: CliToolProxyConfigToolStatus) => {
                const busy = cliProxyConfigBusy[tool.id];

                return (
                  <div key={tool.id} className="border rounded-lg px-3 py-2 bg-background flex items-center justify-between gap-3">
                    <div className="min-w-0">
                      <div className="font-medium text-sm truncate">
                        {tool.name}{" "}
                        <span className={tool.ok ? "text-success" : "text-warning"}>
                          ({tool.ok ? t("settings.cliProxyConfig.ok") : t("settings.cliProxyConfig.needsFix")})
                        </span>
                      </div>
                    </div>

                    <Button
                      size="sm"
                      disabled={tool.ok || busy}
                      onClick={async () => {
                        setCliProxyConfigBusy((prev) => ({ ...prev, [tool.id]: true }));
                        try {
                          const res = await applyCliToolsProxyConfig({ tools: [tool.id] });
                          setCliToolsProxyConfig(res.status);
                          const applied = res.applied.find((x) => x.id === tool.id);
                          if (applied?.ok) {
                            toast.success(t("settings.cliProxyConfig.applied"));
                          } else {
                            toast.error(t("settings.cliProxyConfig.applyFail"), {
                              description: humanizeIssue(applied?.issue, t),
                            });
                          }
                        } catch (e) {
                          toast.error(t("settings.cliProxyConfig.applyFail"), { description: humanizeApiError(e, t) });
                        } finally {
                          setCliProxyConfigBusy((prev) => ({ ...prev, [tool.id]: false }));
                          void refreshCliToolsProxyConfigStatus();
                        }
                      }}
                    >
                      {t("settings.cliProxyConfig.apply")}
                    </Button>
                  </div>
                );
              })}
            </CardContent>
          </Card>

          {/* CLI 更新 */}
          <Card>
            <CardHeader>
              <div className="flex items-start justify-between gap-3">
                <div className="space-y-1">
                  <CardTitle className="flex items-center gap-2">
                    <RefreshCw className="h-4 w-4" />
                    {t("settings.cliTools.title")}
                  </CardTitle>
                  <CardDescription>{t("settings.cliTools.subtitle")}</CardDescription>
                </div>
                <Button
                  size="sm"
                  variant="outline"
                  onClick={refreshCliToolsStatus}
                  disabled={cliToolsLoading}
                  className="gap-2"
                >
                  <RefreshCw className={`h-4 w-4 ${cliToolsLoading ? "animate-spin" : ""}`} />
                  {t("settings.cliTools.refresh")}
                </Button>
              </div>
            </CardHeader>
            <CardContent className="space-y-4">
              {!cliToolsStatus ? (
                <div className="text-sm text-muted-foreground">
                  {cliToolsLoading ? t("common.loading") : "-"}
                </div>
              ) : null}

              {(cliToolsStatus?.tools ?? []).map((tool: CliToolStatus) => {
                const installed = tool.installed;
                const version = tool.version ?? "-";
                const busy = cliToolBusy[tool.id];
                const autoEnabled =
                  tool.id === "gemini"
                    ? (appSettings?.gemini_cli_auto_update_enabled ?? false)
                    : tool.id === "claude"
                      ? (appSettings?.claude_code_auto_update_enabled ?? false)
                      : (appSettings?.codex_auto_update_enabled ?? false);

                return (
                  <div key={tool.id} className="flex items-center justify-between gap-4">
                    <div className="min-w-0">
                      <div className="font-medium text-sm truncate">{tool.name}</div>
                      <div className="text-xs text-muted-foreground">
                        {installed
                          ? t("settings.cliTools.installedWithVersion", { version })
                          : t("settings.cliTools.notInstalled")}
                      </div>
                    </div>
                    <div className="flex items-center gap-3">
                      <Button
                        size="sm"
                        variant="outline"
                        disabled={busy}
                        onClick={async () => {
                          setCliToolBusy((prev) => ({ ...prev, [tool.id]: true }));
                          try {
                            await installCliToolWithToast({
                              tool,
                              t,
                              onToolUpdated: (nextTool) =>
                                setCliToolsStatus((prev) =>
                                  prev
                                    ? {
                                        ...prev,
                                        tools: prev.tools.map((t0) => (t0.id === nextTool.id ? nextTool : t0)),
                                      }
                                    : prev
                                ),
                            });
                          } finally {
                            setCliToolBusy((prev) => ({ ...prev, [tool.id]: false }));
                          }
                        }}
                      >
                        {installed ? t("settings.cliTools.update") : t("settings.cliTools.install")}
                      </Button>

                      <div className="flex items-center gap-2">
                        <div className="text-xs text-muted-foreground">
                          {t("settings.cliTools.autoEnable")}
                        </div>
                        <Switch
                          checked={autoEnabled}
                          onCheckedChange={async (v) => {
                            if (!appSettings) return;
                            const prev = autoEnabled;
                            const patch =
                              tool.id === "gemini"
                                ? { gemini_cli_auto_update_enabled: v }
                                : tool.id === "claude"
                                  ? { claude_code_auto_update_enabled: v }
                                  : { codex_auto_update_enabled: v };
                            setAppSettings({ ...appSettings, ...patch } as AppSettings);
                            try {
                              const next = await updateSettings(patch);
                              setAppSettings(next);
                              toast.success(t("settings.cliTools.saved"));
                            } catch (e) {
                              setAppSettings({ ...appSettings, ...(
                                tool.id === "gemini"
                                  ? { gemini_cli_auto_update_enabled: prev }
                                  : tool.id === "claude"
                                    ? { claude_code_auto_update_enabled: prev }
                                    : { codex_auto_update_enabled: prev }
                              ) } as AppSettings);
                              toast.error(t("settings.cliTools.saveFail"), { description: humanizeApiError(e, t) });
                            }
                          }}
                          disabled={!appSettings}
                        />
                      </div>
                    </div>
                  </div>
                );
              })}
            </CardContent>
          </Card>

        </TabsContent>

        {/* 数据标签页 */}
        <TabsContent value="data" className="mt-2 space-y-4">
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
                    onClick={async () => {
                      try {
                        await openDataDir();
                      } catch (e) {
                        toast.error(t("settings.storage.openFail"), { description: humanizeApiError(e, t) });
                      }
                    }}
                    disabled={!health?.data_dir || health.data_dir === "-"}
                  >
                    {t("common.open")}
                  </Button>
                </div>
                <p className="text-xs text-muted-foreground">
                  {t("settings.storage.dataDirHint")}
                </p>
              </div>
            </CardContent>
          </Card>

          {/* 数据库管理 */}
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <Database className="h-4 w-4" />
                {t("settings.maintenance.databaseTitle")}
              </CardTitle>
              <CardDescription>{t("settings.maintenance.databaseSubtitle")}</CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="space-y-2">
                <label className="text-sm font-medium">{t("settings.storage.dbFile")}</label>
                <Input value={health?.db_path ?? "-"} disabled className="font-mono text-sm" />
              </div>

              <div className="space-y-2">
                <label className="text-sm font-medium">{t("settings.storage.dbSize")}</label>
                <div className="flex gap-2">
                  <Input value={dbSize ? formatBytes(dbSize.total_bytes) : "-"} disabled className="font-mono text-sm" />
                  <Button variant="outline" onClick={() => void refreshDbSize()} disabled={dbSizeLoading}>
                    {t("common.refresh")}
                  </Button>
                </div>
              </div>

              <Dialog
                open={recordsPromptOpen}
                onOpenChange={(v) => {
                  if (recordsClearing) return;
                  setRecordsPromptOpen(v);
                }}
              >
                <DialogContent className="sm:max-w-[520px]">
                  <DialogHeader>
                    <DialogTitle>{t("settings.records.confirmTitle")}</DialogTitle>
                    <DialogDescription>
                      {t(
                        recordsTimeScope === "date_range"
                          ? recordsType === "errors"
                            ? "settings.records.confirmDateRangeErrors"
                            : "settings.records.confirmDateRange"
                          : recordsType === "errors"
                            ? "settings.records.confirmErrors"
                            : "settings.records.confirmAll",
                        { range: recordsDateStr }
                      )}
                    </DialogDescription>
                  </DialogHeader>
                  <DialogFooter>
                    <Button variant="outline" onClick={() => setRecordsPromptOpen(false)} disabled={recordsClearing}>
                      {t("common.cancel")}
                    </Button>
                    <Button
                      variant="destructive"
                      onClick={async () => {
                        setRecordsClearing(true);
                        try {
                          const msRange = recordsTimeScope === "date_range" ? dateRangeToMs(recordsDateRange) : null;
                          if (recordsTimeScope === "date_range" && !msRange) {
                            toast.error(t("settings.records.invalidDate"));
                            return;
                          }

                          const res = await clearRecords({
                            mode: recordsType,
                            start_ms: msRange?.start_ms,
                            end_ms: msRange?.end_ms,
                          });
                          toast.success(t("settings.records.cleared"), {
                            description: t(
                              recordsType === "errors"
                                ? "settings.records.clearedDetailErrors"
                                : "settings.records.clearedDetail",
                              {
                                count: res.usage_events_deleted.toLocaleString(),
                              }
                            ),
                          });
                          setRecordsPromptOpen(false);
                          setRecordsDateRange(undefined);
                          await refreshDbSize();
                        } catch (e) {
                          toast.error(t("settings.records.clearFail"), { description: humanizeApiError(e, t) });
                        } finally {
                          setRecordsClearing(false);
                        }
                      }}
                      disabled={recordsClearing}
                    >
                      {recordsClearing ? t("settings.records.clearing") : t("settings.records.clear")}
                    </Button>
                  </DialogFooter>
                </DialogContent>
              </Dialog>

              <div className="flex items-center justify-between gap-4">
                <div className="flex-1 min-w-0">
                  <div className="font-medium text-sm">{t("settings.maintenance.clearRecords")}</div>
                  <div className="text-xs text-muted-foreground">{t("settings.maintenance.clearHint")}</div>
                </div>
                <div className="flex items-center gap-2 flex-shrink-0">
                  <Select value={recordsType} onValueChange={(v) => setRecordsType(v as typeof recordsType)} disabled={recordsClearing}>
                    <SelectTrigger className="w-[140px]">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="all">{t("settings.maintenance.typeAll")}</SelectItem>
                      <SelectItem value="errors">{t("settings.maintenance.typeErrors")}</SelectItem>
                    </SelectContent>
                  </Select>
                  <Select value={recordsTimeScope} onValueChange={(v) => setRecordsTimeScope(v as typeof recordsTimeScope)} disabled={recordsClearing}>
                    <SelectTrigger className="w-[120px]">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="all">{t("settings.maintenance.timeAll")}</SelectItem>
                      <SelectItem value="date_range">{t("settings.maintenance.timeRange")}</SelectItem>
                    </SelectContent>
                  </Select>
                  {recordsTimeScope === "date_range" && (
                    <DateRangePicker
                      value={recordsDateRange}
                      onChange={setRecordsDateRange}
                      placeholder={t("settings.records.selectRange")}
                      className="w-[260px]"
                      disabled={recordsClearing}
                      locale={locale}
                    />
                  )}
                  <Button
                    variant="destructive"
                    size="sm"
                    onClick={() => {
                      if (recordsTimeScope === "date_range" && !recordsDateRange?.from) {
                        toast.error(t("settings.records.invalidDate"));
                        return;
                      }
                      setRecordsPromptOpen(true);
                    }}
                    disabled={recordsClearing || (recordsTimeScope === "date_range" && !recordsDateRange?.from)}
                  >
                    {t("settings.records.clear")}
                  </Button>
                </div>
              </div>
            </CardContent>
          </Card>

          {/* 系统日志 */}
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
                      toast.error(t("settings.logging.saveFail"), { description: humanizeApiError(e, t) });
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
                        toast.error(t("settings.logging.saveFail"), { description: humanizeApiError(e, t) });
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

              <div className="space-y-2">
                <label className="text-sm font-medium">{t("settings.maintenance.logsSize")}</label>
                <div className="flex gap-2">
                  <Input value={logsSize ? formatBytes(logsSize.total_bytes) : "-"} disabled className="font-mono text-sm" />
                  <Button variant="outline" onClick={() => void refreshLogsSize()} disabled={logsSizeLoading}>
                    {t("common.refresh")}
                  </Button>
                </div>
              </div>

              <Dialog
                open={logsPromptOpen}
                onOpenChange={(v) => {
                  if (logsClearing) return;
                  setLogsPromptOpen(v);
                }}
              >
                <DialogContent className="sm:max-w-[520px]">
                  <DialogHeader>
                    <DialogTitle>{t("settings.logging.confirmTitle")}</DialogTitle>
                    <DialogDescription>
                      {t(logsScope === "date_range" ? "settings.logging.confirmDateRange" : "settings.logging.confirmAll", {
                        range: logsDateStr,
                      })}
                    </DialogDescription>
                  </DialogHeader>
                  <DialogFooter>
                    <Button variant="outline" onClick={() => setLogsPromptOpen(false)} disabled={logsClearing}>
                      {t("common.cancel")}
                    </Button>
                    <Button
                      variant="destructive"
                      onClick={async () => {
                        setLogsClearing(true);
                        try {
                          if (logsScope === "date_range") {
                            const r = dateRangeToStrings(logsDateRange);
                            if (!r) {
                              toast.error(t("settings.logging.invalidDate"));
                              return;
                            }
                            const res = await clearLogs({ mode: "date_range", start_date: r.start, end_date: r.end });
                            toast.success(t("settings.logging.cleared"), {
                              description: t("settings.logging.clearedDetail", {
                                deleted: res.deleted_files,
                                truncated: res.truncated_files,
                              }),
                            });
                          } else {
                            const res = await clearLogs({ mode: "all" });
                            toast.success(t("settings.logging.cleared"), {
                              description: t("settings.logging.clearedDetail", {
                                deleted: res.deleted_files,
                                truncated: res.truncated_files,
                              }),
                            });
                          }

                          setLogsPromptOpen(false);
                          setLogsDateRange(undefined);
                          await refreshLogsSize();
                        } catch (e) {
                          toast.error(t("settings.logging.clearFail"), { description: humanizeApiError(e, t) });
                        } finally {
                          setLogsClearing(false);
                        }
                      }}
                      disabled={logsClearing}
                    >
                      {logsClearing ? t("settings.logging.clearing") : t("settings.logging.clear")}
                    </Button>
                  </DialogFooter>
                </DialogContent>
              </Dialog>

              <div className="flex items-center justify-between gap-4">
                <div className="flex-1 min-w-0">
                  <div className="font-medium text-sm">{t("settings.logging.clearLogs")}</div>
                  <div className="text-xs text-muted-foreground">{t("settings.maintenance.clearHint")}</div>
                </div>
                <div className="flex items-center gap-2 flex-shrink-0">
                  <Select value={logsScope} onValueChange={(v) => setLogsScope(v as any)} disabled={logsClearing}>
                    <SelectTrigger className="w-[140px]">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="all">{t("settings.maintenance.timeAll")}</SelectItem>
                      <SelectItem value="date_range">{t("settings.maintenance.timeRange")}</SelectItem>
                    </SelectContent>
                  </Select>
                  {logsScope === "date_range" && (
                    <DateRangePicker
                      value={logsDateRange}
                      onChange={setLogsDateRange}
                      placeholder={t("settings.logging.selectRange")}
                      className="w-[260px]"
                      disabled={logsClearing}
                      locale={locale}
                    />
                  )}
                  <Button
                    variant="destructive"
                    size="sm"
                    onClick={() => {
                      if (logsScope === "date_range" && !logsDateRange?.from) {
                        toast.error(t("settings.logging.invalidDate"));
                        return;
                      }
                      setLogsPromptOpen(true);
                    }}
                    disabled={logsClearing || (logsScope === "date_range" && !logsDateRange?.from)}
                  >
                    {t("settings.logging.clear")}
                  </Button>
                </div>
              </div>
            </CardContent>
          </Card>
        </TabsContent>

        <TabsContent value="chatBridge" className="mt-2 space-y-4">
          {chatBridgeTab}
        </TabsContent>

        {/* 系统标签页 */}
        <TabsContent value="system" className="mt-2 space-y-4">
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

              <div className="flex items-center justify-between gap-4">
                <div>
                  <div className="font-medium text-sm">{t("settings.serviceInfo.lanAccessible")}</div>
                  <div className="text-xs text-muted-foreground">
                    {t("settings.serviceInfo.lanAccessibleHint")}
                  </div>
                </div>
                <Switch
                  checked={appSettings?.server_lan_accessible ?? false}
                  onCheckedChange={async (v) => {
                    if (!appSettings) return;
                    const prev = appSettings.server_lan_accessible;
                    setAppSettings({ ...appSettings, server_lan_accessible: v });
                    setServerLanSaving(true);
                    try {
                      const next = await updateSettings({ server_lan_accessible: v });
                      setAppSettings(next);
                      toast.success(t("settings.serviceInfo.saved"));
                      // Restart backend in-place so the setting takes effect without restarting the window/UI.
                      if (prev !== v) {
                        postIpc({ type: "request-restart-backend" });
                      }
                    } catch (e) {
                      setAppSettings({ ...appSettings, server_lan_accessible: prev });
                      toast.error(t("settings.serviceInfo.saveFail"), { description: humanizeApiError(e, t) });
                    } finally {
                      setServerLanSaving(false);
                    }
                  }}
                  disabled={!appSettings || serverLanSaving}
                />
              </div>
            </CardContent>
          </Card>

          {/* 兼容性 */}
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <Info className="h-4 w-4" />
                {t("settings.compatibility.title")}
              </CardTitle>
              <CardDescription>{t("settings.compatibility.subtitle")}</CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="flex items-center justify-between gap-4">
                <div>
                  <div className="font-medium text-sm">{t("settings.compatibility.mockCountTokens")}</div>
                  <div className="text-xs text-muted-foreground">
                    {t("settings.compatibility.mockCountTokensHint")}
                  </div>
                </div>
                <Switch
                  checked={appSettings?.anthropic_count_tokens_mock_enabled ?? false}
                  onCheckedChange={async (v) => {
                    if (!appSettings) return;
                    const prev = appSettings.anthropic_count_tokens_mock_enabled;
                    setAppSettings({ ...appSettings, anthropic_count_tokens_mock_enabled: v });
                    setCountTokensMockSaving(true);
                    try {
                      const next = await updateSettings({ anthropic_count_tokens_mock_enabled: v });
                      setAppSettings(next);
                      toast.success(t("settings.compatibility.saved"));
                    } catch (e) {
                      setAppSettings({ ...appSettings, anthropic_count_tokens_mock_enabled: prev });
                      toast.error(t("settings.compatibility.saveFail"), { description: humanizeApiError(e, t) });
                    } finally {
                      setCountTokensMockSaving(false);
                    }
                  }}
                  disabled={!appSettings || countTokensMockSaving}
                />
              </div>
            </CardContent>
          </Card>

          {/* CliSwitch 更新 */}
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <RefreshCw className="h-4 w-4" />
                {t("settings.update.title")}
              </CardTitle>
              <CardDescription>{t("settings.update.subtitle")}</CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <UpdatePromptDialog
                open={updatePromptOpen}
                onOpenChange={setUpdatePromptOpen}
                title={t("settings.update.promptTitle")}
                description={t("settings.update.promptDesc", { version: updatePromptVersion ?? "-" })}
                overviewTitle={t("settings.update.promptOverviewTitle")}
                loadingText={t("settings.update.promptLoading")}
                loadFailText={t("settings.update.promptLoadFail")}
                sections={updateChangelogSections}
                loading={updateChangelogLoading}
                loadError={updateChangelogError}
                updateText={t("settings.update.updateNow")}
                laterText={t("settings.update.later")}
                ignoreText={t("settings.update.ignore")}
                busy={updateDownloading || updateIgnoring}
                onLater={() => setUpdatePromptOpen(false)}
                onUpdate={async () => {
                  setUpdateDownloading(true);
                  try {
                    const dl = await downloadUpdate();
                    setUpdateStatus(dl.status);
                    toast.success(t("settings.update.downloading"));
                    setUpdatePromptOpen(false);
                  } catch (e) {
                    toast.error(t("settings.update.downloadFail"), { description: humanizeApiError(e, t) });
                  } finally {
                    setUpdateDownloading(false);
                  }
                }}
                onIgnore={async () => {
                  const v = updatePromptVersion;
                  if (!v) return;
                  setUpdateIgnoring(true);
                  try {
                    const st = await ignoreUpdate(v);
                    setUpdateStatus(st);
                    toast.success(t("settings.update.ignoredToast", { version: v }));
                    setUpdatePromptOpen(false);
                  } catch (e) {
                    toast.error(t("settings.update.ignoreFail"), { description: humanizeApiError(e, t) });
                  } finally {
                    setUpdateIgnoring(false);
                  }
                }}
              />

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
                        const res = await checkUpdate();
                        setUpdateCheckResult(res);
                        if (res.update_available && res.latest_version) {
                          if (res.latest_ignored) {
                            toast.info(t("settings.update.ignoredToast", { version: res.latest_version }));
                          } else {
                            openUpdatePrompt(res.latest_version);
                          }
                        }
                        const st = await getUpdateStatus().catch(() => null);
                        if (st) setUpdateStatus(st);
                      }
                    } catch (e) {
                      setAppSettings({ ...appSettings, app_auto_update_enabled: prev });
                      toast.error(t("settings.update.saveFail"), { description: humanizeApiError(e, t) });
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

                        if (st?.pending_version) {
                          const pending = st.pending_version;
                          const latest = res.latest_version;
                          if (res.update_available && latest && latest !== pending) {
                            if (res.latest_ignored) {
                              toast.info(t("settings.update.ignoredToast", { version: latest }));
                              return;
                            }
                            toast.success(t("settings.update.found", { version: latest }));
                            openUpdatePrompt(latest);
                            return;
                          }
                          reopenUpdateReadyPrompt(st);
                          return;
                        }

                        if (!res.update_available) {
                          toast.success(t("settings.update.uptodate"));
                          return;
                        }

                        const latest = res.latest_version ?? "-";
                        if (res.latest_ignored) {
                          toast.info(t("settings.update.ignoredToast", { version: latest }));
                          return;
                        }
                        toast.success(t("settings.update.found", { version: latest }));
                        if (res.latest_version) openUpdatePrompt(res.latest_version);
                      } catch (e) {
                        toast.error(t("settings.update.checkFail"), { description: humanizeApiError(e, t) });
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
