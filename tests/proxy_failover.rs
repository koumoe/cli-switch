use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
    routing::any,
};
use bytes::Bytes;
use cliswitch::{proxy, storage};
use futures_util::StreamExt as _;
use futures_util::stream;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
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

async fn spawn_upstream_capturing_json_body() -> (String, Arc<Mutex<Vec<serde_json::Value>>>) {
    let captured = Arc::new(Mutex::new(Vec::new()));
    let captured_for_handler = captured.clone();
    let app = Router::new().route(
        "/{*path}",
        any(move |request: Request<Body>| {
            let captured = captured_for_handler.clone();
            async move {
                let body = to_bytes(request.into_body(), 1024 * 1024).await.unwrap();
                captured
                    .lock()
                    .unwrap()
                    .push(serde_json::from_slice(&body).unwrap());
                (
                    StatusCode::OK,
                    [(axum::http::header::CONTENT_TYPE, "application/json")],
                    r#"{"id":"resp_test","output":[],"usage":{"input_tokens":1,"output_tokens":1,"total_tokens":2}}"#,
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

    (format!("http://127.0.0.1:{}", addr.port()), captured)
}

async fn spawn_upstream_typed(
    status: StatusCode,
    content_type: &'static str,
    body: &'static str,
) -> String {
    let app = Router::new().route(
        "/{*path}",
        any(move || async move {
            (
                status,
                [(axum::http::header::CONTENT_TYPE, content_type)],
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

async fn spawn_upstream_typed_counted(
    status: StatusCode,
    content_type: &'static str,
    body: &'static str,
) -> (String, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let calls_for_handler = calls.clone();
    let app = Router::new().route(
        "/{*path}",
        any(move || {
            let calls = calls_for_handler.clone();
            async move {
                calls.fetch_add(1, Ordering::Relaxed);
                (
                    status,
                    [(axum::http::header::CONTENT_TYPE, content_type)],
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

async fn spawn_upstream_sequence(
    responses: Vec<(StatusCode, &'static str)>,
) -> (String, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let calls2 = calls.clone();
    let responses = Arc::new(responses);
    let app = Router::new().route(
        "/{*path}",
        any(move || {
            let calls = calls2.clone();
            let responses = responses.clone();
            async move {
                let idx = calls.fetch_add(1, Ordering::Relaxed);
                let (status, body) = responses
                    .get(idx)
                    .copied()
                    .or_else(|| responses.last().copied())
                    .expect("responses");
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

async fn create_openai_channel(
    db_path: std::path::PathBuf,
    name: &str,
    base_url: String,
    priority: i64,
    retry_times: i64,
    ignore_channel_protection: bool,
) {
    storage::create_channel(
        db_path,
        storage::CreateChannel {
            name: name.to_string(),
            protocol: storage::Protocol::Openai,
            base_url,
            auth_type: None,
            auth_ref: format!("token-{name}"),
            checkin_url: None,
            priority,
            retry_times,
            ignore_channel_protection,
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
    .expect("create channel");
}

async fn forward_openai_responses(
    db_path: std::path::PathBuf,
    base_url: String,
) -> axum::response::Response {
    create_openai_channel(db_path.clone(), "openai", base_url, 10, 1, false).await;
    let request = Request::builder()
        .method("POST")
        .uri("/v1/responses")
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            r#"{"model":"gpt-test","input":"hello","stream":true}"#,
        ))
        .expect("request");

    proxy::forward(
        &reqwest::Client::new(),
        db_path,
        storage::Protocol::Openai,
        "/v1",
        request,
    )
    .await
    .expect("forward")
}

#[tokio::test]
async fn openai_responses_reasoning_id_sanitizer_respects_setting() {
    let (base_url, captured) = spawn_upstream_capturing_json_body().await;
    let db_path = temp_db_path();
    storage::init_db(&db_path).unwrap();
    create_openai_channel(
        db_path.clone(),
        "openai",
        format!("{base_url}/v1"),
        10,
        1,
        false,
    )
    .await;
    let channels = Arc::new(storage::list_channels(db_path.clone()).await.unwrap());

    for enabled in [true, false] {
        let mut settings = storage::get_app_settings(db_path.clone()).await.unwrap();
        settings.openai_responses_reasoning_id_sanitizer_enabled = enabled;
        let request = Request::builder()
            .method("POST")
            .uri("/v1/responses")
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                r#"{"model":"gpt-test","input":[{"type":"message","id":"item_message","role":"assistant","content":[]},{"type":"reasoning","id":"item_reasoning","encrypted_content":"opaque","summary":[]},{"type":"function_call","id":"item_call","call_id":"call_1","name":"tool","arguments":"{}"}],"stream":false}"#,
            ))
            .unwrap();
        let response = proxy::forward_with_config(
            &reqwest::Client::new(),
            None,
            db_path.clone(),
            storage::Protocol::Openai,
            "/v1",
            request,
            proxy::ProxyConfigSnapshot {
                settings: Arc::new(settings),
                channels: channels.clone(),
                channels_cache: None,
                codex_identity: Arc::new(proxy::CodexClientIdentity::for_version(None)),
            },
        )
        .await
        .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let _ = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    }

    let captured = captured.lock().unwrap();
    assert_eq!(captured.len(), 2);
    assert_eq!(captured[0]["input"].as_array().unwrap().len(), 3);
    assert_eq!(captured[0]["input"][0]["id"], "item_message");
    assert!(captured[0]["input"][1].get("id").is_none());
    assert_eq!(captured[0]["input"][1]["encrypted_content"], "opaque");
    assert_eq!(captured[0]["input"][1]["summary"], serde_json::json!([]));
    assert_eq!(captured[0]["input"][2]["id"], "item_call");
    assert_eq!(captured[0]["input"][2]["call_id"], "call_1");
    assert_eq!(captured[1]["input"][1]["id"], "item_reasoning");
}

#[tokio::test]
async fn managed_openai_account_forwards_responses_with_dynamic_oauth_credentials() {
    #[derive(Clone, Default)]
    struct Captured {
        authorization: String,
        account_id: String,
        originator: String,
        user_agent: String,
        version: String,
        responses_lite: String,
        path: String,
        body: serde_json::Value,
    }

    let captured = Arc::new(Mutex::new(Vec::<Captured>::new()));
    let captured_handler = captured.clone();
    let app = Router::new().route(
        "/codex/responses",
        any(move |request: Request<Body>| {
            let captured = captured_handler.clone();
            async move {
                let authorization = request
                    .headers()
                    .get(axum::http::header::AUTHORIZATION)
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or_default()
                    .to_string();
                let account_id = request
                    .headers()
                    .get("chatgpt-account-id")
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or_default()
                    .to_string();
                let originator = request
                    .headers()
                    .get("originator")
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or_default()
                    .to_string();
                let user_agent = request
                    .headers()
                    .get(axum::http::header::USER_AGENT)
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or_default()
                    .to_string();
                let version = request
                    .headers()
                    .get("version")
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or_default()
                    .to_string();
                let responses_lite = request
                    .headers()
                    .get("X-OpenAI-Internal-Codex-Responses-Lite")
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or_default()
                    .to_string();
                let path = request.uri().path().to_string();
                let body = to_bytes(request.into_body(), 1024 * 1024).await.unwrap();
                let body = serde_json::from_slice(&body).unwrap();
                captured.lock().unwrap().push(Captured {
                    authorization,
                    account_id,
                    originator,
                    user_agent,
                    version,
                    responses_lite,
                    path,
                    body,
                });
                let mut headers = axum::http::HeaderMap::new();
                headers.insert(axum::http::header::CONTENT_TYPE, "text/event-stream".parse().unwrap());
                headers.insert("x-codex-primary-used-percent", "42".parse().unwrap());
                headers.insert("x-codex-primary-window-minutes", "10080".parse().unwrap());
                headers.insert("x-codex-primary-reset-after-seconds", "60".parse().unwrap());
                (
                    StatusCode::OK,
                    headers,
                    "data: {\"type\":\"response.completed\",\"response\":{\"usage\":{\"input_tokens\":1,\"output_tokens\":1,\"total_tokens\":2}}}\n\n",
                )
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        // Simulate the transient connection reset seen at the ChatGPT edge.
        if let Ok((stream, _)) = listener.accept().await {
            drop(stream);
        }
        let _ = axum::serve(listener, app).await;
    });

    let db_path = temp_db_path();
    storage::init_db(&db_path).unwrap();
    let account = storage::upsert_openai_account_tokens(
        db_path.clone(),
        Some("OpenAI test".to_string()),
        storage::OpenAiAccountTokens {
            access_token: "oauth-access-token".to_string(),
            refresh_token: Some("oauth-refresh-token".to_string()),
            id_token: Some("id-token".to_string()),
            token_expires_at_ms: Some(i64::MAX),
            account_id: "chatgpt-account-1".to_string(),
            email: Some("user@example.com".to_string()),
            display_name: None,
            plan_type: Some("plus".to_string()),
        },
    )
    .await
    .unwrap();
    storage::create_channel(
        db_path.clone(),
        storage::CreateChannel {
            name: "OpenAI managed".to_string(),
            protocol: storage::Protocol::Openai,
            base_url: format!("http://{addr}/codex"),
            auth_type: Some("managed_account".to_string()),
            auth_ref: String::new(),
            checkin_url: None,
            priority: 10,
            retry_times: 1,
            ignore_channel_protection: false,
            recharge_currency: None,
            real_multiplier: None,
            enabled: true,
            managed_by_remote: Some(true),
            managed_remote_provider: Some(storage::ManagedRemoteProvider::Openai),
            managed_remote_account_id: Some(account.id.clone()),
            managed_remote_resource_id: Some(account.remote_user_id.clone()),
            managed_remote_resource_name: None,
            managed_remote_group_name: None,
            managed_remote_group_id: None,
        },
    )
    .await
    .unwrap();

    let settings = Arc::new(storage::get_app_settings(db_path.clone()).await.unwrap());
    let channels = Arc::new(storage::list_channels(db_path.clone()).await.unwrap());
    for responses_lite in [false, true] {
        let mut request = Request::builder()
            .method("POST")
            .uri("/v1/responses")
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .header(axum::http::header::USER_AGENT, "codex_cli_rs/0.21.0")
            .header("originator", "codex_cli_rs")
            .header("version", "0.21.0");
        if responses_lite {
            request = request.header("X-OpenAI-Internal-Codex-Responses-Lite", "true");
        }
        let request = request
            .body(Body::from(
                r#"{"model":"gpt-5.6-sol","input":[{"type":"reasoning","id":"item_oauth_reasoning","encrypted_content":"opaque","summary":[]},{"type":"message","id":"item_message","role":"user","content":[{"type":"input_text","text":"hello"}]}],"parallel_tool_calls":true,"temperature":0.5}"#,
            ))
            .unwrap();
        let response = proxy::forward_with_config(
            &reqwest::Client::new(),
            None,
            db_path.clone(),
            storage::Protocol::Openai,
            "/v1",
            request,
            proxy::ProxyConfigSnapshot {
                settings: settings.clone(),
                channels: channels.clone(),
                channels_cache: None,
                codex_identity: Arc::new(proxy::CodexClientIdentity::for_version(Some("0.149.1"))),
            },
        )
        .await
        .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let _ = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    }

    let captured = captured.lock().unwrap().clone();
    assert_eq!(captured.len(), 2);
    for request in &captured {
        assert_eq!(request.authorization, "Bearer oauth-access-token");
        assert_eq!(request.account_id, "chatgpt-account-1");
        assert_eq!(request.originator, "codex-tui");
        assert_eq!(request.version, "0.149.1");
        assert!(request.user_agent.starts_with("codex-tui/0.149.1 "));
        assert_eq!(request.path, "/codex/responses");
        assert_eq!(request.body["model"], "gpt-5.6-sol");
        assert_eq!(request.body["stream"], true);
        assert_eq!(request.body["store"], false);
        assert!(request.body.get("temperature").is_none());
        assert!(request.body["input"][0].get("id").is_none());
        assert_eq!(request.body["input"][0]["encrypted_content"], "opaque");
        assert_eq!(request.body["input"][0]["summary"], serde_json::json!([]));
        assert_eq!(request.body["input"][1]["id"], "item_message");
    }
    assert_eq!(captured[0].responses_lite, "");
    assert_eq!(captured[0].body["parallel_tool_calls"], true);
    assert_eq!(captured[1].responses_lite, "true");
    assert_eq!(captured[1].body["parallel_tool_calls"], false);

    for _ in 0..20 {
        let updated =
            storage::get_openai_account_without_secret(db_path.clone(), account.id.clone())
                .await
                .unwrap();
        if updated.quota.primary.is_some() {
            assert_eq!(updated.quota.primary.unwrap().used_percent, 42.0);
            return;
        }
        sleep(Duration::from_millis(10)).await;
    }
    panic!("OpenAI quota headers were not persisted");
}

#[tokio::test]
async fn managed_openai_account_load_failure_fails_over_to_next_channel() {
    let (fallback_url, fallback_calls) =
        spawn_upstream_counted(StatusCode::OK, r#"{"ok":true}"#).await;
    let db_path = temp_db_path();
    storage::init_db(&db_path).unwrap();
    storage::create_channel(
        db_path.clone(),
        storage::CreateChannel {
            name: "broken OpenAI account".to_string(),
            protocol: storage::Protocol::Openai,
            base_url: "https://chatgpt.com/backend-api/codex".to_string(),
            auth_type: Some("managed_account".to_string()),
            auth_ref: String::new(),
            checkin_url: None,
            priority: 20,
            retry_times: 1,
            ignore_channel_protection: false,
            recharge_currency: None,
            real_multiplier: None,
            enabled: true,
            managed_by_remote: Some(true),
            managed_remote_provider: Some(storage::ManagedRemoteProvider::Openai),
            managed_remote_account_id: Some("missing-account".to_string()),
            managed_remote_resource_id: None,
            managed_remote_resource_name: None,
            managed_remote_group_name: None,
            managed_remote_group_id: None,
        },
    )
    .await
    .unwrap();
    create_openai_channel(db_path.clone(), "fallback", fallback_url, 10, 1, false).await;

    let request = Request::builder()
        .method("POST")
        .uri("/v1/responses")
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"model":"gpt-test","input":"hello"}"#))
        .unwrap();
    let response = proxy::forward(
        &reqwest::Client::new(),
        db_path,
        storage::Protocol::Openai,
        "/v1",
        request,
    )
    .await
    .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(fallback_calls.load(Ordering::Relaxed), 1);
}

async fn spawn_upstream_stream_error() -> String {
    // Use a raw TCP server that sends a truncated chunked response so reqwest can
    // successfully receive response headers, but fail while reading the body stream.
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("local_addr");

    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else {
                break;
            };
            tokio::spawn(async move {
                // Best-effort read request headers to avoid closing too early.
                let mut buf = [0u8; 4096];
                let mut seen = Vec::<u8>::new();
                for _ in 0..8 {
                    let Ok(n) = sock.read(&mut buf).await else {
                        break;
                    };
                    if n == 0 {
                        break;
                    }
                    seen.extend_from_slice(&buf[..n]);
                    if seen.windows(4).any(|w| w == b"\r\n\r\n") {
                        break;
                    }
                    if seen.len() > 16 * 1024 {
                        break;
                    }
                }

                let body =
                    br#"data: {"usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}}

"#;
                let hdr = concat!(
                    "HTTP/1.1 200 OK\r\n",
                    "Content-Type: text/event-stream\r\n",
                    "Transfer-Encoding: chunked\r\n",
                    "\r\n"
                );

                if sock.write_all(hdr.as_bytes()).await.is_ok() {
                    let _ = sock
                        .write_all(format!("{:x}\r\n", body.len()).as_bytes())
                        .await;
                    let _ = sock.write_all(body).await;
                    let _ = sock.write_all(b"\r\n").await;
                    let _ = sock.flush().await;
                }

                // Drop the socket without sending the terminating `0\r\n\r\n`.
            });
        }
    });

    format!("http://127.0.0.1:{}", addr.port())
}

async fn spawn_upstream_stream_openai_terminal_then_error() -> String {
    // Send a terminal marker and then abruptly close a chunked response so reqwest yields a body
    // error _after_ we already observed `response.completed`.
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("local_addr");

    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else {
                break;
            };
            tokio::spawn(async move {
                // Best-effort read request headers to avoid closing too early.
                let mut buf = [0u8; 4096];
                let mut seen = Vec::<u8>::new();
                for _ in 0..8 {
                    let Ok(n) = sock.read(&mut buf).await else {
                        break;
                    };
                    if n == 0 {
                        break;
                    }
                    seen.extend_from_slice(&buf[..n]);
                    if seen.windows(4).any(|w| w == b"\r\n\r\n") {
                        break;
                    }
                    if seen.len() > 16 * 1024 {
                        break;
                    }
                }

                let body = br#"event: response.completed
data: {"type":"response.completed","usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}}

"#;
                let hdr = concat!(
                    "HTTP/1.1 200 OK\r\n",
                    "Content-Type: text/event-stream\r\n",
                    "Transfer-Encoding: chunked\r\n",
                    "\r\n"
                );

                if sock.write_all(hdr.as_bytes()).await.is_ok() {
                    let _ = sock
                        .write_all(format!("{:x}\r\n", body.len()).as_bytes())
                        .await;
                    let _ = sock.write_all(body).await;
                    let _ = sock.write_all(b"\r\n").await;
                    let _ = sock.flush().await;
                }

                // Drop the socket without sending the terminating `0\r\n\r\n`.
            });
        }
    });

    format!("http://127.0.0.1:{}", addr.port())
}

async fn spawn_upstream_stream_anthropic_terminal_then_error() -> String {
    // Similar to OpenAI: send `message_stop` and then close early to trigger a body error.
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("local_addr");

    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else {
                break;
            };
            tokio::spawn(async move {
                // Best-effort read request headers to avoid closing too early.
                let mut buf = [0u8; 4096];
                let mut seen = Vec::<u8>::new();
                for _ in 0..8 {
                    let Ok(n) = sock.read(&mut buf).await else {
                        break;
                    };
                    if n == 0 {
                        break;
                    }
                    seen.extend_from_slice(&buf[..n]);
                    if seen.windows(4).any(|w| w == b"\r\n\r\n") {
                        break;
                    }
                    if seen.len() > 16 * 1024 {
                        break;
                    }
                }

                let body = br#"event: message_stop
data: {"type":"message_stop","usage":{"input_tokens":1,"output_tokens":1}}

"#;
                let hdr = concat!(
                    "HTTP/1.1 200 OK\r\n",
                    "Content-Type: text/event-stream\r\n",
                    "Transfer-Encoding: chunked\r\n",
                    "\r\n"
                );

                if sock.write_all(hdr.as_bytes()).await.is_ok() {
                    let _ = sock
                        .write_all(format!("{:x}\r\n", body.len()).as_bytes())
                        .await;
                    let _ = sock.write_all(body).await;
                    let _ = sock.write_all(b"\r\n").await;
                    let _ = sock.flush().await;
                }

                // Drop the socket without sending the terminating `0\r\n\r\n`.
            });
        }
    });

    format!("http://127.0.0.1:{}", addr.port())
}

async fn spawn_upstream_stream_ok() -> String {
    let app = Router::new().route(
        "/{*path}",
        any(|| async {
            let s = stream::iter(vec![Ok::<Bytes, std::io::Error>(Bytes::from_static(
                br#"data: {"usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}}

"#,
            ))]);
            (
                StatusCode::OK,
                [(axum::http::header::CONTENT_TYPE, "text/event-stream")],
                Body::from_stream(s),
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

async fn spawn_upstream_stream_openai_terminal_then_hang() -> String {
    let app = Router::new().route(
        "/{*path}",
        any(|| async {
            let first = Bytes::from_static(
                b"event: response.completed\n\
data: {\"type\":\"response.completed\"}\n\
\n",
            );
            let s = stream::once(async move { Ok::<Bytes, std::io::Error>(first) })
                .chain(stream::pending::<Result<Bytes, std::io::Error>>());
            (
                StatusCode::OK,
                [(axum::http::header::CONTENT_TYPE, "text/event-stream")],
                Body::from_stream(s),
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

async fn spawn_upstream_stream_anthropic_terminal_then_hang() -> String {
    let app = Router::new().route(
        "/{*path}",
        any(|| async {
            let first = Bytes::from_static(
                b"event: message_stop\n\
data: {\"type\":\"message_stop\"}\n\
\n",
            );
            let s = stream::once(async move { Ok::<Bytes, std::io::Error>(first) })
                .chain(stream::pending::<Result<Bytes, std::io::Error>>());
            (
                StatusCode::OK,
                [(axum::http::header::CONTENT_TYPE, "text/event-stream")],
                Body::from_stream(s),
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
async fn openai_responses_detects_sse_with_wrong_content_type() {
    let base = spawn_upstream_typed(
        StatusCode::OK,
        "application/json",
        concat!(
            "event: response.created\n",
            "data: {\"type\":\"response.created\",\"response\":{\"status\":\"in_progress\"}}\n\n",
            "event: response.output_text.delta\n",
            "data: {\"type\":\"response.output_text.delta\",\"delta\":\"hello\"}\n\n",
            "event: response.completed\n",
            "data: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\",\"usage\":{\"input_tokens\":4,\"output_tokens\":1,\"total_tokens\":5}}}\n\n",
        ),
    )
    .await;
    let db_path = temp_db_path();
    storage::init_db(&db_path).expect("init_db");

    let response = forward_openai_responses(db_path.clone(), format!("{base}/v1")).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(axum::http::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("text/event-stream")
    );
    let body = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("read body");
    assert!(String::from_utf8_lossy(&body).contains("response.completed"));

    let event = wait_for_usage_event(db_path).await;
    assert!(event.success);
    assert_eq!(event.prompt_tokens, Some(4));
    assert_eq!(event.completion_tokens, Some(1));
    assert_eq!(event.total_tokens, Some(5));
}

#[tokio::test]
async fn openai_responses_preserves_complete_json_when_upstream_ignores_stream() {
    let response_json = r#"{"id":"resp_json","object":"response","status":"completed","output":[{"type":"message","content":[{"type":"output_text","text":"whole response"}]}],"usage":{"input_tokens":11,"output_tokens":4,"total_tokens":15}}"#;

    for content_type in ["application/json", "text/event-stream"] {
        let base = spawn_upstream_typed(StatusCode::OK, content_type, response_json).await;
        let db_path = temp_db_path();
        storage::init_db(&db_path).expect("init_db");

        let response = forward_openai_responses(db_path.clone(), format!("{base}/v1")).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(axum::http::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            Some("application/json")
        );
        let body = to_bytes(response.into_body(), 1024 * 1024)
            .await
            .expect("read body");
        let body: serde_json::Value = serde_json::from_slice(&body).expect("parse response JSON");
        assert_eq!(body["id"], "resp_json");
        assert_eq!(body["output"][0]["content"][0]["text"], "whole response");

        let event = wait_for_usage_event(db_path).await;
        assert!(event.success);
        assert_eq!(event.prompt_tokens, Some(11));
        assert_eq!(event.completion_tokens, Some(4));
        assert_eq!(event.total_tokens, Some(15));
    }
}

#[tokio::test]
async fn openai_responses_rejects_non_sse_invalid_json_body() {
    let base = spawn_upstream_typed(StatusCode::OK, "application/json", "not valid JSON").await;
    let db_path = temp_db_path();
    storage::init_db(&db_path).expect("init_db");

    let response = forward_openai_responses(db_path.clone(), format!("{base}/v1")).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(axum::http::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("text/event-stream")
    );
    let body = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("read body");
    let body = String::from_utf8_lossy(&body);
    assert!(body.contains("event: response.failed"));
    assert!(body.contains("openai_responses_incomplete_stream"));

    let event = wait_for_usage_event(db_path).await;
    assert!(!event.success);
    assert_eq!(
        event.error_kind.as_deref(),
        Some("openai_responses_incomplete_stream")
    );
}

#[tokio::test]
async fn openai_responses_records_failed_terminal_event() {
    let base = spawn_upstream_typed(
        StatusCode::OK,
        "text/event-stream",
        concat!(
            "event: response.failed\n",
            "data: {\"type\":\"response.failed\",\"response\":{\"status\":\"failed\",\"error\":{\"code\":\"context_length_exceeded\",\"message\":\"input is too long\"}}}\n\n",
        ),
    )
    .await;
    let db_path = temp_db_path();
    storage::init_db(&db_path).expect("init_db");

    let response = forward_openai_responses(db_path.clone(), format!("{base}/v1")).await;
    assert_eq!(response.status(), StatusCode::OK);
    let _ = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("read body");

    let event = wait_for_usage_event(db_path).await;
    assert!(!event.success);
    assert_eq!(
        event.error_kind.as_deref(),
        Some("upstream_sse:response.failed")
    );
    assert_eq!(event.error_detail.as_deref(), Some("input is too long"));
}

#[tokio::test]
async fn openai_responses_request_failure_does_not_fail_over() {
    let (failed_base, failed_calls) = spawn_upstream_typed_counted(
        StatusCode::OK,
        "text/event-stream",
        concat!(
            "event: response.failed\n",
            "data: {\"type\":\"response.failed\",\"response\":{\"status\":\"failed\",\"error\":{\"code\":\"context_length_exceeded\",\"message\":\"input is too long\"}}}\n\n",
        ),
    )
    .await;
    let (healthy_base, healthy_calls) = spawn_upstream_typed_counted(
        StatusCode::OK,
        "text/event-stream",
        concat!(
            "event: response.output_text.delta\n",
            "data: {\"type\":\"response.output_text.delta\",\"delta\":\"should not run\"}\n\n",
            "event: response.completed\n",
            "data: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\",\"usage\":{\"input_tokens\":1,\"output_tokens\":1}}}\n\n",
        ),
    )
    .await;
    let db_path = temp_db_path();
    storage::init_db(&db_path).expect("init_db");
    create_openai_channel(
        db_path.clone(),
        "request-failure",
        format!("{failed_base}/v1"),
        20,
        1,
        false,
    )
    .await;
    create_openai_channel(
        db_path.clone(),
        "healthy",
        format!("{healthy_base}/v1"),
        10,
        1,
        false,
    )
    .await;

    let request = Request::builder()
        .method("POST")
        .uri("/v1/responses")
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            r#"{"model":"gpt-test","input":"hello","stream":true}"#,
        ))
        .expect("request");
    let response = proxy::forward(
        &reqwest::Client::new(),
        db_path.clone(),
        storage::Protocol::Openai,
        "/v1",
        request,
    )
    .await
    .expect("forward");
    let body = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("read body");

    assert_eq!(failed_calls.load(Ordering::Relaxed), 1);
    assert_eq!(healthy_calls.load(Ordering::Relaxed), 0);
    let body = String::from_utf8_lossy(&body);
    assert!(body.contains("context_length_exceeded"));
    assert!(!body.contains("should not run"));

    let event = wait_for_usage_event(db_path).await;
    assert!(!event.success);
    assert_eq!(
        event.error_kind.as_deref(),
        Some("upstream_sse:response.failed")
    );
}

#[tokio::test]
async fn openai_responses_accepts_incomplete_terminal_with_usage() {
    let base = spawn_upstream_typed(
        StatusCode::OK,
        "text/event-stream",
        concat!(
            "event: response.output_text.delta\n",
            "data: {\"type\":\"response.output_text.delta\",\"delta\":\"partial answer\"}\n\n",
            "event: response.incomplete\n",
            "data: {\"type\":\"response.incomplete\",\"response\":{\"status\":\"incomplete\",\"incomplete_details\":{\"reason\":\"max_output_tokens\"},\"usage\":{\"input_tokens\":7,\"output_tokens\":3,\"total_tokens\":10}}}\n\n",
        ),
    )
    .await;
    let db_path = temp_db_path();
    storage::init_db(&db_path).expect("init_db");

    let response = forward_openai_responses(db_path.clone(), format!("{base}/v1")).await;
    assert_eq!(response.status(), StatusCode::OK);
    let _ = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("read body");

    let event = wait_for_usage_event(db_path).await;
    assert!(event.success);
    assert_eq!(event.prompt_tokens, Some(7));
    assert_eq!(event.completion_tokens, Some(3));
    assert_eq!(event.total_tokens, Some(10));
}

#[tokio::test]
async fn openai_responses_records_missing_terminal_event() {
    let base = spawn_upstream_typed(
        StatusCode::OK,
        "text/event-stream",
        concat!(
            "event: response.created\n",
            "data: {\"type\":\"response.created\",\"response\":{\"status\":\"in_progress\"}}\n\n",
            "event: response.output_text.delta\n",
            "data: {\"type\":\"response.output_text.delta\",\"delta\":\"partial\"}\n\n",
        ),
    )
    .await;
    let db_path = temp_db_path();
    storage::init_db(&db_path).expect("init_db");

    let response = forward_openai_responses(db_path.clone(), format!("{base}/v1")).await;
    assert_eq!(response.status(), StatusCode::OK);
    let _ = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("read body");

    let event = wait_for_usage_event(db_path).await;
    assert!(!event.success);
    assert_eq!(
        event.error_kind.as_deref(),
        Some("openai_responses_incomplete_stream")
    );
}

#[tokio::test]
async fn openai_responses_records_empty_completed_as_silent_refusal() {
    let base = spawn_upstream_typed(
        StatusCode::OK,
        "text/event-stream",
        concat!(
            "event: response.created\n",
            "data: {\"type\":\"response.created\",\"response\":{\"status\":\"in_progress\"}}\n\n",
            "event: response.completed\n",
            "data: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\",\"output\":[]}}\n\n",
        ),
    )
    .await;
    let db_path = temp_db_path();
    storage::init_db(&db_path).expect("init_db");

    let response = forward_openai_responses(db_path.clone(), format!("{base}/v1")).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("read body");
    let body = String::from_utf8_lossy(&body);
    assert!(body.contains("event: response.failed"));
    assert!(body.contains("openai_silent_refusal"));

    let event = wait_for_usage_event(db_path).await;
    assert!(!event.success);
    assert_eq!(event.error_kind.as_deref(), Some("openai_silent_refusal"));
}

#[tokio::test]
async fn openai_responses_silent_refusal_fails_over_before_output() {
    let (failed_base, failed_calls) = spawn_upstream_typed_counted(
        StatusCode::OK,
        "text/event-stream",
        concat!(
            "event: response.created\n",
            "data: {\"type\":\"response.created\",\"response\":{\"status\":\"in_progress\"}}\n\n",
            "event: response.completed\n",
            "data: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\",\"output\":[]}}\n\n",
        ),
    )
    .await;
    let (healthy_base, healthy_calls) = spawn_upstream_typed_counted(
        StatusCode::OK,
        "text/event-stream",
        concat!(
            "event: response.output_text.delta\n",
            "data: {\"type\":\"response.output_text.delta\",\"delta\":\"fallback answer\"}\n\n",
            "event: response.completed\n",
            "data: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\",\"usage\":{\"input_tokens\":8,\"output_tokens\":2,\"total_tokens\":10}}}\n\n",
        ),
    )
    .await;
    let db_path = temp_db_path();
    storage::init_db(&db_path).expect("init_db");
    create_openai_channel(
        db_path.clone(),
        "silent",
        format!("{failed_base}/v1"),
        20,
        1,
        false,
    )
    .await;
    create_openai_channel(
        db_path.clone(),
        "healthy",
        format!("{healthy_base}/v1"),
        10,
        1,
        false,
    )
    .await;

    let request = Request::builder()
        .method("POST")
        .uri("/v1/responses")
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            r#"{"model":"gpt-test","input":"hello","stream":true}"#,
        ))
        .expect("request");
    let response = proxy::forward(
        &reqwest::Client::new(),
        db_path.clone(),
        storage::Protocol::Openai,
        "/v1",
        request,
    )
    .await
    .expect("forward");
    let body = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("read body");

    assert_eq!(failed_calls.load(Ordering::Relaxed), 1);
    assert_eq!(healthy_calls.load(Ordering::Relaxed), 1);
    let body = String::from_utf8_lossy(&body);
    assert!(body.contains("fallback answer"));
    assert!(!body.contains("openai_silent_refusal"));

    for _ in 0..100 {
        let events = storage::list_usage_events_recent(db_path.clone(), 10)
            .await
            .expect("list usage events");
        if events.len() >= 2 {
            assert!(events.iter().any(|event| {
                !event.success && event.error_kind.as_deref() == Some("openai_silent_refusal")
            }));
            assert!(
                events
                    .iter()
                    .any(|event| event.success && event.total_tokens == Some(10))
            );
            return;
        }
        sleep(Duration::from_millis(10)).await;
    }
    panic!("timeout waiting for failover usage events");
}

#[tokio::test]
async fn openai_responses_auth_failure_fails_over_before_output() {
    let (failed_base, failed_calls) = spawn_upstream_typed_counted(
        StatusCode::OK,
        "text/event-stream",
        concat!(
            "event: response.failed\n",
            "data: {\"type\":\"response.failed\",\"response\":{\"status\":\"failed\",\"error\":{\"code\":\"unauthorized\",\"message\":\"credential rejected\"}}}\n\n",
        ),
    )
    .await;
    let (healthy_base, healthy_calls) = spawn_upstream_typed_counted(
        StatusCode::OK,
        "text/event-stream",
        concat!(
            "event: response.output_text.delta\n",
            "data: {\"type\":\"response.output_text.delta\",\"delta\":\"healthy account\"}\n\n",
            "event: response.completed\n",
            "data: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\",\"usage\":{\"input_tokens\":2,\"output_tokens\":2,\"total_tokens\":4}}}\n\n",
        ),
    )
    .await;
    let db_path = temp_db_path();
    storage::init_db(&db_path).expect("init_db");
    create_openai_channel(
        db_path.clone(),
        "unauthorized",
        format!("{failed_base}/v1"),
        20,
        1,
        false,
    )
    .await;
    create_openai_channel(
        db_path.clone(),
        "healthy",
        format!("{healthy_base}/v1"),
        10,
        1,
        false,
    )
    .await;

    let request = Request::builder()
        .method("POST")
        .uri("/v1/responses")
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            r#"{"model":"gpt-test","input":"hello","stream":true}"#,
        ))
        .expect("request");
    let response = proxy::forward(
        &reqwest::Client::new(),
        db_path.clone(),
        storage::Protocol::Openai,
        "/v1",
        request,
    )
    .await
    .expect("forward");
    let body = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("read body");

    assert_eq!(failed_calls.load(Ordering::Relaxed), 1);
    assert_eq!(healthy_calls.load(Ordering::Relaxed), 1);
    assert!(String::from_utf8_lossy(&body).contains("healthy account"));

    for _ in 0..100 {
        let events = storage::list_usage_events_recent(db_path.clone(), 10)
            .await
            .expect("list usage events");
        if events.len() >= 2 {
            assert!(events.iter().any(|event| {
                !event.success && event.error_detail.as_deref() == Some("credential rejected")
            }));
            assert!(
                events
                    .iter()
                    .any(|event| event.success && event.total_tokens == Some(4))
            );
            return;
        }
        sleep(Duration::from_millis(10)).await;
    }
    panic!("timeout waiting for authentication failover usage events");
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
            checkin_url: None,
            priority: 30,
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
    .expect("create c1");
    storage::create_channel(
        db_path.clone(),
        storage::CreateChannel {
            name: "c2".to_string(),
            protocol: storage::Protocol::Openai,
            base_url: format!("{base2}/v1"),
            auth_type: None,
            auth_ref: "t2".to_string(),
            checkin_url: None,
            priority: 20,
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
    .expect("create c2");
    storage::create_channel(
        db_path.clone(),
        storage::CreateChannel {
            name: "c3".to_string(),
            protocol: storage::Protocol::Openai,
            base_url: format!("{base3}/v1"),
            auth_type: None,
            auth_ref: "t3".to_string(),
            checkin_url: None,
            priority: 10,
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
                checkin_url: None,
                priority,
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
async fn channel_retry_retries_same_channel_until_success() {
    let (base, calls) = spawn_upstream_sequence(vec![
        (StatusCode::INTERNAL_SERVER_ERROR, r#"{"err":"retry-1"}"#),
        (StatusCode::BAD_GATEWAY, r#"{"err":"retry-2"}"#),
        (StatusCode::OK, r#"{"ok":true}"#),
    ])
    .await;

    let db_path = temp_db_path();
    storage::init_db(&db_path).expect("init_db");
    storage::update_app_settings(
        db_path.clone(),
        storage::AppSettingsPatch {
            channel_retry_enabled: Some(true),
            ..Default::default()
        },
    )
    .await
    .expect("update settings");

    create_openai_channel(db_path.clone(), "c1", format!("{base}/v1"), 10, 3, false).await;

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
    assert_eq!(calls.load(Ordering::Relaxed), 3);
}

#[tokio::test]
async fn channel_retry_stops_after_channel_protection_triggers() {
    let (base1, c1_calls) =
        spawn_upstream_counted(StatusCode::INTERNAL_SERVER_ERROR, r#"{"err":"c1"}"#).await;
    let (base2, c2_calls) = spawn_upstream_counted(StatusCode::OK, r#"{"ok":true}"#).await;

    let db_path = temp_db_path();
    storage::init_db(&db_path).expect("init_db");
    storage::update_app_settings(
        db_path.clone(),
        storage::AppSettingsPatch {
            channel_retry_enabled: Some(true),
            auto_disable_enabled: Some(true),
            auto_disable_window_minutes: Some(3),
            auto_disable_failure_times: Some(2),
            auto_disable_disable_minutes: Some(30),
            ..Default::default()
        },
    )
    .await
    .expect("update settings");

    create_openai_channel(db_path.clone(), "c1", format!("{base1}/v1"), 20, 5, false).await;
    create_openai_channel(db_path.clone(), "c2", format!("{base2}/v1"), 10, 1, false).await;

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
    assert_eq!(c1_calls.load(Ordering::Relaxed), 2);
    assert_eq!(c2_calls.load(Ordering::Relaxed), 1);

    let channels = storage::list_channels(db_path.clone())
        .await
        .expect("list channels");
    let c1 = channels
        .iter()
        .find(|channel| channel.name == "c1")
        .expect("c1");
    assert!(c1.auto_disabled_until_ms > 0);
}

#[tokio::test]
async fn channel_retry_ignore_channel_protection_keeps_retrying_same_channel() {
    let (base1, c1_calls) =
        spawn_upstream_counted(StatusCode::INTERNAL_SERVER_ERROR, r#"{"err":"c1"}"#).await;
    let (base2, c2_calls) = spawn_upstream_counted(StatusCode::OK, r#"{"ok":true}"#).await;

    let db_path = temp_db_path();
    storage::init_db(&db_path).expect("init_db");
    storage::update_app_settings(
        db_path.clone(),
        storage::AppSettingsPatch {
            channel_retry_enabled: Some(true),
            auto_disable_enabled: Some(true),
            auto_disable_window_minutes: Some(3),
            auto_disable_failure_times: Some(1),
            auto_disable_disable_minutes: Some(30),
            ..Default::default()
        },
    )
    .await
    .expect("update settings");

    create_openai_channel(db_path.clone(), "c1", format!("{base1}/v1"), 20, 3, true).await;
    create_openai_channel(db_path.clone(), "c2", format!("{base2}/v1"), 10, 1, false).await;

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
    assert_eq!(c1_calls.load(Ordering::Relaxed), 3);
    assert_eq!(c2_calls.load(Ordering::Relaxed), 1);

    let channels = storage::list_channels(db_path.clone())
        .await
        .expect("list channels");
    let c1 = channels
        .iter()
        .find(|channel| channel.name == "c1")
        .expect("c1");
    assert_eq!(c1.auto_disabled_until_ms, 0);
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
            checkin_url: None,
            priority: 10,
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
async fn stream_error_still_records_usage_event() {
    let base = spawn_upstream_stream_error().await;

    let db_path = temp_db_path();
    storage::init_db(&db_path).expect("init_db");

    storage::create_channel(
        db_path.clone(),
        storage::CreateChannel {
            name: "c1".to_string(),
            protocol: storage::Protocol::Openai,
            base_url: format!("{base}/v1"),
            auth_type: None,
            auth_ref: "t1".to_string(),
            checkin_url: None,
            priority: 10,
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
    .expect("create channel");

    let client = reqwest::Client::builder().build().expect("client");
    let req = Request::builder()
        .method("POST")
        .uri("/v1/chat/completions")
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"model":"gpt-test","stream":true}"#))
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
    assert!(
        to_bytes(resp.into_body(), 1024 * 1024).await.is_err(),
        "expected body read error"
    );

    let event = wait_for_usage_event(db_path.clone()).await;
    assert!(!event.success, "expected usage event success=false");
    assert!(
        event
            .error_kind
            .as_deref()
            .unwrap_or("")
            .starts_with("stream_error:"),
        "expected error_kind to start with stream_error:, got: {:?}",
        event.error_kind
    );
}

#[tokio::test]
async fn stream_drop_still_records_usage_event() {
    let base = spawn_upstream_stream_ok().await;

    let db_path = temp_db_path();
    storage::init_db(&db_path).expect("init_db");

    storage::create_channel(
        db_path.clone(),
        storage::CreateChannel {
            name: "c1".to_string(),
            protocol: storage::Protocol::Openai,
            base_url: format!("{base}/v1"),
            auth_type: None,
            auth_ref: "t1".to_string(),
            checkin_url: None,
            priority: 10,
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
    .expect("create channel");

    let client = reqwest::Client::builder().build().expect("client");
    let req = Request::builder()
        .method("POST")
        .uri("/v1/chat/completions")
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"model":"gpt-test","stream":true}"#))
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
    drop(resp); // simulate client disconnect / early drop

    let event = wait_for_usage_event(db_path.clone()).await;
    assert!(!event.success, "expected usage event success=false");
    assert!(
        event
            .error_kind
            .as_deref()
            .unwrap_or("")
            .contains("stream_dropped"),
        "expected error_kind to include stream_dropped, got: {:?}",
        event.error_kind
    );
}

#[tokio::test]
async fn stream_drop_after_openai_terminal_is_success() {
    let base = spawn_upstream_stream_openai_terminal_then_hang().await;

    let db_path = temp_db_path();
    storage::init_db(&db_path).expect("init_db");

    storage::create_channel(
        db_path.clone(),
        storage::CreateChannel {
            name: "c1".to_string(),
            protocol: storage::Protocol::Openai,
            base_url: format!("{base}/v1"),
            auth_type: None,
            auth_ref: "t1".to_string(),
            checkin_url: None,
            priority: 10,
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
    .expect("create channel");

    let client = reqwest::Client::builder().build().expect("client");
    let req = Request::builder()
        .method("POST")
        .uri("/v1/chat/completions")
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"model":"gpt-test","stream":true}"#))
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

    // Read enough data to observe the terminal marker, then drop the client stream.
    let mut s = resp.into_body().into_data_stream();
    let first = s.next().await.expect("first frame").expect("frame ok");
    assert!(
        String::from_utf8_lossy(&first).contains("response.completed"),
        "expected response.completed in first chunk"
    );
    drop(s);

    let event = wait_for_usage_event(db_path.clone()).await;
    assert!(event.success, "expected usage event success=true");
    assert!(
        event.error_kind.is_none(),
        "expected error_kind=None, got: {:?}",
        event.error_kind
    );
}

#[tokio::test]
async fn stream_drop_after_anthropic_terminal_is_success() {
    let base = spawn_upstream_stream_anthropic_terminal_then_hang().await;

    let db_path = temp_db_path();
    storage::init_db(&db_path).expect("init_db");

    storage::create_channel(
        db_path.clone(),
        storage::CreateChannel {
            name: "c1".to_string(),
            protocol: storage::Protocol::Anthropic,
            base_url: format!("{base}/v1"),
            auth_type: None,
            auth_ref: "t1".to_string(),
            checkin_url: None,
            priority: 10,
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
    .expect("create channel");

    let client = reqwest::Client::builder().build().expect("client");
    let req = Request::builder()
        .method("POST")
        .uri("/v1/messages")
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"model":"claude-test","stream":true}"#))
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

    assert_eq!(resp.status(), StatusCode::OK);

    // Read enough data to observe the terminal marker, then drop the client stream.
    let mut s = resp.into_body().into_data_stream();
    let first = s.next().await.expect("first frame").expect("frame ok");
    assert!(
        String::from_utf8_lossy(&first).contains("message_stop"),
        "expected message_stop in first chunk"
    );
    drop(s);

    let event = wait_for_usage_event(db_path.clone()).await;
    assert!(event.success, "expected usage event success=true");
    assert!(
        event.error_kind.is_none(),
        "expected error_kind=None, got: {:?}",
        event.error_kind
    );
}

#[tokio::test]
async fn stream_upstream_error_after_openai_terminal_is_success() {
    let base = spawn_upstream_stream_openai_terminal_then_error().await;

    let db_path = temp_db_path();
    storage::init_db(&db_path).expect("init_db");

    storage::create_channel(
        db_path.clone(),
        storage::CreateChannel {
            name: "c1".to_string(),
            protocol: storage::Protocol::Openai,
            base_url: format!("{base}/v1"),
            auth_type: None,
            auth_ref: "t1".to_string(),
            checkin_url: None,
            priority: 10,
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
    .expect("create channel");

    let client = reqwest::Client::builder().build().expect("client");
    let req = Request::builder()
        .method("POST")
        .uri("/v1/chat/completions")
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"model":"gpt-test","stream":true}"#))
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
    assert!(
        String::from_utf8_lossy(&bytes).contains("response.completed"),
        "expected response.completed in body"
    );

    let event = wait_for_usage_event(db_path.clone()).await;
    assert!(event.success, "expected usage event success=true");
    assert!(
        event.error_kind.is_none(),
        "expected error_kind=None, got: {:?}",
        event.error_kind
    );
}

#[tokio::test]
async fn stream_upstream_error_after_anthropic_terminal_is_success() {
    let base = spawn_upstream_stream_anthropic_terminal_then_error().await;

    let db_path = temp_db_path();
    storage::init_db(&db_path).expect("init_db");

    storage::create_channel(
        db_path.clone(),
        storage::CreateChannel {
            name: "c1".to_string(),
            protocol: storage::Protocol::Anthropic,
            base_url: format!("{base}/v1"),
            auth_type: None,
            auth_ref: "t1".to_string(),
            checkin_url: None,
            priority: 10,
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
    .expect("create channel");

    let client = reqwest::Client::builder().build().expect("client");
    let req = Request::builder()
        .method("POST")
        .uri("/v1/messages")
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"model":"claude-test","stream":true}"#))
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

    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), 1024 * 1024)
        .await
        .expect("read body");
    assert!(
        String::from_utf8_lossy(&bytes).contains("message_stop"),
        "expected message_stop in body"
    );

    let event = wait_for_usage_event(db_path.clone()).await;
    assert!(event.success, "expected usage event success=true");
    assert!(
        event.error_kind.is_none(),
        "expected error_kind=None, got: {:?}",
        event.error_kind
    );
}

#[tokio::test]
async fn anthropic_count_tokens_failover_and_no_usage_log() {
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
            checkin_url: None,
            priority: 30,
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
    .expect("create c1");
    storage::create_channel(
        db_path.clone(),
        storage::CreateChannel {
            name: "c2".to_string(),
            protocol: storage::Protocol::Anthropic,
            base_url: format!("{base2}/v1"),
            auth_type: None,
            auth_ref: "t2".to_string(),
            checkin_url: None,
            priority: 20,
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

    // Some upstreams don't support this endpoint (403/404). We should try the next channel
    // instead of letting Claude Code fall back to haiku.
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), 1024 * 1024)
        .await
        .expect("read body");
    assert_eq!(
        std::str::from_utf8(&bytes).unwrap(),
        r#"{"input_tokens":123}"#
    );

    assert_eq!(c1_calls.load(Ordering::Relaxed), 1);
    assert_eq!(c2_calls.load(Ordering::Relaxed), 1);
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
            checkin_url: None,
            priority: 10,
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

#[tokio::test]
async fn anthropic_count_tokens_mock_enabled_returns_local_estimate_without_channels() {
    let db_path = temp_db_path();
    storage::init_db(&db_path).expect("init_db");
    storage::update_app_settings(
        db_path.clone(),
        storage::AppSettingsPatch {
            anthropic_count_tokens_mock_enabled: Some(true),
            ..Default::default()
        },
    )
    .await
    .expect("update settings");

    let client = reqwest::Client::builder().build().expect("client");
    let body = r#"{"model":"claude-test","messages":[{"role":"user","content":"foo"}],"tools":[{"name":"mcp__chrome-devtools__performance_analyze_insight","description":"Provides more detailed information on a specific Performance Insight.","input_schema":{"type":"object","properties":{"insightSetId":{"type":"string"},"insightName":{"type":"string"}},"required":["insightSetId","insightName"],"additionalProperties":false}}]}"#;
    let req = Request::builder()
        .method("POST")
        .uri("/v1/messages/count_tokens?beta=true")
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
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

    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), 1024 * 1024)
        .await
        .expect("read body");
    let v: serde_json::Value = serde_json::from_slice(&bytes).expect("parse resp json");
    let got = v
        .get("input_tokens")
        .and_then(|n| n.as_i64())
        .expect("input_tokens missing");

    let req_v: serde_json::Value = serde_json::from_str(body).expect("parse req json");
    let canonical = serde_json::to_string(&req_v).expect("canonical json");
    let ascii_bytes = canonical.as_bytes().iter().filter(|b| b.is_ascii()).count();
    let non_ascii_chars = canonical.chars().filter(|c| !c.is_ascii()).count();
    let expected = (ascii_bytes.div_ceil(4) + non_ascii_chars) as i64;
    assert_eq!(got, expected);

    assert_no_usage_events(db_path.clone()).await;
}

#[tokio::test]
async fn anthropic_count_tokens_mock_enabled_does_not_hit_upstream() {
    let (base, calls) = spawn_upstream_counted(StatusCode::OK, r#"{"input_tokens":999}"#).await;

    let db_path = temp_db_path();
    storage::init_db(&db_path).expect("init_db");
    storage::update_app_settings(
        db_path.clone(),
        storage::AppSettingsPatch {
            anthropic_count_tokens_mock_enabled: Some(true),
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
            checkin_url: None,
            priority: 10,
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
    .expect("create channel");

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
    assert_eq!(resp.status(), StatusCode::OK);

    // If we still hit upstream, we'd get {"input_tokens":999} and calls would be 1.
    let bytes = to_bytes(resp.into_body(), 1024 * 1024)
        .await
        .expect("read body");
    assert_ne!(
        std::str::from_utf8(&bytes).unwrap(),
        r#"{"input_tokens":999}"#
    );
    assert_eq!(calls.load(Ordering::Relaxed), 0);
    assert_no_usage_events(db_path.clone()).await;
}
