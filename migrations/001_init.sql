PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;

CREATE TABLE IF NOT EXISTS channels (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  protocol TEXT NOT NULL CHECK(protocol IN ('openai','anthropic','gemini')),
  base_url TEXT NOT NULL,
  auth_type TEXT NOT NULL,
  auth_ref TEXT NOT NULL,
  checkin_url TEXT NULL,
  priority INTEGER NOT NULL DEFAULT 0,
  recharge_currency TEXT NOT NULL DEFAULT 'CNY' CHECK(recharge_currency IN ('CNY','USD')),
  real_multiplier REAL NOT NULL DEFAULT 1.0,
  enabled INTEGER NOT NULL,
  auto_disabled_until_ms INTEGER NOT NULL DEFAULT 0,
  created_at_ms INTEGER NOT NULL,
  updated_at_ms INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS routes (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  protocol TEXT NOT NULL CHECK(protocol IN ('openai','anthropic','gemini')),
  match_model TEXT NULL,
  enabled INTEGER NOT NULL,
  created_at_ms INTEGER NOT NULL,
  updated_at_ms INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS route_channels (
  route_id TEXT NOT NULL,
  channel_id TEXT NOT NULL,
  priority INTEGER NOT NULL,
  cooldown_until_ms INTEGER NULL,
  PRIMARY KEY (route_id, channel_id)
);

CREATE TABLE IF NOT EXISTS pricing_models (
  model_id TEXT PRIMARY KEY,
  prompt_price TEXT NULL,
  completion_price TEXT NULL,
  request_price TEXT NULL,
  cache_read_price TEXT NULL,
  cache_write_price TEXT NULL,
  raw_json TEXT NULL,
  updated_at_ms INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS usage_events (
  id TEXT PRIMARY KEY,
  request_id TEXT NULL,
  ts_ms INTEGER NOT NULL,
  protocol TEXT NOT NULL CHECK(protocol IN ('openai','anthropic','gemini')),
  route_id TEXT NULL,
  channel_id TEXT NOT NULL,
  model TEXT NULL,
  success INTEGER NOT NULL,
  http_status INTEGER NULL,
  error_kind TEXT NULL,
  error_detail TEXT NULL,
  latency_ms INTEGER NOT NULL,
  ttft_ms INTEGER NULL,
  prompt_tokens INTEGER NULL,
  completion_tokens INTEGER NULL,
  total_tokens INTEGER NULL,
  cache_read_tokens INTEGER NULL,
  cache_write_tokens INTEGER NULL,
  estimated_cost_usd TEXT NULL
);

CREATE INDEX IF NOT EXISTS idx_usage_ts ON usage_events(ts_ms);
CREATE INDEX IF NOT EXISTS idx_usage_channel_ts ON usage_events(channel_id, ts_ms);
CREATE INDEX IF NOT EXISTS idx_usage_success_ts ON usage_events(success, ts_ms);
CREATE INDEX IF NOT EXISTS idx_usage_request_ts ON usage_events(request_id, ts_ms);

CREATE TABLE IF NOT EXISTS channel_failures (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  channel_id TEXT NOT NULL,
  at_ms INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_channel_failures_channel_ts ON channel_failures(channel_id, at_ms);

CREATE TABLE IF NOT EXISTS channel_checkins (
  channel_id TEXT NOT NULL,
  date TEXT NOT NULL,
  completed_at_ms INTEGER NOT NULL,
  PRIMARY KEY (channel_id, date)
);

CREATE INDEX IF NOT EXISTS idx_channel_checkins_date ON channel_checkins(date, completed_at_ms);

CREATE TABLE IF NOT EXISTS app_settings (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL,
  updated_at_ms INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS ignored_updates (
  version TEXT PRIMARY KEY,
  ignored_at_ms INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS prompt_projects (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  path TEXT NOT NULL UNIQUE,
  created_at_ms INTEGER NOT NULL,
  updated_at_ms INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS prompt_documents (
  id TEXT PRIMARY KEY,
  tool TEXT NOT NULL CHECK(tool IN ('gemini','claude','codex')),
  scope TEXT NOT NULL CHECK(scope IN ('global','project')),
  scope_key TEXT NOT NULL,
  project_id TEXT NULL,
  content_md TEXT NOT NULL,
  created_at_ms INTEGER NOT NULL,
  updated_at_ms INTEGER NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_prompt_documents_tool_scope_key
ON prompt_documents(tool, scope_key);

CREATE INDEX IF NOT EXISTS idx_prompt_projects_name
ON prompt_projects(name, updated_at_ms DESC);

CREATE INDEX IF NOT EXISTS idx_prompt_documents_project_id
ON prompt_documents(project_id, updated_at_ms DESC);
