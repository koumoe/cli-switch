import { useEffect, useMemo, useState, type ReactNode } from "react";
import { zodResolver } from "@hookform/resolvers/zod";
import { useForm } from "react-hook-form";
import { RefreshCw } from "lucide-react";
import { toast } from "sonner";

import { updateSettings } from "@/api";
import {
  Badge,
  Button,
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
import { SettingsFieldText, SettingsFooter, SettingsRow, SettingsSection } from "./settings-layout";

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

const settingsDescriptionClassName =
  "mt-0.5 text-[10.5px] leading-snug text-muted-foreground";

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
    <Form {...form}>
      <form onSubmit={submit}>
        <SettingsSection title={t("settings.channelProtection.title")} first>
          <FormField
            control={form.control}
            name="auto_disable_enabled"
            render={({ field }) => (
              <FormItem className="space-y-0">
                <SettingsRow>
                  <SettingsFieldText
                    label={
                      <FormLabel className="text-[12.5px] font-semibold">
                        {t("settings.channelProtection.enable")}
                      </FormLabel>
                    }
                    hint={
                      <UiFormDescription className={settingsDescriptionClassName}>
                        {t("settings.channelProtection.enableHint")}
                      </UiFormDescription>
                    }
                  />
                  <FormControl>
                    <Switch checked={field.value} onCheckedChange={field.onChange} disabled={!settings} />
                  </FormControl>
                </SettingsRow>
              </FormItem>
            )}
          />

          <FormField
            control={form.control}
            name="auto_disable_window_minutes"
            render={({ field }) => (
              <FormItem className="space-y-0">
                <SettingsRow className="items-start">
                  <SettingsFieldText
                    label={
                      <FormLabel className="text-[12.5px] font-semibold">
                        {t("settings.channelProtection.windowMinutes")}
                      </FormLabel>
                    }
                    hint={
                      <UiFormDescription className={settingsDescriptionClassName}>
                        {t("settings.channelProtection.windowMinutesHint")}
                      </UiFormDescription>
                    }
                  />
                  <div className="w-[110px] shrink-0">
                    <FormControl>
                      <Input {...field} type="number" min="1" disabled={!settings || !enabled} />
                    </FormControl>
                    <FormMessage className="mt-1 text-[10.5px]" />
                  </div>
                </SettingsRow>
              </FormItem>
            )}
          />

          <FormField
            control={form.control}
            name="auto_disable_failure_times"
            render={({ field }) => (
              <FormItem className="space-y-0">
                <SettingsRow className="items-start">
                  <SettingsFieldText
                    label={
                      <FormLabel className="text-[12.5px] font-semibold">
                        {t("settings.channelProtection.failureTimes")}
                      </FormLabel>
                    }
                    hint={
                      <UiFormDescription className={settingsDescriptionClassName}>
                        {t("settings.channelProtection.failureTimesHint")}
                      </UiFormDescription>
                    }
                  />
                  <div className="w-[110px] shrink-0">
                    <FormControl>
                      <Input {...field} type="number" min="1" disabled={!settings || !enabled} />
                    </FormControl>
                    <FormMessage className="mt-1 text-[10.5px]" />
                  </div>
                </SettingsRow>
              </FormItem>
            )}
          />

          <FormField
            control={form.control}
            name="auto_disable_disable_minutes"
            render={({ field }) => (
              <FormItem className="space-y-0">
                <SettingsRow className="items-start">
                  <SettingsFieldText
                    label={
                      <FormLabel className="text-[12.5px] font-semibold">
                        {t("settings.channelProtection.pauseMinutes")}
                      </FormLabel>
                    }
                    hint={
                      <UiFormDescription className={settingsDescriptionClassName}>
                        {t("settings.channelProtection.pauseMinutesHint")}
                      </UiFormDescription>
                    }
                  />
                  <div className="w-[110px] shrink-0">
                    <FormControl>
                      <Input {...field} type="number" min="1" disabled={!settings || !enabled} />
                    </FormControl>
                    <FormMessage className="mt-1 text-[10.5px]" />
                  </div>
                </SettingsRow>
              </FormItem>
            )}
          />

          <SettingsFooter>
            <Button size="sm" type="submit" disabled={!settings || saving}>
              {t("common.save")}
            </Button>
          </SettingsFooter>
        </SettingsSection>
      </form>
    </Form>
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
    <Form {...form}>
      <form onSubmit={submit}>
        <SettingsSection title={t("settings.channelRetry.title")}>
          <FormField
            control={form.control}
            name="channel_retry_enabled"
            render={({ field }) => (
              <FormItem className="space-y-0">
                <SettingsRow>
                  <SettingsFieldText
                    label={
                      <FormLabel className="text-[12.5px] font-semibold">
                        {t("settings.channelRetry.enable")}
                      </FormLabel>
                    }
                    hint={
                      <UiFormDescription className={settingsDescriptionClassName}>
                        {t("settings.channelRetry.enableHint")}
                      </UiFormDescription>
                    }
                  />
                  <FormControl>
                    <Switch checked={field.value} onCheckedChange={field.onChange} disabled={!settings} />
                  </FormControl>
                </SettingsRow>
              </FormItem>
            )}
          />

          <SettingsFooter>
            <Button size="sm" type="submit" disabled={!settings || saving}>
              {t("common.save")}
            </Button>
          </SettingsFooter>
        </SettingsSection>
      </form>
    </Form>
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
    <Form {...form}>
      <form onSubmit={submit}>
        <SettingsSection title={t("settings.newApiManaged.title")}>
          <FormField
            control={form.control}
            name="remote_managed_channel_missing_prompt_enabled"
            render={({ field }) => (
              <FormItem className="space-y-0">
                <SettingsRow>
                  <SettingsFieldText
                    label={
                      <FormLabel className="text-[12.5px] font-semibold">
                        {t("settings.newApiManaged.missingPrompt")}
                      </FormLabel>
                    }
                    hint={
                      <UiFormDescription className={settingsDescriptionClassName}>
                        {t("settings.newApiManaged.missingPromptHint")}
                      </UiFormDescription>
                    }
                  />
                  <FormControl>
                    <Switch checked={field.value} onCheckedChange={field.onChange} disabled={!settings} />
                  </FormControl>
                </SettingsRow>
              </FormItem>
            )}
          />

          <FormField
            control={form.control}
            name="remote_managed_channel_sync_multiplier_enabled"
            render={({ field }) => (
              <FormItem className="space-y-0">
                <SettingsRow>
                  <SettingsFieldText
                    label={
                      <FormLabel className="text-[12.5px] font-semibold">
                        {t("settings.newApiManaged.syncMultiplier")}
                      </FormLabel>
                    }
                    hint={
                      <UiFormDescription className={settingsDescriptionClassName}>
                        {t("settings.newApiManaged.syncMultiplierHint")}
                      </UiFormDescription>
                    }
                  />
                  <FormControl>
                    <Switch checked={field.value} onCheckedChange={field.onChange} disabled={!settings} />
                  </FormControl>
                </SettingsRow>
              </FormItem>
            )}
          />

          {syncMultiplierEnabled ? (
            <FormField
              control={form.control}
              name="remote_managed_channel_sync_free_multiplier_enabled"
              render={({ field }) => (
                <FormItem className="space-y-0">
                  <SettingsRow>
                    <SettingsFieldText
                      label={
                        <FormLabel className="text-[12.5px] font-semibold">
                          {t("settings.newApiManaged.ignoreFreeMultiplier")}
                        </FormLabel>
                      }
                      hint={
                        <UiFormDescription className={settingsDescriptionClassName}>
                          {t("settings.newApiManaged.ignoreFreeMultiplierHint")}
                        </UiFormDescription>
                      }
                    />
                    <FormControl>
                      <Switch checked={field.value} onCheckedChange={field.onChange} disabled={!settings} />
                    </FormControl>
                  </SettingsRow>
                </FormItem>
              )}
            />
          ) : null}

          <SettingsFooter>
            <Button size="sm" type="submit" disabled={!settings || saving}>
              {t("common.save")}
            </Button>
          </SettingsFooter>
        </SettingsSection>
      </form>
    </Form>
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
    <Form {...form}>
      <form onSubmit={submit}>
        <SettingsSection title={t("settings.systemNotifications.title")} first>
          <FormField
            control={form.control}
            name="system_notifications_enabled"
            render={({ field }) => (
              <FormItem className="space-y-0">
                <SettingsRow>
                  <SettingsFieldText
                    label={
                      <FormLabel className="text-[12.5px] font-semibold">
                        {t("settings.systemNotifications.enable")}
                      </FormLabel>
                    }
                    hint={
                      <UiFormDescription className={settingsDescriptionClassName}>
                        {t("settings.systemNotifications.enableHint")}
                      </UiFormDescription>
                    }
                  />
                  <FormControl>
                    <Switch checked={field.value} onCheckedChange={field.onChange} disabled={!settings || saving} />
                  </FormControl>
                </SettingsRow>
              </FormItem>
            )}
          />

          <SettingsFooter>
            <Button size="sm" type="submit" disabled={!settings || saving}>
              {t("common.save")}
            </Button>
          </SettingsFooter>
        </SettingsSection>
      </form>
    </Form>
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
    <Form {...form}>
      <form onSubmit={submit}>
        <SettingsSection title={t("settings.newApiManaged.title")}>
          <FormField
            control={form.control}
            name="remote_low_balance_system_notification_enabled"
            render={({ field }) => (
              <FormItem className="space-y-0">
                <SettingsRow>
                  <SettingsFieldText
                    label={
                      <FormLabel className="text-[12.5px] font-semibold">
                        {t("settings.systemNotifications.lowBalance")}
                      </FormLabel>
                    }
                    hint={
                      <UiFormDescription className={settingsDescriptionClassName}>
                        {t("settings.systemNotifications.lowBalanceHint")}
                      </UiFormDescription>
                    }
                  />
                  <FormControl>
                    <Switch checked={field.value} onCheckedChange={field.onChange} disabled={!settings || saving} />
                  </FormControl>
                </SettingsRow>
              </FormItem>
            )}
          />

          <FormField
            control={form.control}
            name="remote_managed_channel_missing_system_notification_enabled"
            render={({ field }) => (
              <FormItem className="space-y-0">
                <SettingsRow>
                  <SettingsFieldText
                    label={
                      <FormLabel className="text-[12.5px] font-semibold">
                        {t("settings.systemNotifications.managedChannelMissing")}
                      </FormLabel>
                    }
                    hint={
                      <UiFormDescription className={settingsDescriptionClassName}>
                        {t("settings.systemNotifications.managedChannelMissingHint")}
                      </UiFormDescription>
                    }
                  />
                  <FormControl>
                    <Switch checked={field.value} onCheckedChange={field.onChange} disabled={!settings || saving} />
                  </FormControl>
                </SettingsRow>
              </FormItem>
            )}
          />

          <FormField
            control={form.control}
            name="remote_managed_channel_multiplier_system_notification_enabled"
            render={({ field }) => (
              <FormItem className="space-y-0">
                <SettingsRow>
                  <SettingsFieldText
                    label={
                      <FormLabel className="text-[12.5px] font-semibold">
                        {t("settings.systemNotifications.managedChannelMultiplier")}
                      </FormLabel>
                    }
                    hint={
                      <UiFormDescription className={settingsDescriptionClassName}>
                        {t("settings.systemNotifications.managedChannelMultiplierHint")}
                      </UiFormDescription>
                    }
                  />
                  <FormControl>
                    <Switch checked={field.value} onCheckedChange={field.onChange} disabled={!settings || saving} />
                  </FormControl>
                </SettingsRow>
              </FormItem>
            )}
          />

          <FormField
            control={form.control}
            name="remote_group_added_system_notification_enabled"
            render={({ field }) => (
              <FormItem className="space-y-0">
                <SettingsRow>
                  <SettingsFieldText
                    label={
                      <FormLabel className="text-[12.5px] font-semibold">
                        {t("settings.systemNotifications.remoteGroupAdded")}
                      </FormLabel>
                    }
                    hint={
                      <UiFormDescription className={settingsDescriptionClassName}>
                        {t("settings.systemNotifications.remoteGroupAddedHint")}
                      </UiFormDescription>
                    }
                  />
                  <FormControl>
                    <Switch checked={field.value} onCheckedChange={field.onChange} disabled={!settings || saving} />
                  </FormControl>
                </SettingsRow>
              </FormItem>
            )}
          />

          <SettingsFooter>
            <Button size="sm" type="submit" disabled={!settings || saving}>
              {t("common.save")}
            </Button>
          </SettingsFooter>
        </SettingsSection>
      </form>
    </Form>
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
    <Form {...form}>
      <form onSubmit={submit}>
        <SettingsSection title={t("settings.pricingData.title")}>
          <SettingsRow>
            <SettingsFieldText
              label={t("settings.pricingData.status")}
              hint={
                <>
                  {t("settings.pricingData.count", { count: pricing?.count ?? 0 })}
                  {" · "}
                  {t("settings.pricingData.lastSync", {
                    time: pricing?.last_sync_ms ? new Date(pricing.last_sync_ms).toLocaleString() : "-",
                  })}
                </>
              }
            />
            <Button
              type="button"
              size="sm"
              variant="outline"
              onClick={() => void onSync()}
              disabled={syncing}
              className="gap-1.5"
            >
              <RefreshCw className={`h-3.5 w-3.5 ${syncing ? "animate-spin" : ""}`} />
              {t("settings.pricingData.sync")}
            </Button>
          </SettingsRow>

          <FormField
            control={form.control}
            name="pricing_auto_update_enabled"
            render={({ field }) => (
              <FormItem className="space-y-0">
                <SettingsRow>
                  <SettingsFieldText
                    label={
                      <FormLabel className="text-[12.5px] font-semibold">
                        {t("settings.pricingData.autoUpdate")}
                      </FormLabel>
                    }
                    hint={
                      <UiFormDescription className={settingsDescriptionClassName}>
                        {t("settings.pricingData.autoUpdateHint")}
                      </UiFormDescription>
                    }
                  />
                  <FormControl>
                    <Switch checked={field.value} onCheckedChange={field.onChange} disabled={!settings || saving} />
                  </FormControl>
                </SettingsRow>
              </FormItem>
            )}
          />

          <FormField
            control={form.control}
            name="pricing_auto_update_interval_hours"
            render={({ field }) => (
              <FormItem className="space-y-0">
                <SettingsRow className="items-start">
                  <SettingsFieldText
                    label={
                      <FormLabel className="text-[12.5px] font-semibold">
                        {t("settings.pricingData.intervalHours")}
                      </FormLabel>
                    }
                    hint={
                      <UiFormDescription className={settingsDescriptionClassName}>
                        {t("settings.pricingData.intervalHoursHint")}
                      </UiFormDescription>
                    }
                  />
                  <div className="w-[140px] shrink-0">
                    <FormControl>
                      <Input
                        {...field}
                        type="number"
                        min="1"
                        max="8760"
                        disabled={!settings || saving || !enabled}
                      />
                    </FormControl>
                    <FormMessage className="mt-1 text-[10.5px]" />
                  </div>
                </SettingsRow>
              </FormItem>
            )}
          />

          <SettingsFooter>
            <Button size="sm" type="submit" disabled={!settings || saving}>
              {t("common.save")}
            </Button>
          </SettingsFooter>
        </SettingsSection>
      </form>
    </Form>
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
    <Form {...form}>
      <form onSubmit={submit}>
        <SettingsSection title={t("settings.windowClose.title")}>
          <FormField
            control={form.control}
            name="close_behavior"
            render={({ field }) => (
              <FormItem className="space-y-0">
                <SettingsRow className="items-start">
                  <SettingsFieldText
                    label={
                      <FormLabel className="text-[12.5px] font-semibold">
                        {t("settings.windowClose.behavior")}
                      </FormLabel>
                    }
                    hint={
                      <UiFormDescription className={settingsDescriptionClassName}>
                        {t("settings.windowClose.behaviorHint")}
                      </UiFormDescription>
                    }
                  />
                  <div className="w-[220px] shrink-0">
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
                    <FormMessage className="mt-1 text-[10.5px]" />
                  </div>
                </SettingsRow>
              </FormItem>
            )}
          />

          <SettingsFooter>
            <Button size="sm" type="submit" disabled={!settings || saving}>
              {t("common.save")}
            </Button>
          </SettingsFooter>
        </SettingsSection>
      </form>
    </Form>
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
    <Form {...form}>
      <form onSubmit={submit}>
        <SettingsSection title={t("settings.startup.title")}>
          <FormField
            control={form.control}
            name="auto_start_enabled"
            render={({ field }) => (
              <FormItem className="space-y-0">
                <SettingsRow>
                  <SettingsFieldText
                    label={
                      <FormLabel className="text-[12.5px] font-semibold">
                        {t("settings.startup.enable")}
                      </FormLabel>
                    }
                    hint={
                      <UiFormDescription className={settingsDescriptionClassName}>
                        {t("settings.startup.enableHint")}
                      </UiFormDescription>
                    }
                  />
                  <FormControl>
                    <Switch checked={field.value} onCheckedChange={field.onChange} disabled={!settings || saving} />
                  </FormControl>
                </SettingsRow>
              </FormItem>
            )}
          />

          <FormField
            control={form.control}
            name="auto_start_launch_mode"
            render={({ field }) => (
              <FormItem className="space-y-0">
                <SettingsRow className="items-start">
                  <SettingsFieldText
                    label={
                      <FormLabel className="text-[12.5px] font-semibold">
                        {t("settings.startup.launchMode")}
                      </FormLabel>
                    }
                    hint={
                      <UiFormDescription className={settingsDescriptionClassName}>
                        {t("settings.startup.launchModeHint")}
                      </UiFormDescription>
                    }
                  />
                  <div className="w-[220px] shrink-0">
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
                    <FormMessage className="mt-1 text-[10.5px]" />
                  </div>
                </SettingsRow>
              </FormItem>
            )}
          />

          <SettingsFooter>
            <Button size="sm" type="submit" disabled={!settings || saving}>
              {t("common.save")}
            </Button>
          </SettingsFooter>
        </SettingsSection>
      </form>
    </Form>
  );
}

export function LoggingSettingsCard({
  settings,
  onSaved,
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
    <Form {...form}>
      <form onSubmit={submit}>
        <SettingsSection title={t("settings.logging.title")}>
          <FormField
            control={form.control}
            name="log_level"
            render={({ field }) => (
              <FormItem className="space-y-0">
                <SettingsRow className="items-start">
                  <SettingsFieldText
                    label={
                      <FormLabel className="text-[12.5px] font-semibold">
                        {t("settings.logging.level")}
                      </FormLabel>
                    }
                    hint={
                      <UiFormDescription className={settingsDescriptionClassName}>
                        {t("settings.logging.levelHint")}
                      </UiFormDescription>
                    }
                  />
                  <div className="w-[180px] shrink-0">
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
                    <FormMessage className="mt-1 text-[10.5px]" />
                  </div>
                </SettingsRow>
              </FormItem>
            )}
          />

          <FormField
            control={form.control}
            name="log_retention_days"
            render={({ field }) => (
              <FormItem className="space-y-0">
                <SettingsRow className="items-start">
                  <SettingsFieldText
                    label={
                      <FormLabel className="text-[12.5px] font-semibold">
                        {t("settings.logging.retentionDays")}
                      </FormLabel>
                    }
                    hint={
                      <UiFormDescription className={settingsDescriptionClassName}>
                        {t("settings.logging.retentionHint")}
                      </UiFormDescription>
                    }
                  />
                  <div className="w-[120px] shrink-0">
                    <FormControl>
                      <Input {...field} type="number" min="1" max="3650" className="font-mono" />
                    </FormControl>
                    <FormMessage className="mt-1 text-[10.5px]" />
                  </div>
                </SettingsRow>
              </FormItem>
            )}
          />

          <SettingsFooter>
            <Button size="sm" type="submit" disabled={!settings || saving}>
              {t("common.save")}
            </Button>
          </SettingsFooter>
        </SettingsSection>
      </form>
    </Form>
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
    <Form {...form}>
      <form onSubmit={submit}>
        <SettingsSection title={t("settings.serviceInfo.title")} first>
          <SettingsRow>
            <SettingsFieldText
              label={t("settings.serviceInfo.host")}
              hint={t("settings.serviceInfo.hostHint")}
            />
            <span className="text-[11px] font-mono text-muted-foreground">{apiHost}</span>
          </SettingsRow>

          <SettingsRow>
            <SettingsFieldText
              label={t("settings.serviceInfo.port")}
              hint={t("settings.serviceInfo.portHint")}
            />
            <span className="text-[11px] font-mono text-muted-foreground">{apiPort}</span>
          </SettingsRow>

          <FormField
            control={form.control}
            name="server_lan_accessible"
            render={({ field }) => (
              <FormItem className="space-y-0">
                <SettingsRow>
                  <SettingsFieldText
                    label={
                      <FormLabel className="text-[12.5px] font-semibold">
                        {t("settings.serviceInfo.lanAccessible")}
                      </FormLabel>
                    }
                    hint={
                      <UiFormDescription className={settingsDescriptionClassName}>
                        {t("settings.serviceInfo.lanAccessibleHint")}
                      </UiFormDescription>
                    }
                  />
                  <FormControl>
                    <Switch checked={field.value} onCheckedChange={field.onChange} disabled={!settings || saving} />
                  </FormControl>
                </SettingsRow>
              </FormItem>
            )}
          />

          <SettingsFooter>
            <Button size="sm" type="submit" disabled={!settings || saving}>
              {t("common.save")}
            </Button>
          </SettingsFooter>
        </SettingsSection>
      </form>
    </Form>
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
    <Form {...form}>
      <form onSubmit={submit}>
        <SettingsSection title={t("settings.compatibility.title")}>
          <FormField
            control={form.control}
            name="anthropic_count_tokens_mock_enabled"
            render={({ field }) => (
              <FormItem className="space-y-0">
                <SettingsRow>
                  <SettingsFieldText
                    label={
                      <FormLabel className="text-[12.5px] font-semibold">
                        {t("settings.compatibility.mockCountTokens")}
                      </FormLabel>
                    }
                    hint={
                      <UiFormDescription className={settingsDescriptionClassName}>
                        {t("settings.compatibility.mockCountTokensHint")}
                      </UiFormDescription>
                    }
                  />
                  <FormControl>
                    <Switch checked={field.value} onCheckedChange={field.onChange} disabled={!settings || saving} />
                  </FormControl>
                </SettingsRow>
              </FormItem>
            )}
          />

          <SettingsFooter>
            <Button size="sm" type="submit" disabled={!settings || saving}>
              {t("common.save")}
            </Button>
          </SettingsFooter>
        </SettingsSection>
      </form>
    </Form>
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
    <>
      {dialog}
      <Form {...form}>
        <form onSubmit={submit}>
          <SettingsSection title={t("settings.update.title")}>
            <FormField
              control={form.control}
              name="app_auto_update_enabled"
              render={({ field }) => (
                <FormItem className="space-y-0">
                  <SettingsRow>
                    <SettingsFieldText
                      label={
                        <FormLabel className="text-[12.5px] font-semibold">
                          {t("settings.update.autoEnable")}
                        </FormLabel>
                      }
                      hint={
                        <UiFormDescription className={settingsDescriptionClassName}>
                          {t("settings.update.autoEnableHint")}
                        </UiFormDescription>
                      }
                    />
                    <FormControl>
                      <Switch checked={field.value} onCheckedChange={field.onChange} disabled={!settings || saving} />
                    </FormControl>
                  </SettingsRow>
                </FormItem>
              )}
            />

            <SettingsRow>
              <SettingsFieldText
                label={t("settings.update.status")}
                hint={
                  <>
                    {updateStatusText}
                    {updateServerVersion ? (
                      <>
                        <br />
                        {t("settings.update.serverVersion", { version: updateServerVersion })}
                      </>
                    ) : null}
                  </>
                }
              />
              <Button
                type="button"
                size="sm"
                variant="outline"
                onClick={() => void onCheck()}
                disabled={updateChecking}
              >
                {t("settings.update.check")}
              </Button>
            </SettingsRow>

            <SettingsFooter>
              <Button size="sm" type="submit" disabled={!settings || saving}>
                {t("common.save")}
              </Button>
            </SettingsFooter>
          </SettingsSection>
        </form>
      </Form>
    </>
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
    <Form {...form}>
      <form onSubmit={submit}>
        <SettingsSection title={t("settings.chatBridge.configTitle")} first>
          <FormField
            control={form.control}
            name="chat_bridge_enabled"
            render={({ field }) => (
              <FormItem className="space-y-0">
                <SettingsRow>
                  <SettingsFieldText
                    label={
                      <FormLabel className="text-[12.5px] font-semibold">
                        {t("settings.chatBridge.enable")}
                      </FormLabel>
                    }
                    hint={
                      <UiFormDescription className={settingsDescriptionClassName}>
                        {t("settings.chatBridge.enableHint")}
                      </UiFormDescription>
                    }
                  />
                  <FormControl>
                    <Switch checked={field.value} onCheckedChange={field.onChange} disabled={!settings || saving} />
                  </FormControl>
                </SettingsRow>
              </FormItem>
            )}
          />

          <FormField
            control={form.control}
            name="chat_bridge_allow_new_projects"
            render={({ field }) => (
              <FormItem className="space-y-0">
                <SettingsRow>
                  <SettingsFieldText
                    label={
                      <FormLabel className="text-[12.5px] font-semibold">
                        {t("settings.chatBridge.allowNewProjects")}
                      </FormLabel>
                    }
                    hint={
                      <UiFormDescription className={settingsDescriptionClassName}>
                        {t("settings.chatBridge.allowNewProjectsHint")}
                      </UiFormDescription>
                    }
                  />
                  <FormControl>
                    <Switch checked={field.value} onCheckedChange={field.onChange} disabled={!settings || saving} />
                  </FormControl>
                </SettingsRow>
              </FormItem>
            )}
          />

          <FormField
            control={form.control}
            name="chat_bridge_turn_timeout_minutes"
            render={({ field }) => (
              <FormItem className="space-y-0">
                <SettingsRow className="items-start">
                  <SettingsFieldText
                    label={
                      <FormLabel className="text-[12.5px] font-semibold">
                        {t("settings.chatBridge.turnTimeoutMinutes")}
                      </FormLabel>
                    }
                    hint={
                      <UiFormDescription className={settingsDescriptionClassName}>
                        {t("settings.chatBridge.turnTimeoutMinutesHint")}
                      </UiFormDescription>
                    }
                  />
                  <div className="w-[140px] shrink-0">
                    <FormControl>
                      <Input {...field} type="number" min="0" disabled={!settings || saving} />
                    </FormControl>
                    <FormMessage className="mt-1 text-[10.5px]" />
                  </div>
                </SettingsRow>
              </FormItem>
            )}
          />

          <SettingsFooter>
            <Button size="sm" type="submit" disabled={!settings || saving}>
              {t("common.save")}
            </Button>
          </SettingsFooter>
        </SettingsSection>
      </form>
    </Form>
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
    <Form {...form}>
      <form onSubmit={submit}>
        <FormField
          control={form.control}
          name="chat_bridge_telegram_enabled"
          render={({ field }) => (
            <FormItem className="space-y-0">
              <SettingsRow>
                <SettingsFieldText
                  label={
                    <FormLabel className="text-[12.5px] font-semibold">
                      {t("settings.chatBridge.telegramEnable")}
                    </FormLabel>
                  }
                  hint={
                    <UiFormDescription className={settingsDescriptionClassName}>
                      {t("settings.chatBridge.telegramEnableHint")}
                    </UiFormDescription>
                  }
                />
                <FormControl>
                  <Switch checked={field.value} onCheckedChange={field.onChange} disabled={!settings || saving} />
                </FormControl>
              </SettingsRow>
            </FormItem>
          )}
        />

        <FormField
          control={form.control}
          name="chat_bridge_telegram_bot_token"
          render={({ field }) => (
            <FormItem className="space-y-0">
              <SettingsRow className="items-start">
                <SettingsFieldText
                  label={
                    <FormLabel className="text-[12.5px] font-semibold">
                      {t("settings.chatBridge.telegramToken")}
                    </FormLabel>
                  }
                  hint={
                    <UiFormDescription className={settingsDescriptionClassName}>
                      {tokenConfigured
                        ? t("settings.chatBridge.telegramTokenHintConfigured")
                        : t("settings.chatBridge.telegramTokenHint")}
                    </UiFormDescription>
                  }
                />
                <div className="w-[220px] shrink-0">
                  <FormControl>
                    <Input
                      {...field}
                      type="password"
                      autoComplete="new-password"
                      placeholder={
                        tokenConfigured
                          ? t("settings.chatBridge.telegramTokenPlaceholderConfigured")
                          : t("settings.chatBridge.telegramTokenPlaceholder")
                      }
                      disabled={!settings || saving}
                    />
                  </FormControl>
                  <FormMessage className="mt-1 text-[10.5px]" />
                </div>
              </SettingsRow>
            </FormItem>
          )}
        />

        <SettingsRow>
          <SettingsFieldText
            label={t("settings.chatBridge.bindings.title")}
            hint={t("settings.chatBridge.bindings.summary", { count: bindingCount })}
          />
          <div className="flex items-center gap-2">
            <Button type="button" size="sm" variant="outline" onClick={onOpenPairing}>
              {t("settings.chatBridge.actions.pairing")}
            </Button>
            <Button type="button" size="sm" variant="outline" onClick={onOpenBindings}>
              {t("settings.chatBridge.actions.bindings")}
            </Button>
          </div>
        </SettingsRow>

        <SettingsFooter>
          <Button size="sm" type="submit" disabled={!settings || saving}>
            {t("common.save")}
          </Button>
        </SettingsFooter>
      </form>
    </Form>
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
    <Form {...form}>
      <form onSubmit={submit}>
        <FormField
          control={form.control}
          name="chat_bridge_discord_enabled"
          render={({ field }) => (
            <FormItem className="space-y-0">
              <SettingsRow>
                <SettingsFieldText
                  label={
                    <FormLabel className="text-[12.5px] font-semibold">
                      {t("settings.chatBridge.discordEnable")}
                    </FormLabel>
                  }
                  hint={
                    <UiFormDescription className={settingsDescriptionClassName}>
                      {t("settings.chatBridge.discordEnableHint")}
                    </UiFormDescription>
                  }
                />
                <FormControl>
                  <Switch checked={field.value} onCheckedChange={field.onChange} disabled={!settings || saving} />
                </FormControl>
              </SettingsRow>
            </FormItem>
          )}
        />

        <FormField
          control={form.control}
          name="chat_bridge_discord_bot_token"
          render={({ field }) => (
            <FormItem className="space-y-0">
              <SettingsRow className="items-start">
                <SettingsFieldText
                  label={
                    <FormLabel className="text-[12.5px] font-semibold">
                      {t("settings.chatBridge.discordToken")}
                    </FormLabel>
                  }
                  hint={
                    <UiFormDescription className={settingsDescriptionClassName}>
                      {tokenConfigured
                        ? t("settings.chatBridge.discordTokenHintConfigured")
                        : t("settings.chatBridge.discordTokenHint")}
                    </UiFormDescription>
                  }
                />
                <div className="w-[220px] shrink-0">
                  <FormControl>
                    <Input
                      {...field}
                      type="password"
                      autoComplete="new-password"
                      placeholder={
                        tokenConfigured
                          ? t("settings.chatBridge.discordTokenPlaceholderConfigured")
                          : t("settings.chatBridge.discordTokenPlaceholder")
                      }
                      disabled={!settings || saving}
                    />
                  </FormControl>
                  <FormMessage className="mt-1 text-[10.5px]" />
                </div>
              </SettingsRow>
            </FormItem>
          )}
        />

        <SettingsRow>
          <SettingsFieldText
            label={t("settings.chatBridge.bindings.title")}
            hint={t("settings.chatBridge.bindings.summary", { count: bindingCount })}
          />
          <div className="flex items-center gap-2">
            <Button type="button" size="sm" variant="outline" onClick={onOpenPairing}>
              {t("settings.chatBridge.actions.pairing")}
            </Button>
            <Button type="button" size="sm" variant="outline" onClick={onOpenBindings}>
              {t("settings.chatBridge.actions.bindings")}
            </Button>
          </div>
        </SettingsRow>

        <SettingsFooter>
          <Button size="sm" type="submit" disabled={!settings || saving}>
            {t("common.save")}
          </Button>
        </SettingsFooter>
      </form>
    </Form>
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
    <Form {...form}>
      <form onSubmit={submit}>
        <FormField
          control={form.control}
          name="chat_bridge_whatsapp_enabled"
          render={({ field }) => (
            <FormItem className="space-y-0">
              <SettingsRow>
                <SettingsFieldText
                  label={
                    <FormLabel className="text-[12.5px] font-semibold">
                      {t("settings.chatBridge.whatsappEnable")}
                    </FormLabel>
                  }
                  hint={
                    <UiFormDescription className={settingsDescriptionClassName}>
                      {t("settings.chatBridge.whatsappEnableHint")}
                    </UiFormDescription>
                  }
                />
                <FormControl>
                  <Switch checked={field.value} onCheckedChange={field.onChange} disabled={!settings || saving} />
                </FormControl>
              </SettingsRow>
            </FormItem>
          )}
        />

        <SettingsRow>
          <SettingsFieldText
            label={t("settings.chatBridge.whatsapp.qrTitle")}
            hint={t("settings.chatBridge.whatsapp.runtimeHint")}
          />
          <div className="flex items-center gap-2">
            <Badge variant={statusTone} className="w-fit">
              {statusLabel}
            </Badge>
            <Button type="button" size="sm" onClick={onOpenLoginDialog} disabled={actionBusy}>
              {t("settings.chatBridge.whatsapp.dialogAction")}
            </Button>
          </div>
        </SettingsRow>

        <SettingsRow>
          <SettingsFieldText
            label={t("settings.chatBridge.bindings.title")}
            hint={t("settings.chatBridge.bindings.summary", { count: bindingCount })}
          />
          <div className="flex items-center gap-2">
            <Button type="button" size="sm" variant="outline" onClick={onOpenPairing}>
              {t("settings.chatBridge.actions.pairing")}
            </Button>
            <Button type="button" size="sm" variant="outline" onClick={onOpenBindings}>
              {t("settings.chatBridge.actions.bindings")}
            </Button>
          </div>
        </SettingsRow>

        <SettingsFooter>
          <Button size="sm" type="submit" disabled={!settings || saving}>
            {t("common.save")}
          </Button>
        </SettingsFooter>
      </form>
    </Form>
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
    <Form {...form}>
      <form onSubmit={submit}>
        <FormField
          control={form.control}
          name="chat_bridge_weixin_enabled"
          render={({ field }) => (
            <FormItem className="space-y-0">
              <SettingsRow>
                <SettingsFieldText
                  label={
                    <FormLabel className="text-[12.5px] font-semibold">
                      {t("settings.chatBridge.weixinEnable")}
                    </FormLabel>
                  }
                  hint={
                    <UiFormDescription className={settingsDescriptionClassName}>
                      {t("settings.chatBridge.weixinEnableHint")}
                    </UiFormDescription>
                  }
                />
                <FormControl>
                  <Switch checked={field.value} onCheckedChange={field.onChange} disabled={!settings || saving} />
                </FormControl>
              </SettingsRow>
            </FormItem>
          )}
        />

        <SettingsRow>
          <SettingsFieldText
            label={t("settings.chatBridge.weixin.qrTitle")}
            hint={t("settings.chatBridge.weixin.runtimeHint")}
          />
          <div className="flex items-center gap-2">
            <Badge variant={statusTone} className="w-fit">
              {statusLabel}
            </Badge>
            <Button type="button" size="sm" onClick={onOpenLoginDialog} disabled={actionBusy}>
              {t("settings.chatBridge.weixin.dialogAction")}
            </Button>
          </div>
        </SettingsRow>

        <SettingsRow>
          <SettingsFieldText
            label={t("settings.chatBridge.bindings.title")}
            hint={t("settings.chatBridge.bindings.summary", { count: bindingCount })}
          />
          <div className="flex items-center gap-2">
            <Button type="button" size="sm" variant="outline" onClick={onOpenPairing}>
              {t("settings.chatBridge.actions.pairing")}
            </Button>
            <Button type="button" size="sm" variant="outline" onClick={onOpenBindings}>
              {t("settings.chatBridge.actions.bindings")}
            </Button>
          </div>
        </SettingsRow>

        <SettingsFooter>
          <Button size="sm" type="submit" disabled={!settings || saving}>
            {t("common.save")}
          </Button>
        </SettingsFooter>
      </form>
    </Form>
  );
}
