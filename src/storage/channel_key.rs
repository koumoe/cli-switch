use rusqlite::{OptionalExtension as _, params};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

use super::with_conn;

/// 脱敏 API Key，只显示前缀和后4位
pub fn mask_auth_ref(auth_ref: &str) -> String {
    let s = auth_ref.trim();
    let len = s.chars().count();
    if len <= 12 {
        return "••••••••".to_string();
    }
    // 保留前缀（如 sk-proj- 等）和后4位
    let prefix_len = if s.starts_with("sk-proj-") {
        8
    } else if s.starts_with("sk-") {
        3
    } else {
        4
    };
    let suffix_len = 4;
    let prefix: String = s.chars().take(prefix_len).collect();
    let suffix: String = s.chars().skip(len - suffix_len).collect();
    format!("{}••••••••{}", prefix, suffix)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelKey {
    pub id: String,
    pub channel_id: String,
    #[serde(skip_serializing)]
    pub auth_ref: String,
    pub auth_ref_masked: String,
    pub priority: i64,
    pub enabled: bool,
    pub auto_disabled_until_ms: i64,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

pub async fn list_channel_keys(
    db_path: PathBuf,
    channel_id: String,
) -> anyhow::Result<Vec<ChannelKey>> {
    with_conn(db_path, move |conn| {
        let mut stmt = conn.prepare(
            r#"
            SELECT id, channel_id, auth_ref, priority, enabled, auto_disabled_until_ms, created_at_ms, updated_at_ms
            FROM channel_keys
            WHERE channel_id = ?1
            ORDER BY priority DESC, created_at_ms ASC
            "#,
        )?;
        let rows = stmt.query_map([channel_id], |row| {
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
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    })
    .await
}

pub(crate) fn replace_channel_keys_tx(
    tx: &rusqlite::Transaction<'_>,
    channel_id: &str,
    auth_refs_in_priority_order: Vec<String>,
    ts: i64,
) -> anyhow::Result<Vec<ChannelKey>> {
    let mut old_ids = Vec::<String>::new();
    {
        let mut stmt = tx.prepare(r#"SELECT id FROM channel_keys WHERE channel_id = ?1"#)?;
        let mut rows = stmt.query([channel_id])?;
        while let Some(row) = rows.next()? {
            old_ids.push(row.get::<_, String>(0)?);
        }
    }

    for id in &old_ids {
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
        r#"DELETE FROM channel_keys WHERE channel_id = ?1"#,
        params![channel_id],
    )?;

    let mut keys = Vec::<ChannelKey>::new();
    let n = auth_refs_in_priority_order.len() as i64;
    for (idx, auth_ref) in auth_refs_in_priority_order.into_iter().enumerate() {
        let id = Uuid::new_v4().to_string();
        let priority = n - (idx as i64);
        tx.execute(
            r#"
            INSERT INTO channel_keys (id, channel_id, auth_ref, priority, enabled, created_at_ms, updated_at_ms)
            VALUES (?1, ?2, ?3, ?4, 1, ?5, ?5)
            "#,
            params![id, channel_id, auth_ref, priority, ts],
        )?;
        keys.push(ChannelKey {
            id,
            channel_id: channel_id.to_string(),
            auth_ref_masked: mask_auth_ref(&auth_ref),
            auth_ref,
            priority,
            enabled: true,
            auto_disabled_until_ms: 0,
            created_at_ms: ts,
            updated_at_ms: ts,
        });
    }

    Ok(keys)
}

const KEEP_PREFIX: &str = "__KEEP__:";

/// 解析 auth_refs，支持 `__KEEP__:id` 前缀保留现有 Key
/// 返回 (保留的 key_ids, 新的 auth_refs)
pub fn parse_auth_refs_with_keep(auth_refs: Vec<String>) -> (Vec<String>, Vec<String>) {
    let mut keep_ids = Vec::new();
    let mut new_refs = Vec::new();
    for r in auth_refs {
        if let Some(id) = r.strip_prefix(KEEP_PREFIX) {
            keep_ids.push(id.to_string());
        } else {
            new_refs.push(r);
        }
    }
    (keep_ids, new_refs)
}

/// 更新 channel_keys，支持保留现有 Key 和添加新 Key
/// auth_refs 可以是 `__KEEP__:id` 格式（保留现有 Key）或普通字符串（新 Key）
pub(crate) fn update_channel_keys_tx(
    tx: &rusqlite::Transaction<'_>,
    channel_id: &str,
    auth_refs_in_priority_order: Vec<String>,
    ts: i64,
) -> anyhow::Result<Vec<ChannelKey>> {
    // 解析保留和新增
    let (keep_ids, _new_refs) = parse_auth_refs_with_keep(auth_refs_in_priority_order.clone());

    // 获取该 channel 下所有现有 keys
    let mut existing_keys = std::collections::HashMap::<String, (String, bool, i64, i64)>::new();
    {
        let mut stmt = tx.prepare(
            r#"SELECT id, auth_ref, enabled, auto_disabled_until_ms, created_at_ms FROM channel_keys WHERE channel_id = ?1"#
        )?;
        let mut rows = stmt.query([channel_id])?;
        while let Some(row) = rows.next()? {
            let id: String = row.get(0)?;
            let auth_ref: String = row.get(1)?;
            let enabled: bool = row.get::<_, i64>(2)? != 0;
            let auto_disabled_until_ms: i64 = row.get::<_, Option<i64>>(3)?.unwrap_or(0);
            let created_at_ms: i64 = row.get(4)?;
            existing_keys.insert(
                id.clone(),
                (auth_ref, enabled, auto_disabled_until_ms, created_at_ms),
            );
        }
    }

    // 验证保留的 key_ids 都属于当前 channel
    for id in &keep_ids {
        if !existing_keys.contains_key(id) {
            return Err(anyhow::anyhow!("key not found: {id}"));
        }
    }

    // 确定要删除的 keys（不在保留列表中的）
    let keep_set: std::collections::HashSet<String> = keep_ids.iter().cloned().collect();
    let delete_ids: Vec<String> = existing_keys
        .keys()
        .filter(|id| !keep_set.contains(*id))
        .cloned()
        .collect();

    // 删除不再需要的 keys 及其关联数据
    for id in &delete_ids {
        tx.execute(
            r#"DELETE FROM endpoint_key_failures WHERE key_id = ?1"#,
            params![id],
        )?;
        tx.execute(
            r#"DELETE FROM endpoint_key_states WHERE key_id = ?1"#,
            params![id],
        )?;
        tx.execute(r#"DELETE FROM key_failures WHERE key_id = ?1"#, params![id])?;
        tx.execute(r#"DELETE FROM channel_keys WHERE id = ?1"#, params![id])?;
    }

    // 计算总数以分配优先级
    let total_count = auth_refs_in_priority_order.len() as i64;
    let mut keys = Vec::<ChannelKey>::new();

    for (idx, ref_or_keep) in auth_refs_in_priority_order.into_iter().enumerate() {
        let priority = total_count - (idx as i64);

        if let Some(key_id) = ref_or_keep.strip_prefix(KEEP_PREFIX) {
            // 保留现有 Key，更新优先级
            if let Some((auth_ref, enabled, auto_disabled_until_ms, created_at_ms)) =
                existing_keys.get(key_id)
            {
                tx.execute(
                    r#"UPDATE channel_keys SET priority = ?2, updated_at_ms = ?3 WHERE id = ?1"#,
                    params![key_id, priority, ts],
                )?;
                keys.push(ChannelKey {
                    id: key_id.to_string(),
                    channel_id: channel_id.to_string(),
                    auth_ref_masked: mask_auth_ref(auth_ref),
                    auth_ref: auth_ref.clone(),
                    priority,
                    enabled: *enabled,
                    auto_disabled_until_ms: *auto_disabled_until_ms,
                    created_at_ms: *created_at_ms,
                    updated_at_ms: ts,
                });
            }
        } else {
            // 新 Key
            let id = Uuid::new_v4().to_string();
            tx.execute(
                r#"
                INSERT INTO channel_keys (id, channel_id, auth_ref, priority, enabled, created_at_ms, updated_at_ms)
                VALUES (?1, ?2, ?3, ?4, 1, ?5, ?5)
                "#,
                params![id, channel_id, ref_or_keep, priority, ts],
            )?;
            keys.push(ChannelKey {
                id,
                channel_id: channel_id.to_string(),
                auth_ref_masked: mask_auth_ref(&ref_or_keep),
                auth_ref: ref_or_keep,
                priority,
                enabled: true,
                auto_disabled_until_ms: 0,
                created_at_ms: ts,
                updated_at_ms: ts,
            });
        }
    }

    Ok(keys)
}

pub(crate) fn get_primary_key_auth_ref_tx(
    tx: &rusqlite::Transaction<'_>,
    channel_id: &str,
) -> anyhow::Result<Option<String>> {
    tx.query_row(
        r#"
        SELECT auth_ref
        FROM channel_keys
        WHERE channel_id = ?1 AND enabled = 1
        ORDER BY priority DESC, created_at_ms ASC
        LIMIT 1
        "#,
        params![channel_id],
        |row| row.get(0),
    )
    .optional()
    .map_err(Into::into)
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
            r#"
            UPDATE channel_keys
            SET auto_disabled_until_ms = ?2, updated_at_ms = ?3
            WHERE id = ?1
            "#,
            params![key_id, disabled_until_ms, now_ms],
        )?;
        tx.execute(
            r#"DELETE FROM key_failures WHERE key_id = ?1"#,
            params![key_id],
        )?;
        tx.commit()?;
        Ok(Some(disabled_until_ms))
    })
    .await
}

pub async fn clear_key_failures(db_path: PathBuf, key_id: String) -> anyhow::Result<()> {
    with_conn(db_path, move |conn| {
        conn.execute(
            r#"DELETE FROM key_failures WHERE key_id = ?1"#,
            params![key_id],
        )?;
        Ok(())
    })
    .await
}
