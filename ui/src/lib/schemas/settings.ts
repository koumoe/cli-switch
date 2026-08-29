import { z } from "zod";

import { integerStringSchema, type Translate } from "@/lib/schemas/common";

export function createChannelProtectionSchema(t: Translate) {
  const message = t("settings.channelProtection.invalid");
  return z.object({
    auto_disable_enabled: z.boolean(),
    auto_disable_window_minutes: integerStringSchema(message, { min: 1 }),
    auto_disable_failure_times: integerStringSchema(message, { min: 1 }),
    auto_disable_disable_minutes: integerStringSchema(message, { min: 1 }),
  });
}

export function createChannelRetrySchema() {
  return z.object({
    channel_retry_enabled: z.boolean(),
  });
}

export function createNewApiManagedSchema() {
  return z.object({
    remote_managed_channel_missing_prompt_enabled: z.boolean(),
    remote_managed_channel_sync_multiplier_enabled: z.boolean(),
    remote_managed_channel_sync_free_multiplier_enabled: z.boolean(),
  });
}

export function createSystemNotificationsSchema() {
  return z.object({
    system_notifications_enabled: z.boolean(),
  });
}

export function createRemoteNotificationsSchema() {
  return z.object({
    remote_low_balance_system_notification_enabled: z.boolean(),
    remote_managed_channel_missing_system_notification_enabled: z.boolean(),
    remote_managed_channel_multiplier_system_notification_enabled: z.boolean(),
    remote_group_added_system_notification_enabled: z.boolean(),
  });
}

export function createPricingDataSchema(t: Translate) {
  return z.object({
    pricing_auto_update_enabled: z.boolean(),
    pricing_auto_update_interval_hours: integerStringSchema(t("settings.pricingData.intervalInvalid"), {
      min: 1,
      max: 8760,
    }),
  });
}

export function createWindowCloseSchema() {
  return z.object({
    close_behavior: z.enum(["ask", "minimize_to_tray", "quit"]),
  });
}

export function createStartupSchema() {
  return z.object({
    auto_start_enabled: z.boolean(),
    auto_start_launch_mode: z.enum(["show_window", "minimize_to_tray"]),
  });
}

export function createLoggingSchema(t: Translate) {
  return z.object({
    log_level: z.enum(["none", "debug", "info", "warning", "error"]),
    log_retention_days: integerStringSchema(t("settings.logging.retentionInvalid"), {
      min: 1,
      max: 3650,
    }),
  });
}

export function createServiceInfoSchema() {
  return z.object({
    server_lan_accessible: z.boolean(),
  });
}

export function createCompatibilitySchema() {
  return z.object({
    openai_responses_reasoning_id_sanitizer_enabled: z.boolean(),
    anthropic_count_tokens_mock_enabled: z.boolean(),
  });
}

export function createAppUpdateSchema() {
  return z.object({
    app_auto_update_enabled: z.boolean(),
  });
}

export function createChatBridgeBaseSchema(t: Translate) {
  return z.object({
    chat_bridge_enabled: z.boolean(),
    chat_bridge_allow_new_projects: z.boolean(),
    chat_bridge_turn_timeout_minutes: integerStringSchema(t("settings.chatBridge.turnTimeoutInvalid"), {
      min: 0,
    }),
  });
}

export function createChatBridgeTelegramSchema() {
  return z.object({
    chat_bridge_telegram_enabled: z.boolean(),
    chat_bridge_telegram_bot_token: z.string(),
  });
}

export function createChatBridgeDiscordSchema() {
  return z.object({
    chat_bridge_discord_enabled: z.boolean(),
    chat_bridge_discord_bot_token: z.string(),
  });
}

export function createChatBridgeWhatsAppSchema() {
  return z.object({
    chat_bridge_whatsapp_enabled: z.boolean(),
  });
}

export function createChatBridgeWeixinSchema() {
  return z.object({
    chat_bridge_weixin_enabled: z.boolean(),
  });
}
