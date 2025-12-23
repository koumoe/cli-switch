use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
    routing::any,
};
use cliswitch::{proxy, storage};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
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

async fn spawn_upstream_counted(
    status: StatusCode,
    body: &'static str,
) -> (String, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let calls2 = calls.clone();
    let app = Router::new().route(
        "/{*path}",
        any(move || {
            let calls = calls2.clone();
            async move {
                calls.fetch_add(1, Ordering::Relaxed);
                (
                    status,
                    [(axum::http::header::CONTENT_TYPE, "application/json")],
                    body,
                )
            }
        }),
    );

    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("local_addr");
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    (format!("http://127.0.0.1:{}", addr.port()), calls)
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

async fn assert_no_usage_events(db_path: std::path::PathBuf) {
    sleep(Duration::from_millis(50)).await;
    let events = storage::list_usage_events_recent(db_path, 10)
        .await
        .expect("list usage events");
    assert!(
        events.is_empty(),
        "expected no usage events, got {}",
        events.len()
    );
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
            recharge_currency: None,
            real_multiplier: None,
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
            recharge_currency: None,
            real_multiplier: None,
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
            recharge_currency: None,
            real_multiplier: None,
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
                recharge_currency: None,
                real_multiplier: None,
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
            recharge_currency: None,
            real_multiplier: None,
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
        .body(Body::from(
            r#"{"contents":[{"role":"user","parts":[{"text":"hi"}]}]}"#,
        ))
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

#[tokio::test]
async fn anthropic_count_tokens_no_failover_and_no_usage_log() {
    let (base1, c1_calls) = spawn_upstream_counted(
        StatusCode::FORBIDDEN,
        r#"{"error":{"message":"count_tokens endpoint is not enabled","type":"permission_error"},"type":"error"}"#,
    )
    .await;
    let (base2, c2_calls) = spawn_upstream_counted(StatusCode::OK, r#"{"input_tokens":123}"#).await;

    let db_path = temp_db_path();
    storage::init_db(&db_path).expect("init_db");

    storage::create_channel(
        db_path.clone(),
        storage::CreateChannel {
            name: "c1".to_string(),
            protocol: storage::Protocol::Anthropic,
            base_url: format!("{base1}/v1"),
            auth_type: None,
            auth_ref: "t1".to_string(),
            priority: 30,
            recharge_currency: None,
            real_multiplier: None,
            enabled: true,
        },
    )
    .await
    .expect("create c1");
    storage::create_channel(
        db_path.clone(),
        storage::CreateChannel {
            name: "c2".to_string(),
            protocol: storage::Protocol::Anthropic,
            base_url: format!("{base2}/v1"),
            auth_type: None,
            auth_ref: "t2".to_string(),
            priority: 20,
            recharge_currency: None,
            real_multiplier: None,
            enabled: true,
        },
    )
    .await
    .expect("create c2");

    let client = reqwest::Client::builder().build().expect("client");
    let req = Request::builder()
        .method("POST")
        .uri("/v1/messages/count_tokens?beta=true")
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            r#"{"model":"claude-test","messages":[{"role":"user","content":"hi"}]}"#,
        ))
        .expect("req");

    let resp = proxy::forward(
        &client,
        db_path.clone(),
        storage::Protocol::Anthropic,
        "/v1",
        req,
    )
    .await
    .expect("forward");

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    let bytes = to_bytes(resp.into_body(), 1024 * 1024)
        .await
        .expect("read body");
    assert_eq!(
        std::str::from_utf8(&bytes).unwrap(),
        r#"{"error":{"message":"count_tokens endpoint is not enabled","type":"permission_error"},"type":"error"}"#
    );

    assert_eq!(c1_calls.load(Ordering::Relaxed), 1);
    assert_eq!(c2_calls.load(Ordering::Relaxed), 0);
    assert_no_usage_events(db_path.clone()).await;
}

#[tokio::test]
async fn anthropic_count_tokens_does_not_auto_disable() {
    let base = spawn_upstream(StatusCode::INTERNAL_SERVER_ERROR, r#"{"err":"nope"}"#).await;

    let db_path = temp_db_path();
    storage::init_db(&db_path).expect("init_db");
    storage::update_app_settings(
        db_path.clone(),
        storage::AppSettingsPatch {
            auto_disable_enabled: Some(true),
            auto_disable_window_minutes: Some(3),
            auto_disable_failure_times: Some(1),
            auto_disable_disable_minutes: Some(30),
            ..Default::default()
        },
    )
    .await
    .expect("update settings");

    storage::create_channel(
        db_path.clone(),
        storage::CreateChannel {
            name: "c1".to_string(),
            protocol: storage::Protocol::Anthropic,
            base_url: format!("{base}/v1"),
            auth_type: None,
            auth_ref: "t1".to_string(),
            priority: 10,
            recharge_currency: None,
            real_multiplier: None,
            enabled: true,
        },
    )
    .await
    .expect("create c1");

    let client = reqwest::Client::builder().build().expect("client");
    let req = Request::builder()
        .method("POST")
        .uri("/v1/messages/count_tokens")
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            r#"{"model":"claude-test","messages":[{"role":"user","content":"hi"}]}"#,
        ))
        .expect("req");

    let resp = proxy::forward(
        &client,
        db_path.clone(),
        storage::Protocol::Anthropic,
        "/v1",
        req,
    )
    .await
    .expect("forward");
    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);

    let channels = storage::list_channels(db_path.clone())
        .await
        .expect("list channels");
    assert_eq!(channels.len(), 1);
    assert_eq!(channels[0].auto_disabled_until_ms, 0);

    assert_no_usage_events(db_path.clone()).await;
}
