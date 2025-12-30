use rusqlite::types::{FromSql, FromSqlError, ValueRef};
use rusqlite::{OptionalExtension as _, params};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

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
    pub priority: i64,
    pub recharge_currency: RechargeCurrency,
    pub real_multiplier: f64,
    pub enabled: bool,
    pub auto_disabled_until_ms: i64,
    pub use_key_pool: bool,
    pub use_endpoint_pool: bool,
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
        let mut stmt = conn.prepare(
            r#"
            SELECT id, name, protocol, base_url, auth_type, auth_ref, priority, recharge_currency, real_multiplier, enabled, auto_disabled_until_ms, use_key_pool, use_endpoint_pool, created_at_ms, updated_at_ms
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
            Ok(Channel {
                id: row.get(0)?,
                name: row.get(1)?,
                protocol,
                base_url: normalize_base_url(protocol, &base_url),
                auth_type: row.get(4)?,
                auth_ref: row.get(5)?,
                priority: row.get(6)?,
                recharge_currency: row
                    .get::<_, Option<RechargeCurrency>>(7)?
                    .unwrap_or(RechargeCurrency::Cny),
                real_multiplier: row.get::<_, Option<f64>>(8)?.unwrap_or(1.0),
                enabled: row.get::<_, i64>(9)? != 0,
                auto_disabled_until_ms: row.get::<_, Option<i64>>(10)?.unwrap_or(0),
                use_key_pool: row.get::<_, Option<i64>>(11)?.unwrap_or(0) != 0,
                use_endpoint_pool: row.get::<_, Option<i64>>(12)?.unwrap_or(0) != 0,
                created_at_ms: row.get(13)?,
                updated_at_ms: row.get(14)?,
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
    pub base_url: String,
    pub auth_type: Option<String>,
    pub auth_ref: String,
    #[serde(default)]
    pub priority: i64,
    pub recharge_currency: Option<RechargeCurrency>,
    pub real_multiplier: Option<f64>,
    pub enabled: bool,
    #[serde(default)]
    pub use_key_pool: bool,
    #[serde(default)]
    pub use_endpoint_pool: bool,
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
        let base_url = normalize_base_url(input.protocol, &input.base_url);
        let recharge_currency = input.recharge_currency.unwrap_or(RechargeCurrency::Cny);
        let real_multiplier = input.real_multiplier.unwrap_or(1.0);
        conn.execute(
            r#"
            INSERT INTO channels (id, name, protocol, base_url, auth_type, auth_ref, priority, recharge_currency, real_multiplier, enabled, use_key_pool, use_endpoint_pool, created_at_ms, updated_at_ms)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
            "#,
            params![
                id,
                input.name,
                input.protocol.as_str(),
                base_url,
                auth_type,
                input.auth_ref,
                input.priority,
                recharge_currency.as_str(),
                real_multiplier,
                if input.enabled { 1 } else { 0 },
                if input.use_key_pool { 1 } else { 0 },
                if input.use_endpoint_pool { 1 } else { 0 },
                ts,
                ts,
            ],
        )?;

        Ok(Channel {
            id,
            name: input.name,
            protocol: input.protocol,
            base_url,
            auth_type,
            auth_ref: input.auth_ref,
            priority: input.priority,
            recharge_currency,
            real_multiplier,
            enabled: input.enabled,
            auto_disabled_until_ms: 0,
            use_key_pool: input.use_key_pool,
            use_endpoint_pool: input.use_endpoint_pool,
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
    pub auth_type: Option<String>,
    pub auth_ref: Option<String>,
    pub priority: Option<i64>,
    pub recharge_currency: Option<RechargeCurrency>,
    pub real_multiplier: Option<f64>,
    pub enabled: Option<bool>,
    pub use_key_pool: Option<bool>,
    pub use_endpoint_pool: Option<bool>,
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
                SELECT id, name, protocol, base_url, auth_type, auth_ref, priority, recharge_currency, real_multiplier, enabled, auto_disabled_until_ms, use_key_pool, use_endpoint_pool, created_at_ms, updated_at_ms
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
                    priority: row.get(6)?,
                    recharge_currency: row
                        .get::<_, Option<RechargeCurrency>>(7)?
                        .unwrap_or(RechargeCurrency::Cny),
                    real_multiplier: row.get::<_, Option<f64>>(8)?.unwrap_or(1.0),
                    enabled: row.get::<_, i64>(9)? != 0,
                    auto_disabled_until_ms: row.get::<_, Option<i64>>(10)?.unwrap_or(0),
                    use_key_pool: row.get::<_, Option<i64>>(11)?.unwrap_or(0) != 0,
                    use_endpoint_pool: row.get::<_, Option<i64>>(12)?.unwrap_or(0) != 0,
                    created_at_ms: row.get(13)?,
                    updated_at_ms: row.get(14)?,
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
        if let Some(v) = input.base_url {
            channel.base_url = normalize_base_url(channel.protocol, &v);
        }
        if let Some(v) = input.auth_type {
            channel.auth_type = v;
        }
        if let Some(v) = input.auth_ref {
            channel.auth_ref = v;
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
        if let Some(v) = input.use_key_pool {
            channel.use_key_pool = v;
        }
        if let Some(v) = input.use_endpoint_pool {
            channel.use_endpoint_pool = v;
        }
        channel.updated_at_ms = ts;

        let tx = conn.unchecked_transaction()?;
        tx.execute(
            r#"
            UPDATE channels
            SET name = ?2, base_url = ?3, auth_type = ?4, auth_ref = ?5, priority = ?6, recharge_currency = ?7, real_multiplier = ?8, enabled = ?9, auto_disabled_until_ms = ?10, use_key_pool = ?11, use_endpoint_pool = ?12, updated_at_ms = ?13
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
                if channel.use_key_pool { 1 } else { 0 },
                if channel.use_endpoint_pool { 1 } else { 0 },
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
        let mut stmt = conn.prepare(
            r#"
            SELECT id, name, protocol, base_url, auth_type, auth_ref, priority, recharge_currency, real_multiplier, enabled, auto_disabled_until_ms, use_key_pool, use_endpoint_pool, created_at_ms, updated_at_ms
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
                priority: row.get(6)?,
                recharge_currency: row
                    .get::<_, Option<RechargeCurrency>>(7)?
                    .unwrap_or(RechargeCurrency::Cny),
                real_multiplier: row.get::<_, Option<f64>>(8)?.unwrap_or(1.0),
                enabled: row.get::<_, i64>(9)? != 0,
                auto_disabled_until_ms: row.get::<_, Option<i64>>(10)?.unwrap_or(0),
                use_key_pool: row.get::<_, Option<i64>>(11)?.unwrap_or(0) != 0,
                use_endpoint_pool: row.get::<_, Option<i64>>(12)?.unwrap_or(0) != 0,
                created_at_ms: row.get(13)?,
                updated_at_ms: row.get(14)?,
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
        let deleted = tx.execute(r#"DELETE FROM channels WHERE id = ?1"#, params![channel_id])?;
        tx.commit()?;

        if deleted == 0 {
            return Err(anyhow::anyhow!("channel not found"));
        }
        Ok(())
    })
    .await
}

// ========== ChannelKey ==========

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelKey {
    pub id: String,
    pub channel_id: String,
    pub auth_ref: String,
    pub label: Option<String>,
    pub priority: i64,
    pub enabled: bool,
    pub auto_disabled_until_ms: i64,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

pub fn key_is_auto_disabled(key: &ChannelKey, now_ms: i64) -> bool {
    key.auto_disabled_until_ms > now_ms
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateChannelKey {
    pub auth_ref: String,
    pub label: Option<String>,
    #[serde(default)]
    pub priority: i64,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

pub async fn list_channel_keys(db_path: PathBuf, channel_id: String) -> anyhow::Result<Vec<ChannelKey>> {
    with_conn(db_path, move |conn| {
        let mut stmt = conn.prepare(
            r#"
            SELECT id, channel_id, auth_ref, label, priority, enabled, auto_disabled_until_ms, created_at_ms, updated_at_ms
            FROM channel_keys
            WHERE channel_id = ?1
            ORDER BY priority DESC, created_at_ms ASC
            "#,
        )?;
        let rows = stmt.query_map([channel_id], |row| {
            Ok(ChannelKey {
                id: row.get(0)?,
                channel_id: row.get(1)?,
                auth_ref: row.get(2)?,
                label: row.get(3)?,
                priority: row.get(4)?,
                enabled: row.get::<_, i64>(5)? != 0,
                auto_disabled_until_ms: row.get::<_, Option<i64>>(6)?.unwrap_or(0),
                created_at_ms: row.get(7)?,
                updated_at_ms: row.get(8)?,
            })
        })?;

        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    })
    .await
}

pub async fn create_channel_key(
    db_path: PathBuf,
    channel_id: String,
    input: CreateChannelKey,
) -> anyhow::Result<ChannelKey> {
    with_conn(db_path, move |conn| {
        let ts = now_ms();
        let id = Uuid::new_v4().to_string();
        conn.execute(
            r#"
            INSERT INTO channel_keys (id, channel_id, auth_ref, label, priority, enabled, auto_disabled_until_ms, created_at_ms, updated_at_ms)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7, ?8)
            "#,
            params![
                id,
                channel_id,
                input.auth_ref,
                input.label,
                input.priority,
                if input.enabled { 1 } else { 0 },
                ts,
                ts,
            ],
        )?;

        Ok(ChannelKey {
            id,
            channel_id,
            auth_ref: input.auth_ref,
            label: input.label,
            priority: input.priority,
            enabled: input.enabled,
            auto_disabled_until_ms: 0,
            created_at_ms: ts,
            updated_at_ms: ts,
        })
    })
    .await
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateChannelKey {
    pub auth_ref: Option<String>,
    pub label: Option<String>,
    pub priority: Option<i64>,
    pub enabled: Option<bool>,
}

pub async fn update_channel_key(
    db_path: PathBuf,
    key_id: String,
    input: UpdateChannelKey,
) -> anyhow::Result<()> {
    with_conn(db_path, move |conn| {
        let ts = now_ms();
        let clear_failures = input.enabled == Some(true);

        let mut sets = vec!["updated_at_ms = ?2".to_string()];
        let mut param_idx = 3usize;

        if input.auth_ref.is_some() {
            sets.push(format!("auth_ref = ?{param_idx}"));
            param_idx += 1;
        }
        if input.label.is_some() {
            sets.push(format!("label = ?{param_idx}"));
            param_idx += 1;
        }
        if input.priority.is_some() {
            sets.push(format!("priority = ?{param_idx}"));
            param_idx += 1;
        }
        if input.enabled.is_some() {
            sets.push(format!("enabled = ?{param_idx}"));
            if input.enabled == Some(true) {
                sets.push("auto_disabled_until_ms = 0".to_string());
            }
        }

        let sql = format!(
            "UPDATE channel_keys SET {} WHERE id = ?1",
            sets.join(", ")
        );

        let tx = conn.unchecked_transaction()?;

        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = vec![
            Box::new(key_id.clone()),
            Box::new(ts),
        ];
        if let Some(ref v) = input.auth_ref {
            params_vec.push(Box::new(v.clone()));
        }
        if let Some(ref v) = input.label {
            params_vec.push(Box::new(v.clone()));
        }
        if let Some(v) = input.priority {
            params_vec.push(Box::new(v));
        }
        if let Some(v) = input.enabled {
            params_vec.push(Box::new(if v { 1i64 } else { 0i64 }));
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|b| b.as_ref()).collect();
        let updated = tx.execute(&sql, params_refs.as_slice())?;

        if clear_failures {
            tx.execute(
                r#"DELETE FROM key_failures WHERE key_id = ?1"#,
                params![key_id],
            )?;
        }
        tx.commit()?;

        if updated == 0 {
            return Err(anyhow::anyhow!("key not found"));
        }
        Ok(())
    })
    .await
}

pub async fn delete_channel_key(db_path: PathBuf, key_id: String) -> anyhow::Result<()> {
    with_conn(db_path, move |conn| {
        let deleted = conn.execute(r#"DELETE FROM channel_keys WHERE id = ?1"#, params![key_id])?;
        if deleted == 0 {
            return Err(anyhow::anyhow!("key not found"));
        }
        Ok(())
    })
    .await
}

pub async fn set_channel_key_enabled(
    db_path: PathBuf,
    key_id: String,
    enabled: bool,
) -> anyhow::Result<()> {
    with_conn(db_path, move |conn| {
        let ts = now_ms();
        let tx = conn.unchecked_transaction()?;
        let updated = if enabled {
            tx.execute(
                r#"UPDATE channel_keys SET enabled = 1, auto_disabled_until_ms = 0, updated_at_ms = ?2 WHERE id = ?1"#,
                params![key_id, ts],
            )?
        } else {
            tx.execute(
                r#"UPDATE channel_keys SET enabled = 0, updated_at_ms = ?2 WHERE id = ?1"#,
                params![key_id, ts],
            )?
        };
        if enabled {
            tx.execute(r#"DELETE FROM key_failures WHERE key_id = ?1"#, params![key_id])?;
        }
        tx.commit()?;

        if updated == 0 {
            return Err(anyhow::anyhow!("key not found"));
        }
        Ok(())
    })
    .await
}

pub async fn record_key_failure_and_maybe_disable(
    db_path: PathBuf,
    key_id: String,
    now_ms: i64,
    window_minutes: i64,
    failure_times: i64,
    disable_minutes: i64,
) -> anyhow::Result<Option<i64>> {
    if window_minutes < 1 || disable_minutes < 1 || failure_times < 1 {
        anyhow::bail!("auto_disable 配置非法");
    }
    let window_ms = window_minutes.saturating_mul(60_000);
    let disable_ms = disable_minutes.saturating_mul(60_000);

    with_conn(db_path, move |conn| {
        let tx = conn.unchecked_transaction()?;
        let cutoff_ms = now_ms.saturating_sub(window_ms);

        tx.execute(
            r#"DELETE FROM key_failures WHERE key_id = ?1 AND at_ms < ?2"#,
            params![key_id, cutoff_ms],
        )?;
        tx.execute(
            r#"INSERT INTO key_failures (key_id, at_ms) VALUES (?1, ?2)"#,
            params![key_id, now_ms],
        )?;

        let cnt: i64 = tx.query_row(
            r#"SELECT COUNT(*) FROM key_failures WHERE key_id = ?1 AND at_ms >= ?2"#,
            params![key_id, cutoff_ms],
            |row| row.get(0),
        )?;

        if cnt < failure_times {
            tx.commit()?;
            return Ok(None);
        }

        let disabled_until_ms = now_ms.saturating_add(disable_ms);
        tx.execute(
            r#"UPDATE channel_keys SET auto_disabled_until_ms = ?2, updated_at_ms = ?3 WHERE id = ?1"#,
            params![key_id, disabled_until_ms, now_ms],
        )?;
        tx.execute(r#"DELETE FROM key_failures WHERE key_id = ?1"#, params![key_id])?;
        tx.commit()?;
        Ok(Some(disabled_until_ms))
    })
    .await
}

pub async fn clear_key_failures(db_path: PathBuf, key_id: String) -> anyhow::Result<()> {
    with_conn(db_path, move |conn| {
        conn.execute(r#"DELETE FROM key_failures WHERE key_id = ?1"#, params![key_id])?;
        Ok(())
    })
    .await
}

// ========== ChannelEndpoint ==========

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelEndpoint {
    pub id: String,
    pub channel_id: String,
    pub base_url: String,
    pub label: Option<String>,
    pub priority: i64,
    pub enabled: bool,
    pub auto_disabled_until_ms: i64,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

pub fn endpoint_is_auto_disabled(endpoint: &ChannelEndpoint, now_ms: i64) -> bool {
    endpoint.auto_disabled_until_ms > now_ms
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateChannelEndpoint {
    pub base_url: String,
    pub label: Option<String>,
    #[serde(default)]
    pub priority: i64,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

pub async fn list_channel_endpoints(db_path: PathBuf, channel_id: String) -> anyhow::Result<Vec<ChannelEndpoint>> {
    with_conn(db_path, move |conn| {
        let mut stmt = conn.prepare(
            r#"
            SELECT id, channel_id, base_url, label, priority, enabled, auto_disabled_until_ms, created_at_ms, updated_at_ms
            FROM channel_endpoints
            WHERE channel_id = ?1
            ORDER BY priority DESC, created_at_ms ASC
            "#,
        )?;
        let rows = stmt.query_map([channel_id], |row| {
            Ok(ChannelEndpoint {
                id: row.get(0)?,
                channel_id: row.get(1)?,
                base_url: row.get(2)?,
                label: row.get(3)?,
                priority: row.get(4)?,
                enabled: row.get::<_, i64>(5)? != 0,
                auto_disabled_until_ms: row.get::<_, Option<i64>>(6)?.unwrap_or(0),
                created_at_ms: row.get(7)?,
                updated_at_ms: row.get(8)?,
            })
        })?;

        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    })
    .await
}

pub async fn create_channel_endpoint(
    db_path: PathBuf,
    channel_id: String,
    protocol: Protocol,
    input: CreateChannelEndpoint,
) -> anyhow::Result<ChannelEndpoint> {
    with_conn(db_path, move |conn| {
        let ts = now_ms();
        let id = Uuid::new_v4().to_string();
        let base_url = normalize_base_url(protocol, &input.base_url);
        conn.execute(
            r#"
            INSERT INTO channel_endpoints (id, channel_id, base_url, label, priority, enabled, auto_disabled_until_ms, created_at_ms, updated_at_ms)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7, ?8)
            "#,
            params![
                id,
                channel_id,
                base_url,
                input.label,
                input.priority,
                if input.enabled { 1 } else { 0 },
                ts,
                ts,
            ],
        )?;

        Ok(ChannelEndpoint {
            id,
            channel_id,
            base_url,
            label: input.label,
            priority: input.priority,
            enabled: input.enabled,
            auto_disabled_until_ms: 0,
            created_at_ms: ts,
            updated_at_ms: ts,
        })
    })
    .await
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateChannelEndpoint {
    pub base_url: Option<String>,
    pub label: Option<String>,
    pub priority: Option<i64>,
    pub enabled: Option<bool>,
}

pub async fn update_channel_endpoint(
    db_path: PathBuf,
    endpoint_id: String,
    protocol: Protocol,
    input: UpdateChannelEndpoint,
) -> anyhow::Result<()> {
    with_conn(db_path, move |conn| {
        let ts = now_ms();
        let clear_failures = input.enabled == Some(true);

        let mut sets = vec!["updated_at_ms = ?2".to_string()];
        let mut param_idx = 3usize;

        if input.base_url.is_some() {
            sets.push(format!("base_url = ?{param_idx}"));
            param_idx += 1;
        }
        if input.label.is_some() {
            sets.push(format!("label = ?{param_idx}"));
            param_idx += 1;
        }
        if input.priority.is_some() {
            sets.push(format!("priority = ?{param_idx}"));
            param_idx += 1;
        }
        if input.enabled.is_some() {
            sets.push(format!("enabled = ?{param_idx}"));
            if input.enabled == Some(true) {
                sets.push("auto_disabled_until_ms = 0".to_string());
            }
        }

        let sql = format!(
            "UPDATE channel_endpoints SET {} WHERE id = ?1",
            sets.join(", ")
        );

        let tx = conn.unchecked_transaction()?;

        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = vec![
            Box::new(endpoint_id.clone()),
            Box::new(ts),
        ];
        if let Some(ref v) = input.base_url {
            params_vec.push(Box::new(normalize_base_url(protocol, v)));
        }
        if let Some(ref v) = input.label {
            params_vec.push(Box::new(v.clone()));
        }
        if let Some(v) = input.priority {
            params_vec.push(Box::new(v));
        }
        if let Some(v) = input.enabled {
            params_vec.push(Box::new(if v { 1i64 } else { 0i64 }));
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|b| b.as_ref()).collect();
        let updated = tx.execute(&sql, params_refs.as_slice())?;

        if clear_failures {
            tx.execute(
                r#"DELETE FROM endpoint_failures WHERE endpoint_id = ?1"#,
                params![endpoint_id],
            )?;
        }
        tx.commit()?;

        if updated == 0 {
            return Err(anyhow::anyhow!("endpoint not found"));
        }
        Ok(())
    })
    .await
}

pub async fn delete_channel_endpoint(db_path: PathBuf, endpoint_id: String) -> anyhow::Result<()> {
    with_conn(db_path, move |conn| {
        let deleted = conn.execute(r#"DELETE FROM channel_endpoints WHERE id = ?1"#, params![endpoint_id])?;
        if deleted == 0 {
            return Err(anyhow::anyhow!("endpoint not found"));
        }
        Ok(())
    })
    .await
}

pub async fn set_channel_endpoint_enabled(
    db_path: PathBuf,
    endpoint_id: String,
    enabled: bool,
) -> anyhow::Result<()> {
    with_conn(db_path, move |conn| {
        let ts = now_ms();
        let tx = conn.unchecked_transaction()?;
        let updated = if enabled {
            tx.execute(
                r#"UPDATE channel_endpoints SET enabled = 1, auto_disabled_until_ms = 0, updated_at_ms = ?2 WHERE id = ?1"#,
                params![endpoint_id, ts],
            )?
        } else {
            tx.execute(
                r#"UPDATE channel_endpoints SET enabled = 0, updated_at_ms = ?2 WHERE id = ?1"#,
                params![endpoint_id, ts],
            )?
        };
        if enabled {
            tx.execute(r#"DELETE FROM endpoint_failures WHERE endpoint_id = ?1"#, params![endpoint_id])?;
        }
        tx.commit()?;

        if updated == 0 {
            return Err(anyhow::anyhow!("endpoint not found"));
        }
        Ok(())
    })
    .await
}

pub async fn record_endpoint_failure_and_maybe_disable(
    db_path: PathBuf,
    endpoint_id: String,
    now_ms: i64,
    window_minutes: i64,
    failure_times: i64,
    disable_minutes: i64,
) -> anyhow::Result<Option<i64>> {
    if window_minutes < 1 || disable_minutes < 1 || failure_times < 1 {
        anyhow::bail!("auto_disable 配置非法");
    }
    let window_ms = window_minutes.saturating_mul(60_000);
    let disable_ms = disable_minutes.saturating_mul(60_000);

    with_conn(db_path, move |conn| {
        let tx = conn.unchecked_transaction()?;
        let cutoff_ms = now_ms.saturating_sub(window_ms);

        tx.execute(
            r#"DELETE FROM endpoint_failures WHERE endpoint_id = ?1 AND at_ms < ?2"#,
            params![endpoint_id, cutoff_ms],
        )?;
        tx.execute(
            r#"INSERT INTO endpoint_failures (endpoint_id, at_ms) VALUES (?1, ?2)"#,
            params![endpoint_id, now_ms],
        )?;

        let cnt: i64 = tx.query_row(
            r#"SELECT COUNT(*) FROM endpoint_failures WHERE endpoint_id = ?1 AND at_ms >= ?2"#,
            params![endpoint_id, cutoff_ms],
            |row| row.get(0),
        )?;

        if cnt < failure_times {
            tx.commit()?;
            return Ok(None);
        }

        let disabled_until_ms = now_ms.saturating_add(disable_ms);
        tx.execute(
            r#"UPDATE channel_endpoints SET auto_disabled_until_ms = ?2, updated_at_ms = ?3 WHERE id = ?1"#,
            params![endpoint_id, disabled_until_ms, now_ms],
        )?;
        tx.execute(r#"DELETE FROM endpoint_failures WHERE endpoint_id = ?1"#, params![endpoint_id])?;
        tx.commit()?;
        Ok(Some(disabled_until_ms))
    })
    .await
}

pub async fn clear_endpoint_failures(db_path: PathBuf, endpoint_id: String) -> anyhow::Result<()> {
    with_conn(db_path, move |conn| {
        conn.execute(r#"DELETE FROM endpoint_failures WHERE endpoint_id = ?1"#, params![endpoint_id])?;
        Ok(())
    })
    .await
}
