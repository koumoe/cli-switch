import { useEffect, useMemo, useState, type ReactNode } from "react";
import { zodResolver } from "@hookform/resolvers/zod";
import { useForm } from "react-hook-form";
import { Bell, Bot, Database, DollarSign, Info, Monitor, Power, RefreshCw, ScrollText, Shield } from "lucide-react";
import { toast } from "sonner";

import { updateSettings } from "@/api";
import {
  Badge,
  Button,
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
  Form,
  FormControl,
  FormDescription as UiFormDescription,
  FormField,
  FormItem,
  FormLabel,
  FormMessage,
  Input,
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
  Switch,
} from "@/components/ui";
import { useI18n } from "@/hooks/use-i18n";
import { humanizeApiError } from "@/lib/error";
import { setLogLevel, type LogLevel } from "@/lib/logger";
import {
  createAppUpdateSchema,
  createChannelProtectionSchema,
  createChannelRetrySchema,
  createChatBridgeBaseSchema,
  createChatBridgeDiscordSchema,
  createChatBridgeTelegramSchema,
  createChatBridgeWeixinSchema,
  createChatBridgeWhatsAppSchema,
  createCompatibilitySchema,
  createLoggingSchema,
  createNewApiManagedSchema,
  createPricingDataSchema,
  createRemoteNotificationsSchema,
  createServiceInfoSchema,
  createStartupSchema,
  createSystemNotificationsSchema,
  createWindowCloseSchema,
} from "@/lib/schemas/settings";
import { postIpc } from "@/lib/ipc";
import type { AppSettings, PricingStatus } from "@/types/api";

type SettingsSectionProps = {
  settings: AppSettings | null;
  onSaved: (next: AppSettings) => void;
};

type ChannelProtectionFormValues = {
  auto_disable_enabled: boolean;
  auto_disable_window_minutes: string;
  auto_disable_failure_times: string;
  auto_disable_disable_minutes: string;
};

type ChannelRetryFormValues = {
  channel_retry_enabled: boolean;
};

type NewApiManagedFormValues = {
  remote_managed_channel_missing_prompt_enabled: boolean;
  remote_managed_channel_sync_multiplier_enabled: boolean;
  remote_managed_channel_sync_free_multiplier_enabled: boolean;
};

type SystemNotificationsFormValues = {
  system_notifications_enabled: boolean;
};

type RemoteNotificationsFormValues = {
  remote_low_balance_system_notification_enabled: boolean;
  remote_managed_channel_missing_system_notification_enabled: boolean;
  remote_managed_channel_multiplier_system_notification_enabled: boolean;
  remote_group_added_system_notification_enabled: boolean;
};

type PricingDataFormValues = {
  pricing_auto_update_enabled: boolean;
  pricing_auto_update_interval_hours: string;
};

type WindowCloseFormValues = {
  close_behavior: "ask" | "minimize_to_tray" | "quit";
};

type StartupFormValues = {
  auto_start_enabled: boolean;
  auto_start_launch_mode: "show_window" | "minimize_to_tray";
};

type LoggingFormValues = {
  log_level: LogLevel;
  log_retention_days: string;
};

type ServiceInfoFormValues = {
  server_lan_accessible: boolean;
};

type CompatibilityFormValues = {
  anthropic_count_tokens_mock_enabled: boolean;
};

type AppUpdateFormValues = {
  app_auto_update_enabled: boolean;
};

type ChatBridgeBaseFormValues = {
  chat_bridge_enabled: boolean;
  chat_bridge_allow_new_projects: boolean;
  chat_bridge_turn_timeout_minutes: string;
};

type ChatBridgeTelegramFormValues = {
  chat_bridge_telegram_enabled: boolean;
  chat_bridge_telegram_bot_token: string;
};

type ChatBridgeDiscordFormValues = {
  chat_bridge_discord_enabled: boolean;
  chat_bridge_discord_bot_token: string;
};

type ChatBridgeWhatsAppFormValues = {
  chat_bridge_whatsapp_enabled: boolean;
};

type ChatBridgeWeixinFormValues = {
  chat_bridge_weixin_enabled: boolean;
};

type PricingDataSettingsCardProps = SettingsSectionProps & {
  pricing: PricingStatus | null;
  syncing: boolean;
  onSync: () => void | Promise<void>;
};

type LoggingSettingsCardProps = SettingsSectionProps & {
  dataDir: string | null;
  logsSizeText: string;
  logsSizeLoading: boolean;
  onRefreshLogsSize: () => void | Promise<void>;
  children?: ReactNode;
};

type ServiceInfoSettingsCardProps = SettingsSectionProps & {
  apiHost: string;
  apiPort: string;
};

type AppUpdateSettingsCardProps = SettingsSectionProps & {
  dialog: ReactNode;
  updateStatusText: string;
  updateServerVersion: string | null;
  updateChecking: boolean;
  onCheck: () => void | Promise<void>;
  onAutoUpdateChange: (enabled: boolean) => Promise<void>;
};

type ChatBridgeBaseSettingsCardProps = SettingsSectionProps;

type ChatBridgeTelegramSettingsCardProps = SettingsSectionProps & {
  bindingCount: number;
  tokenConfigured: boolean;
  onOpenPairing: () => void;
  onOpenBindings: () => void;
};

type ChatBridgeDiscordSettingsCardProps = SettingsSectionProps & {
  bindingCount: number;
  tokenConfigured: boolean;
  onOpenPairing: () => void;
  onOpenBindings: () => void;
};

type ChatBridgeWhatsAppSettingsCardProps = SettingsSectionProps & {
  bindingCount: number;
  statusTone: "success" | "secondary" | "destructive";
  statusLabel: string;
  actionBusy: boolean;
  onOpenLoginDialog: () => void;
  onOpenPairing: () => void;
  onOpenBindings: () => void;
};

type ChatBridgeWeixinSettingsCardProps = SettingsSectionProps & {
  bindingCount: number;
  statusTone: "success" | "secondary" | "destructive";
  statusLabel: string;
  actionBusy: boolean;
  onOpenLoginDialog: () => void;
  onOpenPairing: () => void;
  onOpenBindings: () => void;
};

function channelProtectionDefaults(settings: AppSettings | null): ChannelProtectionFormValues {
  return {
    auto_disable_enabled: settings?.auto_disable_enabled ?? false,
    auto_disable_window_minutes: String(settings?.auto_disable_window_minutes ?? 3),
    auto_disable_failure_times: String(settings?.auto_disable_failure_times ?? 5),
    auto_disable_disable_minutes: String(settings?.auto_disable_disable_minutes ?? 30),
  };
}

function channelRetryDefaults(settings: AppSettings | null): ChannelRetryFormValues {
  return {
    channel_retry_enabled: settings?.channel_retry_enabled ?? false,
  };
}

function newApiManagedDefaults(settings: AppSettings | null): NewApiManagedFormValues {
  return {
    remote_managed_channel_missing_prompt_enabled:
      settings?.remote_managed_channel_missing_prompt_enabled ?? true,
    remote_managed_channel_sync_multiplier_enabled:
      settings?.remote_managed_channel_sync_multiplier_enabled ?? true,
    remote_managed_channel_sync_free_multiplier_enabled:
      settings?.remote_managed_channel_sync_free_multiplier_enabled ?? false,
  };
}

function systemNotificationsDefaults(settings: AppSettings | null): SystemNotificationsFormValues {
  return {
    system_notifications_enabled: settings?.system_notifications_enabled ?? true,
  };
}

function remoteNotificationsDefaults(settings: AppSettings | null): RemoteNotificationsFormValues {
  return {
    remote_low_balance_system_notification_enabled:
      settings?.remote_low_balance_system_notification_enabled ?? true,
    remote_managed_channel_missing_system_notification_enabled:
      settings?.remote_managed_channel_missing_system_notification_enabled ?? true,
    remote_managed_channel_multiplier_system_notification_enabled:
      settings?.remote_managed_channel_multiplier_system_notification_enabled ?? true,
    remote_group_added_system_notification_enabled:
      settings?.remote_group_added_system_notification_enabled ?? true,
  };
}

function pricingDataDefaults(settings: AppSettings | null): PricingDataFormValues {
  return {
    pricing_auto_update_enabled: settings?.pricing_auto_update_enabled ?? false,
    pricing_auto_update_interval_hours: String(settings?.pricing_auto_update_interval_hours ?? 24),
  };
}

function windowCloseDefaults(settings: AppSettings | null): WindowCloseFormValues {
  return {
    close_behavior: settings?.close_behavior ?? "ask",
  };
}

function startupDefaults(settings: AppSettings | null): StartupFormValues {
  return {
    auto_start_enabled: settings?.auto_start_enabled ?? false,
    auto_start_launch_mode: settings?.auto_start_launch_mode ?? "show_window",
  };
}

function loggingDefaults(settings: AppSettings | null): LoggingFormValues {
  return {
    log_level: settings?.log_level ?? "warning",
    log_retention_days: String(settings?.log_retention_days ?? 30),
  };
}

function serviceInfoDefaults(settings: AppSettings | null): ServiceInfoFormValues {
  return {
    server_lan_accessible: settings?.server_lan_accessible ?? false,
  };
}

function compatibilityDefaults(settings: AppSettings | null): CompatibilityFormValues {
  return {
    anthropic_count_tokens_mock_enabled: settings?.anthropic_count_tokens_mock_enabled ?? false,
  };
}

function appUpdateDefaults(settings: AppSettings | null): AppUpdateFormValues {
  return {
    app_auto_update_enabled: settings?.app_auto_update_enabled ?? false,
  };
}

function chatBridgeBaseDefaults(settings: AppSettings | null): ChatBridgeBaseFormValues {
  return {
    chat_bridge_enabled: settings?.chat_bridge_enabled ?? false,
    chat_bridge_allow_new_projects: settings?.chat_bridge_allow_new_projects ?? false,
    chat_bridge_turn_timeout_minutes: String(settings?.chat_bridge_turn_timeout_minutes ?? 0),
  };
}

function chatBridgeTelegramDefaults(settings: AppSettings | null): ChatBridgeTelegramFormValues {
  return {
    chat_bridge_telegram_enabled: settings?.chat_bridge_telegram_enabled ?? false,
    chat_bridge_telegram_bot_token: "",
  };
}

function chatBridgeDiscordDefaults(settings: AppSettings | null): ChatBridgeDiscordFormValues {
  return {
    chat_bridge_discord_enabled: settings?.chat_bridge_discord_enabled ?? false,
    chat_bridge_discord_bot_token: "",
  };
}

function chatBridgeWhatsAppDefaults(settings: AppSettings | null): ChatBridgeWhatsAppFormValues {
  return {
    chat_bridge_whatsapp_enabled: settings?.chat_bridge_whatsapp_enabled ?? false,
  };
}

function chatBridgeWeixinDefaults(settings: AppSettings | null): ChatBridgeWeixinFormValues {
  return {
    chat_bridge_weixin_enabled: settings?.chat_bridge_weixin_enabled ?? false,
  };
}

export function ChannelProtectionSettingsCard({ settings, onSaved }: SettingsSectionProps) {
  const { t } = useI18n();
  const [saving, setSaving] = useState(false);
  const schema = useMemo(() => createChannelProtectionSchema(t), [t]);
  const form = useForm<ChannelProtectionFormValues>({
    resolver: zodResolver(schema),
    defaultValues: channelProtectionDefaults(settings),
  });
  const enabled = form.watch("auto_disable_enabled");

  useEffect(() => {
    form.reset(channelProtectionDefaults(settings));
  }, [form, settings]);

  const submit = form.handleSubmit(
    async (values) => {
      setSaving(true);
      try {
        const next = await updateSettings({
          auto_disable_enabled: values.auto_disable_enabled,
          auto_disable_window_minutes: Number.parseInt(values.auto_disable_window_minutes, 10),
          auto_disable_failure_times: Number.parseInt(values.auto_disable_failure_times, 10),
          auto_disable_disable_minutes: Number.parseInt(values.auto_disable_disable_minutes, 10),
        });
        onSaved(next);
        toast.success(t("settings.channelProtection.saved"));
      } catch (e) {
        toast.error(t("settings.channelProtection.saveFail"), {
          description: humanizeApiError(e, t),
        });
      } finally {
        setSaving(false);
      }
    },
    () => {
      toast.error(t("settings.channelProtection.invalid"));
    },
  );

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <Shield className="h-4 w-4" />
          {t("settings.channelProtection.title")}
        </CardTitle>
        <CardDescription>{t("settings.channelProtection.subtitle")}</CardDescription>
      </CardHeader>
      <CardContent>
        <Form {...form}>
          <form onSubmit={submit} className="space-y-4">
            <FormField
              control={form.control}
              name="auto_disable_enabled"
              render={({ field }) => (
                <FormItem className="flex flex-row items-center justify-between gap-4 space-y-0">
                  <div>
                    <FormLabel className="font-medium text-sm">{t("settings.channelProtection.enable")}</FormLabel>
                    <UiFormDescription>{t("settings.channelProtection.enableHint")}</UiFormDescription>
                  </div>
                  <FormControl>
                    <Switch checked={field.value} onCheckedChange={field.onChange} disabled={!settings} />
                  </FormControl>
                </FormItem>
              )}
            />

            <div className="grid grid-cols-3 gap-3">
              <FormField
                control={form.control}
                name="auto_disable_window_minutes"
                render={({ field }) => (
                  <FormItem>
                    <FormLabel>{t("settings.channelProtection.windowMinutes")}</FormLabel>
                    <FormControl>
                      <Input {...field} type="number" min="1" className="h-8" disabled={!settings || !enabled} />
                    </FormControl>
                    <UiFormDescription>{t("settings.channelProtection.windowMinutesHint")}</UiFormDescription>
                    <FormMessage />
                  </FormItem>
                )}
              />

              <FormField
                control={form.control}
                name="auto_disable_failure_times"
                render={({ field }) => (
                  <FormItem>
                    <FormLabel>{t("settings.channelProtection.failureTimes")}</FormLabel>
                    <FormControl>
                      <Input {...field} type="number" min="1" className="h-8" disabled={!settings || !enabled} />
                    </FormControl>
                    <UiFormDescription>{t("settings.channelProtection.failureTimesHint")}</UiFormDescription>
                    <FormMessage />
                  </FormItem>
                )}
              />

              <FormField
                control={form.control}
                name="auto_disable_disable_minutes"
                render={({ field }) => (
                  <FormItem>
                    <FormLabel>{t("settings.channelProtection.pauseMinutes")}</FormLabel>
                    <FormControl>
                      <Input {...field} type="number" min="1" className="h-8" disabled={!settings || !enabled} />
                    </FormControl>
                    <UiFormDescription>{t("settings.channelProtection.pauseMinutesHint")}</UiFormDescription>
                    <FormMessage />
                  </FormItem>
                )}
              />
            </div>

            <div className="flex justify-end">
              <Button size="sm" type="submit" disabled={!settings || saving}>
                {t("common.save")}
              </Button>
            </div>
          </form>
        </Form>
      </CardContent>
    </Card>
  );
}

export function ChannelRetrySettingsCard({ settings, onSaved }: SettingsSectionProps) {
  const { t } = useI18n();
  const [saving, setSaving] = useState(false);
  const schema = useMemo(() => createChannelRetrySchema(), []);
  const form = useForm<ChannelRetryFormValues>({
    resolver: zodResolver(schema),
    defaultValues: channelRetryDefaults(settings),
  });

  useEffect(() => {
    form.reset(channelRetryDefaults(settings));
  }, [form, settings]);

  const submit = form.handleSubmit(async (values) => {
    setSaving(true);
    try {
      const next = await updateSettings({
        channel_retry_enabled: values.channel_retry_enabled,
      });
      onSaved(next);
      toast.success(t("settings.channelRetry.saved"));
    } catch (e) {
      toast.error(t("settings.channelRetry.saveFail"), {
        description: humanizeApiError(e, t),
      });
    } finally {
      setSaving(false);
    }
  });

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <RefreshCw className="h-4 w-4" />
          {t("settings.channelRetry.title")}
        </CardTitle>
        <CardDescription>{t("settings.channelRetry.subtitle")}</CardDescription>
      </CardHeader>
      <CardContent>
        <Form {...form}>
          <form onSubmit={submit} className="space-y-4">
            <FormField
              control={form.control}
              name="channel_retry_enabled"
              render={({ field }) => (
                <FormItem className="flex flex-row items-center justify-between gap-4 space-y-0">
                  <div>
                    <FormLabel className="font-medium text-sm">{t("settings.channelRetry.enable")}</FormLabel>
                    <UiFormDescription>{t("settings.channelRetry.enableHint")}</UiFormDescription>
                  </div>
                  <FormControl>
                    <Switch checked={field.value} onCheckedChange={field.onChange} disabled={!settings} />
                  </FormControl>
                </FormItem>
              )}
            />

            <div className="flex justify-end">
              <Button size="sm" type="submit" disabled={!settings || saving}>
                {t("common.save")}
              </Button>
            </div>
          </form>
        </Form>
      </CardContent>
    </Card>
  );
}

export function NewApiManagedSettingsCard({ settings, onSaved }: SettingsSectionProps) {
  const { t } = useI18n();
  const [saving, setSaving] = useState(false);
  const schema = useMemo(() => createNewApiManagedSchema(), []);
  const form = useForm<NewApiManagedFormValues>({
    resolver: zodResolver(schema),
    defaultValues: newApiManagedDefaults(settings),
  });
  const syncMultiplierEnabled = form.watch("remote_managed_channel_sync_multiplier_enabled");

  useEffect(() => {
    form.reset(newApiManagedDefaults(settings));
  }, [form, settings]);

  const submit = form.handleSubmit(async (values) => {
    setSaving(true);
    try {
      const next = await updateSettings({
        remote_managed_channel_missing_prompt_enabled:
          values.remote_managed_channel_missing_prompt_enabled,
        remote_managed_channel_sync_multiplier_enabled:
          values.remote_managed_channel_sync_multiplier_enabled,
        remote_managed_channel_sync_free_multiplier_enabled:
          values.remote_managed_channel_sync_free_multiplier_enabled,
      });
      onSaved(next);
      toast.success(t("settings.newApiManaged.saved"));
    } catch (e) {
      toast.error(t("settings.newApiManaged.saveFail"), {
        description: humanizeApiError(e, t),
      });
    } finally {
      setSaving(false);
    }
  });

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <Bot className="h-4 w-4" />
          {t("settings.newApiManaged.title")}
        </CardTitle>
        <CardDescription>{t("settings.newApiManaged.subtitle")}</CardDescription>
      </CardHeader>
      <CardContent>
        <Form {...form}>
          <form onSubmit={submit} className="space-y-4">
            <FormField
              control={form.control}
              name="remote_managed_channel_missing_prompt_enabled"
              render={({ field }) => (
                <FormItem className="flex flex-row items-center justify-between gap-4 space-y-0">
                  <div>
                    <FormLabel className="font-medium text-sm">{t("settings.newApiManaged.missingPrompt")}</FormLabel>
                    <UiFormDescription>{t("settings.newApiManaged.missingPromptHint")}</UiFormDescription>
                  </div>
                  <FormControl>
                    <Switch checked={field.value} onCheckedChange={field.onChange} disabled={!settings} />
                  </FormControl>
                </FormItem>
              )}
            />

            <FormField
              control={form.control}
              name="remote_managed_channel_sync_multiplier_enabled"
              render={({ field }) => (
                <FormItem className="flex flex-row items-center justify-between gap-4 space-y-0">
                  <div>
                    <FormLabel className="font-medium text-sm">{t("settings.newApiManaged.syncMultiplier")}</FormLabel>
                    <UiFormDescription>{t("settings.newApiManaged.syncMultiplierHint")}</UiFormDescription>
                  </div>
                  <FormControl>
                    <Switch checked={field.value} onCheckedChange={field.onChange} disabled={!settings} />
                  </FormControl>
                </FormItem>
              )}
            />

            {syncMultiplierEnabled ? (
              <FormField
                control={form.control}
                name="remote_managed_channel_sync_free_multiplier_enabled"
                render={({ field }) => (
                  <FormItem className="flex flex-row items-center justify-between gap-4 space-y-0">
                    <div>
                      <FormLabel className="font-medium text-sm">{t("settings.newApiManaged.ignoreFreeMultiplier")}</FormLabel>
                      <UiFormDescription>{t("settings.newApiManaged.ignoreFreeMultiplierHint")}</UiFormDescription>
                    </div>
                    <FormControl>
                      <Switch checked={field.value} onCheckedChange={field.onChange} disabled={!settings} />
                    </FormControl>
                  </FormItem>
                )}
              />
            ) : null}

            <div className="flex justify-end">
              <Button size="sm" type="submit" disabled={!settings || saving}>
                {t("common.save")}
              </Button>
            </div>
          </form>
        </Form>
      </CardContent>
    </Card>
  );
}

export function SystemNotificationsSettingsCard({ settings, onSaved }: SettingsSectionProps) {
  const { t } = useI18n();
  const [saving, setSaving] = useState(false);
  const schema = useMemo(() => createSystemNotificationsSchema(), []);
  const form = useForm<SystemNotificationsFormValues>({
    resolver: zodResolver(schema),
    defaultValues: systemNotificationsDefaults(settings),
  });

  useEffect(() => {
    form.reset(systemNotificationsDefaults(settings));
  }, [form, settings]);

  const submit = form.handleSubmit(async (values) => {
    setSaving(true);
    try {
      const next = await updateSettings({
        system_notifications_enabled: values.system_notifications_enabled,
      });
      onSaved(next);
      toast.success(t("settings.systemNotifications.saved"));
    } catch (e) {
      toast.error(t("settings.systemNotifications.saveFail"), {
        description: humanizeApiError(e, t),
      });
    } finally {
      setSaving(false);
    }
  });

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <Bell className="h-4 w-4" />
          {t("settings.systemNotifications.title")}
        </CardTitle>
        <CardDescription>{t("settings.systemNotifications.subtitle")}</CardDescription>
      </CardHeader>
      <CardContent>
        <Form {...form}>
          <form onSubmit={submit} className="space-y-4">
            <FormField
              control={form.control}
              name="system_notifications_enabled"
              render={({ field }) => (
                <FormItem className="flex flex-row items-center justify-between gap-4 space-y-0">
                  <div>
                    <FormLabel className="font-medium text-sm">{t("settings.systemNotifications.enable")}</FormLabel>
                    <UiFormDescription>{t("settings.systemNotifications.enableHint")}</UiFormDescription>
                  </div>
                  <FormControl>
                    <Switch checked={field.value} onCheckedChange={field.onChange} disabled={!settings || saving} />
                  </FormControl>
                </FormItem>
              )}
            />

            <div className="flex justify-end">
              <Button size="sm" type="submit" disabled={!settings || saving}>
                {t("common.save")}
              </Button>
            </div>
          </form>
        </Form>
      </CardContent>
    </Card>
  );
}

export function RemoteSystemNotificationsSettingsCard({ settings, onSaved }: SettingsSectionProps) {
  const { t } = useI18n();
  const [saving, setSaving] = useState(false);
  const schema = useMemo(() => createRemoteNotificationsSchema(), []);
  const form = useForm<RemoteNotificationsFormValues>({
    resolver: zodResolver(schema),
    defaultValues: remoteNotificationsDefaults(settings),
  });

  useEffect(() => {
    form.reset(remoteNotificationsDefaults(settings));
  }, [form, settings]);

  const submit = form.handleSubmit(async (values) => {
    setSaving(true);
    try {
      const next = await updateSettings({
        remote_low_balance_system_notification_enabled:
          values.remote_low_balance_system_notification_enabled,
        remote_managed_channel_missing_system_notification_enabled:
          values.remote_managed_channel_missing_system_notification_enabled,
        remote_managed_channel_multiplier_system_notification_enabled:
          values.remote_managed_channel_multiplier_system_notification_enabled,
        remote_group_added_system_notification_enabled:
          values.remote_group_added_system_notification_enabled,
      });
      onSaved(next);
      toast.success(t("settings.systemNotifications.saved"));
    } catch (e) {
      toast.error(t("settings.systemNotifications.saveFail"), {
        description: humanizeApiError(e, t),
      });
    } finally {
      setSaving(false);
    }
  });

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <Bot className="h-4 w-4" />
          {t("settings.newApiManaged.title")}
        </CardTitle>
        <CardDescription>{t("settings.systemNotifications.newApiSubtitle")}</CardDescription>
      </CardHeader>
      <CardContent>
        <Form {...form}>
          <form onSubmit={submit} className="space-y-4">
            <FormField
              control={form.control}
              name="remote_low_balance_system_notification_enabled"
              render={({ field }) => (
                <FormItem className="flex flex-row items-center justify-between gap-4 space-y-0">
                  <div>
                    <FormLabel className="font-medium text-sm">{t("settings.systemNotifications.lowBalance")}</FormLabel>
                    <UiFormDescription>{t("settings.systemNotifications.lowBalanceHint")}</UiFormDescription>
                  </div>
                  <FormControl>
                    <Switch checked={field.value} onCheckedChange={field.onChange} disabled={!settings || saving} />
                  </FormControl>
                </FormItem>
              )}
            />

            <FormField
              control={form.control}
              name="remote_managed_channel_missing_system_notification_enabled"
              render={({ field }) => (
                <FormItem className="flex flex-row items-center justify-between gap-4 space-y-0">
                  <div>
                    <FormLabel className="font-medium text-sm">{t("settings.systemNotifications.managedChannelMissing")}</FormLabel>
                    <UiFormDescription>{t("settings.systemNotifications.managedChannelMissingHint")}</UiFormDescription>
                  </div>
                  <FormControl>
                    <Switch checked={field.value} onCheckedChange={field.onChange} disabled={!settings || saving} />
                  </FormControl>
                </FormItem>
              )}
            />

            <FormField
              control={form.control}
              name="remote_managed_channel_multiplier_system_notification_enabled"
              render={({ field }) => (
                <FormItem className="flex flex-row items-center justify-between gap-4 space-y-0">
                  <div>
                    <FormLabel className="font-medium text-sm">{t("settings.systemNotifications.managedChannelMultiplier")}</FormLabel>
                    <UiFormDescription>{t("settings.systemNotifications.managedChannelMultiplierHint")}</UiFormDescription>
                  </div>
                  <FormControl>
                    <Switch checked={field.value} onCheckedChange={field.onChange} disabled={!settings || saving} />
                  </FormControl>
                </FormItem>
              )}
            />

            <FormField
              control={form.control}
              name="remote_group_added_system_notification_enabled"
              render={({ field }) => (
                <FormItem className="flex flex-row items-center justify-between gap-4 space-y-0">
                  <div>
                    <FormLabel className="font-medium text-sm">{t("settings.systemNotifications.remoteGroupAdded")}</FormLabel>
                    <UiFormDescription>{t("settings.systemNotifications.remoteGroupAddedHint")}</UiFormDescription>
                  </div>
                  <FormControl>
                    <Switch checked={field.value} onCheckedChange={field.onChange} disabled={!settings || saving} />
                  </FormControl>
                </FormItem>
              )}
            />

            <div className="flex justify-end">
              <Button size="sm" type="submit" disabled={!settings || saving}>
                {t("common.save")}
              </Button>
            </div>
          </form>
        </Form>
      </CardContent>
    </Card>
  );
}

export function PricingDataSettingsCard({
  settings,
  pricing,
  syncing,
  onSync,
  onSaved,
}: PricingDataSettingsCardProps) {
  const { t } = useI18n();
  const [saving, setSaving] = useState(false);
  const schema = useMemo(() => createPricingDataSchema(t), [t]);
  const form = useForm<PricingDataFormValues>({
    resolver: zodResolver(schema),
    defaultValues: pricingDataDefaults(settings),
  });
  const enabled = form.watch("pricing_auto_update_enabled");

  useEffect(() => {
    form.reset(pricingDataDefaults(settings));
  }, [form, settings]);

  const submit = form.handleSubmit(
    async (values) => {
      setSaving(true);
      try {
        const next = await updateSettings({
          pricing_auto_update_enabled: values.pricing_auto_update_enabled,
          pricing_auto_update_interval_hours: Number.parseInt(values.pricing_auto_update_interval_hours, 10),
        });
        onSaved(next);
        toast.success(t("settings.pricingData.saved"));
      } catch (e) {
        toast.error(t("settings.pricingData.saveFail"), {
          description: humanizeApiError(e, t),
        });
      } finally {
        setSaving(false);
      }
    },
    () => {
      toast.error(t("settings.pricingData.intervalInvalid"));
    },
  );

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <DollarSign className="h-4 w-4" />
          {t("settings.pricingData.title")}
        </CardTitle>
        <CardDescription>{t("settings.pricingData.subtitle")}</CardDescription>
      </CardHeader>
      <CardContent>
        <Form {...form}>
          <form onSubmit={submit} className="space-y-4">
            <div className="flex items-center justify-between gap-3">
              <div>
                <div className="font-medium text-sm">{t("settings.pricingData.status")}</div>
                <div className="text-xs text-muted-foreground">
                  {t("settings.pricingData.count", { count: pricing?.count ?? 0 })}
                  {" · "}
                  {t("settings.pricingData.lastSync", {
                    time: pricing?.last_sync_ms ? new Date(pricing.last_sync_ms).toLocaleString() : "-",
                  })}
                </div>
              </div>
              <Button
                type="button"
                size="sm"
                variant="outline"
                onClick={() => void onSync()}
                disabled={syncing}
                className="gap-2"
              >
                <RefreshCw className={`h-4 w-4 ${syncing ? "animate-spin" : ""}`} />
                {t("settings.pricingData.sync")}
              </Button>
            </div>

            <FormField
              control={form.control}
              name="pricing_auto_update_enabled"
              render={({ field }) => (
                <FormItem className="flex flex-row items-center justify-between gap-4 space-y-0">
                  <div>
                    <FormLabel className="font-medium text-sm">{t("settings.pricingData.autoUpdate")}</FormLabel>
                    <UiFormDescription>{t("settings.pricingData.autoUpdateHint")}</UiFormDescription>
                  </div>
                  <FormControl>
                    <Switch checked={field.value} onCheckedChange={field.onChange} disabled={!settings || saving} />
                  </FormControl>
                </FormItem>
              )}
            />

            <FormField
              control={form.control}
              name="pricing_auto_update_interval_hours"
              render={({ field }) => (
                <FormItem className="flex items-center justify-between gap-4">
                  <div>
                    <FormLabel className="font-medium text-sm">{t("settings.pricingData.intervalHours")}</FormLabel>
                    <UiFormDescription>{t("settings.pricingData.intervalHoursHint")}</UiFormDescription>
                  </div>
                  <div className="w-[140px]">
                    <FormControl>
                      <Input {...field} type="number" min="1" max="8760" className="h-8" disabled={!settings || saving || !enabled} />
                    </FormControl>
                  </div>
                  <FormMessage />
                </FormItem>
              )}
            />

            <div className="flex justify-end">
              <Button size="sm" type="submit" disabled={!settings || saving}>
                {t("common.save")}
              </Button>
            </div>
          </form>
        </Form>
      </CardContent>
    </Card>
  );
}

export function WindowCloseSettingsCard({ settings, onSaved }: SettingsSectionProps) {
  const { t } = useI18n();
  const [saving, setSaving] = useState(false);
  const schema = useMemo(() => createWindowCloseSchema(), []);
  const form = useForm<WindowCloseFormValues>({
    resolver: zodResolver(schema),
    defaultValues: windowCloseDefaults(settings),
  });

  useEffect(() => {
    form.reset(windowCloseDefaults(settings));
  }, [form, settings]);

  const submit = form.handleSubmit(async (values) => {
    setSaving(true);
    try {
      const next = await updateSettings({ close_behavior: values.close_behavior });
      onSaved(next);
      toast.success(t("settings.windowClose.saved"));
    } catch (e) {
      toast.error(t("settings.windowClose.saveFail"), {
        description: humanizeApiError(e, t),
      });
    } finally {
      setSaving(false);
    }
  });

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <Monitor className="h-4 w-4" />
          {t("settings.windowClose.title")}
        </CardTitle>
        <CardDescription>{t("settings.windowClose.subtitle")}</CardDescription>
      </CardHeader>
      <CardContent>
        <Form {...form}>
          <form onSubmit={submit} className="space-y-4">
            <FormField
              control={form.control}
              name="close_behavior"
              render={({ field }) => (
                <FormItem className="flex items-center justify-between gap-4">
                  <div>
                    <FormLabel className="font-medium text-sm">{t("settings.windowClose.behavior")}</FormLabel>
                    <UiFormDescription>{t("settings.windowClose.behaviorHint")}</UiFormDescription>
                  </div>
                  <div className="w-[220px]">
                    <Select value={field.value} onValueChange={field.onChange}>
                      <FormControl>
                        <SelectTrigger>
                          <SelectValue />
                        </SelectTrigger>
                      </FormControl>
                      <SelectContent>
                        <SelectItem value="ask">{t("settings.windowClose.ask")}</SelectItem>
                        <SelectItem value="minimize_to_tray">{t("settings.windowClose.minimize")}</SelectItem>
                        <SelectItem value="quit">{t("settings.windowClose.quit")}</SelectItem>
                      </SelectContent>
                    </Select>
                  </div>
                  <FormMessage />
                </FormItem>
              )}
            />

            <div className="flex justify-end">
              <Button size="sm" type="submit" disabled={!settings || saving}>
                {t("common.save")}
              </Button>
            </div>
          </form>
        </Form>
      </CardContent>
    </Card>
  );
}

export function StartupSettingsCard({ settings, onSaved }: SettingsSectionProps) {
  const { t } = useI18n();
  const [saving, setSaving] = useState(false);
  const schema = useMemo(() => createStartupSchema(), []);
  const form = useForm<StartupFormValues>({
    resolver: zodResolver(schema),
    defaultValues: startupDefaults(settings),
  });

  useEffect(() => {
    form.reset(startupDefaults(settings));
  }, [form, settings]);

  const submit = form.handleSubmit(async (values) => {
    setSaving(true);
    try {
      const next = await updateSettings({
        auto_start_enabled: values.auto_start_enabled,
        auto_start_launch_mode: values.auto_start_launch_mode,
      });
      onSaved(next);
      toast.success(t("settings.startup.saved"));
    } catch (e) {
      toast.error(t("settings.startup.saveFail"), {
        description: humanizeApiError(e, t),
      });
    } finally {
      setSaving(false);
    }
  });

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <Power className="h-4 w-4" />
          {t("settings.startup.title")}
        </CardTitle>
        <CardDescription>{t("settings.startup.subtitle")}</CardDescription>
      </CardHeader>
      <CardContent>
        <Form {...form}>
          <form onSubmit={submit} className="space-y-4">
            <FormField
              control={form.control}
              name="auto_start_enabled"
              render={({ field }) => (
                <FormItem className="flex flex-row items-center justify-between gap-4 space-y-0">
                  <div>
                    <FormLabel className="font-medium text-sm">{t("settings.startup.enable")}</FormLabel>
                    <UiFormDescription>{t("settings.startup.enableHint")}</UiFormDescription>
                  </div>
                  <FormControl>
                    <Switch checked={field.value} onCheckedChange={field.onChange} disabled={!settings || saving} />
                  </FormControl>
                </FormItem>
              )}
            />

            <FormField
              control={form.control}
              name="auto_start_launch_mode"
              render={({ field }) => (
                <FormItem className="flex items-center justify-between gap-4">
                  <div>
                    <FormLabel className="font-medium text-sm">{t("settings.startup.launchMode")}</FormLabel>
                    <UiFormDescription>{t("settings.startup.launchModeHint")}</UiFormDescription>
                  </div>
                  <div className="w-[220px]">
                    <Select value={field.value} onValueChange={field.onChange}>
                      <FormControl>
                        <SelectTrigger>
                          <SelectValue />
                        </SelectTrigger>
                      </FormControl>
                      <SelectContent>
                        <SelectItem value="show_window">{t("settings.startup.launchShow")}</SelectItem>
                        <SelectItem value="minimize_to_tray">{t("settings.startup.launchMinimize")}</SelectItem>
                      </SelectContent>
                    </Select>
                  </div>
                  <FormMessage />
                </FormItem>
              )}
            />

            <div className="flex justify-end">
              <Button size="sm" type="submit" disabled={!settings || saving}>
                {t("common.save")}
              </Button>
            </div>
          </form>
        </Form>
      </CardContent>
    </Card>
  );
}

export function LoggingSettingsCard({
  settings,
  dataDir,
  logsSizeText,
  logsSizeLoading,
  onRefreshLogsSize,
  onSaved,
  children,
}: LoggingSettingsCardProps) {
  const { t } = useI18n();
  const [saving, setSaving] = useState(false);
  const schema = useMemo(() => createLoggingSchema(t), [t]);
  const form = useForm<LoggingFormValues>({
    resolver: zodResolver(schema),
    defaultValues: loggingDefaults(settings),
  });

  useEffect(() => {
    form.reset(loggingDefaults(settings));
  }, [form, settings]);

  const submit = form.handleSubmit(
    async (values) => {
      setSaving(true);
      try {
        const next = await updateSettings({
          log_level: values.log_level,
          log_retention_days: Number.parseInt(values.log_retention_days, 10),
        });
        onSaved(next);
        setLogLevel(next.log_level);
        toast.success(t("settings.logging.saved"));
      } catch (e) {
        toast.error(t("settings.logging.saveFail"), {
          description: humanizeApiError(e, t),
        });
      } finally {
        setSaving(false);
      }
    },
    () => {
      toast.error(t("settings.logging.retentionInvalid"));
    },
  );

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <ScrollText className="h-4 w-4" />
          {t("settings.logging.title")}
        </CardTitle>
        <CardDescription>{t("settings.logging.subtitle")}</CardDescription>
      </CardHeader>
      <CardContent>
        <Form {...form}>
          <form onSubmit={submit} className="space-y-4">
            <FormField
              control={form.control}
              name="log_level"
              render={({ field }) => (
                <FormItem className="flex items-center justify-between gap-4">
                  <div>
                    <FormLabel className="font-medium text-sm">{t("settings.logging.level")}</FormLabel>
                    <UiFormDescription>{t("settings.logging.levelHint")}</UiFormDescription>
                  </div>
                  <div className="w-[180px]">
                    <Select value={field.value} onValueChange={field.onChange}>
                      <FormControl>
                        <SelectTrigger>
                          <SelectValue />
                        </SelectTrigger>
                      </FormControl>
                      <SelectContent>
                        <SelectItem value="none">{t("settings.logging.levelNone")}</SelectItem>
                        <SelectItem value="debug">{t("settings.logging.levelDebug")}</SelectItem>
                        <SelectItem value="info">{t("settings.logging.levelInfo")}</SelectItem>
                        <SelectItem value="warning">{t("settings.logging.levelWarning")}</SelectItem>
                        <SelectItem value="error">{t("settings.logging.levelError")}</SelectItem>
                      </SelectContent>
                    </Select>
                  </div>
                  <FormMessage />
                </FormItem>
              )}
            />

            <FormField
              control={form.control}
              name="log_retention_days"
              render={({ field }) => (
                <FormItem className="flex items-center justify-between gap-4">
                  <div>
                    <FormLabel className="font-medium text-sm">{t("settings.logging.retentionDays")}</FormLabel>
                    <UiFormDescription>{t("settings.logging.retentionHint")}</UiFormDescription>
                  </div>
                  <div className="w-[120px]">
                    <FormControl>
                      <Input {...field} type="number" min="1" max="3650" className="font-mono text-sm" />
                    </FormControl>
                  </div>
                  <FormMessage />
                </FormItem>
              )}
            />

            <div className="space-y-2">
              <label className="text-sm font-medium">{t("settings.logging.dir")}</label>
              <Input value={dataDir ?? "-"} disabled className="font-mono text-sm" />
            </div>

            <div className="space-y-2">
              <label className="text-sm font-medium">{t("settings.maintenance.logsSize")}</label>
              <div className="flex gap-2">
                <Input value={logsSizeText} disabled className="font-mono text-sm" />
                <Button type="button" variant="outline" onClick={() => void onRefreshLogsSize()} disabled={logsSizeLoading}>
                  {t("common.refresh")}
                </Button>
              </div>
            </div>

            <div className="flex justify-end">
              <Button size="sm" type="submit" disabled={!settings || saving}>
                {t("common.save")}
              </Button>
            </div>

          </form>
        </Form>
        {children}
      </CardContent>
    </Card>
  );
}

export function ServiceInfoSettingsCard({
  settings,
  apiHost,
  apiPort,
  onSaved,
}: ServiceInfoSettingsCardProps) {
  const { t } = useI18n();
  const [saving, setSaving] = useState(false);
  const schema = useMemo(() => createServiceInfoSchema(), []);
  const form = useForm<ServiceInfoFormValues>({
    resolver: zodResolver(schema),
    defaultValues: serviceInfoDefaults(settings),
  });

  useEffect(() => {
    form.reset(serviceInfoDefaults(settings));
  }, [form, settings]);

  const submit = form.handleSubmit(async (values) => {
    setSaving(true);
    try {
      const previous = settings?.server_lan_accessible ?? false;
      const next = await updateSettings({
        server_lan_accessible: values.server_lan_accessible,
      });
      onSaved(next);
      toast.success(t("settings.serviceInfo.saved"));
      if (previous !== values.server_lan_accessible) {
        postIpc({ type: "request-restart-backend" });
      }
    } catch (e) {
      toast.error(t("settings.serviceInfo.saveFail"), {
        description: humanizeApiError(e, t),
      });
    } finally {
      setSaving(false);
    }
  });

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <Database className="h-4 w-4" />
          {t("settings.serviceInfo.title")}
        </CardTitle>
        <CardDescription>{t("settings.serviceInfo.subtitle")}</CardDescription>
      </CardHeader>
      <CardContent>
        <Form {...form}>
          <form onSubmit={submit} className="space-y-4">
            <div className="grid grid-cols-2 gap-4">
              <div className="space-y-2">
                <label className="text-sm font-medium">{t("settings.serviceInfo.host")}</label>
                <Input value={apiHost} disabled />
                <p className="text-xs text-muted-foreground">{t("settings.serviceInfo.hostHint")}</p>
              </div>
              <div className="space-y-2">
                <label className="text-sm font-medium">{t("settings.serviceInfo.port")}</label>
                <Input value={apiPort} disabled />
                <p className="text-xs text-muted-foreground">{t("settings.serviceInfo.portHint")}</p>
              </div>
            </div>

            <FormField
              control={form.control}
              name="server_lan_accessible"
              render={({ field }) => (
                <FormItem className="flex flex-row items-center justify-between gap-4 space-y-0">
                  <div>
                    <FormLabel className="font-medium text-sm">{t("settings.serviceInfo.lanAccessible")}</FormLabel>
                    <UiFormDescription>{t("settings.serviceInfo.lanAccessibleHint")}</UiFormDescription>
                  </div>
                  <FormControl>
                    <Switch checked={field.value} onCheckedChange={field.onChange} disabled={!settings || saving} />
                  </FormControl>
                </FormItem>
              )}
            />

            <div className="flex justify-end">
              <Button size="sm" type="submit" disabled={!settings || saving}>
                {t("common.save")}
              </Button>
            </div>
          </form>
        </Form>
      </CardContent>
    </Card>
  );
}

export function CompatibilitySettingsCard({ settings, onSaved }: SettingsSectionProps) {
  const { t } = useI18n();
  const [saving, setSaving] = useState(false);
  const schema = useMemo(() => createCompatibilitySchema(), []);
  const form = useForm<CompatibilityFormValues>({
    resolver: zodResolver(schema),
    defaultValues: compatibilityDefaults(settings),
  });

  useEffect(() => {
    form.reset(compatibilityDefaults(settings));
  }, [form, settings]);

  const submit = form.handleSubmit(async (values) => {
    setSaving(true);
    try {
      const next = await updateSettings({
        anthropic_count_tokens_mock_enabled: values.anthropic_count_tokens_mock_enabled,
      });
      onSaved(next);
      toast.success(t("settings.compatibility.saved"));
    } catch (e) {
      toast.error(t("settings.compatibility.saveFail"), {
        description: humanizeApiError(e, t),
      });
    } finally {
      setSaving(false);
    }
  });

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <Info className="h-4 w-4" />
          {t("settings.compatibility.title")}
        </CardTitle>
        <CardDescription>{t("settings.compatibility.subtitle")}</CardDescription>
      </CardHeader>
      <CardContent>
        <Form {...form}>
          <form onSubmit={submit} className="space-y-4">
            <FormField
              control={form.control}
              name="anthropic_count_tokens_mock_enabled"
              render={({ field }) => (
                <FormItem className="flex flex-row items-center justify-between gap-4 space-y-0">
                  <div>
                    <FormLabel className="font-medium text-sm">{t("settings.compatibility.mockCountTokens")}</FormLabel>
                    <UiFormDescription>{t("settings.compatibility.mockCountTokensHint")}</UiFormDescription>
                  </div>
                  <FormControl>
                    <Switch checked={field.value} onCheckedChange={field.onChange} disabled={!settings || saving} />
                  </FormControl>
                </FormItem>
              )}
            />

            <div className="flex justify-end">
              <Button size="sm" type="submit" disabled={!settings || saving}>
                {t("common.save")}
              </Button>
            </div>
          </form>
        </Form>
      </CardContent>
    </Card>
  );
}

export function AppUpdateSettingsCard({
  settings,
  dialog,
  updateStatusText,
  updateServerVersion,
  updateChecking,
  onCheck,
  onAutoUpdateChange,
}: AppUpdateSettingsCardProps) {
  const { t } = useI18n();
  const [saving, setSaving] = useState(false);
  const schema = useMemo(() => createAppUpdateSchema(), []);
  const form = useForm<AppUpdateFormValues>({
    resolver: zodResolver(schema),
    defaultValues: appUpdateDefaults(settings),
  });

  useEffect(() => {
    form.reset(appUpdateDefaults(settings));
  }, [form, settings]);

  const submit = form.handleSubmit(async (values) => {
    setSaving(true);
    try {
      await onAutoUpdateChange(values.app_auto_update_enabled);
    } finally {
      setSaving(false);
    }
  });

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <RefreshCw className="h-4 w-4" />
          {t("settings.update.title")}
        </CardTitle>
        <CardDescription>{t("settings.update.subtitle")}</CardDescription>
      </CardHeader>
      <CardContent>
        {dialog}
        <Form {...form}>
          <form onSubmit={submit} className="space-y-4">
            <FormField
              control={form.control}
              name="app_auto_update_enabled"
              render={({ field }) => (
                <FormItem className="flex flex-row items-center justify-between gap-4 space-y-0">
                  <div>
                    <FormLabel className="font-medium text-sm">{t("settings.update.autoEnable")}</FormLabel>
                    <UiFormDescription>{t("settings.update.autoEnableHint")}</UiFormDescription>
                  </div>
                  <FormControl>
                    <Switch checked={field.value} onCheckedChange={field.onChange} disabled={!settings || saving} />
                  </FormControl>
                </FormItem>
              )}
            />

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
                <Button type="button" size="sm" variant="outline" onClick={() => void onCheck()} disabled={updateChecking}>
                  {t("settings.update.check")}
                </Button>
                <Button size="sm" type="submit" disabled={!settings || saving}>
                  {t("common.save")}
                </Button>
              </div>
            </div>
          </form>
        </Form>
      </CardContent>
    </Card>
  );
}

export function ChatBridgeBaseSettingsCard({ settings, onSaved }: ChatBridgeBaseSettingsCardProps) {
  const { t } = useI18n();
  const [saving, setSaving] = useState(false);
  const schema = useMemo(() => createChatBridgeBaseSchema(t), [t]);
  const form = useForm<ChatBridgeBaseFormValues>({
    resolver: zodResolver(schema),
    defaultValues: chatBridgeBaseDefaults(settings),
  });

  useEffect(() => {
    form.reset(chatBridgeBaseDefaults(settings));
  }, [form, settings]);

  const submit = form.handleSubmit(
    async (values) => {
      setSaving(true);
      try {
        const next = await updateSettings({
          chat_bridge_enabled: values.chat_bridge_enabled,
          chat_bridge_allow_new_projects: values.chat_bridge_allow_new_projects,
          chat_bridge_turn_timeout_minutes: Number.parseInt(values.chat_bridge_turn_timeout_minutes, 10),
        });
        onSaved(next);
        toast.success(t("settings.chatBridge.saved"));
      } catch (e) {
        toast.error(t("settings.chatBridge.saveFail"), {
          description: humanizeApiError(e, t),
        });
      } finally {
        setSaving(false);
      }
    },
    () => {
      toast.error(t("settings.chatBridge.turnTimeoutInvalid"));
    },
  );

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <Bot className="h-4 w-4" />
          {t("settings.chatBridge.configTitle")}
        </CardTitle>
        <CardDescription>{t("settings.chatBridge.configHint")}</CardDescription>
      </CardHeader>
      <CardContent>
        <Form {...form}>
          <form onSubmit={submit} className="space-y-4">
            <FormField
              control={form.control}
              name="chat_bridge_enabled"
              render={({ field }) => (
                <FormItem className="flex flex-row items-center justify-between gap-4 space-y-0">
                  <div>
                    <FormLabel className="font-medium text-sm">{t("settings.chatBridge.enable")}</FormLabel>
                    <UiFormDescription>{t("settings.chatBridge.enableHint")}</UiFormDescription>
                  </div>
                  <FormControl>
                    <Switch checked={field.value} onCheckedChange={field.onChange} disabled={!settings || saving} />
                  </FormControl>
                </FormItem>
              )}
            />

            <FormField
              control={form.control}
              name="chat_bridge_allow_new_projects"
              render={({ field }) => (
                <FormItem className="flex flex-row items-center justify-between gap-4 space-y-0">
                  <div>
                    <FormLabel className="font-medium text-sm">{t("settings.chatBridge.allowNewProjects")}</FormLabel>
                    <UiFormDescription>{t("settings.chatBridge.allowNewProjectsHint")}</UiFormDescription>
                  </div>
                  <FormControl>
                    <Switch checked={field.value} onCheckedChange={field.onChange} disabled={!settings || saving} />
                  </FormControl>
                </FormItem>
              )}
            />

            <FormField
              control={form.control}
              name="chat_bridge_turn_timeout_minutes"
              render={({ field }) => (
                <FormItem className="flex items-center justify-between gap-4">
                  <div>
                    <FormLabel className="font-medium text-sm">{t("settings.chatBridge.turnTimeoutMinutes")}</FormLabel>
                    <UiFormDescription>{t("settings.chatBridge.turnTimeoutMinutesHint")}</UiFormDescription>
                  </div>
                  <div className="w-[140px]">
                    <FormControl>
                      <Input {...field} type="number" min="0" className="h-8" disabled={!settings || saving} />
                    </FormControl>
                  </div>
                  <FormMessage />
                </FormItem>
              )}
            />

            <div className="flex justify-end">
              <Button size="sm" type="submit" disabled={!settings || saving}>
                {t("common.save")}
              </Button>
            </div>
          </form>
        </Form>
      </CardContent>
    </Card>
  );
}

export function ChatBridgeTelegramSettingsCard({
  settings,
  bindingCount,
  tokenConfigured,
  onOpenPairing,
  onOpenBindings,
  onSaved,
}: ChatBridgeTelegramSettingsCardProps) {
  const { t } = useI18n();
  const [saving, setSaving] = useState(false);
  const schema = useMemo(() => createChatBridgeTelegramSchema(), []);
  const form = useForm<ChatBridgeTelegramFormValues>({
    resolver: zodResolver(schema),
    defaultValues: chatBridgeTelegramDefaults(settings),
  });

  useEffect(() => {
    form.reset(chatBridgeTelegramDefaults(settings));
  }, [form, settings]);

  const submit = form.handleSubmit(async (values) => {
    setSaving(true);
    try {
      const patch: Partial<AppSettings> = {
        chat_bridge_telegram_enabled: values.chat_bridge_telegram_enabled,
      };
      if (values.chat_bridge_telegram_bot_token.trim()) {
        patch.chat_bridge_telegram_bot_token = values.chat_bridge_telegram_bot_token.trim();
      }
      const next = await updateSettings(patch);
      onSaved(next);
      form.reset(chatBridgeTelegramDefaults(next));
      toast.success(t("settings.chatBridge.saved"));
    } catch (e) {
      toast.error(t("settings.chatBridge.saveFail"), {
        description: humanizeApiError(e, t),
      });
    } finally {
      setSaving(false);
    }
  });

  return (
    <div className="rounded-lg border p-4 space-y-4">
      <Form {...form}>
        <form onSubmit={submit} className="space-y-4">
          <FormField
            control={form.control}
            name="chat_bridge_telegram_enabled"
            render={({ field }) => (
              <FormItem className="flex flex-row items-center justify-between gap-4 space-y-0">
                <div>
                  <FormLabel className="font-medium text-sm">{t("settings.chatBridge.telegramEnable")}</FormLabel>
                  <UiFormDescription>{t("settings.chatBridge.telegramEnableHint")}</UiFormDescription>
                </div>
                <FormControl>
                  <Switch checked={field.value} onCheckedChange={field.onChange} disabled={!settings || saving} />
                </FormControl>
              </FormItem>
            )}
          />

          <FormField
            control={form.control}
            name="chat_bridge_telegram_bot_token"
            render={({ field }) => (
              <FormItem>
                <FormLabel>{t("settings.chatBridge.telegramToken")}</FormLabel>
                <FormControl>
                  <Input
                    {...field}
                    type="password"
                    autoComplete="new-password"
                    placeholder={tokenConfigured
                      ? t("settings.chatBridge.telegramTokenPlaceholderConfigured")
                      : t("settings.chatBridge.telegramTokenPlaceholder")}
                    disabled={!settings || saving}
                  />
                </FormControl>
                <UiFormDescription>
                  {tokenConfigured
                    ? t("settings.chatBridge.telegramTokenHintConfigured")
                    : t("settings.chatBridge.telegramTokenHint")}
                </UiFormDescription>
                <FormMessage />
              </FormItem>
            )}
          />

          <div className="flex items-center justify-between gap-4 rounded-lg border bg-muted/30 px-3 py-3">
            <div className="space-y-1">
              <div className="font-medium text-sm">{t("settings.chatBridge.bindings.title")}</div>
              <div className="text-xs text-muted-foreground">
                {t("settings.chatBridge.bindings.summary", { count: bindingCount })}
              </div>
            </div>
            <div className="flex items-center gap-2">
              <Button type="button" size="sm" variant="outline" onClick={onOpenPairing}>
                {t("settings.chatBridge.actions.pairing")}
              </Button>
              <Button type="button" size="sm" variant="outline" onClick={onOpenBindings}>
                {t("settings.chatBridge.actions.bindings")}
              </Button>
            </div>
          </div>

          <div className="flex justify-end">
            <Button size="sm" type="submit" disabled={!settings || saving}>
              {t("common.save")}
            </Button>
          </div>
        </form>
      </Form>
    </div>
  );
}

export function ChatBridgeDiscordSettingsCard({
  settings,
  bindingCount,
  tokenConfigured,
  onOpenPairing,
  onOpenBindings,
  onSaved,
}: ChatBridgeDiscordSettingsCardProps) {
  const { t } = useI18n();
  const [saving, setSaving] = useState(false);
  const schema = useMemo(() => createChatBridgeDiscordSchema(), []);
  const form = useForm<ChatBridgeDiscordFormValues>({
    resolver: zodResolver(schema),
    defaultValues: chatBridgeDiscordDefaults(settings),
  });

  useEffect(() => {
    form.reset(chatBridgeDiscordDefaults(settings));
  }, [form, settings]);

  const submit = form.handleSubmit(async (values) => {
    setSaving(true);
    try {
      const patch: Partial<AppSettings> = {
        chat_bridge_discord_enabled: values.chat_bridge_discord_enabled,
      };
      if (values.chat_bridge_discord_bot_token.trim()) {
        patch.chat_bridge_discord_bot_token = values.chat_bridge_discord_bot_token.trim();
      }
      const next = await updateSettings(patch);
      onSaved(next);
      form.reset(chatBridgeDiscordDefaults(next));
      toast.success(t("settings.chatBridge.saved"));
    } catch (e) {
      toast.error(t("settings.chatBridge.saveFail"), {
        description: humanizeApiError(e, t),
      });
    } finally {
      setSaving(false);
    }
  });

  return (
    <div className="rounded-lg border p-4 space-y-4">
      <Form {...form}>
        <form onSubmit={submit} className="space-y-4">
          <FormField
            control={form.control}
            name="chat_bridge_discord_enabled"
            render={({ field }) => (
              <FormItem className="flex flex-row items-center justify-between gap-4 space-y-0">
                <div>
                  <FormLabel className="font-medium text-sm">{t("settings.chatBridge.discordEnable")}</FormLabel>
                  <UiFormDescription>{t("settings.chatBridge.discordEnableHint")}</UiFormDescription>
                </div>
                <FormControl>
                  <Switch checked={field.value} onCheckedChange={field.onChange} disabled={!settings || saving} />
                </FormControl>
              </FormItem>
            )}
          />

          <FormField
            control={form.control}
            name="chat_bridge_discord_bot_token"
            render={({ field }) => (
              <FormItem>
                <FormLabel>{t("settings.chatBridge.discordToken")}</FormLabel>
                <FormControl>
                  <Input
                    {...field}
                    type="password"
                    autoComplete="new-password"
                    placeholder={tokenConfigured
                      ? t("settings.chatBridge.discordTokenPlaceholderConfigured")
                      : t("settings.chatBridge.discordTokenPlaceholder")}
                    disabled={!settings || saving}
                  />
                </FormControl>
                <UiFormDescription>
                  {tokenConfigured
                    ? t("settings.chatBridge.discordTokenHintConfigured")
                    : t("settings.chatBridge.discordTokenHint")}
                </UiFormDescription>
                <FormMessage />
              </FormItem>
            )}
          />

          <div className="flex items-center justify-between gap-4 rounded-lg border bg-muted/30 px-3 py-3">
            <div className="space-y-1">
              <div className="font-medium text-sm">{t("settings.chatBridge.bindings.title")}</div>
              <div className="text-xs text-muted-foreground">
                {t("settings.chatBridge.bindings.summary", { count: bindingCount })}
              </div>
            </div>
            <div className="flex items-center gap-2">
              <Button type="button" size="sm" variant="outline" onClick={onOpenPairing}>
                {t("settings.chatBridge.actions.pairing")}
              </Button>
              <Button type="button" size="sm" variant="outline" onClick={onOpenBindings}>
                {t("settings.chatBridge.actions.bindings")}
              </Button>
            </div>
          </div>

          <div className="flex justify-end">
            <Button size="sm" type="submit" disabled={!settings || saving}>
              {t("common.save")}
            </Button>
          </div>
        </form>
      </Form>
    </div>
  );
}

export function ChatBridgeWhatsAppSettingsCard({
  settings,
  bindingCount,
  statusTone,
  statusLabel,
  actionBusy,
  onOpenLoginDialog,
  onOpenPairing,
  onOpenBindings,
  onSaved,
}: ChatBridgeWhatsAppSettingsCardProps) {
  const { t } = useI18n();
  const [saving, setSaving] = useState(false);
  const schema = useMemo(() => createChatBridgeWhatsAppSchema(), []);
  const form = useForm<ChatBridgeWhatsAppFormValues>({
    resolver: zodResolver(schema),
    defaultValues: chatBridgeWhatsAppDefaults(settings),
  });

  useEffect(() => {
    form.reset(chatBridgeWhatsAppDefaults(settings));
  }, [form, settings]);

  const submit = form.handleSubmit(async (values) => {
    setSaving(true);
    try {
      const next = await updateSettings({
        chat_bridge_whatsapp_enabled: values.chat_bridge_whatsapp_enabled,
      });
      onSaved(next);
      toast.success(t("settings.chatBridge.saved"));
    } catch (e) {
      toast.error(t("settings.chatBridge.saveFail"), {
        description: humanizeApiError(e, t),
      });
    } finally {
      setSaving(false);
    }
  });

  return (
    <div className="rounded-lg border p-4 space-y-4">
      <Form {...form}>
        <form onSubmit={submit} className="space-y-4">
          <FormField
            control={form.control}
            name="chat_bridge_whatsapp_enabled"
            render={({ field }) => (
              <FormItem className="flex flex-row items-center justify-between gap-4 space-y-0">
                <div>
                  <FormLabel className="font-medium text-sm">{t("settings.chatBridge.whatsappEnable")}</FormLabel>
                  <UiFormDescription>{t("settings.chatBridge.whatsappEnableHint")}</UiFormDescription>
                </div>
                <FormControl>
                  <Switch checked={field.value} onCheckedChange={field.onChange} disabled={!settings || saving} />
                </FormControl>
              </FormItem>
            )}
          />

          <div className="rounded-lg border bg-muted/20 px-4 py-3 space-y-3">
            <div className="flex items-center justify-between gap-4">
              <div className="flex items-center gap-2">
                <div className="font-medium text-sm">{t("settings.chatBridge.whatsapp.qrTitle")}</div>
                <Badge variant={statusTone} className="w-fit">
                  {statusLabel}
                </Badge>
              </div>
              <Button type="button" size="sm" onClick={onOpenLoginDialog} disabled={actionBusy}>
                {t("settings.chatBridge.whatsapp.dialogAction")}
              </Button>
            </div>
          </div>

          <div className="flex items-center justify-between gap-4 rounded-lg border bg-muted/30 px-3 py-3">
            <div className="space-y-1">
              <div className="font-medium text-sm">{t("settings.chatBridge.bindings.title")}</div>
              <div className="text-xs text-muted-foreground">
                {t("settings.chatBridge.bindings.summary", { count: bindingCount })}
              </div>
            </div>
            <div className="flex items-center gap-2">
              <Button type="button" size="sm" variant="outline" onClick={onOpenPairing}>
                {t("settings.chatBridge.actions.pairing")}
              </Button>
              <Button type="button" size="sm" variant="outline" onClick={onOpenBindings}>
                {t("settings.chatBridge.actions.bindings")}
              </Button>
            </div>
          </div>

          <div className="flex justify-end">
            <Button size="sm" type="submit" disabled={!settings || saving}>
              {t("common.save")}
            </Button>
          </div>
        </form>
      </Form>
    </div>
  );
}

export function ChatBridgeWeixinSettingsCard({
  settings,
  bindingCount,
  statusTone,
  statusLabel,
  actionBusy,
  onOpenLoginDialog,
  onOpenPairing,
  onOpenBindings,
  onSaved,
}: ChatBridgeWeixinSettingsCardProps) {
  const { t } = useI18n();
  const [saving, setSaving] = useState(false);
  const schema = useMemo(() => createChatBridgeWeixinSchema(), []);
  const form = useForm<ChatBridgeWeixinFormValues>({
    resolver: zodResolver(schema),
    defaultValues: chatBridgeWeixinDefaults(settings),
  });

  useEffect(() => {
    form.reset(chatBridgeWeixinDefaults(settings));
  }, [form, settings]);

  const submit = form.handleSubmit(async (values) => {
    setSaving(true);
    try {
      const next = await updateSettings({
        chat_bridge_weixin_enabled: values.chat_bridge_weixin_enabled,
      });
      onSaved(next);
      toast.success(t("settings.chatBridge.saved"));
    } catch (e) {
      toast.error(t("settings.chatBridge.saveFail"), {
        description: humanizeApiError(e, t),
      });
    } finally {
      setSaving(false);
    }
  });

  return (
    <div className="rounded-lg border p-4 space-y-4">
      <Form {...form}>
        <form onSubmit={submit} className="space-y-4">
          <FormField
            control={form.control}
            name="chat_bridge_weixin_enabled"
            render={({ field }) => (
              <FormItem className="flex flex-row items-center justify-between gap-4 space-y-0">
                <div>
                  <FormLabel className="font-medium text-sm">{t("settings.chatBridge.weixinEnable")}</FormLabel>
                  <UiFormDescription>{t("settings.chatBridge.weixinEnableHint")}</UiFormDescription>
                </div>
                <FormControl>
                  <Switch checked={field.value} onCheckedChange={field.onChange} disabled={!settings || saving} />
                </FormControl>
              </FormItem>
            )}
          />

          <div className="rounded-lg border bg-muted/20 px-4 py-3 space-y-3">
            <div className="flex items-center justify-between gap-4">
              <div className="flex items-center gap-2">
                <div className="font-medium text-sm">{t("settings.chatBridge.weixin.qrTitle")}</div>
                <Badge variant={statusTone} className="w-fit">
                  {statusLabel}
                </Badge>
              </div>
              <Button type="button" size="sm" onClick={onOpenLoginDialog} disabled={actionBusy}>
                {t("settings.chatBridge.weixin.dialogAction")}
              </Button>
            </div>
          </div>

          <div className="flex items-center justify-between gap-4 rounded-lg border bg-muted/30 px-3 py-3">
            <div className="space-y-1">
              <div className="font-medium text-sm">{t("settings.chatBridge.bindings.title")}</div>
              <div className="text-xs text-muted-foreground">
                {t("settings.chatBridge.bindings.summary", { count: bindingCount })}
              </div>
            </div>
            <div className="flex items-center gap-2">
              <Button type="button" size="sm" variant="outline" onClick={onOpenPairing}>
                {t("settings.chatBridge.actions.pairing")}
              </Button>
              <Button type="button" size="sm" variant="outline" onClick={onOpenBindings}>
                {t("settings.chatBridge.actions.bindings")}
              </Button>
            </div>
          </div>

          <div className="flex justify-end">
            <Button size="sm" type="submit" disabled={!settings || saving}>
              {t("common.save")}
            </Button>
          </div>
        </form>
      </Form>
    </div>
  );
}
