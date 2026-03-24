use anyhow::Context as _;
use serde::Deserialize;
use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader};
use tokio::process::{ChildStderr, ChildStdin, ChildStdout};
use tokio::sync::{Mutex, mpsc, oneshot};

#[derive(Debug)]
pub(super) struct NodeRequest {
    pub id: String,
    pub line: String,
    pub resp_tx: oneshot::Sender<anyhow::Result<serde_json::Value>>,
}

#[derive(Debug, Deserialize)]
struct NodeResponseEnvelope {
    id: String,
    ok: bool,
    #[serde(default)]
    result: Option<serde_json::Value>,
    #[serde(default)]
    error: Option<String>,
}

pub(super) type PendingResponses =
    Arc<Mutex<HashMap<String, oneshot::Sender<anyhow::Result<serde_json::Value>>>>>;

#[derive(Clone)]
pub(super) struct NodeBridgeClient {
    req_tx: mpsc::Sender<NodeRequest>,
    bridge_name: &'static str,
    send_timeout: Duration,
}

impl NodeBridgeClient {
    pub(super) fn new(
        req_tx: mpsc::Sender<NodeRequest>,
        bridge_name: &'static str,
        send_timeout: Duration,
    ) -> Self {
        Self {
            req_tx,
            bridge_name,
            send_timeout,
        }
    }

    pub(super) async fn send_request(
        &self,
        payload: serde_json::Value,
    ) -> anyhow::Result<serde_json::Value> {
        let id = payload
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        if id.is_empty() {
            anyhow::bail!("node request missing id");
        }

        let line = serde_json::to_string(&payload)
            .with_context(|| format!("serialize {} request failed", self.bridge_name))?;
        let (tx, rx) = oneshot::channel();
        let req = NodeRequest {
            id,
            line,
            resp_tx: tx,
        };
        self.req_tx
            .send(req)
            .await
            .with_context(|| format!("send {} request failed", self.bridge_name))?;

        tokio::time::timeout(self.send_timeout, rx)
            .await
            .with_context(|| format!("wait {} response timed out", self.bridge_name))?
            .with_context(|| format!("{} response channel closed", self.bridge_name))?
    }

    pub(super) async fn send_ping(&self) -> anyhow::Result<serde_json::Value> {
        let id = uuid::Uuid::new_v4().to_string();
        self.send_request(serde_json::json!({ "id": id, "type": "ping" }))
            .await
    }
}

pub(super) struct KillProcessOnDrop {
    pub pid: Option<u32>,
}

impl Drop for KillProcessOnDrop {
    fn drop(&mut self) {
        if let Some(pid) = self.pid {
            crate::process::kill_process_tree_best_effort(pid);
        }
    }
}

pub(super) fn new_pending_responses() -> PendingResponses {
    Arc::new(Mutex::new(HashMap::new()))
}

pub(super) fn spawn_stdin_task(
    child_stdin: ChildStdin,
    mut req_rx: mpsc::Receiver<NodeRequest>,
    pending: PendingResponses,
    bridge_name: &'static str,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut stdin = child_stdin;
        while let Some(req) = req_rx.recv().await {
            {
                let mut guard = pending.lock().await;
                guard.insert(req.id.clone(), req.resp_tx);
            }

            if let Err(err) = stdin.write_all(req.line.as_bytes()).await {
                resolve_write_failure(
                    &pending,
                    &req.id,
                    anyhow::anyhow!(err).context(format!("write to {} failed", bridge_name)),
                )
                .await;
                continue;
            }
            if let Err(err) = stdin.write_all(b"\n").await {
                resolve_write_failure(
                    &pending,
                    &req.id,
                    anyhow::anyhow!(err)
                        .context(format!("write newline to {} failed", bridge_name)),
                )
                .await;
            }
        }
    })
}

pub(super) fn spawn_stdout_task<F, Fut>(
    child_stdout: ChildStdout,
    pending: PendingResponses,
    bridge_name: &'static str,
    mut on_event: F,
) -> tokio::task::JoinHandle<()>
where
    F: FnMut(serde_json::Value) -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    tokio::spawn(async move {
        let mut lines = BufReader::new(child_stdout).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let parsed = match serde_json::from_str::<serde_json::Value>(trimmed) {
                Ok(v) => v,
                Err(err) => {
                    tracing::warn!(
                        err = %err,
                        line = %trimmed,
                        bridge = bridge_name,
                        "decode node bridge json failed"
                    );
                    continue;
                }
            };

            if handle_response_message(&parsed, &pending, bridge_name).await {
                continue;
            }

            on_event(parsed).await;
        }
    })
}

pub(super) fn spawn_stderr_task(
    stderr: ChildStderr,
    bridge_name: &'static str,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let line = line.trim();
            if !line.is_empty() {
                tracing::debug!(line, bridge = bridge_name, "node bridge stderr");
            }
        }
    })
}

pub(super) async fn close_pending_responses(pending: &PendingResponses, bridge_name: &'static str) {
    let mut guard = pending.lock().await;
    for (_, tx) in guard.drain() {
        let _ = tx.send(Err(anyhow::anyhow!("{bridge_name} closed")));
    }
}

async fn resolve_write_failure(pending: &PendingResponses, id: &str, err: anyhow::Error) {
    let tx = {
        let mut guard = pending.lock().await;
        guard.remove(id)
    };
    if let Some(tx) = tx {
        let _ = tx.send(Err(err));
    }
}

async fn handle_response_message(
    parsed: &serde_json::Value,
    pending: &PendingResponses,
    bridge_name: &'static str,
) -> bool {
    if parsed.get("id").is_none() {
        return false;
    }

    let env: NodeResponseEnvelope = match serde_json::from_value(parsed.clone()) {
        Ok(v) => v,
        Err(err) => {
            tracing::warn!(err = %err, bridge = bridge_name, "decode node bridge response failed");
            return true;
        }
    };
    let tx = {
        let mut guard = pending.lock().await;
        guard.remove(&env.id)
    };
    if let Some(tx) = tx {
        let result = if env.ok {
            Ok(env.result.unwrap_or_else(|| serde_json::json!({})))
        } else {
            Err(anyhow::anyhow!(
                env.error.unwrap_or_else(|| "unknown error".to_string())
            ))
        };
        let _ = tx.send(result);
    }
    true
}
