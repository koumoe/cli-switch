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
async fn active_binding_blocks_rebinding_same_platform() {
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
        "tg-user-2".to_string(),
        Some("@other".to_string()),
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
async fn init_db_migrates_legacy_chat_bridge_schema_in_one_step() {
    let db_path = temp_db_path();
    remove_sqlite_artifacts(&db_path);

    let conn = open_conn(&db_path);
    conn.execute_batch(
        r#"
        CREATE TABLE chat_users (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          display_name TEXT,
          created_at INTEGER NOT NULL,
          last_active INTEGER
        );

        CREATE TABLE pairing_tokens (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          token_hash TEXT NOT NULL UNIQUE,
          token_hint TEXT,
          created_by_user_id INTEGER,
          created_at INTEGER NOT NULL,
          expires_at INTEGER NOT NULL,
          used_at INTEGER,
          used_by_platform TEXT,
          used_by_sender_id TEXT
        );

        CREATE TABLE chat_bindings (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          user_id INTEGER NOT NULL,
          platform TEXT NOT NULL,
          platform_user_id TEXT NOT NULL,
          display_name TEXT,
          bound_at INTEGER NOT NULL,
          is_active INTEGER NOT NULL DEFAULT 1,
          UNIQUE(platform, platform_user_id)
        );

        CREATE TABLE bridge_known_projects (
          path TEXT PRIMARY KEY,
          display_name TEXT NOT NULL,
          added_by_user_id INTEGER NOT NULL,
          created_at INTEGER NOT NULL,
          updated_at INTEGER NOT NULL
        );

        CREATE TABLE bridge_sessions (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          alias TEXT,
          owner INTEGER NOT NULL,
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
          UNIQUE(owner, alias)
        );

        CREATE TABLE chat_audit_log (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          user_id INTEGER NOT NULL,
          platform TEXT NOT NULL,
          sender_id TEXT NOT NULL,
          chat_id TEXT NOT NULL,
          message_type TEXT NOT NULL,
          content TEXT NOT NULL,
          session_id INTEGER,
          created_at INTEGER NOT NULL
        );

        INSERT INTO chat_users (id, display_name, created_at, last_active)
        VALUES (1, '@koumoe', 1000, 1000);

        INSERT INTO pairing_tokens (
          id, token_hash, token_hint, created_by_user_id, created_at, expires_at, used_at, used_by_platform, used_by_sender_id
        ) VALUES (
          1, 'legacy-hash', 'ck_legacy', 1, 1000, 2000, NULL, NULL, NULL
        );

        INSERT INTO chat_bindings (
          id, user_id, platform, platform_user_id, display_name, bound_at, is_active
        ) VALUES (
          1, 1, 'telegram', 'tg-user-1', '@koumoe', 1000, 1
        );

        INSERT INTO bridge_known_projects (
          path, display_name, added_by_user_id, created_at, updated_at
        ) VALUES (
          '/tmp/legacy-project', 'legacy-project', 1, 1000, 1000
        );

        INSERT INTO bridge_sessions (
          id, alias, owner, cli_type, cli_session_ref, project_id, project_name, working_dir, permission_mode, status, is_default, created_at, last_active
        ) VALUES (
          1, 'alpha', 1, 'codex', NULL, '/tmp/legacy-project', 'legacy-project', '/tmp/legacy-project', 'safe', 'idle', 1, 1000, 1000
        );

        INSERT INTO chat_audit_log (
          id, user_id, platform, sender_id, chat_id, message_type, content, session_id, created_at
        ) VALUES (
          1, 1, 'telegram', 'tg-user-1', 'chat-1', 'command', '/sessions', 1, 1000
        );
        "#,
    )
    .expect("seed legacy schema");
    drop(conn);

    storage::init_db(&db_path).expect("migrate legacy schema");

    let conn = open_conn(&db_path);
    let bridge_session_columns = conn
        .prepare("PRAGMA table_info(bridge_sessions)")
        .expect("pragma bridge_sessions")
        .query_map([], |row| row.get::<_, String>(1))
        .expect("query columns")
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("collect columns");
    assert!(bridge_session_columns.iter().any(|name| name == "platform"));
    assert!(!bridge_session_columns.iter().any(|name| name == "owner"));

    let chat_user_exists: Option<i64> = conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'chat_users'",
            [],
            |row| row.get(0),
        )
        .ok();
    assert!(chat_user_exists.is_none());

    let bindings = storage::list_chat_bindings(db_path.clone())
        .await
        .expect("list bindings after migration");
    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0].platform, storage::ChatPlatform::Telegram);
    assert_eq!(bindings[0].platform_user_id, "tg-user-1");

    let sessions = storage::list_bridge_sessions_for_platform(
        db_path.clone(),
        storage::ChatPlatform::Telegram,
        false,
    )
    .await
    .expect("list sessions after migration");
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].alias.as_deref(), Some("alpha"));
    assert_eq!(sessions[0].project_name, "legacy-project");

    let audit_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM chat_audit_log", [], |row| row.get(0))
        .expect("count audit rows");
    assert_eq!(audit_count, 1);

    remove_sqlite_artifacts(&db_path);
}

#[tokio::test]
async fn audit_logs_can_be_listed_with_platform_filter() {
    let db_path = temp_db_path();
    remove_sqlite_artifacts(&db_path);
    storage::init_db(&db_path).unwrap();

    storage::create_chat_audit_log(
        db_path.clone(),
        storage::CreateChatAuditLogInput {
            platform: storage::ChatPlatform::Telegram,
            sender_id: "tg-user-1".to_string(),
            chat_id: "tg-chat".to_string(),
            message_type: "chat".to_string(),
            content: "telegram message".to_string(),
            session_id: Some(1),
        },
    )
    .await
    .unwrap();
    storage::create_chat_audit_log(
        db_path.clone(),
        storage::CreateChatAuditLogInput {
            platform: storage::ChatPlatform::Discord,
            sender_id: "dc-user-1".to_string(),
            chat_id: "dc-chat".to_string(),
            message_type: "command".to_string(),
            content: "/sessions".to_string(),
            session_id: None,
        },
    )
    .await
    .unwrap();

    let all = storage::list_chat_audit_logs(
        db_path.clone(),
        storage::ListChatAuditLogsInput {
            platform: None,
            limit: Some(10),
            offset: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(all.items.len(), 2);
    assert_eq!(all.items[0].platform, storage::ChatPlatform::Discord);
    assert!(!all.has_more);

    let telegram = storage::list_chat_audit_logs(
        db_path.clone(),
        storage::ListChatAuditLogsInput {
            platform: Some(storage::ChatPlatform::Telegram),
            limit: Some(10),
            offset: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(telegram.items.len(), 1);
    assert_eq!(telegram.items[0].content, "telegram message");
    assert_eq!(telegram.items[0].session_id, Some(1));

    let paged = storage::list_chat_audit_logs(
        db_path.clone(),
        storage::ListChatAuditLogsInput {
            platform: None,
            limit: Some(1),
            offset: Some(1),
        },
    )
    .await
    .unwrap();
    assert_eq!(paged.items.len(), 1);
    assert_eq!(paged.items[0].platform, storage::ChatPlatform::Telegram);
    assert!(!paged.has_more);
    assert_eq!(paged.offset, 1);

    remove_sqlite_artifacts(&db_path);
}
