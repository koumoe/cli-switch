import { ApiRequestError, extractErrorCode, extractErrorMessage } from "@/lib/error";
import { logger } from "@/lib/logger";
import { getCurrentLocale } from "@/providers/i18n-provider";
import type {
  ApplyCliToolsProxyConfigRequest,
  AppSettings,
  Channel,
  ChannelCheckinsToday,
  ChannelTestResponse,
  ChangelogOverview,
  ChatBridgeBinding,
  ChatBridgePairingToken,
  ChatBridgeWhatsAppStatus,
  ChatBridgeWeixinStatus,
  ClearLogsInput,
  ClearLogsResult,
  ClearRecordsInput,
  ClearRecordsResult,
  CliToolId,
  CliToolProxyConfigApplyResponse,
  CliToolProxyConfigStatus,
  CliToolsStatus,
  CreateChannelInput,
  CreateChatBridgePairingTokenInput,
  CreateRemoteAccountInput,
  CreateRemoteKeyInput,
  CreateRemoteManagedChannelInput,
  CreateRemoteManagedChannelResponse,
  DbSize,
  DeleteChannelInput,
  DeleteProjectDocumentInput,
  DeleteRemoteAccountInput,
  GetProjectDocumentQuery,
  Health,
  InstallCliToolResponse,
  LogsSize,
  PickFolderInput,
  PickFolderResponse,
  PricingModel,
  PricingStatus,
  PricingSyncResponse,
  ProjectDocument,
  ProjectRecord,
  Protocol,
  RemoteAccount,
  RemoteAccountCheckinsToday,
  RemoteAccountDetection,
  RemoteAccountSystemCheckinResult,
  RemoteGroupOption,
  RemoteKey,
  SaveProjectDocumentInput,
  StatsChannels,
  StatsQuery,
  StatsSummary,
  StatsTrend,
  UpdateRemoteAccountInput,
  UpdateChannelInput,
  UpdateCheck,
  UpdateDownloadResponse,
  UpdateStatus,
  UsageListQuery,
  UsageListResult,
  UserFacingIssuePayload,
} from "@/types/api";

export type * from "@/types/api";

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

export function listProjects(tool: CliToolId): Promise<ProjectRecord[]> {
  const p = new URLSearchParams();
  p.set("tool", tool);
  return http<ProjectRecord[]>("GET", `/api/projects?${p.toString()}`);
}

export function deleteProject(tool: CliToolId, projectId: string): Promise<void> {
  const p = new URLSearchParams();
  p.set("tool", tool);
  p.set("project_id", projectId);
  return http<void>("DELETE", `/api/projects?${p.toString()}`);
}

export function getProjectDocument(
  query: GetProjectDocumentQuery
): Promise<ProjectDocument> {
  const p = new URLSearchParams();
  p.set("tool", query.tool);
  p.set("scope", query.scope);
  if (query.project_id) p.set("project_id", query.project_id);
  return http<ProjectDocument>("GET", `/api/projects/document?${p.toString()}`);
}

export function saveProjectDocument(
  input: SaveProjectDocumentInput
): Promise<ProjectDocument> {
  return http<ProjectDocument>("PUT", "/api/projects/document", input);
}

export function deleteProjectDocument(
  input: DeleteProjectDocumentInput
): Promise<void> {
  const p = new URLSearchParams();
  p.set("tool", input.tool);
  p.set("scope", input.scope);
  if (input.project_id) p.set("project_id", input.project_id);
  if (input.expected_updated_at_ms !== undefined && input.expected_updated_at_ms !== null) {
    p.set("expected_updated_at_ms", String(input.expected_updated_at_ms));
  }
  return http<void>("DELETE", `/api/projects/document?${p.toString()}`);
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
  input?: DeleteChannelInput
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
  input?: DeleteRemoteAccountInput
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

export function usageList(query: UsageListQuery): Promise<UsageListResult> {
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

export function getDbSize(): Promise<DbSize> {
  return http<DbSize>("GET", "/api/maintenance/db_size");
}

export function getLogsSize(): Promise<LogsSize> {
  return http<LogsSize>("GET", "/api/maintenance/logs/size");
}

export function clearRecords(input: ClearRecordsInput): Promise<ClearRecordsResult> {
  return http<ClearRecordsResult>("POST", "/api/maintenance/records/clear", input);
}

export function clearLogs(input: ClearLogsInput): Promise<ClearLogsResult> {
  return http<ClearLogsResult>("POST", "/api/maintenance/logs/clear", input);
}
