PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;

CREATE TABLE IF NOT EXISTS channels (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  protocol TEXT NOT NULL CHECK(protocol IN ('openai','anthropic','gemini')),
  base_url TEXT NOT NULL,
  auth_type TEXT NOT NULL,
  auth_ref TEXT NOT NULL,
  priority INTEGER NOT NULL DEFAULT 0,
  recharge_multiplier REAL NOT NULL DEFAULT 1.0,
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

CREATE TABLE IF NOT EXISTS channel_failures (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  channel_id TEXT NOT NULL,
  at_ms INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_channel_failures_channel_ts ON channel_failures(channel_id, at_ms);

CREATE TABLE IF NOT EXISTS app_settings (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL,
  updated_at_ms INTEGER NOT NULL
);
