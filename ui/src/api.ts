export type Protocol = "openai" | "anthropic" | "gemini";

export type Health = {
  status: string;
  version?: string;
  listen_addr?: string;
  data_dir?: string;
  db_path?: string;
};

export type Channel = {
  id: string;
  name: string;
  protocol: Protocol;
  base_url: string;
  auth_type: string;
  auth_ref: string;
  priority: number;
  enabled: boolean;
  created_at_ms: number;
  updated_at_ms: number;
};

export type CreateChannelInput = {
  name: string;
  protocol: Protocol;
  base_url: string;
  auth_type: string;
  auth_ref: string;
  priority: number;
  enabled: boolean;
};

export type UpdateChannelInput = Partial<{
  name: string;
  base_url: string;
  auth_type: string;
  auth_ref: string;
  priority: number;
  enabled: boolean;
}>;

export type ChannelTestResponse = {
  reachable: boolean;
  ok: boolean;
  status: number | null;
  latency_ms: number;
  error: string | null;
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
  ts_ms: number;
  protocol: Protocol;
  route_id: string | null;
  channel_id: string;
  model: string | null;
  success: boolean;
  http_status: number | null;
  error_kind: string | null;
  latency_ms: number;
  ttft_ms: number | null;
  prompt_tokens: number | null;
  completion_tokens: number | null;
  total_tokens: number | null;
  estimated_cost_usd: string | null;
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

async function http<T>(method: string, path: string, body?: unknown): Promise<T> {
  const res = await fetch(path, {
    method,
    headers: body ? { "content-type": "application/json" } : undefined,
    body: body ? JSON.stringify(body) : undefined
  });

  if (res.ok) {
    if (res.status === 204) return undefined as T;
    return (await res.json()) as T;
  }

  const text = await res.text().catch(() => "");
  const msg = text.trim().length > 0 ? text : `${method} ${path} failed: ${res.status}`;
  throw new Error(msg);
}

export function getHealth(): Promise<Health> {
  return http<Health>("GET", "/api/health");
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

export function deleteChannel(id: string): Promise<void> {
  return http<void>("DELETE", `/api/channels/${encodeURIComponent(id)}`);
}

export function testChannel(id: string): Promise<ChannelTestResponse> {
  return http<ChannelTestResponse>("POST", `/api/channels/${encodeURIComponent(id)}/test`);
}

export function reorderChannels(protocol: Protocol, channelIds: string[]): Promise<void> {
  return http<void>("POST", "/api/channels/reorder", { protocol, channel_ids: channelIds });
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

export function statsSummary(range: "today" | "month"): Promise<StatsSummary> {
  return http<StatsSummary>("GET", `/api/stats/summary?range=${encodeURIComponent(range)}`);
}

export function statsChannels(range: "today" | "month"): Promise<StatsChannels> {
  return http<StatsChannels>("GET", `/api/stats/channels?range=${encodeURIComponent(range)}`);
}

export function statsTrend(range: "month"): Promise<StatsTrend> {
  return http<StatsTrend>("GET", `/api/stats/trend?range=${encodeURIComponent(range)}`);
}

export function usageRecent(limit = 200): Promise<UsageEvent[]> {
  return http<UsageEvent[]>("GET", `/api/usage/recent?limit=${encodeURIComponent(String(limit))}`);
}
