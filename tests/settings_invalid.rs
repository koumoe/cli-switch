use cliswitch::{logging, storage};
use rusqlite::{Connection, params};
use std::path::{Path, PathBuf};

fn remove_sqlite_artifacts(path: &Path) {
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_file(PathBuf::from(format!("{}-wal", path.display())));
    let _ = std::fs::remove_file(PathBuf::from(format!("{}-shm", path.display())));
}

fn upsert_setting(conn: &Connection, key: &str, value: &str) {
    conn.execute(
        r#"
        INSERT INTO app_settings (key, value, updated_at_ms)
        VALUES (?1, ?2, ?3)
        ON CONFLICT(key) DO UPDATE SET
          value = excluded.value,
          updated_at_ms = excluded.updated_at_ms
        "#,
        params![key, value, 0i64],
    )
    .unwrap();
}

#[tokio::test]
async fn get_app_settings_keeps_defaults_on_invalid_values() {
    let db_path = std::env::temp_dir().join(format!(
        "cliswitch-test-settings-invalid-{}.sqlite",
        uuid::Uuid::new_v4()
    ));
    remove_sqlite_artifacts(&db_path);

    storage::init_db(&db_path).unwrap();
    let conn = Connection::open(&db_path).unwrap();

    // Simulate DB corruption / legacy values.
    upsert_setting(&conn, "pricing_auto_update_interval_hours", "not-a-number");
    upsert_setting(&conn, "close_behavior", "???");
    upsert_setting(&conn, "auto_start_enabled", "maybe");
    upsert_setting(&conn, "server_lan_accessible", "maybe");
    upsert_setting(&conn, "log_level", "verbose");
    upsert_setting(&conn, "log_retention_days", "NaN");
    upsert_setting(&conn, "chat_bridge_turn_timeout_minutes", "-1");
    upsert_setting(&conn, "system_notifications_enabled", "maybe");
    upsert_setting(
        &conn,
        "remote_low_balance_system_notification_enabled",
        "maybe",
    );
    upsert_setting(
        &conn,
        "remote_managed_channel_missing_system_notification_enabled",
        "maybe",
    );
    upsert_setting(
        &conn,
        "remote_managed_channel_multiplier_system_notification_enabled",
        "maybe",
    );
    upsert_setting(
        &conn,
        "remote_managed_channel_missing_prompt_enabled",
        "maybe",
    );
    upsert_setting(
        &conn,
        "remote_managed_channel_sync_multiplier_enabled",
        "maybe",
    );
    upsert_setting(
        &conn,
        "remote_managed_channel_sync_free_multiplier_enabled",
        "maybe",
    );

    drop(conn);

    let settings = storage::get_app_settings(db_path.clone()).await.unwrap();
    assert_eq!(settings.pricing_auto_update_interval_hours, 24);
    assert_eq!(settings.close_behavior, storage::CloseBehavior::Ask);
    assert!(!settings.auto_start_enabled);
    assert!(!settings.server_lan_accessible);
    assert_eq!(settings.log_level, logging::LogLevel::Warning);
    assert_eq!(settings.log_retention_days, 30);
    assert_eq!(settings.chat_bridge_turn_timeout_minutes, 0);
    assert_eq!(settings.chat_bridge_turn_timeout(), None);
    assert!(settings.system_notifications_enabled);
    assert!(settings.remote_low_balance_system_notification_enabled);
    assert!(settings.remote_managed_channel_missing_system_notification_enabled);
    assert!(settings.remote_managed_channel_multiplier_system_notification_enabled);
    assert!(settings.remote_managed_channel_missing_prompt_enabled);
    assert!(settings.remote_managed_channel_sync_multiplier_enabled);
    assert!(!settings.remote_managed_channel_sync_free_multiplier_enabled);

    remove_sqlite_artifacts(&db_path);
}

#[tokio::test]
async fn default_chat_bridge_turn_timeout_is_disabled() {
    let db_path = std::env::temp_dir().join(format!(
        "cliswitch-test-settings-timeout-default-{}.sqlite",
        uuid::Uuid::new_v4()
    ));
    remove_sqlite_artifacts(&db_path);

    storage::init_db(&db_path).unwrap();

    let settings = storage::get_app_settings(db_path.clone()).await.unwrap();
    assert_eq!(settings.chat_bridge_turn_timeout_minutes, 0);
    assert_eq!(settings.chat_bridge_turn_timeout(), None);
    assert!(settings.system_notifications_enabled);
    assert!(settings.remote_low_balance_system_notification_enabled);
    assert!(settings.remote_managed_channel_missing_system_notification_enabled);
    assert!(settings.remote_managed_channel_multiplier_system_notification_enabled);
    assert!(settings.remote_managed_channel_missing_prompt_enabled);
    assert!(settings.remote_managed_channel_sync_multiplier_enabled);
    assert!(!settings.remote_managed_channel_sync_free_multiplier_enabled);

    remove_sqlite_artifacts(&db_path);
}

#[tokio::test]
async fn update_app_settings_persists_chat_bridge_turn_timeout_settings() {
    let db_path = std::env::temp_dir().join(format!(
        "cliswitch-test-settings-timeout-{}.sqlite",
        uuid::Uuid::new_v4()
    ));
    remove_sqlite_artifacts(&db_path);

    storage::init_db(&db_path).unwrap();

    let updated = storage::update_app_settings(
        db_path.clone(),
        storage::AppSettingsPatch {
            system_notifications_enabled: Some(false),
            remote_low_balance_system_notification_enabled: Some(false),
            remote_managed_channel_missing_system_notification_enabled: Some(false),
            remote_managed_channel_multiplier_system_notification_enabled: Some(false),
            chat_bridge_turn_timeout_minutes: Some(0),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(updated.chat_bridge_turn_timeout_minutes, 0);
    assert_eq!(updated.chat_bridge_turn_timeout(), None);
    assert!(!updated.system_notifications_enabled);
    assert!(!updated.remote_low_balance_system_notification_enabled);
    assert!(!updated.remote_managed_channel_missing_system_notification_enabled);
    assert!(!updated.remote_managed_channel_multiplier_system_notification_enabled);

    let reread = storage::get_app_settings(db_path.clone()).await.unwrap();
    assert_eq!(reread.chat_bridge_turn_timeout_minutes, 0);
    assert_eq!(reread.chat_bridge_turn_timeout(), None);
    assert!(!reread.system_notifications_enabled);
    assert!(!reread.remote_low_balance_system_notification_enabled);
    assert!(!reread.remote_managed_channel_missing_system_notification_enabled);
    assert!(!reread.remote_managed_channel_multiplier_system_notification_enabled);

    remove_sqlite_artifacts(&db_path);
}
