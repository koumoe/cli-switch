use anyhow::Context as _;
use argon2::Argon2;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use rusqlite::{OptionalExtension as _, params};
use std::path::PathBuf;

use super::*;
use crate::i18n::AppLocale;

const PAIRING_TOKEN_SCAN_LIMIT: i64 = 256;
const PAIRING_TOKEN_HINT_LEN: usize = 8;

#[derive(Debug)]
struct PairingTokenCandidate {
    id: i64,
    platform: String,
    token_hash: String,
    token_hint: Option<String>,
    expires_at_ms: i64,
    used_at_ms: Option<i64>,
}

#[derive(Debug)]
struct RawChatBinding {
    id: i64,
    platform: String,
    platform_user_id: String,
    display_name: Option<String>,
    preferred_locale: String,
    bound_at_ms: i64,
    is_active: bool,
}

fn hash_pairing_token(token: &str) -> anyhow::Result<String> {
    let salt = SaltString::encode_b64(uuid::Uuid::new_v4().as_bytes())
        .map_err(|err| anyhow::anyhow!("encode pairing token salt failed: {err}"))?;
    let hash = Argon2::default()
        .hash_password(token.as_bytes(), &salt)
        .map_err(|err| anyhow::anyhow!("hash pairing token failed: {err}"))?;
    Ok(hash.to_string())
}

fn pairing_token_hint(token: &str) -> String {
    token.trim().chars().take(PAIRING_TOKEN_HINT_LEN).collect()
}

fn verify_pairing_token(hash: &str, token: &str) -> anyhow::Result<bool> {
    let parsed = PasswordHash::new(hash)
        .map_err(|err| anyhow::anyhow!("parse pairing token hash failed: {err}"))?;
    Ok(Argon2::default()
        .verify_password(token.as_bytes(), &parsed)
        .is_ok())
}

fn build_binding(raw: RawChatBinding) -> anyhow::Result<ChatBridgeBinding> {
    Ok(ChatBridgeBinding {
        id: raw.id,
        platform: ChatPlatform::parse(&raw.platform)?,
        platform_user_id: raw.platform_user_id,
        display_name: raw.display_name,
        preferred_locale: AppLocale::parse_or_default(&raw.preferred_locale)
            .as_str()
            .to_string(),
        bound_at_ms: raw.bound_at_ms,
        is_active: raw.is_active,
    })
}

fn row_to_pairing_token_candidate(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<PairingTokenCandidate> {
    Ok(PairingTokenCandidate {
        id: row.get(0)?,
        platform: row.get(1)?,
        token_hash: row.get(2)?,
        token_hint: row.get(3)?,
        expires_at_ms: row.get(4)?,
        used_at_ms: row.get(5)?,
    })
}

fn load_pairing_token_candidates(
    conn: &rusqlite::Connection,
    token_hint: Option<&str>,
) -> rusqlite::Result<Vec<PairingTokenCandidate>> {
    let mut stmt = match token_hint {
        Some(_) => conn.prepare(
            r#"
            SELECT id, platform, token_hash, token_hint, expires_at, used_at
            FROM pairing_tokens
            WHERE token_hint = ?1
            ORDER BY created_at DESC
            LIMIT ?2
            "#,
        )?,
        None => conn.prepare(
            r#"
            SELECT id, platform, token_hash, token_hint, expires_at, used_at
            FROM pairing_tokens
            ORDER BY created_at DESC
            LIMIT ?1
            "#,
        )?,
    };

    let rows = match token_hint {
        Some(hint) => stmt.query_map(
            params![hint, PAIRING_TOKEN_SCAN_LIMIT],
            row_to_pairing_token_candidate,
        )?,
        None => stmt.query_map(
            params![PAIRING_TOKEN_SCAN_LIMIT],
            row_to_pairing_token_candidate,
        )?,
    };

    rows.collect::<rusqlite::Result<Vec<_>>>()
}

pub async fn create_pairing_token(
    db_path: PathBuf,
    input: CreatePairingTokenInput,
) -> anyhow::Result<ChatBridgePairingToken> {
    with_conn(db_path, move |conn| {
        let now = now_ms();
        let expires_in_minutes = input
            .expires_in_minutes
            .unwrap_or(DEFAULT_PAIRING_TOKEN_EXPIRES_MINUTES);
        anyhow::ensure!(
            expires_in_minutes >= 1,
            "pairing token expiry must be at least 1 minute"
        );

        let expires_at_ms = now.saturating_add(expires_in_minutes.saturating_mul(60_000));
        let token = format!("ck_{}", uuid::Uuid::new_v4().simple());
        let token_hash = hash_pairing_token(&token)?;
        let token_hint = pairing_token_hint(&token);

        conn.execute(
            r#"
            INSERT INTO pairing_tokens (
              platform,
              token_hint,
              token_hash,
              created_at,
              expires_at,
              used_at,
              used_by_platform,
              used_by_sender_id
            )
            VALUES (?1, ?2, ?3, ?4, ?5, NULL, NULL, NULL)
            "#,
            params![
                input.platform.as_str(),
                token_hint,
                token_hash,
                now,
                expires_at_ms
            ],
        )?;

        Ok(ChatBridgePairingToken {
            token,
            platform: input.platform,
            expires_at_ms,
        })
    })
    .await
}

pub async fn consume_pairing_token(
    db_path: PathBuf,
    token: String,
    platform: ChatPlatform,
    platform_user_id: String,
    display_name: Option<String>,
    preferred_locale: String,
) -> anyhow::Result<ChatBridgeBinding> {
    let token = token.trim().to_string();
    let platform_user_id = platform_user_id.trim().to_string();
    let display_name = normalize_optional_text(display_name);
    if token.is_empty() {
        return Err(StorageError::ChatPairingTokenInvalid.into());
    }
    anyhow::ensure!(
        !platform_user_id.is_empty(),
        "platform_user_id is required to bind chat bridge"
    );
    let preferred_locale = AppLocale::parse_or_default(&preferred_locale)
        .as_str()
        .to_string();

    with_conn(db_path, move |conn| {
        let tx = conn
            .unchecked_transaction()
            .context("begin chat bridge token transaction failed")?;

        let hinted_candidates =
            load_pairing_token_candidates(&tx, Some(&pairing_token_hint(&token)))?;
        let mut matched = None;
        for candidate in hinted_candidates {
            if verify_pairing_token(&candidate.token_hash, &token)? {
                matched = Some(candidate);
                break;
            }
        }
        if matched.is_none() {
            let legacy_candidates = load_pairing_token_candidates(&tx, None)?;
            for candidate in legacy_candidates {
                if candidate.token_hint.is_some() {
                    continue;
                }
                if verify_pairing_token(&candidate.token_hash, &token)? {
                    matched = Some(candidate);
                    break;
                }
            }
        }
        let Some(candidate) = matched else {
            return Err(StorageError::ChatPairingTokenInvalid.into());
        };
        if candidate.used_at_ms.is_some() {
            return Err(StorageError::ChatPairingTokenUsed.into());
        }

        let token_platform = ChatPlatform::parse(&candidate.platform)
            .map_err(|_| StorageError::ChatPairingTokenInvalid)?;
        if token_platform != platform {
            return Err(StorageError::ChatPairingTokenPlatformMismatch {
                expected_platform: token_platform.as_str().to_string(),
                actual_platform: platform.as_str().to_string(),
            }
            .into());
        }

        let now = now_ms();
        if candidate.expires_at_ms < now {
            return Err(StorageError::ChatPairingTokenExpired.into());
        }

        let existing_binding = tx
            .query_row(
                r#"
                SELECT id, platform_user_id, is_active
                FROM chat_bindings
                WHERE platform = ?1 AND platform_user_id = ?2
                "#,
                params![platform.as_str(), platform_user_id.as_str()],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)? != 0,
                    ))
                },
            )
            .optional()?;
        let reusable_binding_id = match existing_binding {
            Some((_binding_id, existing_platform_user_id, true)) => {
                return Err(StorageError::ChatBindingAlreadyExists {
                    platform: platform.as_str().to_string(),
                    platform_user_id: existing_platform_user_id,
                }
                .into());
            }
            Some((binding_id, _, false)) => Some(binding_id),
            None => None,
        };

        let binding_id = if let Some(binding_id) = reusable_binding_id {
            tx.execute(
                r#"
                UPDATE chat_bindings
                SET display_name = ?2, preferred_locale = ?3, bound_at = ?4, is_active = 1
                WHERE id = ?1
                "#,
                params![
                    binding_id,
                    display_name.as_deref(),
                    preferred_locale.as_str(),
                    now
                ],
            )?;
            binding_id
        } else {
            tx.execute(
                r#"
                INSERT INTO chat_bindings (
                  platform, platform_user_id, display_name, preferred_locale, bound_at, is_active
                ) VALUES (?1, ?2, ?3, ?4, ?5, 1)
                "#,
                params![
                    platform.as_str(),
                    platform_user_id.as_str(),
                    display_name.as_deref(),
                    preferred_locale.as_str(),
                    now,
                ],
            )?;
            tx.last_insert_rowid()
        };

        let updated = tx.execute(
            r#"
            UPDATE pairing_tokens
            SET used_at = ?2, used_by_platform = ?3, used_by_sender_id = ?4
            WHERE id = ?1 AND used_at IS NULL
            "#,
            params![
                candidate.id,
                now,
                platform.as_str(),
                platform_user_id.as_str()
            ],
        )?;
        if updated == 0 {
            return Err(StorageError::ChatPairingTokenUsed.into());
        }

        tx.commit()?;

        Ok(ChatBridgeBinding {
            id: binding_id,
            platform,
            platform_user_id,
            display_name,
            preferred_locale,
            bound_at_ms: now,
            is_active: true,
        })
    })
    .await
}

pub async fn list_chat_bindings(db_path: PathBuf) -> anyhow::Result<Vec<ChatBridgeBinding>> {
    with_conn(db_path, move |conn| {
        let mut stmt = conn.prepare(
            r#"
            SELECT
              id,
              platform,
              platform_user_id,
              display_name,
              preferred_locale,
              bound_at,
              is_active
            FROM chat_bindings
            WHERE is_active = 1
            ORDER BY bound_at DESC, id DESC
            "#,
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(RawChatBinding {
                id: row.get(0)?,
                platform: row.get(1)?,
                platform_user_id: row.get(2)?,
                display_name: row.get(3)?,
                preferred_locale: row.get(4)?,
                bound_at_ms: row.get(5)?,
                is_active: row.get::<_, i64>(6)? != 0,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
            .into_iter()
            .map(build_binding)
            .collect()
    })
    .await
}

pub async fn deactivate_chat_binding(db_path: PathBuf, binding_id: i64) -> anyhow::Result<()> {
    with_conn(db_path, move |conn| {
        let updated = conn.execute(
            "UPDATE chat_bindings SET is_active = 0 WHERE id = ?1 AND is_active = 1",
            params![binding_id],
        )?;
        if updated == 0 {
            let exists = conn
                .query_row(
                    "SELECT 1 FROM chat_bindings WHERE id = ?1",
                    params![binding_id],
                    |_| Ok(()),
                )
                .optional()?;
            if exists.is_none() {
                return Err(StorageError::ChatBindingNotFound { binding_id }.into());
            }
        }
        Ok(())
    })
    .await
}

pub async fn resolve_chat_binding(
    db_path: PathBuf,
    platform: ChatPlatform,
    platform_user_id: String,
) -> anyhow::Result<Option<ChatBridgeBinding>> {
    with_conn(db_path, move |conn| {
        let raw = conn
            .query_row(
                r#"
                SELECT
                  id,
                  platform,
                  platform_user_id,
                  display_name,
                  preferred_locale,
                  bound_at,
                  is_active
                FROM chat_bindings
                WHERE platform = ?1 AND platform_user_id = ?2 AND is_active = 1
                "#,
                params![platform.as_str(), platform_user_id],
                |row| {
                    Ok(RawChatBinding {
                        id: row.get(0)?,
                        platform: row.get(1)?,
                        platform_user_id: row.get(2)?,
                        display_name: row.get(3)?,
                        preferred_locale: row.get(4)?,
                        bound_at_ms: row.get(5)?,
                        is_active: row.get::<_, i64>(6)? != 0,
                    })
                },
            )
            .optional()?;
        raw.map(build_binding).transpose()
    })
    .await
}

pub async fn update_chat_binding_locale(
    db_path: PathBuf,
    platform: ChatPlatform,
    platform_user_id: String,
    preferred_locale: String,
) -> anyhow::Result<()> {
    let platform_user_id = platform_user_id.trim().to_string();
    let preferred_locale = AppLocale::parse_or_default(&preferred_locale)
        .as_str()
        .to_string();
    with_conn(db_path, move |conn| {
        let updated = conn.execute(
            r#"
            UPDATE chat_bindings
            SET preferred_locale = ?3
            WHERE platform = ?1 AND platform_user_id = ?2 AND is_active = 1
            "#,
            params![
                platform.as_str(),
                platform_user_id.as_str(),
                preferred_locale.as_str()
            ],
        )?;
        if updated == 0 {
            return Err(StorageError::ChatBindingIdentityNotFound {
                platform: platform.as_str().to_string(),
                platform_user_id: platform_user_id.clone(),
            }
            .into());
        }
        Ok(())
    })
    .await
}
