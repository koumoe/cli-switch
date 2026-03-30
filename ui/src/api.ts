import { ApiRequestError, extractErrorCode, extractErrorMessage } from "@/lib/error";
import { getCurrentLocale, type Locale } from "@/lib/i18n";
import { logger, type LogLevel } from "@/lib/logger";

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

export type PromptScope = "global" | "project";

export type PromptProject = {
  id: string;
  name: string;
  path: string;
  created_at_ms: number;
  updated_at_ms: number;
};

export type PromptDocument = {
  tool: CliToolId;
  scope: PromptScope;
  project_id: string | null;
  content_md: string;
  exists: boolean;
  created_at_ms: number | null;
  updated_at_ms: number | null;
};

export type SavePromptDocumentInput = {
  tool: CliToolId;
  scope: PromptScope;
  project_id?: string | null;
  content_md: string;
  expected_updated_at_ms?: number | null;
};

export type DeletePromptDocumentInput = {
  tool: CliToolId;
  scope: PromptScope;
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
  recharge_currency: "USD" | "CNY";
  real_multiplier: number;
  enabled: boolean;
}>;

export type ChannelCheckinsToday = {
  date: string;
  completed_channel_ids: string[];
};

export type RechargeCurrency = "USD" | "CNY";

export type RemoteAccountProvider = "newapi" | "sub2api";

export type RemoteAccountCheckinMode = "disabled" | "system_api" | "page_open";

export type RemoteAccountBase = {
  id: string;
  base_url: string;
  api_url: string | null;
  user_id: string;
  user_token_configured: boolean;
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
  provider: RemoteAccountProvider;
  base_url: string;
  api_url?: string | null;
  user_id?: string | null;
  user_token?: string | null;
  bearer_token?: string | null;
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
  page_checkin_url: string | null;
  checkin_mode: RemoteAccountCheckinMode;
  auto_checkin_time: string;
  low_balance_alert_threshold: number;
  recharge_currency: RechargeCurrency;
}>;

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

export type Route = {
  id: string;
  name: string;
  protocol: Protocol;
  match_model: string | null;
  enabled: boolean;
  created_at_ms: number;
  updated_at_ms: number;
};

export type CreateRouteInput = {
  name: string;
  protocol: Protocol;
  match_model: string | null;
  enabled: boolean;
};

export type UpdateRouteInput = Partial<{
  name: string;
  match_model: string | null;
  enabled: boolean;
}>;

export type RouteChannel = {
  route_id: string;
  channel_id: string;
  priority: number;
  cooldown_until_ms: number | null;
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
  route_id: string | null;
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

const ISSUE_PAYLOAD_KEYS = new Set(["code", "message", "args", "detail"]);

function isPlainObject(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === "object" && !Array.isArray(value);
}

async function http<T>(method: string, path: string, body?: unknown): Promise<T> {
  const locale = getCurrentLocale();
  const headers: Record<string, string> = {
    "X-CliSwitch-Locale": locale,
  };
  if (body) {
    headers["content-type"] = "application/json";
  }

  const res = await fetch(path, {
    method,
    headers,
    body: body ? JSON.stringify(body) : undefined
  });

  if (res.ok) {
    if (res.status === 204 || res.status === 205) return undefined as T;
    const text = await res.text();
    if (!text.trim()) return undefined as T;
    return JSON.parse(text) as T;
  }

  const text = await res.text().catch(() => "");
  const trimmed = text.trim();

  let code: string | null = null;
  let msg: string | null = null;
  let issue: UserFacingIssuePayload | null = null;
  if (trimmed.length > 0) {
    try {
      const parsed = JSON.parse(trimmed);
      code = extractErrorCode(parsed);
      msg = extractErrorMessage(parsed);
      issue = parseIssuePayload(parsed);
      if (!msg) msg = trimmed;
    } catch {
      msg = trimmed;
    }
  }

  if (path !== "/api/logs/ingest") {
    logger.error("api request failed", {
      method,
      path,
      status: res.status,
      code,
      error: msg ?? null
    }, "api_request_failed");
  }
  throw new ApiRequestError({
    code,
    message: msg,
    issue,
    status: res.status,
    method,
    path
  });
}

function parseIssuePayload(payload: unknown): UserFacingIssuePayload | null {
  if (!isPlainObject(payload)) return null;

  const obj = payload as Record<string, unknown>;
  const nestedIssue = obj.issue;
  const candidate = isPlainObject(nestedIssue)
    ? nestedIssue
    : isTopLevelIssuePayload(obj)
      ? obj
      : null;
  if (!candidate) return null;

  const code = typeof candidate.code === "string" ? candidate.code : null;
  const message = typeof candidate.message === "string" ? candidate.message : null;
  if (!code && !message) return null;

  const argsRaw = candidate.args;
  const args: Record<string, string> = {};
  if (isPlainObject(argsRaw)) {
    for (const [k, v] of Object.entries(argsRaw)) {
      if (typeof v === "string") {
        args[k] = v;
      } else if (v !== null && v !== undefined) {
        args[k] = String(v);
      }
    }
  }

  const detail = typeof candidate.detail === "string" ? candidate.detail : null;

  return {
    code: code ?? "unknown_error",
    message: message ?? code ?? "unknown_error",
    args,
    detail,
  };
}

function isTopLevelIssuePayload(payload: Record<string, unknown>): boolean {
  const keys = Object.keys(payload);
  if (keys.length === 0) return false;
  if (keys.some((key) => !ISSUE_PAYLOAD_KEYS.has(key))) return false;
  if (typeof payload.code !== "string") return false;
  return (
    typeof payload.message === "string" ||
    typeof payload.detail === "string" ||
    isPlainObject(payload.args)
  );
}

export function getHealth(): Promise<Health> {
  return http<Health>("GET", "/api/health");
}

export function getSettings(): Promise<AppSettings> {
  return http<AppSettings>("GET", "/api/settings");
}

export function updateSettings(
  patch: Partial<AppSettings>
): Promise<AppSettings> {
  return http<AppSettings>("PUT", "/api/settings", patch);
}

export function listChatBridgeBindings(): Promise<ChatBridgeBinding[]> {
  return http<ChatBridgeBinding[]>("GET", "/api/chat_bridge/bindings");
}

export function deactivateChatBridgeBinding(bindingId: number): Promise<void> {
  return http<void>("DELETE", `/api/chat_bridge/bindings/${bindingId}`);
}

export function createChatBridgePairingToken(
  input: CreateChatBridgePairingTokenInput
): Promise<ChatBridgePairingToken> {
  return http<ChatBridgePairingToken>("POST", "/api/chat_bridge/pairing_tokens", input);
}

export function getChatBridgeWhatsAppStatus(): Promise<ChatBridgeWhatsAppStatus> {
  return http<ChatBridgeWhatsAppStatus>("GET", "/api/chat_bridge/whatsapp/status");
}

export function startChatBridgeWhatsAppLogin(): Promise<void> {
  return http<void>("POST", "/api/chat_bridge/whatsapp/login", {});
}

export function logoutChatBridgeWhatsApp(): Promise<void> {
  return http<void>("POST", "/api/chat_bridge/whatsapp/logout", {});
}

export function getChatBridgeWeixinStatus(): Promise<ChatBridgeWeixinStatus> {
  return http<ChatBridgeWeixinStatus>("GET", "/api/chat_bridge/weixin/status");
}

export function startChatBridgeWeixinLogin(): Promise<void> {
  return http<void>("POST", "/api/chat_bridge/weixin/login", {});
}

export function logoutChatBridgeWeixin(): Promise<void> {
  return http<void>("POST", "/api/chat_bridge/weixin/logout", {});
}

export function getCliToolsStatus(): Promise<CliToolsStatus> {
  return http<CliToolsStatus>("GET", "/api/tools/status");
}

export function installCliTool(id: CliToolId): Promise<InstallCliToolResponse> {
  return http<InstallCliToolResponse>("POST", "/api/tools/install", { id });
}

export function getCliToolsProxyConfigStatus(): Promise<CliToolProxyConfigStatus> {
  return http<CliToolProxyConfigStatus>("GET", "/api/tools/config/status");
}

export function applyCliToolsProxyConfig(
  input: ApplyCliToolsProxyConfigRequest = {}
): Promise<CliToolProxyConfigApplyResponse> {
  return http<CliToolProxyConfigApplyResponse>("POST", "/api/tools/config/apply", input);
}

export function pickFolder(input: PickFolderInput = {}): Promise<PickFolderResponse> {
  return http<PickFolderResponse>("POST", "/api/system/pick_folder", input);
}

export function listPromptProjects(tool: CliToolId): Promise<PromptProject[]> {
  const p = new URLSearchParams();
  p.set("tool", tool);
  return http<PromptProject[]>("GET", `/api/prompts/projects?${p.toString()}`);
}

export function deletePromptProject(tool: CliToolId, projectId: string): Promise<void> {
  const p = new URLSearchParams();
  p.set("tool", tool);
  p.set("project_id", projectId);
  return http<void>("DELETE", `/api/prompts/projects?${p.toString()}`);
}

export function getPromptDocument(query: {
  tool: CliToolId;
  scope: PromptScope;
  project_id?: string | null;
}): Promise<PromptDocument> {
  const p = new URLSearchParams();
  p.set("tool", query.tool);
  p.set("scope", query.scope);
  if (query.project_id) p.set("project_id", query.project_id);
  return http<PromptDocument>("GET", `/api/prompts/document?${p.toString()}`);
}

export function savePromptDocument(input: SavePromptDocumentInput): Promise<PromptDocument> {
  return http<PromptDocument>("PUT", "/api/prompts/document", input);
}

export function deletePromptDocument(input: DeletePromptDocumentInput): Promise<void> {
  const p = new URLSearchParams();
  p.set("tool", input.tool);
  p.set("scope", input.scope);
  if (input.project_id) p.set("project_id", input.project_id);
  if (input.expected_updated_at_ms !== undefined && input.expected_updated_at_ms !== null) {
    p.set("expected_updated_at_ms", String(input.expected_updated_at_ms));
  }
  return http<void>("DELETE", `/api/prompts/document?${p.toString()}`);
}

export function listChannels(): Promise<Channel[]> {
  return http<Channel[]>("GET", "/api/channels");
}

export function createChannel(input: CreateChannelInput): Promise<Channel> {
  return http<Channel>("POST", "/api/channels", input);
}

export function updateChannel(id: string, input: UpdateChannelInput): Promise<void> {
  return http<void>("PUT", `/api/channels/${encodeURIComponent(id)}`, input);
}

export function enableChannel(id: string): Promise<void> {
  return http<void>("POST", `/api/channels/${encodeURIComponent(id)}/enable`);
}

export function disableChannel(id: string): Promise<void> {
  return http<void>("POST", `/api/channels/${encodeURIComponent(id)}/disable`);
}

export function deleteChannel(
  id: string,
  input?: { sync_remote_delete?: boolean }
): Promise<void> {
  return http<void>("DELETE", `/api/channels/${encodeURIComponent(id)}`, input);
}

export function testChannel(id: string): Promise<ChannelTestResponse> {
  return http<ChannelTestResponse>("POST", `/api/channels/${encodeURIComponent(id)}/test`);
}

export function reorderChannels(protocol: Protocol, channelIds: string[]): Promise<void> {
  return http<void>("POST", "/api/channels/reorder", { protocol, channel_ids: channelIds });
}

export function channelCheckinsToday(): Promise<ChannelCheckinsToday> {
  return http<ChannelCheckinsToday>("GET", "/api/channels/checkins/today");
}

export function completeChannelCheckinToday(id: string): Promise<void> {
  return http<void>("POST", `/api/channels/${encodeURIComponent(id)}/checkins/complete`);
}

export function listRemoteAccounts(): Promise<RemoteAccount[]> {
  return http<RemoteAccount[]>("GET", "/api/remote/accounts");
}

export function detectRemoteAccount(baseUrl: string): Promise<RemoteAccountDetection> {
  return http<RemoteAccountDetection>("POST", "/api/remote/accounts/detect", { base_url: baseUrl });
}

export function createRemoteAccount(input: CreateRemoteAccountInput): Promise<RemoteAccount> {
  return http<RemoteAccount>("POST", "/api/remote/accounts", input);
}

export function updateRemoteAccount(
  id: string,
  input: UpdateRemoteAccountInput
): Promise<RemoteAccount> {
  return http<RemoteAccount>("PUT", `/api/remote/accounts/${encodeURIComponent(id)}`, input);
}

export function refreshRemoteAccount(id: string): Promise<RemoteAccount> {
  return http<RemoteAccount>("POST", `/api/remote/accounts/${encodeURIComponent(id)}/refresh`, {});
}

export function listRemoteAccountGroups(accountId: string): Promise<RemoteGroupOption[]> {
  return http<RemoteGroupOption[]>(
    "GET",
    `/api/remote/accounts/${encodeURIComponent(accountId)}/groups`
  );
}

export function createRemoteAccountKey(
  accountId: string,
  input: CreateRemoteKeyInput
): Promise<RemoteKey> {
  return http<RemoteKey>(
    "POST",
    `/api/remote/accounts/${encodeURIComponent(accountId)}/keys`,
    input
  );
}

export function reorderRemoteAccounts(accountIds: string[]): Promise<void> {
  return http<void>("POST", "/api/remote/accounts/reorder", { account_ids: accountIds });
}

export function remoteAccountCheckinsToday(): Promise<RemoteAccountCheckinsToday> {
  return http<RemoteAccountCheckinsToday>("GET", "/api/remote/accounts/checkins/today");
}

export function completeRemoteAccountCheckinToday(id: string): Promise<void> {
  return http<void>("POST", `/api/remote/accounts/${encodeURIComponent(id)}/checkins/complete`, {});
}

export function remoteAccountSystemCheckin(id: string): Promise<RemoteAccountSystemCheckinResult> {
  return http<RemoteAccountSystemCheckinResult>(
    "POST",
    `/api/remote/accounts/${encodeURIComponent(id)}/checkins/system`,
    {}
  );
}

export function deleteRemoteAccount(
  id: string,
  input?: { delete_managed_channels?: boolean; sync_remote_delete?: boolean }
): Promise<void> {
  return http<void>("DELETE", `/api/remote/accounts/${encodeURIComponent(id)}`, input);
}

export function createRemoteManagedChannel(
  accountId: string,
  input: CreateRemoteManagedChannelInput
): Promise<CreateRemoteManagedChannelResponse> {
  return http<CreateRemoteManagedChannelResponse>(
    "POST",
    `/api/remote/accounts/${encodeURIComponent(accountId)}/managed_channel`,
    input
  );
}

export function openInBrowser(url: string): Promise<void> {
  return http<void>("POST", "/api/system/open", { url });
}

export function openDataDir(): Promise<void> {
  return http<void>("POST", "/api/system/open_data_dir");
}

export function listRoutes(): Promise<Route[]> {
  return http<Route[]>("GET", "/api/routes");
}

export function createRoute(input: CreateRouteInput): Promise<Route> {
  return http<Route>("POST", "/api/routes", input);
}

export function updateRoute(id: string, input: UpdateRouteInput): Promise<void> {
  return http<void>("PUT", `/api/routes/${encodeURIComponent(id)}`, input);
}

export function deleteRoute(id: string): Promise<void> {
  return http<void>("DELETE", `/api/routes/${encodeURIComponent(id)}`);
}

export function listRouteChannels(routeId: string): Promise<RouteChannel[]> {
  return http<RouteChannel[]>("GET", `/api/routes/${encodeURIComponent(routeId)}/channels`);
}

export function reorderRouteChannels(routeId: string, channelIds: string[]): Promise<void> {
  return http<void>("POST", `/api/routes/${encodeURIComponent(routeId)}/channels/reorder`, {
    channel_ids: channelIds
  });
}

export function pricingStatus(): Promise<PricingStatus> {
  return http<PricingStatus>("GET", "/api/pricing/status");
}

export function pricingSync(): Promise<PricingSyncResponse> {
  return http<PricingSyncResponse>("POST", "/api/pricing/sync");
}

export function pricingModels(query: string, limit = 200): Promise<PricingModel[]> {
  const p = new URLSearchParams();
  if (query.trim().length > 0) p.set("query", query.trim());
  p.set("limit", String(limit));
  return http<PricingModel[]>("GET", `/api/pricing/models?${p.toString()}`);
}

export function getUpdateStatus(): Promise<UpdateStatus> {
  return http<UpdateStatus>("GET", "/api/update/status");
}

export function checkUpdate(): Promise<UpdateCheck> {
  return http<UpdateCheck>("POST", "/api/update/check");
}

export function getUpdateChangelog(version: string, locale?: string): Promise<ChangelogOverview> {
  const p = new URLSearchParams();
  p.set("version", version);
  if (locale) p.set("locale", locale);
  return http<ChangelogOverview>("GET", `/api/update/changelog?${p.toString()}`);
}

export function downloadUpdate(): Promise<UpdateDownloadResponse> {
  return http<UpdateDownloadResponse>("POST", "/api/update/download");
}

export function ignoreUpdate(version: string): Promise<UpdateStatus> {
  return http<UpdateStatus>("POST", "/api/update/ignore", { version });
}

function statsQueryToParams(query?: StatsQuery): string {
  const p = new URLSearchParams();
  if (query?.range) p.set("range", query.range);
  if (query?.start_ms !== undefined) p.set("start_ms", String(query.start_ms));
  if (query?.end_ms !== undefined) p.set("end_ms", String(query.end_ms));
  const s = p.toString();
  return s.length > 0 ? `?${s}` : "";
}

export function statsSummary(query?: StatsQuery): Promise<StatsSummary> {
  return http<StatsSummary>("GET", `/api/stats/summary${statsQueryToParams(query)}`);
}

export function statsChannels(query?: StatsQuery): Promise<StatsChannels> {
  return http<StatsChannels>("GET", `/api/stats/channels${statsQueryToParams(query)}`);
}

export function statsTrend(range: "month"): Promise<StatsTrend> {
  return http<StatsTrend>("GET", `/api/stats/trend?range=${encodeURIComponent(range)}`);
}

export function usageList(
  query: Partial<{
    start_ms: number;
    end_ms: number;
    protocol: Protocol;
    channel_id: string;
    model: string;
    request_id: string;
    success: boolean;
    limit: number;
    offset: number;
  }>
): Promise<UsageListResult> {
  const p = new URLSearchParams();
  if (query.start_ms !== undefined) p.set("start_ms", String(query.start_ms));
  if (query.end_ms !== undefined) p.set("end_ms", String(query.end_ms));
  if (query.protocol) p.set("protocol", query.protocol);
  if (query.channel_id) p.set("channel_id", query.channel_id);
  if (query.model) p.set("model", query.model);
  if (query.request_id) p.set("request_id", query.request_id);
  if (query.success !== undefined) p.set("success", String(query.success));
  if (query.limit !== undefined) p.set("limit", String(query.limit));
  if (query.offset !== undefined) p.set("offset", String(query.offset));
  return http<UsageListResult>("GET", `/api/usage/list?${p.toString()}`);
}

export type DbSize = {
  path: string;
  db_bytes: number;
  wal_bytes: number;
  shm_bytes: number;
  total_bytes: number;
};

export function getDbSize(): Promise<DbSize> {
  return http<DbSize>("GET", "/api/maintenance/db_size");
}

export type LogsSize = {
  path: string;
  total_bytes: number;
  file_count: number;
};

export function getLogsSize(): Promise<LogsSize> {
  return http<LogsSize>("GET", "/api/maintenance/logs/size");
}

export type RecordsClearMode = "date_range" | "errors" | "all";

export type ClearRecordsResult = {
  usage_events_deleted: number;
  vacuumed: boolean;
};

export function clearRecords(input: {
  mode: RecordsClearMode;
  start_ms?: number;
  end_ms?: number;
}): Promise<ClearRecordsResult> {
  return http<ClearRecordsResult>("POST", "/api/maintenance/records/clear", input);
}

export type LogsClearMode = "date_range" | "all";

export type ClearLogsResult = {
  deleted_files: number;
  truncated_files: number;
};

export function clearLogs(input: {
  mode: LogsClearMode;
  start_date?: string;
  end_date?: string;
}): Promise<ClearLogsResult> {
  return http<ClearLogsResult>("POST", "/api/maintenance/logs/clear", input);
}
