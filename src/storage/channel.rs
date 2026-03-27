use rusqlite::types::{FromSql, FromSqlError, ValueRef};
use rusqlite::{OptionalExtension as _, params};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

use super::protocol::normalize_base_url;
use super::{Protocol, StorageError, now_ms, with_conn};

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
    pub checkin_url: Option<String>,
    pub priority: i64,
    pub recharge_currency: RechargeCurrency,
    pub real_multiplier: f64,
    pub managed_by_newapi: bool,
    pub newapi_account_id: Option<String>,
    pub newapi_channel_id: Option<i64>,
    pub newapi_token_id: Option<i64>,
    pub newapi_token_name: Option<String>,
    pub newapi_group: Option<String>,
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
        let mut stmt = conn.prepare(
            r#"
            SELECT id, name, protocol, base_url, auth_type, auth_ref, checkin_url, priority, recharge_currency, real_multiplier,
                   managed_by_newapi, newapi_account_id, newapi_channel_id, newapi_token_id, newapi_token_name, newapi_group,
                   enabled, auto_disabled_until_ms, created_at_ms, updated_at_ms
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
                checkin_url: row.get(6)?,
                priority: row.get(7)?,
                recharge_currency: row.get(8)?,
                real_multiplier: row.get(9)?,
                managed_by_newapi: row.get::<_, i64>(10)? != 0,
                newapi_account_id: row.get(11)?,
                newapi_channel_id: row.get(12)?,
                newapi_token_id: row.get(13)?,
                newapi_token_name: row.get(14)?,
                newapi_group: row.get(15)?,
                enabled: row.get::<_, i64>(16)? != 0,
                auto_disabled_until_ms: row.get(17)?,
                created_at_ms: row.get(18)?,
                updated_at_ms: row.get(19)?,
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
    pub checkin_url: Option<String>,
    #[serde(default)]
    pub priority: i64,
    pub recharge_currency: Option<RechargeCurrency>,
    pub real_multiplier: Option<f64>,
    pub enabled: bool,
    #[serde(default)]
    pub managed_by_newapi: Option<bool>,
    pub newapi_account_id: Option<String>,
    pub newapi_channel_id: Option<i64>,
    pub newapi_token_id: Option<i64>,
    pub newapi_token_name: Option<String>,
    pub newapi_group: Option<String>,
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
        let checkin_url = input
            .checkin_url
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let recharge_currency = input.recharge_currency.unwrap_or(RechargeCurrency::Cny);
        let real_multiplier = input.real_multiplier.unwrap_or(1.0);
        let managed_by_newapi = input.managed_by_newapi.unwrap_or(false);
        let newapi_account_id = input
            .newapi_account_id
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let newapi_token_name = input
            .newapi_token_name
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let newapi_group = input
            .newapi_group
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        conn.execute(
            r#"
            INSERT INTO channels (
                id, name, protocol, base_url, auth_type, auth_ref, checkin_url, priority, recharge_currency, real_multiplier,
                managed_by_newapi, newapi_account_id, newapi_channel_id, newapi_token_id, newapi_token_name, newapi_group,
                enabled, created_at_ms, updated_at_ms
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)
            "#,
            params![
                id,
                input.name,
                input.protocol.as_str(),
                base_url,
                auth_type,
                input.auth_ref,
                checkin_url,
                input.priority,
                recharge_currency.as_str(),
                real_multiplier,
                if managed_by_newapi { 1 } else { 0 },
                newapi_account_id,
                input.newapi_channel_id,
                input.newapi_token_id,
                newapi_token_name,
                newapi_group,
                if input.enabled { 1 } else { 0 },
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
            checkin_url,
            priority: input.priority,
            recharge_currency,
            real_multiplier,
            managed_by_newapi,
            newapi_account_id,
            newapi_channel_id: input.newapi_channel_id,
            newapi_token_id: input.newapi_token_id,
            newapi_token_name,
            newapi_group,
            enabled: input.enabled,
            auto_disabled_until_ms: 0,
            created_at_ms: ts,
            updated_at_ms: ts,
        })
    })
    .await
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct UpdateChannel {
    pub name: Option<String>,
    pub base_url: Option<String>,
    pub auth_type: Option<String>,
    pub auth_ref: Option<String>,
    pub checkin_url: Option<String>,
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
                SELECT id, name, protocol, base_url, auth_type, auth_ref, checkin_url, priority, recharge_currency, real_multiplier,
                       managed_by_newapi, newapi_account_id, newapi_channel_id, newapi_token_id, newapi_token_name, newapi_group,
                       enabled, auto_disabled_until_ms, created_at_ms, updated_at_ms
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
                    checkin_url: row.get(6)?,
                    priority: row.get(7)?,
                    recharge_currency: row.get(8)?,
                    real_multiplier: row.get(9)?,
                    managed_by_newapi: row.get::<_, i64>(10)? != 0,
                    newapi_account_id: row.get(11)?,
                    newapi_channel_id: row.get(12)?,
                    newapi_token_id: row.get(13)?,
                    newapi_token_name: row.get(14)?,
                    newapi_group: row.get(15)?,
                    enabled: row.get::<_, i64>(16)? != 0,
                    auto_disabled_until_ms: row.get(17)?,
                    created_at_ms: row.get(18)?,
                    updated_at_ms: row.get(19)?,
                })
            });

            match row {
                Ok(v) => v,
                Err(rusqlite::Error::QueryReturnedNoRows) => {
                    return Err(StorageError::ChannelNotFound {
                        channel_id: channel_id.clone(),
                    }
                    .into());
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
        if let Some(v) = input.checkin_url {
            channel.checkin_url = Some(v.trim().to_string()).filter(|s| !s.is_empty());
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
        tx.execute(
            r#"
            UPDATE channels
            SET name = ?2, base_url = ?3, auth_type = ?4, auth_ref = ?5, checkin_url = ?6, priority = ?7, recharge_currency = ?8, real_multiplier = ?9,
                managed_by_newapi = ?10, newapi_account_id = ?11, newapi_channel_id = ?12, newapi_token_id = ?13, newapi_token_name = ?14, newapi_group = ?15,
                enabled = ?16, auto_disabled_until_ms = ?17, updated_at_ms = ?18
            WHERE id = ?1
            "#,
            params![
                channel.id,
                channel.name,
                channel.base_url,
                channel.auth_type,
                channel.auth_ref,
                channel.checkin_url,
                channel.priority,
                channel.recharge_currency.as_str(),
                channel.real_multiplier,
                if channel.managed_by_newapi { 1 } else { 0 },
                channel.newapi_account_id,
                channel.newapi_channel_id,
                channel.newapi_token_id,
                channel.newapi_token_name,
                channel.newapi_group,
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
            return Err(StorageError::ChannelNotFound {
                channel_id: channel_id.clone(),
            }
            .into());
        }
        Ok(())
    })
    .await
}

pub async fn get_channel(db_path: PathBuf, channel_id: String) -> anyhow::Result<Option<Channel>> {
    with_conn(db_path, move |conn| {
        let mut stmt = conn.prepare(
            r#"
            SELECT id, name, protocol, base_url, auth_type, auth_ref, checkin_url, priority, recharge_currency, real_multiplier,
                   managed_by_newapi, newapi_account_id, newapi_channel_id, newapi_token_id, newapi_token_name, newapi_group,
                   enabled, auto_disabled_until_ms, created_at_ms, updated_at_ms
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
                checkin_url: row.get(6)?,
                priority: row.get(7)?,
                recharge_currency: row.get(8)?,
                real_multiplier: row.get(9)?,
                managed_by_newapi: row.get::<_, i64>(10)? != 0,
                newapi_account_id: row.get(11)?,
                newapi_channel_id: row.get(12)?,
                newapi_token_id: row.get(13)?,
                newapi_token_name: row.get(14)?,
                newapi_group: row.get(15)?,
                enabled: row.get::<_, i64>(16)? != 0,
                auto_disabled_until_ms: row.get(17)?,
                created_at_ms: row.get(18)?,
                updated_at_ms: row.get(19)?,
            })
        })
        .optional()
        .map_err(Into::into)
    })
    .await
}

fn compact_protocol_priorities(
    tx: &rusqlite::Transaction<'_>,
    protocol: Protocol,
    ts: i64,
) -> anyhow::Result<()> {
    let ordered = {
        let mut stmt = tx.prepare(
            r#"
            SELECT id, priority
            FROM channels
            WHERE protocol = ?1
            ORDER BY priority DESC, name ASC, created_at_ms ASC, id ASC
            "#,
        )?;
        let rows = stmt.query_map([protocol.as_str()], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };

    let total = ordered.len() as i64;
    for (idx, (channel_id, current_priority)) in ordered.into_iter().enumerate() {
        let next_priority = total - (idx as i64);
        if current_priority == next_priority {
            continue;
        }
        tx.execute(
            r#"
            UPDATE channels
            SET priority = ?2, updated_at_ms = ?3
            WHERE id = ?1
            "#,
            params![channel_id, next_priority, ts],
        )?;
    }

    Ok(())
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
            return Err(StorageError::ChannelReorderMismatch { reason: "length" }.into());
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
                return Err(StorageError::ChannelReorderMismatch { reason: "coverage" }.into());
            }
            let unknown_id = incoming_set
                .difference(&all_set)
                .next()
                .cloned()
                .unwrap_or_else(|| "<unknown>".to_string());
            return Err(StorageError::ChannelNotFound {
                channel_id: unknown_id,
            }
            .into());
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
        let protocol: Protocol = match tx.query_row(
            r#"SELECT protocol FROM channels WHERE id = ?1"#,
            params![&channel_id],
            |row| row.get(0),
        ) {
            Ok(protocol) => protocol,
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                return Err(StorageError::ChannelNotFound { channel_id }.into());
            }
            Err(err) => return Err(err.into()),
        };

        tx.execute(
            r#"DELETE FROM route_channels WHERE channel_id = ?1"#,
            params![&channel_id],
        )?;
        let deleted = tx.execute(
            r#"DELETE FROM channels WHERE id = ?1"#,
            params![&channel_id],
        )?;
        compact_protocol_priorities(&tx, protocol, now_ms())?;
        tx.commit()?;

        if deleted == 0 {
            return Err(StorageError::ChannelNotFound { channel_id }.into());
        }
        Ok(())
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage;
    use std::path::{Path, PathBuf};

    fn remove_sqlite_artifacts(path: &Path) {
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(PathBuf::from(format!("{}-wal", path.display())));
        let _ = std::fs::remove_file(PathBuf::from(format!("{}-shm", path.display())));
    }

    fn temp_db_path() -> PathBuf {
        std::env::temp_dir().join(format!(
            "cliswitch-test-channel-priority-{}.sqlite",
            uuid::Uuid::new_v4()
        ))
    }

    fn make_channel(name: &str, priority: i64) -> CreateChannel {
        CreateChannel {
            name: name.to_string(),
            protocol: Protocol::Openai,
            base_url: "https://api.openai.com".to_string(),
            auth_type: Some("auto".to_string()),
            auth_ref: format!("sk-{name}"),
            checkin_url: None,
            priority,
            recharge_currency: Some(RechargeCurrency::Cny),
            real_multiplier: Some(1.0),
            enabled: true,
            managed_by_newapi: Some(false),
            newapi_account_id: None,
            newapi_channel_id: None,
            newapi_token_id: None,
            newapi_token_name: None,
            newapi_group: None,
        }
    }

    #[tokio::test]
    async fn delete_channel_compacts_priorities_without_reordering_remaining_channels() {
        let db_path = temp_db_path();
        remove_sqlite_artifacts(&db_path);
        storage::init_db(&db_path).unwrap();

        let c1 = create_channel(db_path.clone(), make_channel("alpha", 5))
            .await
            .unwrap();
        let c2 = create_channel(db_path.clone(), make_channel("bravo", 4))
            .await
            .unwrap();
        let c3 = create_channel(db_path.clone(), make_channel("charlie", 2))
            .await
            .unwrap();
        let c4 = create_channel(db_path.clone(), make_channel("delta", 1))
            .await
            .unwrap();

        delete_channel(db_path.clone(), c2.id).await.unwrap();

        let channels = list_channels(db_path.clone()).await.unwrap();
        let openai = channels
            .into_iter()
            .filter(|channel| channel.protocol == Protocol::Openai)
            .collect::<Vec<_>>();

        assert_eq!(
            openai
                .iter()
                .map(|channel| channel.name.as_str())
                .collect::<Vec<_>>(),
            vec![c1.name.as_str(), c3.name.as_str(), c4.name.as_str()]
        );
        assert_eq!(
            openai
                .iter()
                .map(|channel| channel.priority)
                .collect::<Vec<_>>(),
            vec![3, 2, 1]
        );

        remove_sqlite_artifacts(&db_path);
    }
}
