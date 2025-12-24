use rusqlite::types::{FromSql, FromSqlError, ValueRef};
use rusqlite::{OptionalExtension as _, params};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

use super::channel_endpoint::{
    ChannelEndpoint, get_primary_endpoint_base_url_tx, replace_channel_endpoints_tx,
};
use super::channel_key::{ChannelKey, get_primary_key_auth_ref_tx, mask_auth_ref, replace_channel_keys_tx, update_channel_keys_tx};
use super::protocol::normalize_base_url;
use super::{Protocol, now_ms, with_conn};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum RechargeCurrency {
    #[serde(rename = "CNY")]
    Cny,
    #[serde(rename = "USD")]
    Usd,
}

impl RechargeCurrency {
    pub fn as_str(self) -> &'static str {
        match self {
            RechargeCurrency::Cny => "CNY",
            RechargeCurrency::Usd => "USD",
        }
    }
}

impl std::fmt::Display for RechargeCurrency {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for RechargeCurrency {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "CNY" => Ok(RechargeCurrency::Cny),
            "USD" => Ok(RechargeCurrency::Usd),
            other => Err(anyhow::anyhow!("未知 recharge_currency：{other}")),
        }
    }
}

impl FromSql for RechargeCurrency {
    fn column_result(value: ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        let s = value.as_str()?;
        s.parse::<RechargeCurrency>()
            .map_err(|e| FromSqlError::Other(e.into_boxed_dyn_error()))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Channel {
    pub id: String,
    pub name: String,
    pub protocol: Protocol,
    pub base_url: String,
    pub auth_type: String,
    pub auth_ref: String,
    pub endpoints: Vec<ChannelEndpoint>,
    pub keys: Vec<ChannelKey>,
    pub priority: i64,
    pub recharge_currency: RechargeCurrency,
    pub real_multiplier: f64,
    pub enabled: bool,
    pub auto_disabled_until_ms: i64,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

pub fn channel_is_auto_disabled(channel: &Channel, now_ms: i64) -> bool {
    channel.auto_disabled_until_ms > now_ms
}

pub async fn record_channel_failure_and_maybe_disable(
    db_path: PathBuf,
    channel_id: String,
    now_ms: i64,
    window_minutes: i64,
    failure_times: i64,
    disable_minutes: i64,
) -> anyhow::Result<Option<i64>> {
    if window_minutes < 1 || disable_minutes < 1 || failure_times < 1 {
        anyhow::bail!(
            "auto_disable 配置非法：window_minutes={window_minutes}, failure_times={failure_times}, disable_minutes={disable_minutes}"
        );
    }
    let window_ms = window_minutes.saturating_mul(60_000);
    let disable_ms = disable_minutes.saturating_mul(60_000);

    with_conn(db_path, move |conn| {
        let tx = conn.unchecked_transaction()?;
        let cutoff_ms = now_ms.saturating_sub(window_ms);

        tx.execute(
            r#"DELETE FROM channel_failures WHERE channel_id = ?1 AND at_ms < ?2"#,
            params![channel_id, cutoff_ms],
        )?;
        tx.execute(
            r#"INSERT INTO channel_failures (channel_id, at_ms) VALUES (?1, ?2)"#,
            params![channel_id, now_ms],
        )?;

        let cnt: i64 = tx.query_row(
            r#"SELECT COUNT(*) FROM channel_failures WHERE channel_id = ?1 AND at_ms >= ?2"#,
            params![channel_id, cutoff_ms],
            |row| row.get(0),
        )?;

        if cnt < failure_times {
            tx.commit()?;
            return Ok(None);
        }

        let disabled_until_ms = now_ms.saturating_add(disable_ms);
        tx.execute(
            r#"
            UPDATE channels
            SET auto_disabled_until_ms = ?2, updated_at_ms = ?3
            WHERE id = ?1
            "#,
            params![channel_id, disabled_until_ms, now_ms],
        )?;
        tx.execute(
            r#"DELETE FROM channel_failures WHERE channel_id = ?1"#,
            params![channel_id],
        )?;
        tx.commit()?;
        Ok(Some(disabled_until_ms))
    })
    .await
}

pub async fn clear_channel_failures(db_path: PathBuf, channel_id: String) -> anyhow::Result<()> {
    with_conn(db_path, move |conn| {
        conn.execute(
            r#"DELETE FROM channel_failures WHERE channel_id = ?1"#,
            params![channel_id],
        )?;
        Ok(())
    })
    .await
}

pub async fn list_channels(db_path: PathBuf) -> anyhow::Result<Vec<Channel>> {
    with_conn(db_path, |conn| {
        let mut endpoints_by_channel =
            std::collections::HashMap::<String, Vec<ChannelEndpoint>>::new();
        {
            let mut stmt = conn.prepare(
                r#"
                SELECT id, channel_id, base_url, priority, enabled, auto_disabled_until_ms, created_at_ms, updated_at_ms
                FROM channel_endpoints
                ORDER BY priority DESC, created_at_ms ASC
                "#,
            )?;
            let mut rows = stmt.query([])?;
            while let Some(row) = rows.next()? {
                let channel_id: String = row.get(1)?;
                endpoints_by_channel
                    .entry(channel_id.clone())
                    .or_default()
                    .push(ChannelEndpoint {
                        id: row.get(0)?,
                        channel_id,
                        base_url: row.get(2)?,
                        priority: row.get(3)?,
                        enabled: row.get::<_, i64>(4)? != 0,
                        auto_disabled_until_ms: row.get::<_, Option<i64>>(5)?.unwrap_or(0),
                        created_at_ms: row.get(6)?,
                        updated_at_ms: row.get(7)?,
                    });
            }
        }

        let mut keys_by_channel = std::collections::HashMap::<String, Vec<ChannelKey>>::new();
        {
            let mut stmt = conn.prepare(
                r#"
                SELECT id, channel_id, auth_ref, priority, enabled, auto_disabled_until_ms, created_at_ms, updated_at_ms
                FROM channel_keys
                ORDER BY priority DESC, created_at_ms ASC
                "#,
            )?;
            let mut rows = stmt.query([])?;
            while let Some(row) = rows.next()? {
                let channel_id: String = row.get(1)?;
                let auth_ref: String = row.get(2)?;
                keys_by_channel
                    .entry(channel_id.clone())
                    .or_default()
                    .push(ChannelKey {
                        id: row.get(0)?,
                        channel_id,
                        auth_ref_masked: mask_auth_ref(&auth_ref),
                        auth_ref,
                        priority: row.get(3)?,
                        enabled: row.get::<_, i64>(4)? != 0,
                        auto_disabled_until_ms: row.get::<_, Option<i64>>(5)?.unwrap_or(0),
                        created_at_ms: row.get(6)?,
                        updated_at_ms: row.get(7)?,
                    });
            }
        }

        let mut stmt = conn.prepare(
            r#"
            SELECT id, name, protocol, base_url, auth_type, auth_ref, priority, recharge_currency, real_multiplier, enabled, auto_disabled_until_ms, created_at_ms, updated_at_ms
            FROM channels
            ORDER BY CASE protocol
              WHEN 'openai' THEN 0
              WHEN 'anthropic' THEN 1
              WHEN 'gemini' THEN 2
              ELSE 9
            END, priority DESC, name ASC
            "#,
        )?;
        let rows = stmt.query_map([], |row| {
            let protocol: Protocol = row.get(2)?;
            let base_url: String = row.get(3)?;
            let channel_id: String = row.get(0)?;
            Ok(Channel {
                id: channel_id.clone(),
                name: row.get(1)?,
                protocol,
                base_url: normalize_base_url(protocol, &base_url),
                auth_type: row.get(4)?,
                auth_ref: row.get(5)?,
                endpoints: endpoints_by_channel.remove(&channel_id).unwrap_or_default(),
                keys: keys_by_channel.remove(&channel_id).unwrap_or_default(),
                priority: row.get(6)?,
                recharge_currency: row
                    .get::<_, Option<RechargeCurrency>>(7)?
                    .unwrap_or(RechargeCurrency::Cny),
                real_multiplier: row.get::<_, Option<f64>>(8)?.unwrap_or(1.0),
                enabled: row.get::<_, i64>(9)? != 0,
                auto_disabled_until_ms: row.get::<_, Option<i64>>(10)?.unwrap_or(0),
                created_at_ms: row.get(11)?,
                updated_at_ms: row.get(12)?,
            })
        })?;

        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    })
    .await
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateChannel {
    pub name: String,
    pub protocol: Protocol,
    pub base_url: Option<String>,
    pub base_urls: Option<Vec<String>>,
    pub auth_type: Option<String>,
    pub auth_ref: Option<String>,
    pub auth_refs: Option<Vec<String>>,
    #[serde(default)]
    pub priority: i64,
    pub recharge_currency: Option<RechargeCurrency>,
    pub real_multiplier: Option<f64>,
    pub enabled: bool,
}

fn normalize_lines(v: &str) -> Vec<String> {
    let mut out = Vec::<String>::new();
    let mut seen = std::collections::HashSet::<String>::new();
    for raw in v.lines() {
        let s = raw.trim();
        if s.is_empty() {
            continue;
        }
        if seen.insert(s.to_string()) {
            out.push(s.to_string());
        }
    }
    out
}

fn coerce_list(list: Option<Vec<String>>, single: Option<String>) -> Vec<String> {
    if let Some(vs) = list
        && !vs.is_empty()
    {
        let joined = vs.join("\n");
        return normalize_lines(&joined);
    }
    if let Some(s) = single {
        return normalize_lines(&s);
    }
    Vec::new()
}

pub async fn create_channel(db_path: PathBuf, input: CreateChannel) -> anyhow::Result<Channel> {
    with_conn(db_path, move |conn| {
        let ts = now_ms();
        let id = Uuid::new_v4().to_string();
        let auth_type = input
            .auth_type
            .unwrap_or_else(|| "auto".to_string())
            .trim()
            .to_string();
        let base_urls = coerce_list(input.base_urls, input.base_url);
        let auth_refs = coerce_list(input.auth_refs, input.auth_ref);
        if base_urls.is_empty() {
            return Err(anyhow::anyhow!("base_url 不能为空"));
        }
        if auth_refs.is_empty() {
            return Err(anyhow::anyhow!("auth_ref 不能为空"));
        }

        let base_url = normalize_base_url(input.protocol, &base_urls[0]);
        let recharge_currency = input.recharge_currency.unwrap_or(RechargeCurrency::Cny);
        let real_multiplier = input.real_multiplier.unwrap_or(1.0);
        let tx = conn.unchecked_transaction()?;
        tx.execute(
            r#"
            INSERT INTO channels (id, name, protocol, base_url, auth_type, auth_ref, priority, recharge_currency, real_multiplier, enabled, created_at_ms, updated_at_ms)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
            "#,
            params![
                id,
                input.name,
                input.protocol.as_str(),
                base_url,
                auth_type,
                auth_refs[0].as_str(),
                input.priority,
                recharge_currency.as_str(),
                real_multiplier,
                if input.enabled { 1 } else { 0 },
                ts,
                ts,
            ],
        )?;

        let endpoints = replace_channel_endpoints_tx(&tx, &id, input.protocol, base_urls, ts)?;
        let keys = replace_channel_keys_tx(&tx, &id, auth_refs, ts)?;

        tx.commit()?;

        Ok(Channel {
            id,
            name: input.name,
            protocol: input.protocol,
            base_url,
            auth_type,
            auth_ref: keys
                .first()
                .map(|k| k.auth_ref.clone())
                .unwrap_or_default(),
            endpoints,
            keys,
            priority: input.priority,
            recharge_currency,
            real_multiplier,
            enabled: input.enabled,
            auto_disabled_until_ms: 0,
            created_at_ms: ts,
            updated_at_ms: ts,
        })
    })
    .await
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateChannel {
    pub name: Option<String>,
    pub base_url: Option<String>,
    pub base_urls: Option<Vec<String>>,
    pub auth_type: Option<String>,
    pub auth_ref: Option<String>,
    pub auth_refs: Option<Vec<String>>,
    pub priority: Option<i64>,
    pub recharge_currency: Option<RechargeCurrency>,
    pub real_multiplier: Option<f64>,
    pub enabled: Option<bool>,
}

pub async fn update_channel(
    db_path: PathBuf,
    channel_id: String,
    input: UpdateChannel,
) -> anyhow::Result<()> {
    with_conn(db_path, move |conn| {
        let ts = now_ms();
        let clear_failures = input.enabled == Some(true);

        let mut channel: Channel = {
            let mut stmt = conn.prepare(
                r#"
                SELECT id, name, protocol, base_url, auth_type, auth_ref, priority, recharge_currency, real_multiplier, enabled, auto_disabled_until_ms, created_at_ms, updated_at_ms
                FROM channels
                WHERE id = ?1
                "#,
            )?;
            let row = stmt.query_row([&channel_id], |row| {
                let protocol: Protocol = row.get(2)?;
                let base_url: String = row.get(3)?;
                Ok(Channel {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    protocol,
                    base_url: normalize_base_url(protocol, &base_url),
                    auth_type: row.get(4)?,
                    auth_ref: row.get(5)?,
                    endpoints: Vec::new(),
                    keys: Vec::new(),
                    priority: row.get(6)?,
                    recharge_currency: row
                        .get::<_, Option<RechargeCurrency>>(7)?
                        .unwrap_or(RechargeCurrency::Cny),
                    real_multiplier: row.get::<_, Option<f64>>(8)?.unwrap_or(1.0),
                    enabled: row.get::<_, i64>(9)? != 0,
                    auto_disabled_until_ms: row.get::<_, Option<i64>>(10)?.unwrap_or(0),
                    created_at_ms: row.get(11)?,
                    updated_at_ms: row.get(12)?,
                })
            });

            match row {
                Ok(v) => v,
                Err(rusqlite::Error::QueryReturnedNoRows) => {
                    return Err(anyhow::anyhow!("channel not found: {channel_id}"));
                }
                Err(e) => return Err(e.into()),
            }
        };

        if let Some(v) = input.name {
            channel.name = v;
        }
        if let Some(v) = input.auth_type {
            channel.auth_type = v;
        }
        if let Some(v) = input.priority {
            channel.priority = v;
        }
        if let Some(v) = input.recharge_currency {
            channel.recharge_currency = v;
        }
        if let Some(v) = input.real_multiplier {
            channel.real_multiplier = v;
        }
        if let Some(v) = input.enabled {
            channel.enabled = v;
            if v {
                channel.auto_disabled_until_ms = 0;
            }
        }
        channel.updated_at_ms = ts;

        let tx = conn.unchecked_transaction()?;

        let mut replaced_endpoints = false;
        let mut replaced_keys = false;

        if input.base_urls.is_some() || input.base_url.is_some() {
            let base_urls = coerce_list(input.base_urls, input.base_url);
            if base_urls.is_empty() {
                return Err(anyhow::anyhow!("base_url 不能为空"));
            }
            let endpoints =
                replace_channel_endpoints_tx(&tx, &channel.id, channel.protocol, base_urls, ts)?;
            channel.base_url = endpoints
                .first()
                .map(|e| e.base_url.clone())
                .unwrap_or_else(|| channel.base_url.clone());
            replaced_endpoints = true;
        }

        if input.auth_refs.is_some() || input.auth_ref.is_some() {
            let auth_refs = coerce_list(input.auth_refs, input.auth_ref);
            if auth_refs.is_empty() {
                return Err(anyhow::anyhow!("auth_ref 不能为空"));
            }
            // 使用支持 __KEEP__: 前缀的更新函数
            let keys = update_channel_keys_tx(&tx, &channel.id, auth_refs, ts)?;
            channel.auth_ref = keys
                .first()
                .map(|k| k.auth_ref.clone())
                .unwrap_or_else(|| channel.auth_ref.clone());
            replaced_keys = true;
        }

        if !replaced_endpoints
            && let Some(v) = get_primary_endpoint_base_url_tx(&tx, &channel.id)?
        {
            channel.base_url = v;
        }
        if !replaced_keys && let Some(v) = get_primary_key_auth_ref_tx(&tx, &channel.id)? {
            channel.auth_ref = v;
        }

        tx.execute(
            r#"
            UPDATE channels
            SET name = ?2, base_url = ?3, auth_type = ?4, auth_ref = ?5, priority = ?6, recharge_currency = ?7, real_multiplier = ?8, enabled = ?9, auto_disabled_until_ms = ?10, updated_at_ms = ?11
            WHERE id = ?1
            "#,
            params![
                channel.id,
                channel.name,
                channel.base_url,
                channel.auth_type,
                channel.auth_ref,
                channel.priority,
                channel.recharge_currency.as_str(),
                channel.real_multiplier,
                if channel.enabled { 1 } else { 0 },
                channel.auto_disabled_until_ms,
                channel.updated_at_ms,
            ],
        )?;
        if clear_failures {
            tx.execute(
                r#"DELETE FROM channel_failures WHERE channel_id = ?1"#,
                params![channel.id],
            )?;
        }
        tx.commit()?;

        Ok(())
    })
    .await
}

pub async fn set_channel_enabled(
    db_path: PathBuf,
    channel_id: String,
    enabled: bool,
) -> anyhow::Result<()> {
    with_conn(db_path, move |conn| {
        let ts = now_ms();
        let tx = conn.unchecked_transaction()?;
        let updated = if enabled {
            tx.execute(
                r#"
                UPDATE channels
                SET enabled = ?2, auto_disabled_until_ms = 0, updated_at_ms = ?3
                WHERE id = ?1
                "#,
                params![channel_id, 1i64, ts],
            )?
        } else {
            tx.execute(
                r#"
                UPDATE channels
                SET enabled = ?2, updated_at_ms = ?3
                WHERE id = ?1
                "#,
                params![channel_id, 0i64, ts],
            )?
        };
        if enabled {
            tx.execute(
                r#"DELETE FROM channel_failures WHERE channel_id = ?1"#,
                params![channel_id],
            )?;
        }
        tx.commit()?;

        if updated == 0 {
            return Err(anyhow::anyhow!("channel not found"));
        }
        Ok(())
    })
    .await
}

pub async fn get_channel(db_path: PathBuf, channel_id: String) -> anyhow::Result<Option<Channel>> {
    with_conn(db_path, move |conn| {
        let endpoints = {
            let mut stmt = conn.prepare(
                r#"
                SELECT id, channel_id, base_url, priority, enabled, auto_disabled_until_ms, created_at_ms, updated_at_ms
                FROM channel_endpoints
                WHERE channel_id = ?1
                ORDER BY priority DESC, created_at_ms ASC
                "#,
            )?;
            let rows = stmt.query_map([&channel_id], |row| {
                Ok(ChannelEndpoint {
                    id: row.get(0)?,
                    channel_id: row.get(1)?,
                    base_url: row.get(2)?,
                    priority: row.get(3)?,
                    enabled: row.get::<_, i64>(4)? != 0,
                    auto_disabled_until_ms: row.get::<_, Option<i64>>(5)?.unwrap_or(0),
                    created_at_ms: row.get(6)?,
                    updated_at_ms: row.get(7)?,
                })
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };

        let keys = {
            let mut stmt = conn.prepare(
                r#"
                SELECT id, channel_id, auth_ref, priority, enabled, auto_disabled_until_ms, created_at_ms, updated_at_ms
                FROM channel_keys
                WHERE channel_id = ?1
                ORDER BY priority DESC, created_at_ms ASC
                "#,
            )?;
            let rows = stmt.query_map([&channel_id], |row| {
                let auth_ref: String = row.get(2)?;
                Ok(ChannelKey {
                    id: row.get(0)?,
                    channel_id: row.get(1)?,
                    auth_ref_masked: mask_auth_ref(&auth_ref),
                    auth_ref,
                    priority: row.get(3)?,
                    enabled: row.get::<_, i64>(4)? != 0,
                    auto_disabled_until_ms: row.get::<_, Option<i64>>(5)?.unwrap_or(0),
                    created_at_ms: row.get(6)?,
                    updated_at_ms: row.get(7)?,
                })
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };

        let mut stmt = conn.prepare(
            r#"
            SELECT id, name, protocol, base_url, auth_type, auth_ref, priority, recharge_currency, real_multiplier, enabled, auto_disabled_until_ms, created_at_ms, updated_at_ms
            FROM channels
            WHERE id = ?1
            "#,
        )?;

        stmt.query_row([channel_id], |row| {
            let protocol: Protocol = row.get(2)?;
            let base_url: String = row.get(3)?;
            Ok(Channel {
                id: row.get(0)?,
                name: row.get(1)?,
                protocol,
                base_url: normalize_base_url(protocol, &base_url),
                auth_type: row.get(4)?,
                auth_ref: row.get(5)?,
                endpoints: endpoints.clone(),
                keys: keys.clone(),
                priority: row.get(6)?,
                recharge_currency: row
                    .get::<_, Option<RechargeCurrency>>(7)?
                    .unwrap_or(RechargeCurrency::Cny),
                real_multiplier: row.get::<_, Option<f64>>(8)?.unwrap_or(1.0),
                enabled: row.get::<_, i64>(9)? != 0,
                auto_disabled_until_ms: row.get::<_, Option<i64>>(10)?.unwrap_or(0),
                created_at_ms: row.get(11)?,
                updated_at_ms: row.get(12)?,
            })
        })
        .optional()
        .map_err(Into::into)
    })
    .await
}

pub async fn reorder_channels(
    db_path: PathBuf,
    protocol: Option<Protocol>,
    channel_ids_in_priority_order: Vec<String>,
) -> anyhow::Result<()> {
    with_conn(db_path, move |conn| {
        let mut all_ids = Vec::<String>::new();
        if let Some(p) = protocol {
            let mut stmt = conn.prepare(r#"SELECT id FROM channels WHERE protocol = ?1"#)?;
            let mut rows = stmt.query([p.as_str()])?;
            while let Some(row) = rows.next()? {
                all_ids.push(row.get::<_, String>(0)?);
            }
        } else {
            let mut stmt = conn.prepare(r#"SELECT id FROM channels"#)?;
            let mut rows = stmt.query([])?;
            while let Some(row) = rows.next()? {
                all_ids.push(row.get::<_, String>(0)?);
            }
        }

        if all_ids.len() != channel_ids_in_priority_order.len() {
            return Err(anyhow::anyhow!("channel reorder mismatch: length"));
        }

        let all_set = all_ids
            .into_iter()
            .collect::<std::collections::HashSet<String>>();
        let incoming_set = channel_ids_in_priority_order
            .iter()
            .cloned()
            .collect::<std::collections::HashSet<String>>();
        if incoming_set != all_set {
            if incoming_set.is_subset(&all_set) {
                return Err(anyhow::anyhow!("channel reorder mismatch: coverage"));
            }
            return Err(anyhow::anyhow!("channel not found"));
        }

        let ts = now_ms();
        let tx = conn.unchecked_transaction()?;
        let n = channel_ids_in_priority_order.len() as i64;
        for (idx, channel_id) in channel_ids_in_priority_order.into_iter().enumerate() {
            let priority = n - (idx as i64);
            tx.execute(
                r#"
                UPDATE channels
                SET priority = ?2, updated_at_ms = ?3
                WHERE id = ?1
                "#,
                params![channel_id, priority, ts],
            )?;
        }
        tx.commit()?;
        Ok(())
    })
    .await
}

pub async fn delete_channel(db_path: PathBuf, channel_id: String) -> anyhow::Result<()> {
    with_conn(db_path, move |conn| {
        let tx = conn.unchecked_transaction()?;
        tx.execute(
            r#"DELETE FROM route_channels WHERE channel_id = ?1"#,
            params![channel_id],
        )?;
        let mut endpoint_ids = Vec::<String>::new();
        {
            let mut stmt =
                tx.prepare(r#"SELECT id FROM channel_endpoints WHERE channel_id = ?1"#)?;
            let mut rows = stmt.query([&channel_id])?;
            while let Some(row) = rows.next()? {
                endpoint_ids.push(row.get::<_, String>(0)?);
            }
        }
        let mut key_ids = Vec::<String>::new();
        {
            let mut stmt = tx.prepare(r#"SELECT id FROM channel_keys WHERE channel_id = ?1"#)?;
            let mut rows = stmt.query([&channel_id])?;
            while let Some(row) = rows.next()? {
                key_ids.push(row.get::<_, String>(0)?);
            }
        }

        for id in &endpoint_ids {
            tx.execute(
                r#"DELETE FROM endpoint_key_failures WHERE endpoint_id = ?1"#,
                params![id],
            )?;
            tx.execute(
                r#"DELETE FROM endpoint_key_states WHERE endpoint_id = ?1"#,
                params![id],
            )?;
            tx.execute(
                r#"DELETE FROM endpoint_failures WHERE endpoint_id = ?1"#,
                params![id],
            )?;
        }
        for id in &key_ids {
            tx.execute(
                r#"DELETE FROM endpoint_key_failures WHERE key_id = ?1"#,
                params![id],
            )?;
            tx.execute(
                r#"DELETE FROM endpoint_key_states WHERE key_id = ?1"#,
                params![id],
            )?;
            tx.execute(r#"DELETE FROM key_failures WHERE key_id = ?1"#, params![id])?;
        }

        tx.execute(
            r#"DELETE FROM channel_endpoints WHERE channel_id = ?1"#,
            params![channel_id],
        )?;
        tx.execute(
            r#"DELETE FROM channel_keys WHERE channel_id = ?1"#,
            params![channel_id],
        )?;
        tx.execute(
            r#"DELETE FROM channel_failures WHERE channel_id = ?1"#,
            params![channel_id],
        )?;
        let deleted = tx.execute(r#"DELETE FROM channels WHERE id = ?1"#, params![channel_id])?;
        tx.commit()?;

        if deleted == 0 {
            return Err(anyhow::anyhow!("channel not found"));
        }
        Ok(())
    })
    .await
}
