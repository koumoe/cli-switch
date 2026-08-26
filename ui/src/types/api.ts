import type { LogLevel } from "@/lib/logger";
import type { Locale } from "@/types/locale";

export type Protocol = "openai" | "anthropic" | "gemini";

export type Health = {
  status: string;
  version?: string;
  listen_addr?: string;
  data_dir?: string;
  db_path?: string;
};

export type CloseBehavior = "ask" | "minimize_to_tray" | "quit";

export type AutoStartLaunchMode = "show_window" | "minimize_to_tray";

export type AppSettings = {
  ui_locale: Locale;
  pricing_auto_update_enabled: boolean;
  pricing_auto_update_interval_hours: number;
  close_behavior: CloseBehavior;
  auto_start_enabled: boolean;
  auto_start_launch_mode: AutoStartLaunchMode;
  server_lan_accessible: boolean;
  app_auto_update_enabled: boolean;
  gemini_cli_auto_update_enabled: boolean;
  claude_code_auto_update_enabled: boolean;
  codex_auto_update_enabled: boolean;
  auto_disable_enabled: boolean;
  auto_disable_window_minutes: number;
  auto_disable_failure_times: number;
  auto_disable_disable_minutes: number;
  channel_retry_enabled: boolean;
  anthropic_count_tokens_mock_enabled: boolean;
  log_level: LogLevel;
  log_retention_days: number;
  chat_bridge_enabled: boolean;
  chat_bridge_telegram_enabled: boolean;
  chat_bridge_telegram_bot_token?: string | null;
  chat_bridge_telegram_bot_token_configured: boolean;
  chat_bridge_discord_enabled: boolean;
  chat_bridge_discord_bot_token?: string | null;
  chat_bridge_discord_bot_token_configured: boolean;
  chat_bridge_whatsapp_enabled: boolean;
  chat_bridge_weixin_enabled: boolean;
  chat_bridge_turn_timeout_minutes: number;
  chat_bridge_allow_new_projects: boolean;
  system_notifications_enabled: boolean;
  remote_low_balance_system_notification_enabled: boolean;
  remote_managed_channel_missing_system_notification_enabled: boolean;
  remote_managed_channel_multiplier_system_notification_enabled: boolean;
  remote_group_added_system_notification_enabled: boolean;
  remote_managed_channel_missing_prompt_enabled: boolean;
  remote_managed_channel_sync_multiplier_enabled: boolean;
  remote_managed_channel_sync_free_multiplier_enabled: boolean;
};

export type ChatPlatform = "telegram" | "discord" | "whatsapp" | "weixin";

export type ChatBridgeRuntimeStatusState =
  | "disabled"
  | "starting"
  | "awaiting_qr"
  | "connected"
  | "error";

export type ChatBridgeWhatsAppStatus = {
  state: ChatBridgeRuntimeStatusState;
  connected: boolean;
  me?: string | null;
  qr?: string | null;
  qr_image?: string | null;
  issue?: UserFacingIssuePayload | null;
};

export type ChatBridgeWeixinStatus = {
  state: ChatBridgeRuntimeStatusState;
  connected: boolean;
  me?: string | null;
  qr?: string | null;
  qr_image?: string | null;
  issue?: UserFacingIssuePayload | null;
};

export type ChatBridgeBinding = {
  id: number;
  platform: ChatPlatform;
  platform_user_id: string;
  display_name: string | null;
  bound_at_ms: number;
  is_active: boolean;
};

export type ChatBridgePairingToken = {
  token: string;
  platform: ChatPlatform;
  expires_at_ms: number;
};

export type CreateChatBridgePairingTokenInput = {
  platform: ChatPlatform;
  expires_in_minutes?: number | null;
};

export type CliToolId = "gemini" | "claude" | "codex";

export type ProjectScope = "global" | "project";

export type ProjectRecord = {
  id: string;
  name: string;
  path: string;
  created_at_ms: number;
  updated_at_ms: number;
};

export type ProjectDocument = {
  tool: CliToolId;
  scope: ProjectScope;
  project_id: string | null;
  content_md: string;
  exists: boolean;
  created_at_ms: number | null;
  updated_at_ms: number | null;
};

export type GetProjectDocumentQuery = {
  tool: CliToolId;
  scope: ProjectScope;
  project_id?: string | null;
};

export type SaveProjectDocumentInput = {
  tool: CliToolId;
  scope: ProjectScope;
  project_id?: string | null;
  content_md: string;
  expected_updated_at_ms?: number | null;
};

export type DeleteProjectDocumentInput = {
  tool: CliToolId;
  scope: ProjectScope;
  project_id?: string | null;
  expected_updated_at_ms?: number | null;
};

export type CliToolInstallMethod = "managed_npm_prefix" | "brew" | "npm" | "other";

export type CliToolStatus = {
  id: CliToolId;
  name: string;
  bin: string;
  npm_package: string;
  installed: boolean;
  version: string | null;
  install_method: CliToolInstallMethod;
  install_path: string | null;
  installer_path: string | null;
};

export type CliToolsStatus = {
  os: string;
  tools: CliToolStatus[];
};

export type CliToolProxyConfigCheck = {
  id: string;
  ok: boolean;
  file: string;
  key: string;
  expected: string;
  current: string | null;
  message?: string;
};

export type CliToolProxyConfigToolStatus = {
  id: CliToolId;
  name: string;
  ok: boolean;
  checks: CliToolProxyConfigCheck[];
};

export type CliToolProxyConfigStatus = {
  tools: CliToolProxyConfigToolStatus[];
};

export type ApplyCliToolsProxyConfigRequest = {
  tools?: CliToolId[];
};

export type CliToolProxyConfigApplyItem = {
  id: CliToolId;
  ok: boolean;
  issue?: UserFacingIssuePayload | null;
};

export type CliToolProxyConfigApplyResponse = {
  ok: boolean;
  applied: CliToolProxyConfigApplyItem[];
  status: CliToolProxyConfigStatus;
};

export type InstallCliToolResponse = {
  ok: boolean;
  exit_code: number | null;
  stdout: string;
  stderr: string;
  tool: CliToolStatus;
  terminal_shim_ok: boolean;
  terminal_shim_dir: string | null;
  terminal_shim_issue?: UserFacingIssuePayload | null;
};

export type PickFolderInput = {
  title?: string;
  directory?: string;
};

export type PickFolderResponse = {
  path: string | null;
};

export type Channel = {
  id: string;
  name: string;
  protocol: Protocol;
  base_url: string;
  auth_type: string;
  auth_ref: string;
  checkin_url: string | null;
  priority: number;
  retry_times: number;
  ignore_channel_protection: boolean;
  recharge_currency: "USD" | "CNY";
  real_multiplier: number;
  enabled: boolean;
  auto_disabled_until_ms: number;
  managed_by_remote: boolean;
  managed_remote_provider?: "newapi" | "sub2api" | null;
  managed_remote_account_id?: string | null;
  managed_remote_resource_id?: string | null;
  managed_remote_resource_name?: string | null;
  managed_remote_group_name?: string | null;
  managed_remote_group_id?: number | null;
  created_at_ms: number;
  updated_at_ms: number;
};

export type CreateChannelInput = {
  name: string;
  protocol: Protocol;
  base_url: string;
  auth_type: string;
  auth_ref: string;
  checkin_url: string;
  priority: number;
  retry_times: number;
  ignore_channel_protection: boolean;
  recharge_currency: "USD" | "CNY";
  real_multiplier: number;
  enabled: boolean;
};

export type UpdateChannelInput = Partial<{
  name: string;
  base_url: string;
  auth_type: string;
  auth_ref: string;
  checkin_url: string;
  priority: number;
  retry_times: number;
  ignore_channel_protection: boolean;
  recharge_currency: "USD" | "CNY";
  real_multiplier: number;
  enabled: boolean;
}>;

export type DeleteChannelInput = {
  sync_remote_delete?: boolean;
};

export type ChannelCheckinsToday = {
  date: string;
  completed_channel_ids: string[];
};

export type RechargeCurrency = "USD" | "CNY";

export type RemoteAccountProvider = "newapi" | "sub2api";

export type RemoteAccountCheckinMode = "disabled" | "system_api" | "page_open";

export type RemoteAccountBase = {
  id: string;
  name: string;
  base_url: string;
  api_url: string | null;
  user_id: string;
  user_token_configured: boolean;
  reauth_required: boolean;
  page_checkin_url: string | null;
  checkin_mode: RemoteAccountCheckinMode;
  auto_checkin_enabled: boolean;
  auto_checkin_time: string;
  low_balance_alert_threshold: number;
  recharge_currency: RechargeCurrency;
  remote_username: string | null;
  remote_display_name: string | null;
  last_balance_amount: number | null;
  last_sync_error: string | null;
  last_synced_at_ms: number | null;
  low_balance_alert_notified: boolean;
  last_balance_alert_at_ms: number | null;
  sort_order: number;
  created_at_ms: number;
  updated_at_ms: number;
};

export type NewapiRemoteAccount = RemoteAccountBase & {
  provider: "newapi";
  remote_role: number | null;
  remote_group: string | null;
  quota_display_type: string;
  quota_per_unit: number;
  usd_exchange_rate: number;
  custom_currency_symbol: string | null;
  custom_currency_exchange_rate: number;
  remote_checkin_enabled: boolean;
  remote_turnstile_check_enabled: boolean;
  last_quota: number | null;
  last_used_quota: number | null;
};

export type Sub2ApiRemoteAccount = RemoteAccountBase & {
  provider: "sub2api";
  remote_role_text: string | null;
};

export type RemoteAccount = NewapiRemoteAccount | Sub2ApiRemoteAccount;

export type RemoteAccountDetection = {
  provider: RemoteAccountProvider;
  normalized_base_url: string;
  recommended_api_url: string | null;
  suggested_page_checkin_url: string | null;
  supported_checkin_modes: RemoteAccountCheckinMode[];
};

export type CreateRemoteAccountInput = {
  name?: string;
  provider: RemoteAccountProvider;
  base_url: string;
  api_url?: string | null;
  user_id?: string | null;
  user_token?: string | null;
  bearer_token?: string | null;
  refresh_token?: string | null;
  page_checkin_url?: string | null;
  checkin_mode?: RemoteAccountCheckinMode;
  auto_checkin_time?: string;
  low_balance_alert_threshold?: number;
  recharge_currency?: RechargeCurrency;
};

export type UpdateRemoteAccountInput = Partial<{
  provider: RemoteAccountProvider;
  base_url: string;
  api_url: string | null;
  user_id: string;
  user_token: string;
  bearer_token: string;
  refresh_token: string | null;
  page_checkin_url: string | null;
  checkin_mode: RemoteAccountCheckinMode;
  auto_checkin_time: string;
  low_balance_alert_threshold: number;
  recharge_currency: RechargeCurrency;
}>;

export type DeleteRemoteAccountInput = {
  delete_managed_channels?: boolean;
  sync_remote_delete?: boolean;
};

export type RemoteAccountCheckinsToday = {
  date: string;
  completed_account_ids: string[];
};

export type RemoteGroupOption = {
  id: number | null;
  name: string;
  ratio: number | null;
  description: string | null;
  platform: string | null;
  managed_channel_count: number;
};

export type CreateRemoteKeyInput = {
  name: string;
  group_id?: number | null;
};

export type RemoteKey = {
  id: number;
  key: string;
  name: string;
  group_id: number | null;
  status: string;
};

export type RemoteAccountSystemCheckinResult = {
  quota_awarded: number | null;
  checkin_date: string | null;
  already_checked_in: boolean;
};

export type RemoteManagedChannelMissingPrompt = {
  provider: RemoteAccountProvider;
  channel_id: string;
  channel_name: string;
  account_id: string;
  account_base_url: string;
  group_name: string | null;
  resource_name: string | null;
  missing_group: boolean;
  missing_resource: boolean;
};

export type RemoteManagedChannelMultiplierPrompt = {
  provider: RemoteAccountProvider;
  channel_id: string;
  channel_name: string;
  account_id: string;
  account_base_url: string;
  group_name: string | null;
  current_multiplier: number;
  remote_multiplier: number;
};

export type RemoteGroupAddedAlert = {
  provider: RemoteAccountProvider;
  account_id: string;
  account_base_url: string;
  group_id?: number | null;
  group_name: string;
};

export type CreateRemoteManagedChannelInput = {
  name: string;
  protocol: Protocol;
  group_name: string;
  group_id?: number | null;
  base_url_override?: string | null;
};

export type CreateRemoteManagedChannelResponse = {
  channel: Channel;
};

export type ChannelTestResponse = {
  reachable: boolean;
  ok: boolean;
  status: number | null;
  latency_ms: number;
  issue?: UserFacingIssuePayload | null;
};

export type PricingStatus = {
  count: number;
  last_sync_ms: number | null;
};

export type PricingModel = {
  model_id: string;
  prompt_price: string | null;
  completion_price: string | null;
  request_price: string | null;
  updated_at_ms: number;
};

export type PricingSyncResponse = {
  updated: number;
  updated_at_ms: number;
};

export type UpdateStatus = {
  current_version: string;
  auto_update_enabled: boolean;
  stage: "idle" | "checking" | "downloading" | "staging" | "ready" | "error";
  latest_version: string | null;
  latest_ignored: boolean;
  update_available: boolean;
  pending_version: string | null;
  download_percent: number | null;
  issue?: UserFacingIssuePayload | null;
};

export type UserFacingIssuePayload = {
  code: string;
  message: string;
  args: Record<string, string>;
  detail: string | null;
};

export type UpdateCheck = {
  current_version: string;
  latest_version: string | null;
  latest_ignored: boolean;
  update_available: boolean;
};

export type UpdateDownloadResponse = {
  started: boolean;
  status: UpdateStatus;
};

export type ChangelogSection = {
  title: string;
  items: string[];
};

export type ChangelogOverview = {
  version: string;
  locale: string;
  sections: ChangelogSection[];
};

export type StatsRange = "today" | "yesterday" | "week" | "month" | "custom";

export type StatsQuery = Partial<{
  range: StatsRange;
  start_ms: number;
  end_ms: number;
}>;

export type StatsSummary = {
  range: string;
  start_ms: number;
  requests: number;
  success: number;
  failed: number;
  avg_latency_ms: number | null;
  prompt_tokens: number;
  completion_tokens: number;
  total_tokens: number;
  estimated_cost_usd: string | null;
};

export type ChannelStats = {
  channel_id: string;
  name: string;
  protocol: Protocol;
  requests: number;
  success: number;
  failed: number;
  avg_latency_ms: number | null;
  total_tokens: number;
  estimated_cost_usd: string | null;
};

export type StatsChannels = {
  range: string;
  start_ms: number;
  items: ChannelStats[];
};

export type UsageEvent = {
  id: string;
  request_id: string | null;
  ts_ms: number;
  protocol: Protocol;
  channel_id: string;
  model: string | null;
  success: boolean;
  http_status: number | null;
  error_kind: string | null;
  error_detail: string | null;
  latency_ms: number;
  ttft_ms: number | null;
  prompt_tokens: number | null;
  completion_tokens: number | null;
  total_tokens: number | null;
  cache_read_tokens: number | null;
  cache_write_tokens: number | null;
  estimated_cost_usd: string | null;
};

export type UsageListQuery = Partial<{
  start_ms: number;
  end_ms: number;
  protocol: Protocol;
  channel_id: string;
  model: string;
  request_id: string;
  success: boolean;
  limit: number;
  offset: number;
}>;

export type UsageListResult = {
  total: number;
  items: UsageEvent[];
};

export type TrendPoint = {
  bucket_start_ms: number;
  channel_id: string;
  name: string;
  success: number;
};

export type StatsTrend = {
  range: string;
  start_ms: number;
  unit: "day";
  items: TrendPoint[];
};

export type DbSize = {
  path: string;
  db_bytes: number;
  wal_bytes: number;
  shm_bytes: number;
  total_bytes: number;
};

export type LogsSize = {
  path: string;
  total_bytes: number;
  file_count: number;
};

export type RecordsClearMode = "date_range" | "errors" | "all";

export type ClearRecordsResult = {
  usage_events_deleted: number;
  vacuumed: boolean;
};

export type ClearRecordsInput = {
  mode: RecordsClearMode;
  start_ms?: number;
  end_ms?: number;
};

export type LogsClearMode = "date_range" | "all";

export type ClearLogsResult = {
  deleted_files: number;
  truncated_files: number;
};

export type ClearLogsInput = {
  mode: LogsClearMode;
  start_date?: string;
  end_date?: string;
};
