use axum::{
    Router,
    body::to_bytes,
    http::{HeaderMap, HeaderValue, Method, StatusCode, Uri},
};
use cliswitch::{proxy, server, storage};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Clone, Debug)]
struct Captured {
    method: Method,
    path: String,
    query: String,
    headers: HeaderMap,
}

type Responder = fn(&Uri) -> (StatusCode, Value);

async fn spawn_upstream(responder: Responder) -> (String, Arc<Mutex<Vec<Captured>>>) {
    let captured = Arc::new(Mutex::new(Vec::new()));
    let captured_for_handler = captured.clone();
    let app = Router::new().fallback(move |method: Method, uri: Uri, headers: HeaderMap| {
        let captured = captured_for_handler.clone();
        async move {
            captured.lock().unwrap().push(Captured {
                method,
                path: uri.path().to_string(),
                query: uri.query().unwrap_or_default().to_string(),
                headers,
            });
            let (status, body) = responder(&uri);
            (status, axum::Json(body))
        }
    });

    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("local_addr");
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    (format!("http://127.0.0.1:{}", addr.port()), captured)
}

fn channel(
    name: &str,
    protocol: storage::Protocol,
    base_url: &str,
    priority: i64,
) -> storage::Channel {
    storage::Channel {
        id: format!("id-{name}"),
        name: name.to_string(),
        protocol,
        base_url: base_url.to_string(),
        auth_type: "auto".to_string(),
        auth_ref: format!("token-{name}"),
        checkin_url: None,
        priority,
        retry_times: 1,
        ignore_channel_protection: false,
        recharge_currency: storage::RechargeCurrency::Usd,
        real_multiplier: 1.0,
        managed_by_remote: false,
        managed_remote_provider: None,
        managed_remote_account_id: None,
        managed_remote_resource_id: None,
        managed_remote_resource_name: None,
        managed_remote_group_name: None,
        managed_remote_group_id: None,
        enabled: true,
        auto_disabled_until_ms: 0,
        created_at_ms: 0,
        updated_at_ms: 0,
    }
}

fn anthropic_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));
    headers.insert("x-api-key", HeaderValue::from_static("client-placeholder"));
    headers
}

async fn list(
    protocol: storage::Protocol,
    headers: &HeaderMap,
    channels: &[storage::Channel],
) -> Value {
    let response = proxy::models::list_models(&reqwest::Client::new(), protocol, headers, channels)
        .await
        .expect("list models");
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("body");
    serde_json::from_slice(&bytes).expect("json")
}

fn ids(list: &[Value]) -> Vec<&str> {
    list.iter()
        .map(|model| model["id"].as_str().unwrap())
        .collect()
}

fn openai_gpt(_: &Uri) -> (StatusCode, Value) {
    (
        StatusCode::OK,
        json!({"object": "list", "data": [{"id": "gpt-x", "object": "model", "created": 1, "owned_by": "openai"}]}),
    )
}

fn anthropic_a(_: &Uri) -> (StatusCode, Value) {
    (
        StatusCode::OK,
        json!({"data": [{"type": "model", "id": "claude-a", "display_name": "Claude A", "created_at": "2025-01-01T00:00:00Z"}], "has_more": false}),
    )
}

#[tokio::test]
async fn anthropic_clients_only_see_anthropic_channels_with_channel_credentials() {
    let (openai_url, openai_calls) = spawn_upstream(openai_gpt).await;
    let (anthropic_url, anthropic_calls) = spawn_upstream(anthropic_a).await;
    let channels = [
        channel("openai", storage::Protocol::Openai, &openai_url, 10),
        channel(
            "anthropic",
            storage::Protocol::Anthropic,
            &anthropic_url,
            10,
        ),
    ];

    let body = list(
        storage::Protocol::Anthropic,
        &anthropic_headers(),
        &channels,
    )
    .await;

    assert_eq!(ids(body["data"].as_array().unwrap()), ["claude-a"]);
    assert_eq!(body["has_more"], false);
    assert_eq!(body["first_id"], "claude-a");
    assert!(openai_calls.lock().unwrap().is_empty());

    let calls = anthropic_calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].path, "/v1/models");
    assert_eq!(calls[0].query, "limit=1000");
    assert_eq!(calls[0].headers["x-api-key"], "token-anthropic");
    assert_eq!(calls[0].headers["anthropic-version"], "2023-06-01");
}

#[tokio::test]
async fn merges_enabled_channels_in_priority_order() {
    fn high(_: &Uri) -> (StatusCode, Value) {
        (
            StatusCode::OK,
            json!({"data": [{"id": "claude-a"}, {"id": "claude-b", "display_name": "B from high"}]}),
        )
    }
    fn low(_: &Uri) -> (StatusCode, Value) {
        // OpenAI-shaped answer from an Anthropic relay.
        (
            StatusCode::OK,
            json!({"object": "list", "data": [{"id": "claude-b", "display_name": "B from low"}, {"id": "claude-c", "object": "model", "created": 1_700_000_000}]}),
        )
    }
    fn disabled(_: &Uri) -> (StatusCode, Value) {
        (StatusCode::OK, json!({"data": [{"id": "claude-disabled"}]}))
    }
    fn paused(_: &Uri) -> (StatusCode, Value) {
        (StatusCode::OK, json!({"data": [{"id": "claude-paused"}]}))
    }
    let (high_url, _) = spawn_upstream(high).await;
    let (low_url, _) = spawn_upstream(low).await;
    let (disabled_url, disabled_calls) = spawn_upstream(disabled).await;
    let (paused_url, _) = spawn_upstream(paused).await;

    let mut disabled_channel = channel("disabled", storage::Protocol::Anthropic, &disabled_url, 5);
    disabled_channel.enabled = false;
    let mut paused_channel = channel("paused", storage::Protocol::Anthropic, &paused_url, 1);
    paused_channel.auto_disabled_until_ms = i64::MAX;
    let channels = [
        channel("high", storage::Protocol::Anthropic, &high_url, 20),
        channel("low", storage::Protocol::Anthropic, &low_url, 10),
        disabled_channel,
        paused_channel,
    ];

    let body = list(
        storage::Protocol::Anthropic,
        &anthropic_headers(),
        &channels,
    )
    .await;
    let data = body["data"].as_array().unwrap();

    assert_eq!(
        ids(data),
        ["claude-a", "claude-b", "claude-c", "claude-paused"]
    );
    assert_eq!(data[1]["display_name"], "B from high");
    assert_eq!(data[2]["type"], "model");
    assert_eq!(data[2]["created_at"], "2023-11-14T22:13:20Z");
    assert!(disabled_calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn follows_anthropic_pagination() {
    fn paged(uri: &Uri) -> (StatusCode, Value) {
        if uri
            .query()
            .unwrap_or_default()
            .contains("after_id=claude-b")
        {
            (
                StatusCode::OK,
                json!({"data": [{"id": "claude-c"}], "has_more": false, "last_id": "claude-c"}),
            )
        } else {
            (
                StatusCode::OK,
                json!({"data": [{"id": "claude-a"}, {"id": "claude-b"}], "has_more": true, "last_id": "claude-b"}),
            )
        }
    }
    let (url, calls) = spawn_upstream(paged).await;
    let channels = [channel(
        "paged",
        storage::Protocol::Anthropic,
        &format!("{url}/v1"),
        1,
    )];

    let body = list(
        storage::Protocol::Anthropic,
        &anthropic_headers(),
        &channels,
    )
    .await;

    assert_eq!(
        ids(body["data"].as_array().unwrap()),
        ["claude-a", "claude-b", "claude-c"]
    );
    let calls = calls.lock().unwrap();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[1].path, "/v1/models");
    assert_eq!(calls[1].query, "limit=1000&after_id=claude-b");
}

#[tokio::test]
async fn failed_channels_are_skipped_until_all_fail() {
    fn failing(_: &Uri) -> (StatusCode, Value) {
        (StatusCode::UNAUTHORIZED, json!({"error": "bad key"}))
    }
    let (failing_url, _) = spawn_upstream(failing).await;
    let (ok_url, _) = spawn_upstream(anthropic_a).await;

    let channels = [
        channel("failing", storage::Protocol::Anthropic, &failing_url, 20),
        channel("ok", storage::Protocol::Anthropic, &ok_url, 10),
    ];
    let body = list(
        storage::Protocol::Anthropic,
        &anthropic_headers(),
        &channels,
    )
    .await;
    assert_eq!(ids(body["data"].as_array().unwrap()), ["claude-a"]);

    let only_failing = [channel(
        "failing",
        storage::Protocol::Anthropic,
        &failing_url,
        20,
    )];
    let err = proxy::models::list_models(
        &reqwest::Client::new(),
        storage::Protocol::Anthropic,
        &anthropic_headers(),
        &only_failing,
    )
    .await
    .expect_err("all channels failed");
    assert!(matches!(err, proxy::ProxyError::Upstream(_)), "{err:?}");

    let err = proxy::models::list_models(
        &reqwest::Client::new(),
        storage::Protocol::Gemini,
        &HeaderMap::new(),
        &channels,
    )
    .await
    .expect_err("no gemini channel");
    assert!(matches!(
        err,
        proxy::ProxyError::NoEnabledChannel(storage::Protocol::Gemini)
    ));
}

#[tokio::test]
async fn gemini_lists_models_in_gemini_shape() {
    fn gemini(_: &Uri) -> (StatusCode, Value) {
        (
            StatusCode::OK,
            json!({"models": [{"name": "models/gemini-a", "displayName": "Gemini A"}], "nextPageToken": ""}),
        )
    }
    let (url, calls) = spawn_upstream(gemini).await;
    let channels = [channel("gemini", storage::Protocol::Gemini, &url, 1)];

    let body = list(storage::Protocol::Gemini, &HeaderMap::new(), &channels).await;

    assert_eq!(body["models"][0]["name"], "models/gemini-a");
    assert_eq!(body["models"][0]["displayName"], "Gemini A");
    let calls = calls.lock().unwrap();
    assert_eq!(calls[0].path, "/v1beta/models");
    assert_eq!(calls[0].query, "pageSize=1000&key=token-gemini");
}

#[tokio::test]
async fn openai_skips_oauth_channels_and_retrieve_finds_models() {
    let (oauth_url, oauth_calls) = spawn_upstream(openai_gpt).await;
    let (openai_url, _) = spawn_upstream(openai_gpt).await;
    let mut oauth_channel = channel("oauth", storage::Protocol::Openai, &oauth_url, 20);
    oauth_channel.managed_by_remote = true;
    oauth_channel.managed_remote_provider = Some(storage::ManagedRemoteProvider::Openai);
    oauth_channel.managed_remote_account_id = Some("account".to_string());
    let channels = [
        oauth_channel,
        channel("openai", storage::Protocol::Openai, &openai_url, 10),
    ];

    let body = list(storage::Protocol::Openai, &HeaderMap::new(), &channels).await;
    assert_eq!(body["object"], "list");
    assert_eq!(ids(body["data"].as_array().unwrap()), ["gpt-x"]);
    assert!(oauth_calls.lock().unwrap().is_empty());

    let client = reqwest::Client::new();
    let found = proxy::models::retrieve_model(
        &client,
        storage::Protocol::Openai,
        &HeaderMap::new(),
        &channels,
        "gpt-x",
    )
    .await
    .expect("retrieve")
    .expect("model exists");
    let bytes = to_bytes(found.into_body(), 1024 * 1024).await.unwrap();
    let model: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(model["id"], "gpt-x");
    assert_eq!(model["owned_by"], "openai");

    let missing = proxy::models::retrieve_model(
        &client,
        storage::Protocol::Openai,
        &HeaderMap::new(),
        &channels,
        "gpt-missing",
    )
    .await
    .expect("retrieve");
    assert!(missing.is_none());
}

#[tokio::test]
async fn server_routes_model_lists_by_client_protocol() {
    fn gemini(_: &Uri) -> (StatusCode, Value) {
        (
            StatusCode::OK,
            json!({"models": [{"name": "models/gemini-a"}]}),
        )
    }
    let (openai_url, openai_calls) = spawn_upstream(openai_gpt).await;
    let (anthropic_url, _) = spawn_upstream(anthropic_a).await;
    let (gemini_url, _) = spawn_upstream(gemini).await;

    let dir = std::env::temp_dir().join(format!("cliswitch-models-e2e-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("cliswitch.sqlite3");
    storage::init_db(&db).unwrap();
    for (name, protocol, base_url) in [
        ("openai", storage::Protocol::Openai, openai_url),
        ("anthropic", storage::Protocol::Anthropic, anthropic_url),
        ("gemini", storage::Protocol::Gemini, gemini_url),
    ] {
        storage::create_channel(
            db.clone(),
            storage::CreateChannel {
                name: name.to_string(),
                protocol,
                base_url,
                auth_type: None,
                auth_ref: format!("token-{name}"),
                checkin_url: None,
                priority: 1,
                retry_times: 1,
                ignore_channel_protection: false,
                recharge_currency: None,
                real_multiplier: None,
                managed_by_remote: None,
                managed_remote_provider: None,
                managed_remote_account_id: None,
                managed_remote_resource_id: None,
                managed_remote_resource_name: None,
                managed_remote_group_name: None,
                managed_remote_group_id: None,
                enabled: true,
            },
        )
        .await
        .unwrap();
    }

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let backend = tokio::spawn(server::serve_with_listener(listener, db, false));
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if client
                .get(format!("{base}/api/health"))
                .send()
                .await
                .is_ok_and(|r| r.status().is_success())
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();

    let get_json = |path: &str, anthropic: bool| {
        let mut request = client.get(format!("{base}{path}"));
        if anthropic {
            request = request
                .header("anthropic-version", "2023-06-01")
                .bearer_auth("client-placeholder");
        }
        async move {
            let response = request.send().await.unwrap();
            (response.status(), response.json::<Value>().await.unwrap())
        }
    };

    let (status, body) = get_json("/v1/models?limit=1000", true).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(ids(body["data"].as_array().unwrap()), ["claude-a"]);

    let (status, body) = get_json("/v1/models", false).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["object"], "list");
    assert_eq!(ids(body["data"].as_array().unwrap()), ["gpt-x"]);

    let (status, body) = get_json("/v1beta/models", false).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["models"][0]["name"], "models/gemini-a");

    let (status, body) = get_json("/v1/models/claude-a", true).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["id"], "claude-a");
    assert_eq!(body["display_name"], "Claude A");

    let (status, _) = get_json("/v1/models/claude-missing", true).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Other methods keep the transparent OpenAI forwarding.
    let response = client
        .delete(format!("{base}/v1/models/ft-model"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let forwarded = openai_calls
        .lock()
        .unwrap()
        .iter()
        .find(|call| call.method == Method::DELETE)
        .cloned()
        .expect("DELETE forwarded upstream");
    assert_eq!(forwarded.path, "/v1/models/ft-model");

    backend.abort();
}
