use rusqlite::Connection;
use std::collections::{HashMap, HashSet};

use super::DEFAULT_LEGACY_PLATFORM;
use super::inspect::{table_exists, table_has_column};

#[derive(Debug, Clone)]
pub(super) struct LegacyPairingTokenRow {
    pub(super) id: i64,
    pub(super) platform: String,
    pub(super) token_hash: String,
    pub(super) token_hint: Option<String>,
    pub(super) created_at_ms: i64,
    pub(super) expires_at_ms: i64,
    pub(super) used_at_ms: Option<i64>,
    pub(super) used_by_platform: Option<String>,
    pub(super) used_by_sender_id: Option<String>,
}

#[derive(Debug, Clone)]
pub(super) struct LegacyBindingRow {
    pub(super) id: i64,
    pub(super) user_id: Option<i64>,
    pub(super) platform: String,
    pub(super) platform_user_id: String,
    pub(super) display_name: Option<String>,
    pub(super) bound_at_ms: i64,
    pub(super) is_active: bool,
}

#[derive(Debug, Clone)]
pub(super) struct LegacyKnownProjectRow {
    pub(super) path: String,
    pub(super) display_name: String,
    pub(super) created_at_ms: i64,
    pub(super) updated_at_ms: i64,
}

#[derive(Debug, Clone)]
pub(super) struct LegacySessionRow {
    pub(super) id: i64,
    pub(super) alias: Option<String>,
    pub(super) owner: Option<i64>,
    pub(super) platform: Option<String>,
    pub(super) cli_type: String,
    pub(super) cli_session_ref: Option<String>,
    pub(super) project_id: Option<String>,
    pub(super) project_name: String,
    pub(super) working_dir: String,
    pub(super) permission_mode: String,
    pub(super) status: String,
    pub(super) is_default: bool,
    pub(super) created_at_ms: i64,
    pub(super) last_active_ms: Option<i64>,
}

#[derive(Debug, Clone)]
pub(super) struct MigratedSessionRow {
    pub(super) id: Option<i64>,
    pub(super) alias: Option<String>,
    pub(super) platform: String,
    pub(super) cli_type: String,
    pub(super) cli_session_ref: Option<String>,
    pub(super) project_id: Option<String>,
    pub(super) project_name: String,
    pub(super) working_dir: String,
    pub(super) permission_mode: String,
    pub(super) status: String,
    pub(super) is_default: bool,
    pub(super) created_at_ms: i64,
    pub(super) last_active_ms: Option<i64>,
}

#[derive(Debug, Clone)]
pub(super) struct LegacyAuditLogRow {
    pub(super) id: i64,
    pub(super) platform: String,
    pub(super) sender_id: String,
    pub(super) chat_id: String,
    pub(super) message_type: String,
    pub(super) content: String,
    pub(super) session_id: Option<i64>,
    pub(super) created_at_ms: i64,
}

pub(super) fn load_legacy_pairing_tokens(
    conn: &Connection,
) -> anyhow::Result<Vec<LegacyPairingTokenRow>> {
    let has_platform = table_has_column(conn, "pairing_tokens", "platform")?;
    let mut stmt = if has_platform {
        conn.prepare(
            r#"
            SELECT
              id,
              platform,
              token_hash,
              token_hint,
              created_at,
              expires_at,
              used_at,
              used_by_platform,
              used_by_sender_id
            FROM pairing_tokens
            ORDER BY id ASC
            "#,
        )?
    } else {
        conn.prepare(
            r#"
            SELECT
              id,
              token_hash,
              token_hint,
              created_at,
              expires_at,
              used_at,
              used_by_platform,
              used_by_sender_id
            FROM pairing_tokens
            ORDER BY id ASC
            "#,
        )?
    };

    if has_platform {
        let rows = stmt.query_map([], |row| {
            Ok(LegacyPairingTokenRow {
                id: row.get(0)?,
                platform: row.get(1)?,
                token_hash: row.get(2)?,
                token_hint: row.get(3)?,
                created_at_ms: row.get(4)?,
                expires_at_ms: row.get(5)?,
                used_at_ms: row.get(6)?,
                used_by_platform: row.get(7)?,
                used_by_sender_id: row.get(8)?,
            })
        })?;
        return rows
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into);
    }

    let rows = stmt.query_map([], |row| {
        let used_by_platform = row.get::<_, Option<String>>(6)?;
        Ok(LegacyPairingTokenRow {
            id: row.get(0)?,
            platform: used_by_platform
                .clone()
                .unwrap_or_else(|| DEFAULT_LEGACY_PLATFORM.to_string()),
            token_hash: row.get(1)?,
            token_hint: row.get(2)?,
            created_at_ms: row.get(3)?,
            expires_at_ms: row.get(4)?,
            used_at_ms: row.get(5)?,
            used_by_platform,
            used_by_sender_id: row.get(7)?,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

pub(super) fn load_legacy_bindings(conn: &Connection) -> anyhow::Result<Vec<LegacyBindingRow>> {
    if !table_exists(conn, "chat_bindings")? {
        return Ok(Vec::new());
    }

    let has_user_id = table_has_column(conn, "chat_bindings", "user_id")?;
    let mut stmt = if has_user_id {
        conn.prepare(
            r#"
            SELECT id, user_id, platform, platform_user_id, display_name, bound_at, is_active
            FROM chat_bindings
            ORDER BY bound_at DESC, id DESC
            "#,
        )?
    } else {
        conn.prepare(
            r#"
            SELECT id, platform, platform_user_id, display_name, bound_at, is_active
            FROM chat_bindings
            ORDER BY bound_at DESC, id DESC
            "#,
        )?
    };

    if has_user_id {
        let rows = stmt.query_map([], |row| {
            Ok(LegacyBindingRow {
                id: row.get(0)?,
                user_id: Some(row.get(1)?),
                platform: row.get(2)?,
                platform_user_id: row.get(3)?,
                display_name: row.get(4)?,
                bound_at_ms: row.get(5)?,
                is_active: row.get::<_, i64>(6)? != 0,
            })
        })?;
        return rows
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into);
    }

    let rows = stmt.query_map([], |row| {
        Ok(LegacyBindingRow {
            id: row.get(0)?,
            user_id: None,
            platform: row.get(1)?,
            platform_user_id: row.get(2)?,
            display_name: row.get(3)?,
            bound_at_ms: row.get(4)?,
            is_active: row.get::<_, i64>(5)? != 0,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

pub(super) fn load_legacy_known_projects(
    conn: &Connection,
) -> anyhow::Result<Vec<LegacyKnownProjectRow>> {
    if !table_exists(conn, "bridge_known_projects")? {
        return Ok(Vec::new());
    }

    let mut stmt = conn.prepare(
        r#"
        SELECT path, display_name, created_at, updated_at
        FROM bridge_known_projects
        ORDER BY updated_at DESC, path ASC
        "#,
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(LegacyKnownProjectRow {
            path: row.get(0)?,
            display_name: row.get(1)?,
            created_at_ms: row.get(2)?,
            updated_at_ms: row.get(3)?,
        })
    })?;

    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

pub(super) fn load_legacy_sessions(conn: &Connection) -> anyhow::Result<Vec<LegacySessionRow>> {
    if !table_exists(conn, "bridge_sessions")? {
        return Ok(Vec::new());
    }

    let has_owner = table_has_column(conn, "bridge_sessions", "owner")?;
    let mut stmt = if has_owner {
        conn.prepare(
            r#"
            SELECT
              id,
              alias,
              owner,
              cli_type,
              cli_session_ref,
              project_id,
              project_name,
              working_dir,
              permission_mode,
              status,
              is_default,
              created_at,
              last_active
            FROM bridge_sessions
            ORDER BY id ASC
            "#,
        )?
    } else {
        conn.prepare(
            r#"
            SELECT
              id,
              alias,
              platform,
              cli_type,
              cli_session_ref,
              project_id,
              project_name,
              working_dir,
              permission_mode,
              status,
              is_default,
              created_at,
              last_active
            FROM bridge_sessions
            ORDER BY id ASC
            "#,
        )?
    };

    if has_owner {
        let rows = stmt.query_map([], |row| {
            Ok(LegacySessionRow {
                id: row.get(0)?,
                alias: row.get(1)?,
                owner: Some(row.get(2)?),
                platform: None,
                cli_type: row.get(3)?,
                cli_session_ref: row.get(4)?,
                project_id: row.get(5)?,
                project_name: row.get(6)?,
                working_dir: row.get(7)?,
                permission_mode: row.get(8)?,
                status: row.get(9)?,
                is_default: row.get::<_, i64>(10)? != 0,
                created_at_ms: row.get(11)?,
                last_active_ms: row.get(12)?,
            })
        })?;
        return rows
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into);
    }

    let rows = stmt.query_map([], |row| {
        Ok(LegacySessionRow {
            id: row.get(0)?,
            alias: row.get(1)?,
            owner: None,
            platform: Some(row.get(2)?),
            cli_type: row.get(3)?,
            cli_session_ref: row.get(4)?,
            project_id: row.get(5)?,
            project_name: row.get(6)?,
            working_dir: row.get(7)?,
            permission_mode: row.get(8)?,
            status: row.get(9)?,
            is_default: row.get::<_, i64>(10)? != 0,
            created_at_ms: row.get(11)?,
            last_active_ms: row.get(12)?,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

pub(super) fn load_legacy_audit_logs(conn: &Connection) -> anyhow::Result<Vec<LegacyAuditLogRow>> {
    if !table_exists(conn, "chat_audit_log")? {
        return Ok(Vec::new());
    }

    let mut stmt = conn.prepare(
        r#"
        SELECT id, platform, sender_id, chat_id, message_type, content, session_id, created_at
        FROM chat_audit_log
        ORDER BY id ASC
        "#,
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(LegacyAuditLogRow {
            id: row.get(0)?,
            platform: row.get(1)?,
            sender_id: row.get(2)?,
            chat_id: row.get(3)?,
            message_type: row.get(4)?,
            content: row.get(5)?,
            session_id: row.get(6)?,
            created_at_ms: row.get(7)?,
        })
    })?;

    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

pub(super) fn choose_latest_binding_per_platform(
    items: Vec<LegacyBindingRow>,
) -> Vec<LegacyBindingRow> {
    let mut chosen = HashMap::<String, LegacyBindingRow>::new();
    for item in items {
        let key = item.platform.to_lowercase();
        match chosen.get(&key) {
            Some(existing) => {
                let replace = (item.is_active as i32, item.bound_at_ms, item.id)
                    > (existing.is_active as i32, existing.bound_at_ms, existing.id);
                if replace {
                    chosen.insert(key, item);
                }
            }
            None => {
                chosen.insert(key, item);
            }
        }
    }
    chosen.into_values().collect()
}

pub(super) fn build_owner_platforms(bindings: &[LegacyBindingRow]) -> HashMap<i64, Vec<String>> {
    let mut owner_platforms = HashMap::<i64, Vec<String>>::new();
    for binding in bindings {
        let Some(user_id) = binding.user_id else {
            continue;
        };
        let platforms = owner_platforms.entry(user_id).or_default();
        if !platforms.iter().any(|item| item == &binding.platform) {
            platforms.push(binding.platform.clone());
        }
    }
    owner_platforms
}

pub(super) fn migrate_legacy_sessions(
    items: Vec<LegacySessionRow>,
    owner_platforms: &HashMap<i64, Vec<String>>,
) -> Vec<MigratedSessionRow> {
    let mut migrated = Vec::new();
    let mut preserved_ids = HashSet::<i64>::new();

    for item in items {
        if let Some(platform) = item.platform.clone() {
            migrated.push(MigratedSessionRow {
                id: Some(item.id),
                alias: item.alias,
                platform,
                cli_type: item.cli_type,
                cli_session_ref: item.cli_session_ref,
                project_id: item.project_id,
                project_name: item.project_name,
                working_dir: item.working_dir,
                permission_mode: item.permission_mode,
                status: item.status,
                is_default: item.is_default,
                created_at_ms: item.created_at_ms,
                last_active_ms: item.last_active_ms,
            });
            preserved_ids.insert(item.id);
            continue;
        }

        let Some(owner) = item.owner else {
            continue;
        };
        let Some(platforms) = owner_platforms.get(&owner) else {
            continue;
        };

        for (index, platform) in platforms.iter().enumerate() {
            migrated.push(MigratedSessionRow {
                id: if index == 0 && preserved_ids.insert(item.id) {
                    Some(item.id)
                } else {
                    None
                },
                alias: item.alias.clone(),
                platform: platform.clone(),
                cli_type: item.cli_type.clone(),
                cli_session_ref: item.cli_session_ref.clone(),
                project_id: item.project_id.clone(),
                project_name: item.project_name.clone(),
                working_dir: item.working_dir.clone(),
                permission_mode: item.permission_mode.clone(),
                status: item.status.clone(),
                is_default: item.is_default,
                created_at_ms: item.created_at_ms,
                last_active_ms: item.last_active_ms,
            });
        }
    }

    migrated
}
