use axum::{Json, Router, routing::post};
use cliswitch::{activity, activity_notify, events, server, storage};
use serde_json::{Value, json};
use std::time::Duration;

#[tokio::test]
async fn proxy_to_codex_notify_is_authenticated_correlated_and_idempotent() {
    let dir = std::env::temp_dir().join(format!("cliswitch-notify-e2e-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("cliswitch.sqlite3");
    storage::init_db(&db).unwrap();
    let upstream = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream_url = format!("http://{}/v1", upstream.local_addr().unwrap());
    let upstream_task = tokio::spawn(async move {
        axum::serve(upstream, Router::new().route("/v1/responses", post(|| async {
            Json(json!({"id":"resp_test","object":"response","status":"completed","output":[{"id":"msg_test","type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":"ok"}]}],"usage":{"input_tokens":1,"output_tokens":1,"total_tokens":2}}))
        }))).await.unwrap();
    });
    storage::create_channel(
        db.clone(),
        storage::CreateChannel {
            name: "local fixture".into(),
            protocol: storage::Protocol::Openai,
            base_url: upstream_url,
            auth_type: None,
            auth_ref: "fixture-key".into(),
            checkin_url: None,
            priority: 1,
            retry_times: 0,
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
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let backend = tokio::spawn(server::serve_with_listener(listener, db, false));
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(2))
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

    let thread = uuid::Uuid::new_v4().to_string();
    let turn = "opaque-turn-1";
    let response = client
        .post(format!("{base}/v1/responses"))
        .header("originator", "codex_cli_rs")
        .header("thread-id", &thread)
        .header(
            "x-codex-turn-metadata",
            json!({"thread_id":thread,"turn_id":turn}).to_string(),
        )
        .json(&json!({"model":"fixture-model","input":"test","stream":false}))
        .send()
        .await
        .unwrap();
    assert!(response.status().is_success());
    let _: Value = response.json().await.unwrap();
    let observed = activity::snapshot()
        .entries
        .into_iter()
        .find(|e| e.thread_id.as_deref() == Some(&thread))
        .unwrap();
    assert_eq!(observed.status, activity::ActivityStatus::ResponseFinished);

    let official_payload=json!({"type":"agent-turn-complete","thread-id":thread,"turn-id":turn,"cwd":"C:\\Users\\测试","input-messages":["must not be retained"],"last-assistant-message":"must not be retained"}).to_string();
    let wrong = client
        .post(format!("{base}/api/activities/codex-notify"))
        .bearer_auth("incorrect")
        .body(official_payload.clone())
        .send()
        .await
        .unwrap();
    assert!(!wrong.status().is_success());

    let mut rx = events::subscribe();
    let callback = tokio::process::Command::new(env!("CARGO_BIN_EXE_cliswitch"))
        .arg("--data-dir")
        .arg(&dir)
        .arg("activity-notify")
        .arg(&official_payload)
        .output()
        .await
        .unwrap();
    assert!(
        callback.status.success(),
        "the generated CLI invocation must work"
    );
    assert!(
        callback.stdout.is_empty(),
        "notification payload must never be echoed"
    );
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if let events::AppEvent::ActivityCompleted(entry) = rx.recv().await.unwrap() {
                assert_eq!(entry.completed_turn_id.as_deref(), Some(turn));
                break;
            }
        }
    })
    .await
    .expect("completion event must be delivered even before a later snapshot is read");
    let result: Value = client
        .get(format!("{base}/api/activities"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let completed = result["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["thread_id"] == thread)
        .unwrap();
    assert_eq!(completed["status"], "completed");
    assert_eq!(completed["completed_turn_id"], turn);
    assert!(!result.to_string().contains("must not be retained"));
    while rx.try_recv().is_ok() {}
    activity_notify::deliver_codex_notification(&dir, &official_payload)
        .await
        .unwrap();
    assert!(
        rx.try_recv().is_err(),
        "duplicate notification must not publish another completion"
    );
    let command: Value = client
        .get(format!("{base}/api/activities/codex-notify-command"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(command["command"][1], "--data-dir");
    assert_eq!(command["command"][3], "activity-notify");

    backend.abort();
    let _ = backend.await;
    upstream_task.abort();
    let _ = upstream_task.await;
    assert!(!std::fs::read_dir(&dir).unwrap().any(|e| {
        e.unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with("cliswitch-activity-notify-")
    }));
    activity_notify::deliver_codex_notification(&dir, &official_payload)
        .await
        .unwrap();
    std::fs::remove_dir_all(dir).unwrap();
}
