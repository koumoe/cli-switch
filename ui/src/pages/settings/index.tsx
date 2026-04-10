import React, { useEffect, useState } from "react";
import { useNavigate, useParams } from "@tanstack/react-router";
import { Sun, Moon, Monitor, FolderOpen, Info, Database, Languages, DollarSign, RefreshCw, Power, Palette, Settings2, Cpu, Trash2, Shield, Bell, Bot } from "lucide-react";
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
import { DateRangePicker } from "@/components/composed/date-range-picker";
import { PageHeader } from "@/components/PageHeader";
import { useCurrency } from "@/hooks/use-currency";
import { useI18n } from "@/hooks/use-i18n";
import { useTheme, type Theme } from "@/hooks/use-theme";
import { dateRangeToMs, dateRangeToStrings } from "@/lib/date-utils";
import { humanizeApiError, humanizeIssue } from "@/lib/error";
import { installCliToolWithToast } from "@/lib/cliToolInstaller";
import type { CurrencyMode } from "@/providers/currency-provider";
import type { Locale } from "@/types/locale";
import { setLogLevel } from "@/lib/logger";
import { UpdatePromptDialog } from "@/components/UpdatePromptDialog";
import {
  AppUpdateSettingsCard,
  ChatBridgeBaseSettingsCard,
  ChatBridgeDiscordSettingsCard,
  ChatBridgeTelegramSettingsCard,
  ChatBridgeWeixinSettingsCard,
  ChatBridgeWhatsAppSettingsCard,
  ChannelProtectionSettingsCard,
  ChannelRetrySettingsCard,
  CompatibilitySettingsCard,
  LoggingSettingsCard,
  NewApiManagedSettingsCard,
  PricingDataSettingsCard,
  RemoteSystemNotificationsSettingsCard,
  ServiceInfoSettingsCard,
  StartupSettingsCard,
  SystemNotificationsSettingsCard,
  WindowCloseSettingsCard,
} from "@/pages/settings/form-sections";
import { formatBytes, formatDateTime, formatNumber } from "../../lib";
import {
  applyCliToolsProxyConfig,
  checkUpdate,
  clearLogs,
  clearRecords,
  createChatBridgePairingToken,
  deactivateChatBridgeBinding,
  downloadUpdate,
  getChatBridgeWeixinStatus,
  getChatBridgeWhatsAppStatus,
  getCliToolsProxyConfigStatus,
  getCliToolsStatus,
  getDbSize,
  getHealth,
  getLogsSize,
  getSettings,
  getUpdateChangelog,
  getUpdateStatus,
  ignoreUpdate,
  listChatBridgeBindings,
  logoutChatBridgeWeixin,
  logoutChatBridgeWhatsApp,
  openDataDir,
  pricingStatus,
  pricingSync,
  startChatBridgeWeixinLogin,
  startChatBridgeWhatsAppLogin,
  updateSettings,
} from "@/api";
import type {
  AppSettings,
  ChangelogSection,
  ChatBridgeBinding,
  ChatBridgePairingToken,
  ChatBridgeWhatsAppStatus,
  ChatBridgeWeixinStatus,
  ChatPlatform,
  CliToolId,
  CliToolProxyConfigStatus,
  CliToolProxyConfigToolStatus,
  CliToolStatus,
  CliToolsStatus,
  DbSize,
  Health,
  LogsSize,
  PricingStatus,
  RecordsClearMode,
  UpdateCheck,
  UpdateStatus,
} from "@/types/api";
import type { CliswitchUpdateStatusEvent } from "@/lib/cliswitchEvents";
import { clearUpdateReadyShown } from "@/lib/updateReadyPrompt";
import { AppearanceSettings } from "./appearance-settings";
import { ApplicationSettings } from "./application-settings";
import { ChannelSettings } from "./channel-settings";
import { CliToolsSettings } from "./cli-tools-settings";
import { DataSettings } from "./data-settings";
import { NotificationSettings } from "./notification-settings";
import { SystemSettings } from "./system-settings";

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

type ChatBridgeQrPlatform = "whatsapp" | "weixin";
const SETTINGS_TABS = [
  "appearance",
  "channel",
  "notifications",
  "application",
  "update",
  "data",
  "chatBridge",
  "system",
] as const;
type SettingsTab = (typeof SETTINGS_TABS)[number];
const DEFAULT_SETTINGS_TAB: SettingsTab = "appearance";

function isSettingsTab(value: string | undefined): value is SettingsTab {
  return SETTINGS_TABS.includes(value as SettingsTab);
}

export function SettingsPage() {
  const navigate = useNavigate();
  const routeTab = useParams({
    strict: false,
    select: (params) => (typeof params.tab === "string" ? params.tab : undefined),
  });
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
  const [syncing, setSyncing] = useState(false);
  const [chatBridgeSaving, setChatBridgeSaving] = useState(false);
  const [chatBridgeBindings, setChatBridgeBindings] = useState<ChatBridgeBinding[]>([]);
  const [chatBridgeBindingsLoading, setChatBridgeBindingsLoading] = useState(false);
  const [chatBridgePairingToken, setChatBridgePairingToken] = useState<ChatBridgePairingToken | null>(null);
  const [chatBridgePairingCreating, setChatBridgePairingCreating] = useState(false);
  const [chatBridgePairingPlatform, setChatBridgePairingPlatform] = useState<ChatPlatform>("weixin");
  const [chatBridgeUnbindTarget, setChatBridgeUnbindTarget] = useState<ChatBridgeBinding | null>(null);
  const [chatBridgeUnbinding, setChatBridgeUnbinding] = useState(false);
  const [chatBridgePlatformTab, setChatBridgePlatformTab] = useState<ChatPlatform>("weixin");
  const [chatBridgePairingDialogOpen, setChatBridgePairingDialogOpen] = useState(false);
  const [chatBridgeBindingsDialogPlatform, setChatBridgeBindingsDialogPlatform] = useState<ChatPlatform | null>(null);
  const [chatBridgeLoginDialogPlatform, setChatBridgeLoginDialogPlatform] = useState<ChatBridgeQrPlatform | null>(null);
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

  function openChatBridgeLoginDialog(platform: ChatBridgeQrPlatform) {
    setChatBridgeLoginDialogPlatform(platform);
    if (platform === "whatsapp") {
      void refreshChatBridgeWhatsAppStatus({ silent: true });
      return;
    }
    void refreshChatBridgeWeixinStatus({ silent: true });
  }

  useEffect(() => {
    getHealth()
      .then(setHealth)
      .catch(() => setHealth({ status: "offline" }));

    pricingStatus()
      .then(setPricing)
      .catch(() => setPricing(null));

    getSettings()
      .then((s) => {
        setAppSettings(s);
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
      : health?.status === "offline"
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
  const activeSettingsTab = isSettingsTab(routeTab) ? routeTab : DEFAULT_SETTINGS_TAB;

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
    if (routeTab === undefined || isSettingsTab(routeTab)) return;
    void navigate({
      to: "/settings/{-$tab}",
      params: { tab: undefined },
      replace: true,
    });
  }, [navigate, routeTab]);

  useEffect(() => {
    const whatsappEnabled =
      (appSettings?.chat_bridge_enabled ?? false) &&
      (appSettings?.chat_bridge_whatsapp_enabled ?? false);
    const whatsappPollingActive =
      chatBridgePlatformTab === "whatsapp" || chatBridgeLoginDialogPlatform === "whatsapp";
    if (!whatsappPollingActive || !whatsappEnabled) {
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
    chatBridgeLoginDialogPlatform,
    chatBridgePlatformTab,
  ]);

  useEffect(() => {
    const weixinEnabled =
      (appSettings?.chat_bridge_enabled ?? false) &&
      (appSettings?.chat_bridge_weixin_enabled ?? false);
    const weixinPollingActive =
      chatBridgePlatformTab === "weixin" || chatBridgeLoginDialogPlatform === "weixin";
    if (!weixinPollingActive || !weixinEnabled) {
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
    chatBridgeLoginDialogPlatform,
    chatBridgePlatformTab,
  ]);

  const whatsappStatusKey = chatBridgeWhatsAppStatus?.state ?? "disabled";
  const whatsappConnected = chatBridgeWhatsAppStatus?.connected ?? false;
  const whatsappQrImageSrc = normalizeQrImageSrc(chatBridgeWhatsAppStatus?.qr_image);
  const whatsappStatusLabel = t(`settings.chatBridge.whatsapp.state.${whatsappStatusKey}`);
  const whatsappLastErrorMessage = humanizeIssue(chatBridgeWhatsAppStatus?.issue, t)
    ?? (whatsappStatusKey === "error" ? t("settings.chatBridge.whatsapp.lastErrorGeneric") : null);
  const whatsappStatusTone = whatsappStatusKey === "error"
    ? "destructive"
    : whatsappConnected
      ? "success"
      : "secondary";
  const weixinStatusKey = chatBridgeWeixinStatus?.state ?? "disabled";
  const weixinConnected = chatBridgeWeixinStatus?.connected ?? false;
  const weixinQrImageSrc = normalizeQrImageSrc(chatBridgeWeixinStatus?.qr_image);
  const weixinStatusLabel = t(`settings.chatBridge.weixin.state.${weixinStatusKey}`);
  const weixinLastErrorMessage = humanizeIssue(chatBridgeWeixinStatus?.issue, t)
    ?? (weixinStatusKey === "error" ? t("settings.chatBridge.weixin.lastErrorGeneric") : null);
  const weixinStatusTone = weixinStatusKey === "error"
    ? "destructive"
    : weixinConnected
      ? "success"
      : "secondary";
  const chatBridgeLoginDialogStatus = chatBridgeLoginDialogPlatform === "whatsapp"
    ? chatBridgeWhatsAppStatus
    : chatBridgeLoginDialogPlatform === "weixin"
      ? chatBridgeWeixinStatus
      : null;
  const chatBridgeLoginDialogStatusLoading = chatBridgeLoginDialogPlatform === "whatsapp"
    ? chatBridgeWhatsAppStatusLoading
    : chatBridgeLoginDialogPlatform === "weixin"
      ? chatBridgeWeixinStatusLoading
      : false;
  const chatBridgeLoginDialogActionBusy = chatBridgeLoginDialogPlatform === "whatsapp"
    ? chatBridgeWhatsAppActionBusy
    : chatBridgeLoginDialogPlatform === "weixin"
      ? chatBridgeWeixinActionBusy
      : false;
  const chatBridgeLoginDialogConnected = chatBridgeLoginDialogStatus?.connected ?? false;
  const chatBridgeLoginDialogStatusTone = chatBridgeLoginDialogPlatform === "whatsapp"
    ? whatsappStatusTone
    : chatBridgeLoginDialogPlatform === "weixin"
      ? weixinStatusTone
      : "secondary";
  const chatBridgeLoginDialogStatusLabel = chatBridgeLoginDialogPlatform
    ? t(`settings.chatBridge.${chatBridgeLoginDialogPlatform}.state.${chatBridgeLoginDialogStatus?.state ?? "disabled"}`)
    : "";
  const chatBridgeLoginDialogQrImageSrc = chatBridgeLoginDialogPlatform === "whatsapp"
    ? whatsappQrImageSrc
    : chatBridgeLoginDialogPlatform === "weixin"
      ? weixinQrImageSrc
      : null;
  const chatBridgeLoginDialogLastErrorMessage = chatBridgeLoginDialogPlatform === "whatsapp"
    ? whatsappLastErrorMessage
    : chatBridgeLoginDialogPlatform === "weixin"
      ? weixinLastErrorMessage
      : null;

  const chatBridgeTab = (
    <>
      <ChatBridgeBaseSettingsCard settings={appSettings} onSaved={setAppSettings} />

      <Card>
        <CardHeader>
          <CardTitle>{t("settings.chatBridge.platformConfigTitle")}</CardTitle>
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
              <ChatBridgeTelegramSettingsCard
                settings={appSettings}
                bindingCount={chatBridgeBindingsByPlatform.telegram.length}
                tokenConfigured={appSettings?.chat_bridge_telegram_bot_token_configured ?? false}
                onOpenPairing={() => openChatBridgePairing("telegram")}
                onOpenBindings={() => openChatBridgeBindings("telegram")}
                onSaved={setAppSettings}
              />
            </TabsContent>

            <TabsContent value="discord" className="mt-4">
              <ChatBridgeDiscordSettingsCard
                settings={appSettings}
                bindingCount={chatBridgeBindingsByPlatform.discord.length}
                tokenConfigured={appSettings?.chat_bridge_discord_bot_token_configured ?? false}
                onOpenPairing={() => openChatBridgePairing("discord")}
                onOpenBindings={() => openChatBridgeBindings("discord")}
                onSaved={setAppSettings}
              />
            </TabsContent>

            <TabsContent value="whatsapp" className="mt-4">
              <ChatBridgeWhatsAppSettingsCard
                settings={appSettings}
                bindingCount={chatBridgeBindingsByPlatform.whatsapp.length}
                statusTone={whatsappStatusTone}
                statusLabel={whatsappStatusLabel}
                actionBusy={chatBridgeWhatsAppActionBusy}
                onOpenLoginDialog={() => openChatBridgeLoginDialog("whatsapp")}
                onOpenPairing={() => openChatBridgePairing("whatsapp")}
                onOpenBindings={() => openChatBridgeBindings("whatsapp")}
                onSaved={setAppSettings}
              />
            </TabsContent>

            <TabsContent value="weixin" className="mt-4">
              <ChatBridgeWeixinSettingsCard
                settings={appSettings}
                bindingCount={chatBridgeBindingsByPlatform.weixin.length}
                statusTone={weixinStatusTone}
                statusLabel={weixinStatusLabel}
                actionBusy={chatBridgeWeixinActionBusy}
                onOpenLoginDialog={() => openChatBridgeLoginDialog("weixin")}
                onOpenPairing={() => openChatBridgePairing("weixin")}
                onOpenBindings={() => openChatBridgeBindings("weixin")}
                onSaved={setAppSettings}
              />
            </TabsContent>
          </Tabs>
        </CardContent>
      </Card>

      <Dialog
        open={!!chatBridgeLoginDialogPlatform}
        onOpenChange={(open) => {
          if (!open) {
            setChatBridgeLoginDialogPlatform(null);
          }
        }}
      >
        <DialogContent className="sm:max-w-[560px]">
          <DialogHeader>
            <DialogTitle>
              {chatBridgeLoginDialogPlatform
                ? t(`settings.chatBridge.${chatBridgeLoginDialogPlatform}.qrTitle`)
                : ""}
            </DialogTitle>
            <DialogDescription>
              {chatBridgeLoginDialogPlatform
                ? t(`settings.chatBridge.${chatBridgeLoginDialogPlatform}.dialogHint`)
                : ""}
            </DialogDescription>
          </DialogHeader>

          {chatBridgeLoginDialogPlatform ? (
            <div className="space-y-4">
              <div className="rounded-lg border bg-muted/20 p-4 space-y-3">
                <div className="flex items-start justify-between gap-3">
                  <div className="space-y-1">
                    <div className="font-medium text-sm">
                      {t(`settings.chatBridge.${chatBridgeLoginDialogPlatform}.runtimeTitle`)}
                    </div>
                    <div className="text-xs text-muted-foreground">
                      {t(`settings.chatBridge.${chatBridgeLoginDialogPlatform}.runtimeHint`)}
                    </div>
                  </div>
                  <Badge variant={chatBridgeLoginDialogStatusTone}>{chatBridgeLoginDialogStatusLabel}</Badge>
                </div>

                <div className="grid gap-3 md:grid-cols-2">
                  <div className="rounded-lg border bg-background px-3 py-3 space-y-1">
                    <div className="text-xs text-muted-foreground">
                      {t(`settings.chatBridge.${chatBridgeLoginDialogPlatform}.connectionLabel`)}
                    </div>
                    <div className="text-sm font-medium">
                      {chatBridgeLoginDialogConnected
                        ? t(`settings.chatBridge.${chatBridgeLoginDialogPlatform}.connectionConnected`)
                        : t(`settings.chatBridge.${chatBridgeLoginDialogPlatform}.connectionDisconnected`)}
                    </div>
                  </div>
                  <div className="rounded-lg border bg-background px-3 py-3 space-y-1">
                    <div className="text-xs text-muted-foreground">
                      {t(`settings.chatBridge.${chatBridgeLoginDialogPlatform}.accountLabel`)}
                    </div>
                    <div className="text-sm font-medium break-all">
                      {chatBridgeLoginDialogStatus?.me || t(`settings.chatBridge.${chatBridgeLoginDialogPlatform}.accountEmpty`)}
                    </div>
                  </div>
                </div>

                {chatBridgeLoginDialogLastErrorMessage ? (
                  <div className="rounded-lg border border-destructive/30 bg-destructive/5 px-3 py-3 space-y-1">
                    <div className="text-xs text-muted-foreground">
                      {t(`settings.chatBridge.${chatBridgeLoginDialogPlatform}.lastErrorLabel`)}
                    </div>
                    <div className="text-sm break-words">{chatBridgeLoginDialogLastErrorMessage}</div>
                  </div>
                ) : null}

                <div className="rounded-lg border bg-background px-3 py-3 space-y-3">
                  <div className="space-y-1">
                    <div className="font-medium text-sm">
                      {t(`settings.chatBridge.${chatBridgeLoginDialogPlatform}.qrTitle`)}
                    </div>
                    <div className="text-xs text-muted-foreground">
                      {t(`settings.chatBridge.${chatBridgeLoginDialogPlatform}.qrHint`)}
                    </div>
                  </div>
                  {chatBridgeLoginDialogQrImageSrc ? (
                    <div className="flex justify-center">
                      <img
                        src={chatBridgeLoginDialogQrImageSrc}
                        alt={t(`settings.chatBridge.${chatBridgeLoginDialogPlatform}.qrAlt`)}
                        className="h-56 w-56 rounded-lg border bg-white p-3"
                      />
                    </div>
                  ) : (
                    <div className="rounded-lg border border-dashed px-3 py-6 text-center text-sm text-muted-foreground">
                      {chatBridgeLoginDialogStatusLoading
                        ? t("common.loading")
                        : t(`settings.chatBridge.${chatBridgeLoginDialogPlatform}.qrEmpty`)}
                    </div>
                  )}
                </div>
              </div>
            </div>
          ) : null}

          <DialogFooter className="gap-2 sm:justify-between">
            <Button
              variant="outline"
              onClick={() => setChatBridgeLoginDialogPlatform(null)}
            >
              {t("common.cancel")}
            </Button>
            <div className="flex flex-wrap gap-2">
              <Button
                size="sm"
                onClick={() => {
                  if (chatBridgeLoginDialogPlatform === "whatsapp") {
                    void beginWhatsAppLogin();
                  } else if (chatBridgeLoginDialogPlatform === "weixin") {
                    void beginWeixinLogin();
                  }
                }}
                disabled={!chatBridgeLoginDialogPlatform || !appSettings || chatBridgeSaving || chatBridgeLoginDialogActionBusy}
              >
                {chatBridgeLoginDialogPlatform
                  ? t(`settings.chatBridge.${chatBridgeLoginDialogPlatform}.loginAction`)
                  : ""}
              </Button>
              <Button
                size="sm"
                variant="outline"
                onClick={() => {
                  if (chatBridgeLoginDialogPlatform === "whatsapp") {
                    void disconnectWhatsApp();
                  } else if (chatBridgeLoginDialogPlatform === "weixin") {
                    void disconnectWeixin();
                  }
                }}
                disabled={!chatBridgeLoginDialogPlatform || !appSettings || chatBridgeSaving || chatBridgeLoginDialogActionBusy}
              >
                {chatBridgeLoginDialogPlatform
                  ? t(`settings.chatBridge.${chatBridgeLoginDialogPlatform}.logoutAction`)
                  : ""}
              </Button>
              <Button
                size="sm"
                variant="outline"
                onClick={() => {
                  if (chatBridgeLoginDialogPlatform === "whatsapp") {
                    void refreshChatBridgeWhatsAppStatus();
                  } else if (chatBridgeLoginDialogPlatform === "weixin") {
                    void refreshChatBridgeWeixinStatus();
                  }
                }}
                disabled={!chatBridgeLoginDialogPlatform || chatBridgeLoginDialogStatusLoading || chatBridgeLoginDialogActionBusy}
              >
                {t("common.refresh")}
              </Button>
            </div>
          </DialogFooter>
        </DialogContent>
      </Dialog>

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
      <Tabs
        value={activeSettingsTab}
        onValueChange={(nextValue) => {
          if (!isSettingsTab(nextValue)) return;
          void navigate({
            to: "/settings/{-$tab}",
            params: {
              tab: nextValue === DEFAULT_SETTINGS_TAB ? undefined : nextValue,
            },
            replace: true,
          });
        }}
        className="w-full"
      >
        <TabsList className="w-full justify-start">
          <TabsTrigger value="appearance" className="gap-1.5">
            <Palette className="h-3.5 w-3.5" />
            {t("settings.tabs.appearance")}
          </TabsTrigger>
          <TabsTrigger value="channel" className="gap-1.5">
            <Shield className="h-3.5 w-3.5" />
            {t("settings.tabs.channel")}
          </TabsTrigger>
          <TabsTrigger value="notifications" className="gap-1.5">
            <Bell className="h-3.5 w-3.5" />
            {t("settings.tabs.notifications")}
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
          <AppearanceSettings
            theme={theme}
            themeOptions={themeOptions}
            onThemeChange={setTheme}
            locale={locale}
            locales={locales}
            onLocaleChange={async (value) => {
              try {
                const next = await updateSettings({ ui_locale: value });
                setAppSettings(next);
                setLocale(next.ui_locale);
              } catch (e) {
                toast.error(t("settings.language.saveFail"), {
                  description: humanizeApiError(e, t),
                });
              }
            }}
            currencyMode={currencyMode}
            onCurrencyModeChange={setCurrencyMode}
          />
        </TabsContent>

        {/* 渠道标签页 */}
        <TabsContent value="channel" className="mt-2 space-y-4">
          <ChannelSettings
            settings={appSettings}
            pricing={pricing}
            syncing={syncing}
            onSaved={setAppSettings}
            onSync={async () => {
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
          />
        </TabsContent>

        {/* 系统通知标签页 */}
        <TabsContent value="notifications" className="mt-2 space-y-4">
          <NotificationSettings settings={appSettings} onSaved={setAppSettings} />
        </TabsContent>

        {/* 应用标签页 */}
        <TabsContent value="application" className="mt-2 space-y-4">
          <ApplicationSettings settings={appSettings} onSaved={setAppSettings} />
        </TabsContent>

        {/* CLI 标签页 */}
        <TabsContent value="update" className="mt-2 space-y-4">
          <CliToolsSettings
            cliToolsProxyConfig={cliToolsProxyConfig}
            cliToolsProxyConfigLoading={cliToolsProxyConfigLoading}
            cliProxyConfigBusy={cliProxyConfigBusy}
            cliToolsStatus={cliToolsStatus}
            cliToolsLoading={cliToolsLoading}
            cliToolBusy={cliToolBusy}
            appSettings={appSettings}
            onRefreshCliToolsProxyConfigStatus={refreshCliToolsProxyConfigStatus}
            onApplyCliProxyConfig={async (toolId) => {
              setCliProxyConfigBusy((prev) => ({ ...prev, [toolId]: true }));
              try {
                const res = await applyCliToolsProxyConfig({ tools: [toolId] });
                setCliToolsProxyConfig(res.status);
                const applied = res.applied.find((item) => item.id === toolId);
                if (applied?.ok) {
                  toast.success(t("settings.cliProxyConfig.applied"));
                } else {
                  toast.error(t("settings.cliProxyConfig.applyFail"), {
                    description: humanizeIssue(applied?.issue, t),
                  });
                }
              } catch (e) {
                toast.error(t("settings.cliProxyConfig.applyFail"), {
                  description: humanizeApiError(e, t),
                });
              } finally {
                setCliProxyConfigBusy((prev) => ({ ...prev, [toolId]: false }));
                void refreshCliToolsProxyConfigStatus();
              }
            }}
            onRefreshCliToolsStatus={refreshCliToolsStatus}
            onInstallCliTool={async (toolId) => {
              const tool = cliToolsStatus?.tools.find((item) => item.id === toolId);
              if (!tool) return;
              setCliToolBusy((prev) => ({ ...prev, [toolId]: true }));
              try {
                await installCliToolWithToast({
                  tool,
                  t,
                  onToolUpdated: (nextTool) =>
                    setCliToolsStatus((prev) =>
                      prev
                        ? {
                            ...prev,
                            tools: prev.tools.map((item) =>
                              item.id === nextTool.id ? nextTool : item,
                            ),
                          }
                        : prev,
                    ),
                });
              } finally {
                setCliToolBusy((prev) => ({ ...prev, [toolId]: false }));
              }
            }}
            onCliToolAutoUpdateChange={async (toolId, enabled) => {
              if (!appSettings) return;
              const previous =
                toolId === "gemini"
                  ? appSettings.gemini_cli_auto_update_enabled ?? false
                  : toolId === "claude"
                    ? appSettings.claude_code_auto_update_enabled ?? false
                    : appSettings.codex_auto_update_enabled ?? false;
              const patch =
                toolId === "gemini"
                  ? { gemini_cli_auto_update_enabled: enabled }
                  : toolId === "claude"
                    ? { claude_code_auto_update_enabled: enabled }
                    : { codex_auto_update_enabled: enabled };
              setAppSettings({ ...appSettings, ...patch } as AppSettings);
              try {
                const next = await updateSettings(patch);
                setAppSettings(next);
                toast.success(t("settings.cliTools.saved"));
              } catch (e) {
                const rollback =
                  toolId === "gemini"
                    ? { gemini_cli_auto_update_enabled: previous }
                    : toolId === "claude"
                      ? { claude_code_auto_update_enabled: previous }
                      : { codex_auto_update_enabled: previous };
                setAppSettings({ ...appSettings, ...rollback } as AppSettings);
                toast.error(t("settings.cliTools.saveFail"), {
                  description: humanizeApiError(e, t),
                });
              }
            }}
          />
        </TabsContent>

        {/* 数据标签页 */}
        <TabsContent value="data" className="mt-2 space-y-4">
          <DataSettings
            settings={appSettings}
            onSaved={setAppSettings}
            locale={locale}
            dataDir={health?.data_dir ?? "-"}
            dbPath={health?.db_path ?? "-"}
            dbSizeText={dbSize ? formatBytes(dbSize.total_bytes) : "-"}
            dbSizeLoading={dbSizeLoading}
            onRefreshDbSize={refreshDbSize}
            onOpenDataDir={async () => {
              try {
                await openDataDir();
              } catch (e) {
                toast.error(t("settings.storage.openFail"), {
                  description: humanizeApiError(e, t),
                });
              }
            }}
            recordsType={recordsType}
            onRecordsTypeChange={setRecordsType}
            recordsTimeScope={recordsTimeScope}
            onRecordsTimeScopeChange={setRecordsTimeScope}
            recordsDateRange={recordsDateRange}
            onRecordsDateRangeChange={setRecordsDateRange}
            recordsPromptOpen={recordsPromptOpen}
            onRecordsPromptOpenChange={(open) => {
              if (recordsClearing) return;
              setRecordsPromptOpen(open);
            }}
            recordsClearing={recordsClearing}
            onRequestClearRecords={() => {
              if (recordsTimeScope === "date_range" && !recordsDateRange?.from) {
                toast.error(t("settings.records.invalidDate"));
                return;
              }
              setRecordsPromptOpen(true);
            }}
            onConfirmClearRecords={async () => {
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
                    { count: formatNumber(res.usage_events_deleted) },
                  ),
                });
                setRecordsPromptOpen(false);
                setRecordsDateRange(undefined);
                await refreshDbSize();
              } catch (e) {
                toast.error(t("settings.records.clearFail"), {
                  description: humanizeApiError(e, t),
                });
              } finally {
                setRecordsClearing(false);
              }
            }}
            recordsDateStr={recordsDateStr}
            logsSizeText={logsSize ? formatBytes(logsSize.total_bytes) : "-"}
            logsSizeLoading={logsSizeLoading}
            onRefreshLogsSize={refreshLogsSize}
            logsScope={logsScope}
            onLogsScopeChange={setLogsScope}
            logsDateRange={logsDateRange}
            onLogsDateRangeChange={setLogsDateRange}
            logsPromptOpen={logsPromptOpen}
            onLogsPromptOpenChange={(open) => {
              if (logsClearing) return;
              setLogsPromptOpen(open);
            }}
            logsClearing={logsClearing}
            onRequestClearLogs={() => {
              if (logsScope === "date_range" && !logsDateRange?.from) {
                toast.error(t("settings.logging.invalidDate"));
                return;
              }
              setLogsPromptOpen(true);
            }}
            onConfirmClearLogs={async () => {
              setLogsClearing(true);
              try {
                if (logsScope === "date_range") {
                  const range = dateRangeToStrings(logsDateRange);
                  if (!range) {
                    toast.error(t("settings.logging.invalidDate"));
                    return;
                  }
                  const res = await clearLogs({
                    mode: "date_range",
                    start_date: range.start,
                    end_date: range.end,
                  });
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
                toast.error(t("settings.logging.clearFail"), {
                  description: humanizeApiError(e, t),
                });
              } finally {
                setLogsClearing(false);
              }
            }}
            logsDateStr={logsDateStr}
          />
        </TabsContent>

        <TabsContent value="chatBridge" className="mt-2 space-y-4">
          {chatBridgeTab}
        </TabsContent>

        {/* 系统标签页 */}
        <TabsContent value="system" className="mt-2 space-y-4">
          <SystemSettings
            settings={appSettings}
            health={health}
            backendStatusLabel={backendStatusLabel}
            apiHost={apiHost}
            apiPort={apiPort}
            updateDialog={(
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
                  const version = updatePromptVersion;
                  if (!version) return;
                  setUpdateIgnoring(true);
                  try {
                    const next = await ignoreUpdate(version);
                    setUpdateStatus(next);
                    toast.success(t("settings.update.ignoredToast", { version }));
                    setUpdatePromptOpen(false);
                  } catch (e) {
                    toast.error(t("settings.update.ignoreFail"), { description: humanizeApiError(e, t) });
                  } finally {
                    setUpdateIgnoring(false);
                  }
                }}
              />
            )}
            updateStatusText={updateStatusText}
            updateServerVersion={updateServerVersion}
            updateChecking={updateChecking}
            onSaved={setAppSettings}
            onCheck={async () => {
              setUpdateChecking(true);
              try {
                const res = await checkUpdate();
                setUpdateCheckResult(res);
                const nextStatus = await getUpdateStatus().catch(() => null);
                if (nextStatus) setUpdateStatus(nextStatus);

                if (nextStatus?.pending_version) {
                  const pending = nextStatus.pending_version;
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
                  reopenUpdateReadyPrompt(nextStatus);
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
            onAutoUpdateChange={async (enabled) => {
              try {
                const next = await updateSettings({ app_auto_update_enabled: enabled });
                setAppSettings(next);
                toast.success(t("settings.update.saved"));
                if (enabled) {
                  const res = await checkUpdate();
                  setUpdateCheckResult(res);
                  if (res.update_available && res.latest_version) {
                    if (res.latest_ignored) {
                      toast.info(t("settings.update.ignoredToast", { version: res.latest_version }));
                    } else {
                      openUpdatePrompt(res.latest_version);
                    }
                  }
                  const nextStatus = await getUpdateStatus().catch(() => null);
                  if (nextStatus) setUpdateStatus(nextStatus);
                }
              } catch (e) {
                toast.error(t("settings.update.saveFail"), { description: humanizeApiError(e, t) });
              }
            }}
          />
        </TabsContent>
      </Tabs>
    </div>
  );
}
