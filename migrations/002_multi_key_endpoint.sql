-- 多Key/多端点支持
-- 保持向后兼容：默认使用原有 base_url 和 auth_ref 字段

-- 渠道表新增开关字段
ALTER TABLE channels ADD COLUMN use_key_pool INTEGER NOT NULL DEFAULT 0;
ALTER TABLE channels ADD COLUMN use_endpoint_pool INTEGER NOT NULL DEFAULT 0;

-- 密钥池表（仅当 use_key_pool=1 时使用）
CREATE TABLE IF NOT EXISTS channel_keys (
    id TEXT PRIMARY KEY,
    channel_id TEXT NOT NULL,
    auth_ref TEXT NOT NULL,
    label TEXT,
    priority INTEGER NOT NULL DEFAULT 0,
    enabled INTEGER NOT NULL DEFAULT 1,
    auto_disabled_until_ms INTEGER NOT NULL DEFAULT 0,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    FOREIGN KEY (channel_id) REFERENCES channels(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_channel_keys_channel ON channel_keys(channel_id, priority DESC);

-- 端点池表（仅当 use_endpoint_pool=1 时使用）
CREATE TABLE IF NOT EXISTS channel_endpoints (
    id TEXT PRIMARY KEY,
    channel_id TEXT NOT NULL,
    base_url TEXT NOT NULL,
    label TEXT,
    priority INTEGER NOT NULL DEFAULT 0,
    enabled INTEGER NOT NULL DEFAULT 1,
    auto_disabled_until_ms INTEGER NOT NULL DEFAULT 0,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    FOREIGN KEY (channel_id) REFERENCES channels(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_channel_endpoints_channel ON channel_endpoints(channel_id, priority DESC);

-- 密钥故障记录表
CREATE TABLE IF NOT EXISTS key_failures (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    key_id TEXT NOT NULL,
    at_ms INTEGER NOT NULL,
    FOREIGN KEY (key_id) REFERENCES channel_keys(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_key_failures_key_ts ON key_failures(key_id, at_ms);

-- 端点故障记录表
CREATE TABLE IF NOT EXISTS endpoint_failures (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    endpoint_id TEXT NOT NULL,
    at_ms INTEGER NOT NULL,
    FOREIGN KEY (endpoint_id) REFERENCES channel_endpoints(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_endpoint_failures_endpoint_ts ON endpoint_failures(endpoint_id, at_ms);
