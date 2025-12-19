use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
    routing::any,
};
use cliswitch::{proxy, storage};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::time::{Duration, sleep};

async fn spawn_upstream(status: StatusCode, body: &'static str) -> String {
    let app = Router::new().route(
        "/{*path}",
        any(move || async move {
            (
                status,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                body,
            )
        }),
    );

    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("local_addr");
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    format!("http://127.0.0.1:{}", addr.port())
}

fn temp_db_path() -> std::path::PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("cliswitch-test-{}.sqlite", uuid::Uuid::new_v4()));
    p
}

async fn wait_for_usage_event(db_path: std::path::PathBuf) -> storage::UsageEvent {
    for _ in 0..100 {
        let events = storage::list_usage_events_recent(db_path.clone(), 10)
            .await
            .expect("list usage events");
        if let Some(e) = events.into_iter().next() {
            return e;
        }
        sleep(Duration::from_millis(10)).await;
    }
    panic!("timeout waiting for usage event");
}

#[tokio::test]
async fn failover_on_non_200_until_success() {
    let base1 = spawn_upstream(StatusCode::INTERNAL_SERVER_ERROR, r#"{"err":"c1"}"#).await;
    let base2 = spawn_upstream(StatusCode::BAD_GATEWAY, r#"{"err":"c2"}"#).await;
    let base3 = spawn_upstream(StatusCode::OK, r#"{"ok":true}"#).await;

    let db_path = temp_db_path();
    storage::init_db(&db_path).expect("init_db");

    storage::create_channel(
        db_path.clone(),
        storage::CreateChannel {
            name: "c1".to_string(),
            protocol: storage::Protocol::Openai,
            base_url: format!("{base1}/v1"),
            auth_type: None,
            auth_ref: "t1".to_string(),
            priority: 30,
            enabled: true,
        },
    )
    .await
    .expect("create c1");
    storage::create_channel(
        db_path.clone(),
        storage::CreateChannel {
            name: "c2".to_string(),
            protocol: storage::Protocol::Openai,
            base_url: format!("{base2}/v1"),
            auth_type: None,
            auth_ref: "t2".to_string(),
            priority: 20,
            enabled: true,
        },
    )
    .await
    .expect("create c2");
    storage::create_channel(
        db_path.clone(),
        storage::CreateChannel {
            name: "c3".to_string(),
            protocol: storage::Protocol::Openai,
            base_url: format!("{base3}/v1"),
            auth_type: None,
            auth_ref: "t3".to_string(),
            priority: 10,
            enabled: true,
        },
    )
    .await
    .expect("create c3");

    let client = reqwest::Client::builder().build().expect("client");
    let req = Request::builder()
        .method("POST")
        .uri("/v1/chat/completions")
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"model":"gpt-test"}"#))
        .expect("req");

    let resp = proxy::forward(
        &client,
        db_path.clone(),
        storage::Protocol::Openai,
        "/v1",
        req,
    )
    .await
    .expect("forward");

    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), 1024 * 1024)
        .await
        .expect("read body");
    assert_eq!(std::str::from_utf8(&bytes).unwrap(), r#"{"ok":true}"#);
}

#[tokio::test]
async fn return_last_error_when_all_channels_fail() {
    let base1 = spawn_upstream(StatusCode::INTERNAL_SERVER_ERROR, r#"{"err":"c1"}"#).await;
    let base2 = spawn_upstream(StatusCode::UNAUTHORIZED, r#"{"err":"c2"}"#).await;
    let base3 = spawn_upstream(StatusCode::SERVICE_UNAVAILABLE, r#"{"err":"c3"}"#).await;

    let db_path = temp_db_path();
    storage::init_db(&db_path).expect("init_db");

    for (name, base, priority) in [("c1", base1, 30), ("c2", base2, 20), ("c3", base3, 10)] {
        storage::create_channel(
            db_path.clone(),
            storage::CreateChannel {
                name: name.to_string(),
                protocol: storage::Protocol::Openai,
                base_url: format!("{base}/v1"),
                auth_type: None,
                auth_ref: "t".to_string(),
                priority,
                enabled: true,
            },
        )
        .await
        .expect("create channel");
    }

    let client = reqwest::Client::builder().build().expect("client");
    let req = Request::builder()
        .method("POST")
        .uri("/v1/chat/completions")
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"model":"gpt-test"}"#))
        .expect("req");

    let resp = proxy::forward(
        &client,
        db_path.clone(),
        storage::Protocol::Openai,
        "/v1",
        req,
    )
    .await
    .expect("forward");

    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    let bytes = to_bytes(resp.into_body(), 1024 * 1024)
        .await
        .expect("read body");
    assert_eq!(std::str::from_utf8(&bytes).unwrap(), r#"{"err":"c3"}"#);
}

#[tokio::test]
async fn gemini_logs_include_model_and_cost() {
    let base = spawn_upstream(
        StatusCode::OK,
        r#"{"usageMetadata":{"promptTokenCount":10,"candidatesTokenCount":5,"totalTokenCount":15}}"#,
    )
    .await;

    let db_path = temp_db_path();
    storage::init_db(&db_path).expect("init_db");

    storage::upsert_pricing_models(
        db_path.clone(),
        vec![storage::UpsertPricingModel {
            model_id: "gemini-1.5-pro".to_string(),
            prompt_price: Some("0.125".to_string()),
            completion_price: Some("0.25".to_string()),
            request_price: Some("0.5".to_string()),
            cache_read_price: None,
            cache_write_price: None,
            raw_json: None,
        }],
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_millis() as i64,
    )
    .await
    .expect("upsert pricing");

    storage::create_channel(
        db_path.clone(),
        storage::CreateChannel {
            name: "g1".to_string(),
            protocol: storage::Protocol::Gemini,
            base_url: format!("{base}/v1beta"),
            auth_type: None,
            auth_ref: "t".to_string(),
            priority: 10,
            enabled: true,
        },
    )
    .await
    .expect("create channel");

    let client = reqwest::Client::builder().build().expect("client");
    let req = Request::builder()
        .method("POST")
        .uri("/v1beta/models/gemini-1.5-pro:generateContent")
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"contents":[{"role":"user","parts":[{"text":"hi"}]}]}"#))
        .expect("req");

    let resp = proxy::forward(
        &client,
        db_path.clone(),
        storage::Protocol::Gemini,
        "/v1beta",
        req,
    )
    .await
    .expect("forward");
    assert_eq!(resp.status(), StatusCode::OK);

    let event = wait_for_usage_event(db_path.clone()).await;
    assert_eq!(event.protocol, storage::Protocol::Gemini);
    assert_eq!(event.model.as_deref(), Some("gemini-1.5-pro"));
    assert_eq!(event.estimated_cost_usd.as_deref(), Some("3"));
}
