use anyhow::Context as _;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, watch};

use super::adapter::ChatAdapter;
use super::node_bridge::{
    KillProcessOnDrop, NodeBridgeClient, close_pending_responses, new_pending_responses,
    spawn_stderr_task, spawn_stdin_task, spawn_stdout_task,
};
use super::whatsapp_web::qr_image_data_uri;
use crate::cli_tools::CliExecEnv;
use crate::nodejs;

const BRIDGE_MJS: &str = include_str!("weixin/bridge.mjs");
const BRIDGE_SEND_TIMEOUT: Duration = Duration::from_secs(30);
const DEFAULT_WEIXIN_BASE_URL: &str = "https://ilinkai.weixin.qq.com";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WeixinState {
    Disabled,
    Starting,
    AwaitingQr,
    Connected,
    Error,
}

#[derive(Debug, Clone, Serialize)]
pub struct WeixinStatus {
    pub state: WeixinState,
    pub connected: bool,
    pub me: Option<String>,
    pub qr: Option<String>,
    pub qr_image: Option<String>,
    pub last_error: Option<String>,
}

impl Default for WeixinStatus {
    fn default() -> Self {
        Self {
            state: WeixinState::Disabled,
            connected: false,
            me: None,
            qr: None,
            qr_image: None,
            last_error: None,
        }
    }
}

impl WeixinStatus {
    pub fn disabled() -> Self {
        Self::default()
    }

    fn starting() -> Self {
        Self {
            state: WeixinState::Starting,
            connected: false,
            me: None,
            qr: None,
            qr_image: None,
            last_error: None,
        }
    }

    fn error(err: impl Into<String>) -> Self {
        Self {
            state: WeixinState::Error,
            connected: false,
            me: None,
            qr: None,
            qr_image: None,
            last_error: Some(err.into()),
        }
    }
}

#[derive(Debug, Clone)]
pub enum WeixinControl {
    StartLogin,
    Logout,
}

#[derive(Debug, Deserialize)]
struct NodeIncomingMessage {
    sender_id: String,
    #[serde(default)]
    sender_display_name: Option<String>,
    chat_id: String,
    #[serde(default)]
    message_id: Option<String>,
    timestamp_ms: i64,
    text: String,
}

#[derive(Debug, Deserialize)]
struct NodeStatusEvent {
    state: WeixinState,
    connected: bool,
    #[serde(default)]
    me: Option<String>,
    #[serde(default)]
    qr: Option<String>,
    #[serde(default)]
    qr_image: Option<String>,
    #[serde(default)]
    last_error: Option<String>,
}

fn resolve_weixin_qr_image(qr: Option<&str>, qr_image: Option<&str>) -> Option<String> {
    if let Some(image) = qr_image {
        let trimmed = image.trim();
        if trimmed.starts_with("data:image/") {
            return Some(trimmed.to_string());
        }

        // The upstream field currently points to a LiteApp landing page, not an image file.
        // The official page uses location.href as the QR payload, so we need to encode the page
        // URL itself instead of the raw token.
        if trimmed.starts_with("https://liteapp.weixin.qq.com/q/")
            || trimmed.starts_with("http://liteapp.weixin.qq.com/q/")
        {
            return qr_image_data_uri(trimmed).ok();
        }

        if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
            return Some(trimmed.to_string());
        }
    }

    // Fall back to the raw token when the runtime only provides the code value.
    qr.and_then(|value| qr_image_data_uri(value).ok())
}

impl From<NodeStatusEvent> for WeixinStatus {
    fn from(value: NodeStatusEvent) -> Self {
        let qr_image = resolve_weixin_qr_image(value.qr.as_deref(), value.qr_image.as_deref());
        Self {
            state: value.state,
            connected: value.connected,
            me: value.me,
            qr: value.qr,
            qr_image,
            last_error: value.last_error,
        }
    }
}

#[derive(Clone)]
pub(crate) struct WeixinAdapter {
    node: NodeBridgeClient,
}

impl WeixinAdapter {
    async fn send_ping(&self) -> anyhow::Result<serde_json::Value> {
        self.node.send_ping().await
    }
}

#[async_trait::async_trait]
impl ChatAdapter for WeixinAdapter {
    async fn send_message(
        &self,
        msg: super::adapter::OutgoingMessage,
    ) -> anyhow::Result<super::adapter::SentMessage> {
        if !msg.attachments.is_empty() {
            anyhow::bail!("weixin adapter does not support attachments yet");
        }

        let id = uuid::Uuid::new_v4().to_string();
        let result = self
            .node
            .send_request(serde_json::json!({
                "id": id,
                "type": "send",
                "chat_id": msg.chat_id,
                "reply_to": msg.reply_to,
                "content": msg.content,
            }))
            .await?;

        let Some(message_id) = result
            .get("message_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
        else {
            anyhow::bail!("weixin bridge returned no message_id");
        };

        Ok(super::adapter::SentMessage { message_id })
    }

    async fn edit_message(
        &self,
        _chat_id: &str,
        _message_id: &str,
        _content: &str,
    ) -> anyhow::Result<()> {
        anyhow::bail!("weixin adapter does not support editing messages")
    }

    async fn send_typing(&self, chat_id: &str) -> anyhow::Result<()> {
        let id = uuid::Uuid::new_v4().to_string();
        let _ = self
            .node
            .send_request(serde_json::json!({
                "id": id,
                "type": "typing",
                "chat_id": chat_id,
            }))
            .await?;
        Ok(())
    }

    fn platform(&self) -> crate::storage::ChatPlatform {
        crate::storage::ChatPlatform::Weixin
    }
}

fn weixin_base_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("chat-bridge").join("weixin")
}

fn weixin_state_dir(data_dir: &Path) -> PathBuf {
    weixin_base_dir(data_dir).join("state")
}

fn weixin_bridge_dir(data_dir: &Path) -> PathBuf {
    weixin_base_dir(data_dir).join("bridge")
}

pub(crate) fn logout_by_clearing_auth_state(data_dir: &Path) -> anyhow::Result<()> {
    let dir = weixin_state_dir(data_dir);
    if !dir.is_dir() {
        return Ok(());
    }
    std::fs::remove_dir_all(&dir)
        .with_context(|| format!("remove weixin state dir failed: {}", dir.display()))?;
    Ok(())
}

async fn ensure_bridge_files(bridge_dir: &Path) -> anyhow::Result<()> {
    tokio::fs::create_dir_all(bridge_dir)
        .await
        .with_context(|| format!("create weixin bridge dir failed: {}", bridge_dir.display()))?;
    tokio::fs::write(bridge_dir.join("bridge.mjs"), BRIDGE_MJS)
        .await
        .with_context(|| "write weixin bridge bridge.mjs failed")?;
    Ok(())
}

async fn ensure_node_env(
    http_client: &reqwest::Client,
    data_dir: &Path,
    settings: &crate::storage::AppSettings,
) -> anyhow::Result<CliExecEnv> {
    let mut env = CliExecEnv::new(
        settings.cli_tools_npm_path.as_deref(),
        settings.cli_tools_node_path.as_deref(),
    );

    if env.command_for("node").is_some() {
        return Ok(env);
    }

    let paths = nodejs::ensure_npm_env_installed(http_client, data_dir).await?;
    let npm_path = paths.npm_path.to_string_lossy().to_string();
    let node_path = paths.node_path.to_string_lossy().to_string();
    env = CliExecEnv::new(Some(&npm_path), Some(&node_path));
    Ok(env)
}

pub(super) async fn run_weixin_bridge(
    runtime: super::ChatBridgeRuntime,
    http_client: reqwest::Client,
    status_tx: watch::Sender<WeixinStatus>,
) {
    let _ = status_tx.send(WeixinStatus::starting());

    let data_dir = runtime.data_dir();
    let state_dir = weixin_state_dir(&data_dir);
    let bridge_dir = weixin_bridge_dir(&data_dir);

    if let Err(err) = ensure_bridge_files(&bridge_dir).await {
        let _ = status_tx.send(WeixinStatus::error(err.to_string()));
        return;
    }

    let settings = runtime.settings_snapshot();
    let env = match ensure_node_env(&http_client, &data_dir, settings.as_ref()).await {
        Ok(v) => v,
        Err(err) => {
            let _ = status_tx.send(WeixinStatus::error(err.to_string()));
            return;
        }
    };

    let Some(node_path) = env.find_executable("node") else {
        let _ = status_tx.send(WeixinStatus::error("node is not available"));
        return;
    };

    let script_path = bridge_dir.join("bridge.mjs");

    let mut cmd = tokio::process::Command::new(node_path);
    cmd.current_dir(&bridge_dir);
    cmd.arg(&script_path);
    cmd.env("CLISWITCH_WEIXIN_STATE_DIR", state_dir);
    cmd.env("CLISWITCH_WEIXIN_DEFAULT_BASE_URL", DEFAULT_WEIXIN_BASE_URL);
    cmd.env("CLISWITCH_WEIXIN_BRIDGE_VERSION", env!("CARGO_PKG_VERSION"));
    cmd.stdin(std::process::Stdio::piped());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    #[cfg(unix)]
    {
        unsafe {
            cmd.pre_exec(|| {
                if libc::setpgid(0, 0) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(err) => {
            let _ = status_tx.send(WeixinStatus::error(format!(
                "spawn weixin bridge failed: {err}"
            )));
            return;
        }
    };

    let _kill_guard = KillProcessOnDrop { pid: child.id() };

    let Some(child_stdin) = child.stdin.take() else {
        let _ = status_tx.send(WeixinStatus::error("weixin bridge child stdin unavailable"));
        return;
    };
    let Some(child_stdout) = child.stdout.take() else {
        let _ = status_tx.send(WeixinStatus::error(
            "weixin bridge child stdout unavailable",
        ));
        return;
    };
    let child_stderr = child.stderr.take();

    let (req_tx, req_rx) = mpsc::channel(128);
    let node = NodeBridgeClient::new(req_tx, "weixin bridge", BRIDGE_SEND_TIMEOUT);
    let adapter = WeixinAdapter { node };
    let pending = new_pending_responses();

    let stdin_task = spawn_stdin_task(child_stdin, req_rx, pending.clone(), "weixin bridge");

    let status_tx_reader = status_tx.clone();
    let adapter_for_inbound = adapter.clone();
    let runtime_for_inbound = runtime.clone();
    let stdout_task = spawn_stdout_task(
        child_stdout,
        pending.clone(),
        "weixin bridge",
        move |parsed| {
            let status_tx_reader = status_tx_reader.clone();
            let adapter_for_inbound = adapter_for_inbound.clone();
            let runtime_for_inbound = runtime_for_inbound.clone();
            async move {
                let Some(event) = parsed.get("event").and_then(|v| v.as_str()) else {
                    return;
                };

                match event {
                    "status" => {
                        let status: NodeStatusEvent = match serde_json::from_value(
                            parsed
                                .get("status")
                                .cloned()
                                .unwrap_or_else(|| serde_json::json!({})),
                        ) {
                            Ok(v) => v,
                            Err(err) => {
                                tracing::warn!(err = %err, "decode weixin bridge status failed");
                                return;
                            }
                        };
                        let _ = status_tx_reader.send(status.into());
                    }
                    "message" => {
                        let msg: NodeIncomingMessage = match serde_json::from_value(
                            parsed
                                .get("message")
                                .cloned()
                                .unwrap_or_else(|| serde_json::json!({})),
                        ) {
                            Ok(v) => v,
                            Err(err) => {
                                tracing::warn!(
                                    err = %err,
                                    "decode weixin bridge inbound message failed"
                                );
                                return;
                            }
                        };

                        let incoming = super::adapter::IncomingMessage {
                            platform: crate::storage::ChatPlatform::Weixin,
                            sender_id: msg.sender_id,
                            sender_display_name: msg.sender_display_name,
                            chat_id: msg.chat_id,
                            text: msg.text,
                            attachments: Vec::new(),
                            message_id: msg.message_id,
                            timestamp_ms: msg.timestamp_ms,
                        };

                        let runtime = runtime_for_inbound.clone();
                        let sender: Arc<dyn ChatAdapter> = Arc::new(adapter_for_inbound.clone());
                        tokio::spawn(async move {
                            runtime.handle_message(sender, incoming).await;
                        });
                    }
                    "log" => {
                        let message = parsed
                            .get("message")
                            .and_then(|v| v.as_str())
                            .unwrap_or_default();
                        if !message.is_empty() {
                            tracing::debug!(message, "weixin bridge log");
                        }
                    }
                    "error" => {
                        let err = parsed
                            .get("error")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown");
                        tracing::warn!(err, "weixin bridge error event");
                    }
                    "fatal" => {
                        let err = parsed
                            .get("error")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown");
                        let _ = status_tx_reader.send(WeixinStatus::error(err));
                    }
                    other => {
                        tracing::debug!(event = %other, "ignore weixin bridge event");
                    }
                }
            }
        },
    );

    if let Some(stderr) = child_stderr {
        spawn_stderr_task(stderr, "weixin bridge");
    }

    let _ = adapter.send_ping().await;

    tokio::select! {
        _ = stdin_task => {}
        _ = stdout_task => {}
        status = child.wait() => {
            if let Ok(status) = status {
                tracing::warn!(code = ?status.code(), "weixin bridge exited");
            }
        }
    }

    close_pending_responses(&pending, "weixin bridge").await;
    let _ = status_tx.send(WeixinStatus::error("weixin bridge stopped"));
}

#[cfg(test)]
mod tests {
    use super::{qr_image_data_uri, resolve_weixin_qr_image};

    #[test]
    fn resolve_weixin_qr_image_prefers_existing_data_uri() {
        let data_uri = "data:image/png;base64,Zm9v";
        assert_eq!(
            resolve_weixin_qr_image(Some("unused"), Some(data_uri)),
            Some(data_uri.to_string())
        );
    }

    #[test]
    fn resolve_weixin_qr_image_generates_local_qr_for_remote_page_url() {
        let landing_page_url = "https://liteapp.weixin.qq.com/q/7GiQu1?qrcode=40e22bde5c819e20d9c1d0c11d13012b&bot_type=3";
        let resolved = resolve_weixin_qr_image(
            Some("40e22bde5c819e20d9c1d0c11d13012b"),
            Some(landing_page_url),
        );

        assert_eq!(resolved, qr_image_data_uri(landing_page_url).ok());
    }

    #[test]
    fn resolve_weixin_qr_image_keeps_regular_remote_image_url() {
        let image_url = "https://example.com/qrcode.png";
        assert_eq!(
            resolve_weixin_qr_image(Some("unused"), Some(image_url)),
            Some(image_url.to_string())
        );
    }
}
