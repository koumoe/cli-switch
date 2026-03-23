use anyhow::Context as _;
use base64::Engine as _;
use image::{ImageBuffer, Luma};
use qrcode::QrCode;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader};
use tokio::sync::{Mutex, mpsc, oneshot, watch};

use super::adapter::{ChatAdapter, IncomingAttachment, IncomingAttachmentKind};
use crate::cli_tools::CliExecEnv;
use crate::{nodejs, process};

const BRIDGE_PACKAGE_JSON: &str = include_str!("whatsapp_web/package.json");
const BRIDGE_PACKAGE_LOCK_JSON: &str = include_str!("whatsapp_web/package-lock.json");
const BRIDGE_MJS: &str = include_str!("whatsapp_web/bridge.mjs");

const BRIDGE_INSTALL_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const BRIDGE_SEND_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WhatsAppWebState {
    Disabled,
    Starting,
    AwaitingQr,
    Connected,
    Error,
}

#[derive(Debug, Clone, Serialize)]
pub struct WhatsAppWebStatus {
    pub state: WhatsAppWebState,
    pub connected: bool,
    pub me: Option<String>,
    pub qr: Option<String>,
    pub qr_image: Option<String>,
    pub last_error: Option<String>,
}

impl Default for WhatsAppWebStatus {
    fn default() -> Self {
        Self {
            state: WhatsAppWebState::Disabled,
            connected: false,
            me: None,
            qr: None,
            qr_image: None,
            last_error: None,
        }
    }
}

impl WhatsAppWebStatus {
    pub fn disabled() -> Self {
        Self::default()
    }

    fn starting() -> Self {
        Self {
            state: WhatsAppWebState::Starting,
            connected: false,
            me: None,
            qr: None,
            qr_image: None,
            last_error: None,
        }
    }

    fn error(err: impl Into<String>) -> Self {
        Self {
            state: WhatsAppWebState::Error,
            connected: false,
            me: None,
            qr: None,
            qr_image: None,
            last_error: Some(err.into()),
        }
    }
}

#[derive(Debug, Clone)]
pub enum WhatsAppWebControl {
    StartLogin,
    Logout,
}

#[derive(Debug)]
struct NodeRequest {
    id: String,
    line: String,
    resp_tx: oneshot::Sender<anyhow::Result<serde_json::Value>>,
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

#[derive(Debug, Deserialize)]
struct NodeIncomingAttachment {
    kind: String,
    filename: String,
    #[serde(default)]
    mime_type: Option<String>,
    #[serde(default)]
    caption: Option<String>,
    data_b64: String,
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
    #[serde(default)]
    attachments: Vec<NodeIncomingAttachment>,
}

#[derive(Clone)]
pub(crate) struct WhatsAppWebAdapter {
    req_tx: mpsc::Sender<NodeRequest>,
}

impl WhatsAppWebAdapter {
    async fn send_node_request(
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
            .context("serialize whatsapp web bridge request failed")?;
        let (tx, rx) = oneshot::channel();
        let req = NodeRequest {
            id,
            line,
            resp_tx: tx,
        };
        self.req_tx
            .send(req)
            .await
            .context("send whatsapp web bridge request failed")?;

        tokio::time::timeout(BRIDGE_SEND_TIMEOUT, rx)
            .await
            .context("wait whatsapp web bridge response timed out")?
            .context("whatsapp web bridge response channel closed")?
    }

    async fn send_ping(&self) -> anyhow::Result<serde_json::Value> {
        let id = uuid::Uuid::new_v4().to_string();
        self.send_node_request(serde_json::json!({ "id": id, "type": "ping" }))
            .await
    }
}

#[async_trait::async_trait]
impl ChatAdapter for WhatsAppWebAdapter {
    async fn send_message(
        &self,
        msg: super::adapter::OutgoingMessage,
    ) -> anyhow::Result<super::adapter::SentMessage> {
        let id = uuid::Uuid::new_v4().to_string();
        let attachments: Vec<_> = msg
            .attachments
            .iter()
            .map(|att| {
                serde_json::json!({
                    "filename": att.filename,
                    "mime_type": att.mime_type,
                    "data_b64": base64::engine::general_purpose::STANDARD.encode(&att.data),
                })
            })
            .collect();

        let result = self
            .send_node_request(serde_json::json!({
                "id": id,
                "type": "send",
                "chat_id": msg.chat_id,
                "reply_to": msg.reply_to,
                "content": msg.content,
                "attachments": attachments,
            }))
            .await?;

        let Some(message_id) = result
            .get("message_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
        else {
            anyhow::bail!("whatsapp web bridge returned no message_id");
        };

        Ok(super::adapter::SentMessage { message_id })
    }

    async fn edit_message(
        &self,
        _chat_id: &str,
        _message_id: &str,
        _content: &str,
    ) -> anyhow::Result<()> {
        anyhow::bail!("whatsapp adapter does not support editing messages")
    }

    async fn send_typing(&self, _chat_id: &str) -> anyhow::Result<()> {
        // WhatsApp Web API via Baileys doesn't provide a reliable typing endpoint we can use safely.
        Ok(())
    }

    fn platform(&self) -> crate::storage::ChatPlatform {
        crate::storage::ChatPlatform::WhatsApp
    }
}

struct KillProcessOnDrop {
    pid: Option<u32>,
}

impl Drop for KillProcessOnDrop {
    fn drop(&mut self) {
        if let Some(pid) = self.pid {
            process::kill_process_tree_best_effort(pid);
        }
    }
}

fn whatsapp_base_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("chat-bridge").join("whatsapp-web")
}

fn whatsapp_auth_dir(data_dir: &Path) -> PathBuf {
    whatsapp_base_dir(data_dir).join("auth")
}

fn whatsapp_bridge_dir(data_dir: &Path) -> PathBuf {
    whatsapp_base_dir(data_dir).join("bridge")
}

pub(crate) fn logout_by_clearing_auth_state(data_dir: &Path) -> anyhow::Result<()> {
    let dir = whatsapp_auth_dir(data_dir);
    if !dir.is_dir() {
        return Ok(());
    }
    std::fs::remove_dir_all(&dir)
        .with_context(|| format!("remove whatsapp web auth dir failed: {}", dir.display()))?;
    Ok(())
}

pub(crate) fn qr_image_data_uri(raw: &str) -> anyhow::Result<String> {
    let code = QrCode::new(raw.as_bytes()).context("build whatsapp qr failed")?;
    let width = code.width() as u32;
    let scale = 8u32;
    let quiet = 4u32;
    let size = (width + quiet * 2) * scale;
    let mut img = ImageBuffer::<Luma<u8>, Vec<u8>>::from_pixel(size, size, Luma([255]));

    for y in 0..width {
        for x in 0..width {
            if code[(x as usize, y as usize)] != qrcode::types::Color::Dark {
                continue;
            }
            let start_x = (x + quiet) * scale;
            let start_y = (y + quiet) * scale;
            for dy in 0..scale {
                for dx in 0..scale {
                    img.put_pixel(start_x + dx, start_y + dy, Luma([0]));
                }
            }
        }
    }

    let mut bytes = Vec::new();
    image::DynamicImage::ImageLuma8(img)
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .context("encode whatsapp qr png failed")?;
    Ok(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

async fn ensure_bridge_files(bridge_dir: &Path) -> anyhow::Result<()> {
    tokio::fs::create_dir_all(bridge_dir)
        .await
        .with_context(|| {
            format!(
                "create whatsapp bridge dir failed: {}",
                bridge_dir.display()
            )
        })?;
    tokio::fs::write(bridge_dir.join("package.json"), BRIDGE_PACKAGE_JSON)
        .await
        .with_context(|| "write whatsapp bridge package.json failed")?;
    tokio::fs::write(
        bridge_dir.join("package-lock.json"),
        BRIDGE_PACKAGE_LOCK_JSON,
    )
    .await
    .with_context(|| "write whatsapp bridge package-lock.json failed")?;
    tokio::fs::write(bridge_dir.join("bridge.mjs"), BRIDGE_MJS)
        .await
        .with_context(|| "write whatsapp bridge bridge.mjs failed")?;
    Ok(())
}

fn is_bridge_deps_installed(bridge_dir: &Path) -> bool {
    bridge_dir
        .join("node_modules")
        .join("@whiskeysockets")
        .join("baileys")
        .join("package.json")
        .is_file()
}

fn npm_install_bridge(env: &CliExecEnv, bridge_dir: &Path) -> anyhow::Result<()> {
    let Some(mut cmd) = env.command_for("npm") else {
        anyhow::bail!("npm is not available");
    };
    cmd.current_dir(bridge_dir);
    cmd.args(["ci", "--omit=dev", "--no-audit", "--no-fund", "--silent"]);
    cmd.env("npm_config_update_notifier", "false");
    cmd.env("npm_config_loglevel", "error");
    let out = process::command_output_with_timeout(&mut cmd, BRIDGE_INSTALL_TIMEOUT)
        .context("run npm install for whatsapp bridge failed")?;
    if out.status.success() {
        return Ok(());
    }
    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
    anyhow::bail!(
        "npm install for whatsapp bridge failed: exit={:?} stdout={} stderr={}",
        out.status.code(),
        stdout,
        stderr
    );
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

    if env.command_for("node").is_some() && env.command_for("npm").is_some() {
        return Ok(env);
    }

    let paths = nodejs::ensure_npm_env_installed(http_client, data_dir).await?;
    let npm_path = paths.npm_path.to_string_lossy().to_string();
    let node_path = paths.node_path.to_string_lossy().to_string();
    env = CliExecEnv::new(Some(&npm_path), Some(&node_path));
    Ok(env)
}

fn decode_incoming_attachment(att: NodeIncomingAttachment) -> anyhow::Result<IncomingAttachment> {
    let data = base64::engine::general_purpose::STANDARD
        .decode(att.data_b64.as_bytes())
        .context("decode whatsapp attachment base64 failed")?;

    let kind = match att.kind.as_str() {
        "image" => IncomingAttachmentKind::Image,
        "file" => IncomingAttachmentKind::File,
        other => {
            tracing::debug!(kind = %other, "ignore unsupported whatsapp attachment kind");
            IncomingAttachmentKind::File
        }
    };

    Ok(IncomingAttachment {
        kind,
        filename: att.filename,
        mime_type: att.mime_type,
        data: Arc::from(data.into_boxed_slice()),
        caption: att.caption,
    })
}

pub(super) async fn run_whatsapp_web_bridge(
    runtime: super::ChatBridgeRuntime,
    http_client: reqwest::Client,
    status_tx: watch::Sender<WhatsAppWebStatus>,
) {
    let _ = status_tx.send(WhatsAppWebStatus::starting());

    let data_dir = runtime.data_dir();
    let auth_dir = whatsapp_auth_dir(&data_dir);
    let bridge_dir = whatsapp_bridge_dir(&data_dir);

    if let Err(err) = ensure_bridge_files(&bridge_dir).await {
        let _ = status_tx.send(WhatsAppWebStatus::error(err.to_string()));
        return;
    }

    let settings = runtime.settings_snapshot();
    let env = match ensure_node_env(&http_client, &data_dir, settings.as_ref()).await {
        Ok(v) => v,
        Err(err) => {
            let _ = status_tx.send(WhatsAppWebStatus::error(err.to_string()));
            return;
        }
    };

    if !is_bridge_deps_installed(&bridge_dir) {
        let env2 = env.clone();
        let bridge_dir2 = bridge_dir.clone();
        let install =
            tokio::task::spawn_blocking(move || npm_install_bridge(&env2, &bridge_dir2)).await;
        match install {
            Ok(Ok(())) => {}
            Ok(Err(err)) => {
                let _ = status_tx.send(WhatsAppWebStatus::error(err.to_string()));
                return;
            }
            Err(err) => {
                let _ = status_tx.send(WhatsAppWebStatus::error(format!(
                    "whatsapp bridge install task failed: {err}"
                )));
                return;
            }
        }
    }

    let Some(node_path) = env.find_executable("node") else {
        let _ = status_tx.send(WhatsAppWebStatus::error("node is not available"));
        return;
    };

    let script_path = bridge_dir.join("bridge.mjs");

    let mut cmd = tokio::process::Command::new(node_path);
    cmd.current_dir(&bridge_dir);
    cmd.arg(&script_path);
    cmd.env("CLISWITCH_WHATSAPP_AUTH_DIR", auth_dir);
    cmd.env("CLISWITCH_WHATSAPP_LOG_LEVEL", "silent");
    cmd.env(
        "CLISWITCH_WHATSAPP_BRIDGE_VERSION",
        env!("CARGO_PKG_VERSION"),
    );
    cmd.stdin(std::process::Stdio::piped());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    #[cfg(unix)]
    {
        // Ensure the node process has its own process group so we can kill the whole tree on abort.
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
            let _ = status_tx.send(WhatsAppWebStatus::error(format!(
                "spawn whatsapp web bridge failed: {err}"
            )));
            return;
        }
    };

    let _kill_guard = KillProcessOnDrop { pid: child.id() };

    let Some(child_stdin) = child.stdin.take() else {
        let _ = status_tx.send(WhatsAppWebStatus::error(
            "whatsapp web bridge child stdin unavailable",
        ));
        return;
    };
    let Some(child_stdout) = child.stdout.take() else {
        let _ = status_tx.send(WhatsAppWebStatus::error(
            "whatsapp web bridge child stdout unavailable",
        ));
        return;
    };
    let child_stderr = child.stderr.take();

    let (req_tx, mut req_rx) = mpsc::channel::<NodeRequest>(128);
    let adapter = WhatsAppWebAdapter { req_tx };
    let pending = Arc::new(Mutex::new(HashMap::<
        String,
        oneshot::Sender<anyhow::Result<serde_json::Value>>,
    >::new()));

    let pending_writer = pending.clone();
    let stdin_task = tokio::spawn(async move {
        let mut stdin = child_stdin;
        while let Some(req) = req_rx.recv().await {
            {
                let mut guard = pending_writer.lock().await;
                guard.insert(req.id.clone(), req.resp_tx);
            }

            if let Err(err) = stdin.write_all(req.line.as_bytes()).await {
                let mut guard = pending_writer.lock().await;
                if let Some(tx) = guard.remove(&req.id) {
                    let _ = tx.send(Err(
                        anyhow::anyhow!(err).context("write to whatsapp bridge failed")
                    ));
                }
                continue;
            }
            if let Err(err) = stdin.write_all(b"\n").await {
                let mut guard = pending_writer.lock().await;
                if let Some(tx) = guard.remove(&req.id) {
                    let _ = tx.send(Err(
                        anyhow::anyhow!(err).context("write newline to whatsapp bridge failed")
                    ));
                }
                continue;
            }
        }
    });

    let pending_reader = pending.clone();
    let status_tx_reader = status_tx.clone();
    let adapter_for_inbound = adapter.clone();
    let runtime_for_inbound = runtime.clone();
    let stdout_task = tokio::spawn(async move {
        let mut lines = BufReader::new(child_stdout).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let parsed: serde_json::Value = match serde_json::from_str(trimmed) {
                Ok(v) => v,
                Err(err) => {
                    tracing::warn!(err = %err, line = %trimmed, "decode whatsapp bridge json failed");
                    continue;
                }
            };

            if parsed.get("id").is_some() {
                let env: NodeResponseEnvelope = match serde_json::from_value(parsed) {
                    Ok(v) => v,
                    Err(err) => {
                        tracing::warn!(err = %err, "decode whatsapp bridge response failed");
                        continue;
                    }
                };
                let tx = {
                    let mut guard = pending_reader.lock().await;
                    guard.remove(&env.id)
                };
                if let Some(tx) = tx {
                    let res = if env.ok {
                        env.result.unwrap_or_else(|| serde_json::json!({}))
                    } else {
                        let msg = env.error.unwrap_or_else(|| "unknown error".to_string());
                        let err = anyhow::anyhow!(msg);
                        let _ = tx.send(Err(err));
                        continue;
                    };
                    let _ = tx.send(Ok(res));
                }
                continue;
            }

            let Some(event) = parsed.get("event").and_then(|v| v.as_str()) else {
                continue;
            };

            match event {
                "qr" => {
                    if let Some(qr) = parsed.get("qr").and_then(|v| v.as_str()) {
                        let next = WhatsAppWebStatus {
                            state: WhatsAppWebState::AwaitingQr,
                            connected: false,
                            me: None,
                            qr: Some(qr.to_string()),
                            qr_image: qr_image_data_uri(qr).ok(),
                            last_error: None,
                        };
                        let _ = status_tx_reader.send(next);
                    }
                }
                "ready" => {
                    let me = parsed
                        .get("me")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    let next = WhatsAppWebStatus {
                        state: WhatsAppWebState::Connected,
                        connected: true,
                        me,
                        qr: None,
                        qr_image: None,
                        last_error: None,
                    };
                    let _ = status_tx_reader.send(next);
                }
                "connection" => {
                    let conn = parsed
                        .get("connection")
                        .and_then(|v| v.as_str())
                        .unwrap_or("-");
                    if conn == "open" {
                        // ready will follow with the `me` field, keep it optimistic here.
                        let mut cur = status_tx_reader.borrow().clone();
                        cur.state = WhatsAppWebState::Connected;
                        cur.connected = true;
                        cur.qr = None;
                        cur.qr_image = None;
                        cur.last_error = None;
                        let _ = status_tx_reader.send(cur);
                    } else if conn == "close" {
                        let mut cur = status_tx_reader.borrow().clone();
                        cur.connected = false;
                        cur.state = WhatsAppWebState::Starting;
                        cur.qr = None;
                        cur.qr_image = None;
                        let _ = status_tx_reader.send(cur);
                    }
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
                            tracing::warn!(err = %err, "decode whatsapp bridge inbound message failed");
                            continue;
                        }
                    };

                    let mut attachments = Vec::new();
                    for att in msg.attachments {
                        match decode_incoming_attachment(att) {
                            Ok(v) => attachments.push(v),
                            Err(err) => {
                                tracing::warn!(err = %err, "decode whatsapp bridge attachment failed");
                            }
                        }
                    }

                    let incoming = super::adapter::IncomingMessage {
                        platform: crate::storage::ChatPlatform::WhatsApp,
                        sender_id: msg.sender_id,
                        sender_display_name: msg.sender_display_name,
                        chat_id: msg.chat_id,
                        text: msg.text,
                        attachments,
                        message_id: msg.message_id,
                        timestamp_ms: msg.timestamp_ms,
                    };

                    let runtime = runtime_for_inbound.clone();
                    let sender: Arc<dyn ChatAdapter> = Arc::new(adapter_for_inbound.clone());
                    tokio::spawn(async move {
                        runtime.handle_message(sender, incoming).await;
                    });
                }
                "error" => {
                    let scope = parsed
                        .get("scope")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    let err = parsed
                        .get("error")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    tracing::warn!(scope, err, "whatsapp bridge error event");
                }
                "fatal" => {
                    let err = parsed
                        .get("error")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    let _ = status_tx_reader.send(WhatsAppWebStatus::error(err));
                }
                other => {
                    tracing::debug!(event = %other, "ignore whatsapp bridge event");
                }
            }
        }
    });

    if let Some(stderr) = child_stderr {
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let line = line.trim();
                if !line.is_empty() {
                    tracing::debug!(line, "whatsapp bridge stderr");
                }
            }
        });
    }

    // Keep the adapter alive for as long as stdout task is running.
    let _ = adapter.send_ping().await;

    tokio::select! {
        _ = stdin_task => {}
        _ = stdout_task => {}
        status = child.wait() => {
            if let Ok(status) = status {
                tracing::warn!(code = ?status.code(), "whatsapp bridge exited");
            }
        }
    }

    // Avoid dangling waiters on shutdown.
    let mut guard = pending.lock().await;
    for (_, tx) in guard.drain() {
        let _ = tx.send(Err(anyhow::anyhow!("whatsapp bridge closed")));
    }
    let _ = status_tx.send(WhatsAppWebStatus::error("whatsapp bridge stopped"));
}
