use super::*;
use axum::Router;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use serde_json::{Value, json};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

struct TestDb(PathBuf);
impl Drop for TestDb {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

async fn fixture() -> (TestDb, storage::OpenAiAccount) {
    let path = std::env::temp_dir().join(format!("cliswitch-reset-{}.db", uuid::Uuid::new_v4()));
    storage::init_db(&path).unwrap();
    let account = storage::upsert_openai_account_tokens(
        path.clone(),
        Some("Test account".to_owned()),
        storage::OpenAiAccountTokens {
            access_token: "test-token".to_owned(),
            refresh_token: None,
            id_token: None,
            token_expires_at_ms: None,
            account_id: "remote-test-account".to_owned(),
            email: None,
            display_name: None,
            plan_type: Some("pro".to_owned()),
        },
    )
    .await
    .unwrap();
    storage::update_openai_account_quota(
        path.clone(),
        account.id.clone(),
        storage::OpenAiQuotaSnapshot {
            primary: Some(storage::OpenAiQuotaWindow {
                limit_name: None,
                used_percent: 98.0,
                window_minutes: 300,
                resets_at_ms: None,
            }),
            reset_available_count: Some(2),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    (TestDb(path), account)
}

async fn serve(app: Router) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (format!("http://{address}"), task)
}

fn usage(count: i64) -> Value {
    json!({"rate_limit_reset_credits":{"available_count":count},"rate_limit":{"primary_window":{"used_percent":8,"limit_window_seconds":18000}}})
}

#[tokio::test]
async fn redemption_retries_reuse_the_key_and_refresh_the_correct_account() {
    let (db, account) = fixture().await;
    let key = uuid::Uuid::new_v4().to_string();
    let redeemed = Arc::new(Mutex::new(HashSet::<String>::new()));
    let calls = redeemed.clone();
    let expected_key = key.clone();
    let app = Router::new().route("/reset", post(move |headers: HeaderMap, Json(body): Json<Value>| {
        let calls = calls.clone(); let expected_key = expected_key.clone();
        async move {
            assert_eq!(headers["authorization"], "Bearer test-token");
            assert_eq!(headers["chatgpt-account-id"], "remote-test-account");
            assert_eq!(body, json!({"redeem_request_id":expected_key}));
            let first = calls.lock().unwrap().insert(expected_key);
            Json(json!({"code":if first {"reset"} else {"already_redeemed"},"windows_reset":1}))
        }
    })).route("/usage", get(|headers: HeaderMap| async move {
        assert_eq!(headers["chatgpt-account-id"], "remote-test-account");
        Json(usage(1))
    }));
    let (url, task) = serve(app).await;
    for outcome in [
        OpenAiQuotaResetOutcome::Reset,
        OpenAiQuotaResetOutcome::AlreadyRedeemed,
    ] {
        let response = reset_openai_account_quota_at(
            db.0.clone(),
            &reqwest::Client::new(),
            account.id.clone(),
            &key,
            Some(&format!("{url}/reset")),
            Some(&format!("{url}/usage")),
        )
        .await
        .unwrap();
        assert_eq!(response.outcome, outcome);
        assert!(response.quota_refresh_error.is_none());
        let public = serde_json::to_value(response.account).unwrap();
        assert_eq!(public["id"], account.id);
        assert_eq!(public["quota_reset_available_count"], 1);
        assert_eq!(public["quota_windows"][0]["used_percent"], 8.0);
        assert!(public.get("access_token").is_none());
    }
    assert_eq!(redeemed.lock().unwrap().len(), 1);
    task.abort();
}

#[tokio::test]
async fn known_redemption_survives_a_failed_usage_refresh() {
    for code in ["reset", "already_redeemed"] {
        let (db, account) = fixture().await;
        let app = Router::new()
            .route(
                "/reset",
                post(move || async move { Json(json!({"code":code})) }),
            )
            .route("/usage", get(|| async { StatusCode::SERVICE_UNAVAILABLE }));
        let (url, task) = serve(app).await;
        let response = reset_openai_account_quota_at(
            db.0.clone(),
            &reqwest::Client::new(),
            account.id.clone(),
            &uuid::Uuid::new_v4().to_string(),
            Some(&format!("{url}/reset")),
            Some(&format!("{url}/usage")),
        )
        .await
        .unwrap();
        assert!(response.quota_refresh_error.is_some());
        let public = serde_json::to_value(response).unwrap();
        assert_eq!(public["outcome"], code);
        assert!(public["account"]["quota_reset_available_count"].is_null());
        assert_eq!(public["account"]["quota_windows"][0]["used_percent"], 98.0);
        let saved = storage::get_openai_account_without_secret(db.0.clone(), account.id)
            .await
            .unwrap();
        assert_eq!(saved.quota.reset_available_count, None);
        task.abort();
    }
}

#[tokio::test]
async fn ineligible_and_no_credit_responses_are_not_reported_as_consumed() {
    for code in ["nothing_to_reset", "no_credit"] {
        let (db, account) = fixture().await;
        let app = Router::new()
            .route(
                "/reset",
                post(move || async move { Json(json!({"code":code})) }),
            )
            .route("/usage", get(|| async { Json(usage(0)) }));
        let (url, task) = serve(app).await;
        let response = reset_openai_account_quota_at(
            db.0.clone(),
            &reqwest::Client::new(),
            account.id,
            &uuid::Uuid::new_v4().to_string(),
            Some(&format!("{url}/reset")),
            Some(&format!("{url}/usage")),
        )
        .await
        .unwrap();
        let public = serde_json::to_value(response).unwrap();
        assert_eq!(public["outcome"], code);
        assert_eq!(public["account"]["quota_reset_available_count"], 0);
        task.abort();
    }
}

#[tokio::test]
async fn invalid_keys_fail_before_loading_an_account_or_calling_upstream() {
    let error = reset_openai_account_quota_at(
        PathBuf::from("does-not-exist.db"),
        &reqwest::Client::new(),
        "missing".into(),
        "",
        None,
        None,
    )
    .await
    .unwrap_err();
    assert!(matches!(
        error,
        ApiError::BadRequest {
            code: "openai_quota_reset_invalid_key",
            ..
        }
    ));
}

#[tokio::test]
async fn unexpected_upstream_outcome_is_an_unconfirmed_failure() {
    let (db, account) = fixture().await;
    let app = Router::new().route("/reset", post(|| async { Json(json!({"code":"unknown"})) }));
    let (url, task) = serve(app).await;
    let error = reset_openai_account_quota_at(
        db.0.clone(),
        &reqwest::Client::new(),
        account.id,
        &uuid::Uuid::new_v4().to_string(),
        Some(&format!("{url}/reset")),
        Some(&format!("{url}/usage")),
    )
    .await
    .unwrap_err();
    assert!(matches!(
        error,
        ApiError::BadGateway {
            code: "openai_quota_reset_failed",
            ..
        }
    ));
    task.abort();
}
