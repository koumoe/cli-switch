use cliswitch::{server, storage};
use serde_json::Value;
use std::path::PathBuf;
use tokio::time::{Duration, sleep};

fn temp_db_path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "cliswitch-codex-ticket-status-{}.sqlite",
        uuid::Uuid::new_v4()
    ))
}

#[tokio::test]
async fn codex_ticket_status_endpoint_returns_batch_account_snapshot() {
    let db_path = temp_db_path();
    storage::init_db(&db_path).unwrap();
    let account = storage::upsert_openai_account_tokens(
        db_path.clone(),
        Some("Status test account".to_string()),
        storage::OpenAiAccountTokens {
            access_token: "access-token".to_string(),
            refresh_token: None,
            id_token: None,
            token_expires_at_ms: Some(i64::MAX),
            account_id: "chatgpt-status-account".to_string(),
            email: None,
            display_name: None,
            plan_type: None,
        },
    )
    .await
    .unwrap();
    let ticket = format!("gAAAAA{}", "x".repeat(286));
    storage::upsert_openai_codex_ticket(
        db_path.clone(),
        account.id.clone(),
        "gpt-6-astra".to_string(),
        ticket,
        storage::now_ms(),
        storage::now_ms() + 300_000,
    )
    .await
    .unwrap();
    storage::create_channel(
        db_path.clone(),
        storage::CreateChannel {
            name: "Status test OAuth channel".to_string(),
            protocol: storage::Protocol::Openai,
            base_url: "https://chatgpt.com/backend-api/codex".to_string(),
            auth_type: Some("managed_account".to_string()),
            auth_ref: String::new(),
            checkin_url: None,
            priority: 1,
            retry_times: 1,
            ignore_channel_protection: false,
            recharge_currency: None,
            real_multiplier: None,
            managed_by_remote: Some(true),
            managed_remote_provider: Some(storage::ManagedRemoteProvider::Openai),
            managed_remote_account_id: Some(account.id.clone()),
            managed_remote_resource_id: None,
            managed_remote_resource_name: None,
            managed_remote_group_name: None,
            managed_remote_group_id: None,
            enabled: true,
        },
    )
    .await
    .unwrap();

    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .unwrap();
    let addr = listener.local_addr().unwrap();
    let server_task = tokio::spawn(server::serve_with_listener(
        listener,
        db_path.clone(),
        false,
    ));

    let client = reqwest::Client::new();
    let url = format!("http://{addr}/api/openai/codex-tickets/status");
    let mut response = None;
    for _ in 0..40 {
        if let Ok(candidate) = client.get(&url).send().await {
            response = Some(candidate);
            break;
        }
        sleep(Duration::from_millis(25)).await;
    }
    let response = response.expect("status endpoint did not become ready");
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["issue"], "disabled");
    let tickets = body["tickets"].as_array().unwrap();
    assert_eq!(tickets.len(), 2);
    assert!(tickets.iter().any(|item| {
        item["model"] == "gpt-6-astra" && item["ready"] == true && item["length"] == 292
    }));

    server_task.abort();
    let _ = server_task.await;
}
