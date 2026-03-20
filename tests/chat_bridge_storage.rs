use cliswitch::storage;
use rusqlite::Connection;
use std::path::{Path, PathBuf};

fn remove_sqlite_artifacts(path: &Path) {
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_file(PathBuf::from(format!("{}-wal", path.display())));
    let _ = std::fs::remove_file(PathBuf::from(format!("{}-shm", path.display())));
}

fn temp_db_path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "cliswitch-test-chat-bridge-{}.sqlite",
        uuid::Uuid::new_v4()
    ))
}

fn open_conn(path: &Path) -> Connection {
    Connection::open(path).expect("open sqlite")
}

fn sqlite_table_exists(conn: &Connection, table: &str) -> bool {
    conn.query_row(
        "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1 LIMIT 1",
        [table],
        |_| Ok(()),
    )
    .is_ok()
}

#[tokio::test]
async fn pairing_token_can_create_binding_and_list_it() {
    let db_path = temp_db_path();
    remove_sqlite_artifacts(&db_path);
    storage::init_db(&db_path).unwrap();

    let pairing = storage::create_pairing_token(
        db_path.clone(),
        storage::CreatePairingTokenInput {
            platform: storage::ChatPlatform::Telegram,
            expires_in_minutes: Some(5),
        },
    )
    .await
    .unwrap();

    assert_eq!(pairing.platform, storage::ChatPlatform::Telegram);

    let binding = storage::consume_pairing_token(
        db_path.clone(),
        pairing.token,
        storage::ChatPlatform::Telegram,
        "tg-user-1".to_string(),
        Some("@koumoe".to_string()),
    )
    .await
    .unwrap();

    assert!(binding.id > 0);
    assert_eq!(binding.platform, storage::ChatPlatform::Telegram);
    assert_eq!(binding.platform_user_id, "tg-user-1");
    assert_eq!(binding.display_name.as_deref(), Some("@koumoe"));

    let bindings = storage::list_chat_bindings(db_path.clone()).await.unwrap();
    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0].platform, storage::ChatPlatform::Telegram);
    assert_eq!(bindings[0].platform_user_id, "tg-user-1");

    let conn = open_conn(&db_path);
    assert!(!sqlite_table_exists(&conn, "chat_audit_log"));

    remove_sqlite_artifacts(&db_path);
}

#[tokio::test]
async fn pairing_token_cannot_be_reused() {
    let db_path = temp_db_path();
    remove_sqlite_artifacts(&db_path);
    storage::init_db(&db_path).unwrap();

    let pairing = storage::create_pairing_token(
        db_path.clone(),
        storage::CreatePairingTokenInput {
            platform: storage::ChatPlatform::Telegram,
            expires_in_minutes: Some(5),
        },
    )
    .await
    .unwrap();

    storage::consume_pairing_token(
        db_path.clone(),
        pairing.token.clone(),
        storage::ChatPlatform::Telegram,
        "tg-user-1".to_string(),
        Some("@koumoe".to_string()),
    )
    .await
    .unwrap();

    let err = storage::consume_pairing_token(
        db_path.clone(),
        pairing.token,
        storage::ChatPlatform::Telegram,
        "tg-user-2".to_string(),
        Some("@other".to_string()),
    )
    .await
    .unwrap_err();

    assert!(matches!(
        err.downcast_ref::<storage::StorageError>(),
        Some(storage::StorageError::ChatPairingTokenUsed)
    ));

    remove_sqlite_artifacts(&db_path);
}

#[tokio::test]
async fn pairing_token_is_limited_to_its_platform() {
    let db_path = temp_db_path();
    remove_sqlite_artifacts(&db_path);
    storage::init_db(&db_path).unwrap();

    let pairing = storage::create_pairing_token(
        db_path.clone(),
        storage::CreatePairingTokenInput {
            platform: storage::ChatPlatform::Telegram,
            expires_in_minutes: Some(5),
        },
    )
    .await
    .unwrap();

    let err = storage::consume_pairing_token(
        db_path.clone(),
        pairing.token,
        storage::ChatPlatform::Discord,
        "discord-user-1".to_string(),
        Some("koumoe#1234".to_string()),
    )
    .await
    .unwrap_err();

    assert!(matches!(
        err.downcast_ref::<storage::StorageError>(),
        Some(storage::StorageError::ChatPairingTokenPlatformMismatch { .. })
    ));

    remove_sqlite_artifacts(&db_path);
}

#[tokio::test]
async fn deactivate_binding_hides_it_from_active_list() {
    let db_path = temp_db_path();
    remove_sqlite_artifacts(&db_path);
    storage::init_db(&db_path).unwrap();

    let pairing = storage::create_pairing_token(
        db_path.clone(),
        storage::CreatePairingTokenInput {
            platform: storage::ChatPlatform::Telegram,
            expires_in_minutes: Some(5),
        },
    )
    .await
    .unwrap();

    let binding = storage::consume_pairing_token(
        db_path.clone(),
        pairing.token,
        storage::ChatPlatform::Telegram,
        "tg-user-1".to_string(),
        Some("@koumoe".to_string()),
    )
    .await
    .unwrap();

    storage::deactivate_chat_binding(db_path.clone(), binding.id)
        .await
        .unwrap();

    let bindings = storage::list_chat_bindings(db_path.clone()).await.unwrap();
    assert!(bindings.is_empty());

    remove_sqlite_artifacts(&db_path);
}

#[tokio::test]
async fn inactive_binding_can_be_bound_again() {
    let db_path = temp_db_path();
    remove_sqlite_artifacts(&db_path);
    storage::init_db(&db_path).unwrap();

    let first_pairing = storage::create_pairing_token(
        db_path.clone(),
        storage::CreatePairingTokenInput {
            platform: storage::ChatPlatform::Telegram,
            expires_in_minutes: Some(5),
        },
    )
    .await
    .unwrap();

    let first_binding = storage::consume_pairing_token(
        db_path.clone(),
        first_pairing.token,
        storage::ChatPlatform::Telegram,
        "tg-user-1".to_string(),
        Some("@koumoe".to_string()),
    )
    .await
    .unwrap();

    storage::deactivate_chat_binding(db_path.clone(), first_binding.id)
        .await
        .unwrap();

    let second_pairing = storage::create_pairing_token(
        db_path.clone(),
        storage::CreatePairingTokenInput {
            platform: storage::ChatPlatform::Telegram,
            expires_in_minutes: Some(5),
        },
    )
    .await
    .unwrap();

    let rebound = storage::consume_pairing_token(
        db_path.clone(),
        second_pairing.token,
        storage::ChatPlatform::Telegram,
        "tg-user-1".to_string(),
        Some("@koumoe-rebound".to_string()),
    )
    .await
    .unwrap();

    assert_eq!(rebound.id, first_binding.id);
    assert_eq!(rebound.platform_user_id, "tg-user-1");
    assert_eq!(rebound.display_name.as_deref(), Some("@koumoe-rebound"));
    assert!(rebound.is_active);

    let bindings = storage::list_chat_bindings(db_path.clone()).await.unwrap();
    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0].id, first_binding.id);
    assert_eq!(bindings[0].display_name.as_deref(), Some("@koumoe-rebound"));

    remove_sqlite_artifacts(&db_path);
}

#[tokio::test]
async fn active_binding_blocks_rebinding_same_platform_user() {
    let db_path = temp_db_path();
    remove_sqlite_artifacts(&db_path);
    storage::init_db(&db_path).unwrap();

    let first_pairing = storage::create_pairing_token(
        db_path.clone(),
        storage::CreatePairingTokenInput {
            platform: storage::ChatPlatform::Telegram,
            expires_in_minutes: Some(5),
        },
    )
    .await
    .unwrap();

    storage::consume_pairing_token(
        db_path.clone(),
        first_pairing.token,
        storage::ChatPlatform::Telegram,
        "tg-user-1".to_string(),
        Some("@koumoe".to_string()),
    )
    .await
    .unwrap();

    let second_pairing = storage::create_pairing_token(
        db_path.clone(),
        storage::CreatePairingTokenInput {
            platform: storage::ChatPlatform::Telegram,
            expires_in_minutes: Some(5),
        },
    )
    .await
    .unwrap();

    let err = storage::consume_pairing_token(
        db_path.clone(),
        second_pairing.token,
        storage::ChatPlatform::Telegram,
        "tg-user-1".to_string(),
        Some("@koumoe-again".to_string()),
    )
    .await
    .unwrap_err();

    assert!(matches!(
        err.downcast_ref::<storage::StorageError>(),
        Some(storage::StorageError::ChatBindingAlreadyExists { .. })
    ));

    remove_sqlite_artifacts(&db_path);
}

#[tokio::test]
async fn active_binding_allows_multiple_users_on_same_platform() {
    let db_path = temp_db_path();
    remove_sqlite_artifacts(&db_path);
    storage::init_db(&db_path).unwrap();

    let first_pairing = storage::create_pairing_token(
        db_path.clone(),
        storage::CreatePairingTokenInput {
            platform: storage::ChatPlatform::Telegram,
            expires_in_minutes: Some(5),
        },
    )
    .await
    .unwrap();

    storage::consume_pairing_token(
        db_path.clone(),
        first_pairing.token,
        storage::ChatPlatform::Telegram,
        "tg-user-1".to_string(),
        Some("@koumoe".to_string()),
    )
    .await
    .unwrap();

    let second_pairing = storage::create_pairing_token(
        db_path.clone(),
        storage::CreatePairingTokenInput {
            platform: storage::ChatPlatform::Telegram,
            expires_in_minutes: Some(5),
        },
    )
    .await
    .unwrap();

    let second = storage::consume_pairing_token(
        db_path.clone(),
        second_pairing.token,
        storage::ChatPlatform::Telegram,
        "tg-user-2".to_string(),
        Some("@other".to_string()),
    )
    .await
    .unwrap();
    assert_eq!(second.platform_user_id, "tg-user-2");

    let bindings = storage::list_chat_bindings(db_path.clone()).await.unwrap();
    assert_eq!(bindings.len(), 2);
    assert!(
        bindings
            .iter()
            .any(|item| item.platform == storage::ChatPlatform::Telegram
                && item.platform_user_id == "tg-user-1")
    );
    assert!(
        bindings
            .iter()
            .any(|item| item.platform == storage::ChatPlatform::Telegram
                && item.platform_user_id == "tg-user-2")
    );

    remove_sqlite_artifacts(&db_path);
}

#[test]
fn init_db_migrates_legacy_chat_bindings_unique_platform() {
    let db_path = temp_db_path();
    remove_sqlite_artifacts(&db_path);

    let conn = open_conn(&db_path);
    conn.execute_batch(
        r#"
        CREATE TABLE chat_bindings (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          platform TEXT NOT NULL,
          platform_user_id TEXT NOT NULL,
          display_name TEXT,
          bound_at INTEGER NOT NULL,
          is_active INTEGER NOT NULL DEFAULT 1,
          UNIQUE(platform)
        );
        "#,
    )
    .expect("create legacy chat_bindings");
    drop(conn);

    storage::init_db(&db_path).expect("init db should run chat bridge migrations");

    let conn = open_conn(&db_path);
    let mut stmt = conn
        .prepare("PRAGMA index_list('chat_bindings')")
        .expect("prepare index_list");
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(1)?, row.get::<_, i64>(2)? != 0))
        })
        .expect("query index_list");
    let mut found_composite = false;
    for row in rows {
        let (index_name, is_unique) = row.expect("read index_list row");
        if !is_unique {
            continue;
        }
        let pragma = format!("PRAGMA index_info('{}')", index_name.replace('\'', "''"));
        let mut col_stmt = conn.prepare(&pragma).expect("prepare index_info");
        let cols = col_stmt
            .query_map([], |col_row| col_row.get::<_, String>(2))
            .expect("query index_info")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("collect index_info");
        if cols == ["platform".to_string(), "platform_user_id".to_string()] {
            found_composite = true;
            break;
        }
    }
    assert!(
        found_composite,
        "expected composite unique(platform, platform_user_id)"
    );

    remove_sqlite_artifacts(&db_path);
}
