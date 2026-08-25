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
  retry_times INTEGER NOT NULL DEFAULT 1,
  ignore_channel_protection INTEGER NOT NULL DEFAULT 0,
  recharge_currency TEXT NOT NULL DEFAULT 'CNY' CHECK(recharge_currency IN ('CNY','USD')),
  real_multiplier REAL NOT NULL DEFAULT 1.0,
  managed_by_remote INTEGER NOT NULL DEFAULT 0,
  managed_remote_provider TEXT NULL CHECK(managed_remote_provider IN ('newapi','sub2api')),
  managed_remote_account_id TEXT NULL,
  managed_remote_resource_id TEXT NULL,
  managed_remote_resource_name TEXT NULL,
  managed_remote_group_name TEXT NULL,
  managed_remote_group_id INTEGER NULL,
  enabled INTEGER NOT NULL,
  auto_disabled_until_ms INTEGER NOT NULL DEFAULT 0,
  created_at_ms INTEGER NOT NULL,
  updated_at_ms INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_channels_managed_remote_account
ON channels(managed_remote_provider, managed_remote_account_id);

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

CREATE TABLE IF NOT EXISTS remote_accounts (
  id TEXT PRIMARY KEY,
  provider TEXT NOT NULL CHECK(provider IN ('newapi','sub2api')),
  base_url TEXT NOT NULL,
  api_url TEXT NULL,
  user_id TEXT NOT NULL DEFAULT '',
  user_token TEXT NOT NULL DEFAULT '',
  access_token TEXT NOT NULL DEFAULT '',
  refresh_token TEXT NOT NULL DEFAULT '',
  page_checkin_url TEXT NULL,
  checkin_mode TEXT NOT NULL DEFAULT 'disabled' CHECK(checkin_mode IN ('disabled','system_api','page_open')),
  auto_checkin_enabled INTEGER NOT NULL DEFAULT 0,
  auto_checkin_time TEXT NOT NULL DEFAULT '00:05:00',
  low_balance_alert_threshold REAL NOT NULL DEFAULT 0,
  recharge_currency TEXT NOT NULL DEFAULT 'CNY' CHECK(recharge_currency IN ('CNY','USD')),
  remote_user_id TEXT NULL,
  remote_role NULL,
  remote_username TEXT NULL,
  remote_display_name TEXT NULL,
  remote_group TEXT NULL,
  quota_display_type TEXT NOT NULL DEFAULT 'USD',
  quota_per_unit REAL NOT NULL DEFAULT 500000,
  usd_exchange_rate REAL NOT NULL DEFAULT 1,
  custom_currency_symbol TEXT NULL,
  custom_currency_exchange_rate REAL NOT NULL DEFAULT 1,
  remote_checkin_enabled INTEGER NOT NULL DEFAULT 0,
  remote_turnstile_check_enabled INTEGER NOT NULL DEFAULT 0,
  last_quota INTEGER NULL,
  last_used_quota INTEGER NULL,
  last_balance_amount REAL NULL,
  last_sync_error TEXT NULL,
  reauth_required INTEGER NOT NULL DEFAULT 0,
  last_synced_at_ms INTEGER NULL,
  low_balance_alert_notified INTEGER NOT NULL DEFAULT 0,
  last_balance_alert_at_ms INTEGER NULL,
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at_ms INTEGER NOT NULL,
  updated_at_ms INTEGER NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_remote_accounts_provider_base_user
ON remote_accounts(provider, base_url, user_id);

CREATE TABLE IF NOT EXISTS remote_account_checkins (
  account_id TEXT NOT NULL,
  date TEXT NOT NULL,
  method TEXT NOT NULL DEFAULT 'manual_page' CHECK(method IN ('manual_page','system_api','remote_detected')),
  completed_at_ms INTEGER NOT NULL,
  PRIMARY KEY (account_id, date)
);

CREATE INDEX IF NOT EXISTS idx_remote_account_checkins_date
ON remote_account_checkins(date, completed_at_ms);

CREATE TABLE IF NOT EXISTS remote_account_group_snapshot_states (
  provider TEXT NOT NULL CHECK(provider IN ('newapi','sub2api')),
  account_id TEXT NOT NULL,
  last_synced_at_ms INTEGER NOT NULL,
  updated_at_ms INTEGER NOT NULL,
  PRIMARY KEY (provider, account_id)
);

CREATE TABLE IF NOT EXISTS remote_account_group_snapshots (
  provider TEXT NOT NULL CHECK(provider IN ('newapi','sub2api')),
  account_id TEXT NOT NULL,
  group_key TEXT NOT NULL,
  group_id INTEGER NULL,
  group_name TEXT NOT NULL,
  updated_at_ms INTEGER NOT NULL,
  PRIMARY KEY (provider, account_id, group_key)
);

CREATE INDEX IF NOT EXISTS idx_remote_account_group_snapshots_lookup
ON remote_account_group_snapshots(provider, account_id, group_name);

CREATE TABLE IF NOT EXISTS app_settings (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL,
  updated_at_ms INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS ignored_updates (
  version TEXT PRIMARY KEY,
  ignored_at_ms INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS pairing_tokens (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  platform TEXT NOT NULL,
  token_hash TEXT NOT NULL UNIQUE,
  token_hint TEXT,
  created_at INTEGER NOT NULL,
  expires_at INTEGER NOT NULL,
  used_at INTEGER,
  used_by_platform TEXT,
  used_by_sender_id TEXT
);

CREATE TABLE IF NOT EXISTS chat_bindings (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  platform TEXT NOT NULL,
  platform_user_id TEXT NOT NULL,
  display_name TEXT,
  bound_at INTEGER NOT NULL,
  is_active INTEGER NOT NULL DEFAULT 1,
  UNIQUE(platform, platform_user_id)
);

CREATE TABLE IF NOT EXISTS bridge_known_projects (
  path TEXT PRIMARY KEY,
  display_name TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS bridge_sessions (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  alias TEXT,
  platform TEXT NOT NULL,
  cli_type TEXT NOT NULL,
  cli_session_ref TEXT,
  project_id TEXT,
  project_name TEXT NOT NULL,
  working_dir TEXT NOT NULL,
  permission_mode TEXT NOT NULL DEFAULT 'safe',
  status TEXT NOT NULL DEFAULT 'idle',
  is_default INTEGER NOT NULL DEFAULT 0,
  created_at INTEGER NOT NULL,
  last_active INTEGER,
  UNIQUE(platform, alias)
);

CREATE INDEX IF NOT EXISTS idx_pairing_tokens_lookup_hint
ON pairing_tokens(token_hint, expires_at, used_at);

CREATE INDEX IF NOT EXISTS idx_chat_bindings_bound_at
ON chat_bindings(bound_at DESC);

CREATE INDEX IF NOT EXISTS idx_bridge_sessions_platform_last_active
ON bridge_sessions(platform, status, last_active DESC);
