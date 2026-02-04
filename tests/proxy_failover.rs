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
    Arc,
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
            checkin_url: None,
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
            checkin_url: None,
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
                checkin_url: None,
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
            checkin_url: None,
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
            checkin_url: None,
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
