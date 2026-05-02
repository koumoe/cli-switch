use anyhow::Context as _;
use base64::Engine as _;
use image::{ImageBuffer, Luma};
use qrcode::QrCode;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc, watch};

use super::adapter::{
    Attachment, ChatAdapter, IncomingAttachment, IncomingAttachmentKind, OutgoingMessage,
    SentMessage,
};
use wacore_binary::jid::SERVER_JID;
use wacore_binary::node::NodeContent;
use whatsapp_rust::bot::{Bot, BotHandle};
use whatsapp_rust::download::{Downloadable, MediaType};
use whatsapp_rust::proto_helpers::{self, MessageExt as _};
use whatsapp_rust::request::InfoQuery;
use whatsapp_rust::send::RevokeType;
use whatsapp_rust::store::SqliteStore;
use whatsapp_rust::types::events::{ConnectFailureReason, Event};
use whatsapp_rust::types::message::MessageInfo;
use whatsapp_rust::upload::UploadResponse;
use whatsapp_rust::{Client, Jid, TokioRuntime, waproto::whatsapp as wa};
use whatsapp_rust_tokio_transport::TokioWebSocketTransportFactory;

const STORE_FILE_NAME: &str = "session.sqlite3";
const LOGOUT_MARKER_FILE_NAME: &str = "pending-logout";
const MAX_ATTACHMENT_BYTES: u64 = 20 * 1024 * 1024;
const RECENT_LIMIT: usize = 256;
const PUBLIC_RUNTIME_ERROR_CODE: &str = "runtime_unavailable";
const RUNTIME_SHUTDOWN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

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
        let err = err.into();
        tracing::error!(err = %err, "whatsapp runtime entered error state");
        Self {
            state: WhatsAppWebState::Error,
            connected: false,
            me: None,
            qr: None,
            qr_image: None,
            last_error: Some(PUBLIC_RUNTIME_ERROR_CODE.to_string()),
        }
    }

    fn warn(err: impl Into<String>) -> Self {
        let err = err.into();
        tracing::warn!(err = %err, "whatsapp runtime entered degraded state");
        Self {
            state: WhatsAppWebState::Error,
            connected: false,
            me: None,
            qr: None,
            qr_image: None,
            last_error: Some(PUBLIC_RUNTIME_ERROR_CODE.to_string()),
        }
    }
}

#[derive(Debug, Clone)]
pub enum WhatsAppWebControl {
    StartLogin,
    Logout,
}

#[derive(Debug, Clone)]
pub enum WhatsAppBridgeCommand {
    Logout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum WhatsAppBridgeExit {
    Retryable,
    AuthInvalid,
    Terminal,
    LogoutCompleted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuntimeEventAction {
    Continue,
    Stop(WhatsAppBridgeExit),
}

#[derive(Debug, Clone)]
struct CachedMessage {
    info: MessageInfo,
    message: Box<wa::Message>,
}

#[derive(Debug, Default)]
struct RecentMessageCache {
    entries: HashMap<String, CachedMessage>,
    order: VecDeque<String>,
}

impl RecentMessageCache {
    fn insert(&mut self, id: String, message: CachedMessage) {
        self.order.retain(|existing| existing != &id);
        self.entries.insert(id.clone(), message);
        self.order.push_back(id);
        while self.order.len() > RECENT_LIMIT {
            if let Some(oldest) = self.order.pop_front() {
                self.entries.remove(&oldest);
            }
        }
    }

    fn get(&self, id: &str) -> Option<&CachedMessage> {
        self.entries.get(id)
    }
}

#[derive(Debug, Default)]
struct RecentIdCache {
    entries: HashSet<String>,
    order: VecDeque<String>,
}

impl RecentIdCache {
    fn insert(&mut self, id: String) {
        if self.entries.insert(id.clone()) {
            self.order.push_back(id);
        }
        while self.order.len() > RECENT_LIMIT {
            if let Some(oldest) = self.order.pop_front() {
                self.entries.remove(&oldest);
            }
        }
    }

    fn take(&mut self, id: &str) -> bool {
        if !self.entries.remove(id) {
            return false;
        }
        self.order.retain(|existing| existing != id);
        true
    }
}

#[derive(Clone)]
pub(crate) struct WhatsAppWebAdapter {
    client: Arc<Client>,
    recent_messages: Arc<Mutex<RecentMessageCache>>,
    sent_message_ids: Arc<Mutex<RecentIdCache>>,
}

impl WhatsAppWebAdapter {
    fn new(client: Arc<Client>) -> Self {
        Self {
            client,
            recent_messages: Arc::new(Mutex::new(RecentMessageCache::default())),
            sent_message_ids: Arc::new(Mutex::new(RecentIdCache::default())),
        }
    }

    async fn cache_message(&self, id: &str, message: &wa::Message, info: &MessageInfo) {
        let mut guard = self.recent_messages.lock().await;
        guard.insert(
            id.to_string(),
            CachedMessage {
                info: info.clone(),
                message: Box::new(message.clone()),
            },
        );
    }

    async fn quote_context(&self, chat_id: &Jid, reply_to: &str) -> Option<wa::ContextInfo> {
        let cached = {
            let guard = self.recent_messages.lock().await;
            guard.get(reply_to).cloned()
        }?;

        if cached.info.source.chat.to_non_ad() != chat_id.to_non_ad() {
            return None;
        }

        Some(proto_helpers::build_quote_context_with_info(
            &cached.info.id,
            &cached.info.source.sender,
            &cached.info.source.chat,
            cached.message.as_ref(),
        ))
    }

    async fn remember_sent_message_id(&self, id: &str) {
        let mut guard = self.sent_message_ids.lock().await;
        guard.insert(id.to_string());
    }

    async fn take_sent_message_id(&self, id: &str) -> bool {
        let mut guard = self.sent_message_ids.lock().await;
        guard.take(id)
    }
}

#[async_trait::async_trait]
impl ChatAdapter for WhatsAppWebAdapter {
    async fn send_message(&self, msg: OutgoingMessage) -> anyhow::Result<SentMessage> {
        let chat_id: Jid = msg
            .chat_id
            .parse()
            .with_context(|| format!("parse whatsapp chat_id failed: {}", msg.chat_id))?;

        let quote_context = match msg.reply_to.as_deref() {
            Some(reply_to) => self.quote_context(&chat_id, reply_to).await,
            None => None,
        };

        let mut first_message_id = None;

        if !msg.content.trim().is_empty() {
            let outgoing = build_text_message(&msg.content, quote_context.clone());
            let message_id = self
                .client
                .send_message(chat_id.clone(), outgoing)
                .await
                .context("send whatsapp text message failed")?;
            self.remember_sent_message_id(&message_id).await;
            first_message_id = Some(message_id);
        }

        for (index, attachment) in msg.attachments.iter().enumerate() {
            let include_quote = first_message_id.is_none() && index == 0;
            let outgoing = build_attachment_message(
                &self.client,
                attachment,
                include_quote.then_some(quote_context.clone()).flatten(),
            )
            .await?;
            let message_id = self
                .client
                .send_message(chat_id.clone(), outgoing)
                .await
                .with_context(|| {
                    format!("send whatsapp attachment failed: {}", attachment.filename)
                })?;
            self.remember_sent_message_id(&message_id).await;
            if first_message_id.is_none() {
                first_message_id = Some(message_id);
            }
        }

        let Some(message_id) = first_message_id else {
            anyhow::bail!("outgoing message is empty");
        };

        Ok(SentMessage { message_id })
    }

    async fn edit_message(
        &self,
        chat_id: &str,
        message_id: &str,
        content: &str,
    ) -> anyhow::Result<()> {
        let chat_id: Jid = chat_id
            .parse()
            .with_context(|| format!("parse whatsapp chat_id failed: {chat_id}"))?;
        let message = wa::Message {
            extended_text_message: Some(Box::new(wa::message::ExtendedTextMessage {
                text: Some(content.to_string()),
                ..Default::default()
            })),
            ..Default::default()
        };
        self.client
            .edit_message(chat_id, message_id.to_string(), message)
            .await
            .with_context(|| format!("edit whatsapp message failed: {message_id}"))?;
        self.remember_sent_message_id(message_id).await;
        Ok(())
    }

    async fn delete_message(&self, chat_id: &str, message_id: &str) -> anyhow::Result<()> {
        let chat_id: Jid = chat_id
            .parse()
            .with_context(|| format!("parse whatsapp chat_id failed: {chat_id}"))?;
        self.client
            .revoke_message(chat_id, message_id.to_string(), RevokeType::Sender)
            .await
            .with_context(|| format!("delete whatsapp message failed: {message_id}"))?;
        Ok(())
    }

    async fn send_typing(&self, chat_id: &str) -> anyhow::Result<()> {
        let chat_id: Jid = chat_id
            .parse()
            .with_context(|| format!("parse whatsapp chat_id failed: {chat_id}"))?;
        self.client
            .chatstate()
            .send_composing(&chat_id)
            .await
            .context("send whatsapp typing failed")?;
        Ok(())
    }

    fn platform(&self) -> crate::storage::ChatPlatform {
        crate::storage::ChatPlatform::WhatsApp
    }
}

fn whatsapp_base_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("chat-bridge").join("whatsapp-web")
}

fn whatsapp_store_path(data_dir: &Path) -> PathBuf {
    whatsapp_base_dir(data_dir).join(STORE_FILE_NAME)
}

fn whatsapp_logout_marker_path(data_dir: &Path) -> PathBuf {
    whatsapp_base_dir(data_dir).join(LOGOUT_MARKER_FILE_NAME)
}

fn clear_local_auth_state(data_dir: &Path) -> anyhow::Result<()> {
    let base_dir = whatsapp_base_dir(data_dir);
    if base_dir.exists() {
        std::fs::remove_dir_all(&base_dir)
            .with_context(|| format!("remove whatsapp base dir failed: {}", base_dir.display()))?;
        return Ok(());
    }

    let marker = whatsapp_logout_marker_path(data_dir);
    if marker.is_file() {
        std::fs::remove_file(&marker).with_context(|| {
            format!("remove whatsapp logout marker failed: {}", marker.display())
        })?;
    }

    Ok(())
}

pub(crate) fn logout_by_clearing_auth_state(data_dir: &Path) -> anyhow::Result<()> {
    let base_dir = whatsapp_base_dir(data_dir);
    std::fs::create_dir_all(&base_dir)
        .with_context(|| format!("create whatsapp base dir failed: {}", base_dir.display()))?;
    std::fs::write(whatsapp_logout_marker_path(data_dir), b"logout")
        .with_context(|| "mark whatsapp logout pending failed")?;
    Ok(())
}

fn apply_pending_logout(data_dir: &Path) -> anyhow::Result<()> {
    let marker = whatsapp_logout_marker_path(data_dir);
    if !marker.is_file() {
        return Ok(());
    }

    clear_local_auth_state(data_dir)
}

#[derive(Clone)]
struct ReqwestHttpClient {
    async_client: reqwest::Client,
    blocking_client: reqwest::blocking::Client,
}

impl ReqwestHttpClient {
    fn new(async_client: reqwest::Client) -> anyhow::Result<Self> {
        let blocking_client = reqwest::blocking::Client::builder()
            .build()
            .context("build blocking reqwest client for whatsapp failed")?;
        Ok(Self {
            async_client,
            blocking_client,
        })
    }
}

#[async_trait::async_trait]
impl whatsapp_rust::http::HttpClient for ReqwestHttpClient {
    async fn execute(
        &self,
        request: whatsapp_rust::http::HttpRequest,
    ) -> anyhow::Result<whatsapp_rust::http::HttpResponse> {
        let mut builder = match request.method.as_str() {
            "GET" => self.async_client.get(&request.url),
            "POST" => self.async_client.post(&request.url),
            method => anyhow::bail!("unsupported HTTP method: {method}"),
        };

        for (key, value) in &request.headers {
            builder = builder.header(key, value);
        }
        if let Some(body) = request.body {
            builder = builder.body(body);
        }

        let response = builder.send().await?;
        let status_code = response.status().as_u16();
        let body = response.bytes().await?.to_vec();
        Ok(whatsapp_rust::http::HttpResponse { status_code, body })
    }

    fn execute_streaming(
        &self,
        request: whatsapp_rust::http::HttpRequest,
    ) -> anyhow::Result<wacore::net::StreamingHttpResponse> {
        let mut builder = match request.method.as_str() {
            "GET" => self.blocking_client.get(&request.url),
            "POST" => self.blocking_client.post(&request.url),
            method => anyhow::bail!("unsupported HTTP method: {method}"),
        };

        for (key, value) in &request.headers {
            builder = builder.header(key, value);
        }
        if let Some(body) = request.body {
            builder = builder.body(body);
        }

        let response = builder.send()?;
        let status_code = response.status().as_u16();
        Ok(wacore::net::StreamingHttpResponse {
            status_code,
            body: Box::new(response),
        })
    }
}

async fn disconnect_runtime_client(client: &Arc<Client>, context: &'static str) {
    match tokio::time::timeout(RUNTIME_SHUTDOWN_TIMEOUT, client.disconnect()).await {
        Ok(()) => {}
        Err(_) => {
            tracing::warn!(
                context,
                timeout_secs = RUNTIME_SHUTDOWN_TIMEOUT.as_secs(),
                "whatsapp client disconnect timed out"
            );
        }
    }
}

async fn wait_for_runtime_shutdown(
    bot_handle: &mut std::pin::Pin<&mut BotHandle>,
    context: &'static str,
) {
    match tokio::time::timeout(RUNTIME_SHUTDOWN_TIMEOUT, &mut *bot_handle).await {
        Ok(Ok(())) => {}
        Ok(Err(err)) => {
            tracing::debug!(context, err = %err, "whatsapp runtime task stopped");
        }
        Err(_) => {
            tracing::warn!(
                context,
                timeout_secs = RUNTIME_SHUTDOWN_TIMEOUT.as_secs(),
                "whatsapp runtime shutdown timed out; aborting"
            );
            bot_handle.as_ref().get_ref().abort();
            if let Err(err) = (&mut *bot_handle).await {
                tracing::debug!(context, err = %err, "whatsapp runtime task stopped after abort");
            }
        }
    }
}

async fn abort_runtime(bot_handle: &mut std::pin::Pin<&mut BotHandle>, context: &'static str) {
    bot_handle.as_ref().get_ref().abort();
    wait_for_runtime_shutdown(bot_handle, context).await;
}

fn logout_target_jid(pn: Option<Jid>, lid: Option<Jid>) -> Option<Jid> {
    pn.filter(|jid| jid.device != 0)
        .or_else(|| lid.filter(|jid| jid.device != 0))
}

async fn logout_active_session(client: &Client) -> anyhow::Result<()> {
    let Some(target_jid) = logout_target_jid(client.get_pn().await, client.get_lid().await) else {
        return Ok(());
    };

    let query = InfoQuery::set(
        "md",
        Jid::new("", SERVER_JID),
        Some(NodeContent::Nodes(vec![
            whatsapp_rust::NodeBuilder::new("remove-companion-device")
                .attrs([
                    ("jid", target_jid.to_string()),
                    ("reason", "user_initiated".to_string()),
                ])
                .build(),
        ])),
    )
    .with_timeout(std::time::Duration::from_secs(15));

    client
        .send_iq(query)
        .await
        .context("send whatsapp remove-companion-device IQ failed")?;
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

pub(super) async fn run_whatsapp_web_bridge(
    runtime: super::ChatBridgeRuntime,
    http_client: reqwest::Client,
    status_tx: watch::Sender<WhatsAppWebStatus>,
    mut control_rx: mpsc::UnboundedReceiver<WhatsAppBridgeCommand>,
) -> WhatsAppBridgeExit {
    let _ = status_tx.send(WhatsAppWebStatus::starting());

    let data_dir = runtime.data_dir();
    if let Err(err) = apply_pending_logout(&data_dir) {
        let _ = status_tx.send(WhatsAppWebStatus::error(err.to_string()));
        return WhatsAppBridgeExit::Retryable;
    }

    let base_dir = whatsapp_base_dir(&data_dir);
    if let Err(err) = tokio::fs::create_dir_all(&base_dir).await {
        let _ = status_tx.send(WhatsAppWebStatus::error(format!(
            "create whatsapp base dir failed: {err}"
        )));
        return WhatsAppBridgeExit::Retryable;
    }

    let http_client = match ReqwestHttpClient::new(http_client) {
        Ok(client) => client,
        Err(err) => {
            let _ = status_tx.send(WhatsAppWebStatus::error(err.to_string()));
            return WhatsAppBridgeExit::Retryable;
        }
    };

    let store_path = whatsapp_store_path(&data_dir);
    let store = match SqliteStore::new(store_path.to_string_lossy().as_ref()).await {
        Ok(store) => Arc::new(store),
        Err(err) => {
            let _ = status_tx.send(WhatsAppWebStatus::error(format!(
                "open whatsapp sqlite store failed: {err}"
            )));
            return WhatsAppBridgeExit::Retryable;
        }
    };

    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<Event>();
    let mut bot = match Bot::builder()
        .with_backend(store)
        .with_transport_factory(TokioWebSocketTransportFactory::new())
        .with_http_client(http_client)
        .with_runtime(TokioRuntime)
        .with_push_name("CliSwitch")
        .with_device_props(
            Some(current_os_name().to_string()),
            None,
            Some(wa::device_props::PlatformType::Desktop),
        )
        .skip_history_sync()
        .on_event(move |event, _client| {
            let event_tx = event_tx.clone();
            async move {
                let _ = event_tx.send(event);
            }
        })
        .build()
        .await
    {
        Ok(bot) => bot,
        Err(err) => {
            let _ = status_tx.send(WhatsAppWebStatus::error(format!(
                "build whatsapp runtime failed: {err}"
            )));
            return WhatsAppBridgeExit::Retryable;
        }
    };

    let client = bot.client();
    let adapter = Arc::new(WhatsAppWebAdapter::new(client.clone()));

    let bot_handle = match bot.run().await {
        Ok(handle) => handle,
        Err(err) => {
            let _ = status_tx.send(WhatsAppWebStatus::error(format!(
                "start whatsapp runtime failed: {err}"
            )));
            return WhatsAppBridgeExit::Retryable;
        }
    };
    tokio::pin!(bot_handle);
    let mut logout_requested = false;
    let mut runtime_handle_done = false;

    loop {
        tokio::select! {
            maybe_event = event_rx.recv() => {
                let Some(event) = maybe_event else {
                    break;
                };
                if let RuntimeEventAction::Stop(exit) =
                    handle_runtime_event(event, &runtime, &adapter, &client, &status_tx).await
                {
                    if matches!(exit, WhatsAppBridgeExit::AuthInvalid)
                        && let Err(err) = logout_by_clearing_auth_state(&data_dir)
                    {
                        tracing::warn!(err = %err, "mark whatsapp auth cleanup pending failed");
                    }
                    disconnect_runtime_client(&client, "terminal event").await;
                    abort_runtime(&mut bot_handle, "terminal event").await;
                    return exit;
                }
            }
            cmd = control_rx.recv() => {
                let Some(cmd) = cmd else {
                    continue;
                };
                match cmd {
                    WhatsAppBridgeCommand::Logout => {
                        logout_requested = true;
                        if let Err(err) = logout_active_session(&client).await {
                            tracing::warn!(err = %err, "whatsapp active logout failed");
                        }
                        disconnect_runtime_client(&client, "logout").await;
                        wait_for_runtime_shutdown(&mut bot_handle, "logout").await;
                        runtime_handle_done = true;
                        break;
                    }
                }
            }
            result = &mut bot_handle => {
                if let Err(err) = result {
                    tracing::warn!(err = %err, "whatsapp runtime task failed");
                }
                runtime_handle_done = true;
                break;
            }
        }
    }

    if logout_requested {
        if let Err(err) = clear_local_auth_state(&data_dir) {
            if let Err(mark_err) = logout_by_clearing_auth_state(&data_dir) {
                tracing::warn!(err = %mark_err, "mark whatsapp logout pending failed");
            }
            let _ = status_tx.send(WhatsAppWebStatus::error(err.to_string()));
        }
        return WhatsAppBridgeExit::LogoutCompleted;
    }

    if runtime_handle_done {
        disconnect_runtime_client(&client, "runtime ended").await;
    } else {
        disconnect_runtime_client(&client, "bridge loop stopped").await;
        wait_for_runtime_shutdown(&mut bot_handle, "bridge loop stopped").await;
    }

    let _ = status_tx.send(WhatsAppWebStatus::error("whatsapp bridge stopped"));
    WhatsAppBridgeExit::Retryable
}

async fn handle_runtime_event(
    event: Event,
    runtime: &super::ChatBridgeRuntime,
    adapter: &Arc<WhatsAppWebAdapter>,
    client: &Arc<Client>,
    status_tx: &watch::Sender<WhatsAppWebStatus>,
) -> RuntimeEventAction {
    match event {
        Event::PairingQrCode { code, .. } => {
            let _ = status_tx.send(WhatsAppWebStatus {
                state: WhatsAppWebState::AwaitingQr,
                connected: false,
                me: None,
                qr_image: qr_image_data_uri(&code).ok(),
                qr: Some(code),
                last_error: None,
            });
            RuntimeEventAction::Continue
        }
        Event::Connected(_) => {
            let _ = status_tx.send(WhatsAppWebStatus {
                state: WhatsAppWebState::Connected,
                connected: true,
                me: current_me(client).await,
                qr: None,
                qr_image: None,
                last_error: None,
            });
            RuntimeEventAction::Continue
        }
        Event::Disconnected(_) => {
            let mut current = status_tx.borrow().clone();
            // whatsapp-rust keeps its own reconnect loop; this only reflects transient state.
            current.state = WhatsAppWebState::Starting;
            current.connected = false;
            current.qr = None;
            current.qr_image = None;
            current.last_error = None;
            let _ = status_tx.send(current);
            RuntimeEventAction::Continue
        }
        Event::LoggedOut(info) => {
            let _ = status_tx.send(WhatsAppWebStatus::warn(format!(
                "logged out: {}",
                connect_failure_reason_label(info.reason)
            )));
            RuntimeEventAction::Stop(WhatsAppBridgeExit::AuthInvalid)
        }
        Event::ConnectFailure(failure) => {
            let _ = status_tx.send(WhatsAppWebStatus::warn(format!(
                "connect failure: {} ({})",
                failure.message,
                connect_failure_reason_label(failure.reason)
            )));
            if failure.reason.is_logged_out() {
                RuntimeEventAction::Stop(WhatsAppBridgeExit::AuthInvalid)
            } else if failure.reason.should_reconnect() {
                RuntimeEventAction::Continue
            } else {
                RuntimeEventAction::Stop(WhatsAppBridgeExit::Terminal)
            }
        }
        Event::ClientOutdated(_) => {
            let _ = status_tx.send(WhatsAppWebStatus::warn("client outdated"));
            RuntimeEventAction::Stop(WhatsAppBridgeExit::Terminal)
        }
        Event::StreamReplaced(_) => {
            let _ = status_tx.send(WhatsAppWebStatus::warn("stream replaced"));
            RuntimeEventAction::Stop(WhatsAppBridgeExit::Terminal)
        }
        Event::TemporaryBan(ban) => {
            let _ = status_tx.send(WhatsAppWebStatus::warn(format!(
                "temporary ban: {}",
                ban.code
            )));
            RuntimeEventAction::Stop(WhatsAppBridgeExit::Terminal)
        }
        Event::StreamError(err) => {
            let _ = status_tx.send(WhatsAppWebStatus::warn(format!(
                "stream error: {}",
                err.code
            )));
            RuntimeEventAction::Continue
        }
        Event::Message(message, info) => {
            if adapter.take_sent_message_id(&info.id).await {
                return RuntimeEventAction::Continue;
            }

            let self_chat = if info.source.is_from_me {
                is_self_chat(
                    &info,
                    client.get_pn().await.as_ref(),
                    client.get_lid().await.as_ref(),
                )
            } else {
                false
            };

            if info.source.is_from_me && !self_chat {
                tracing::debug!(
                    message_id = %info.id,
                    chat_id = %info.source.chat.to_non_ad(),
                    sender_id = %info.source.sender.to_non_ad(),
                    "ignore whatsapp from_me message outside self-chat"
                );
                return RuntimeEventAction::Continue;
            }

            if (info.source.chat.server == "broadcast" && info.source.chat.user == "status")
                || info.source.chat.server == "newsletter"
            {
                return RuntimeEventAction::Continue;
            }

            if self_chat {
                tracing::debug!(
                    message_id = %info.id,
                    chat_id = %info.source.chat.to_non_ad(),
                    sender_id = %info.source.sender.to_non_ad(),
                    "processing whatsapp self-chat message"
                );
            }

            adapter
                .cache_message(&info.id, message.as_ref(), &info)
                .await;

            let incoming = super::adapter::IncomingMessage {
                platform: crate::storage::ChatPlatform::WhatsApp,
                sender_id: info.source.sender.to_non_ad().to_string(),
                sender_display_name: display_name_from_info(&info),
                chat_id: info.source.chat.to_non_ad().to_string(),
                text: message.text_content().unwrap_or_default().to_string(),
                attachments: extract_incoming_attachments(client, &info, message.as_ref()).await,
                message_id: Some(info.id.clone()),
                timestamp_ms: info.timestamp.timestamp_millis(),
            };

            let runtime = runtime.clone();
            let sender: Arc<dyn ChatAdapter> = adapter.clone();
            tokio::spawn(async move {
                runtime.handle_message(sender, incoming).await;
            });
            RuntimeEventAction::Continue
        }
        _ => RuntimeEventAction::Continue,
    }
}

fn display_name_from_info(info: &MessageInfo) -> Option<String> {
    let name = info.push_name.trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

fn is_self_chat(info: &MessageInfo, own_pn: Option<&Jid>, own_lid: Option<&Jid>) -> bool {
    let chat = info.source.chat.to_non_ad();
    own_pn.is_some_and(|pn| chat == pn.to_non_ad())
        || own_lid.is_some_and(|lid| chat == lid.to_non_ad())
}

async fn current_me(client: &Client) -> Option<String> {
    if let Some(pn) = client.get_pn().await {
        return Some(pn.to_non_ad().to_string());
    }
    client
        .get_lid()
        .await
        .map(|jid| jid.to_non_ad().to_string())
}

fn connect_failure_reason_label(reason: ConnectFailureReason) -> String {
    match reason {
        ConnectFailureReason::Generic => "generic".to_string(),
        ConnectFailureReason::LoggedOut => "logged_out".to_string(),
        ConnectFailureReason::TempBanned => "temp_banned".to_string(),
        ConnectFailureReason::MainDeviceGone => "main_device_gone".to_string(),
        ConnectFailureReason::UnknownLogout => "unknown_logout".to_string(),
        ConnectFailureReason::ClientOutdated => "client_outdated".to_string(),
        ConnectFailureReason::BadUserAgent => "bad_user_agent".to_string(),
        ConnectFailureReason::CatExpired => "cat_expired".to_string(),
        ConnectFailureReason::CatInvalid => "cat_invalid".to_string(),
        ConnectFailureReason::NotFound => "not_found".to_string(),
        ConnectFailureReason::ClientUnknown => "client_unknown".to_string(),
        ConnectFailureReason::InternalServerError => "internal_server_error".to_string(),
        ConnectFailureReason::Experimental => "experimental".to_string(),
        ConnectFailureReason::ServiceUnavailable => "service_unavailable".to_string(),
        ConnectFailureReason::Unknown(code) => format!("unknown_{code}"),
    }
}

fn current_os_name() -> &'static str {
    match std::env::consts::OS {
        "macos" => "macOS",
        "windows" => "Windows",
        "linux" => "Linux",
        other => other,
    }
}

fn build_text_message(content: &str, context_info: Option<wa::ContextInfo>) -> wa::Message {
    match context_info {
        Some(context_info) => wa::Message {
            extended_text_message: Some(Box::new(wa::message::ExtendedTextMessage {
                text: Some(content.to_string()),
                context_info: Some(Box::new(context_info)),
                ..Default::default()
            })),
            ..Default::default()
        },
        None => wa::Message {
            conversation: Some(content.to_string()),
            ..Default::default()
        },
    }
}

fn ensure_attachment_size_within_limit(attachment: &Attachment) -> anyhow::Result<()> {
    let size = u64::try_from(attachment.data.len()).unwrap_or(u64::MAX);
    if size > MAX_ATTACHMENT_BYTES {
        anyhow::bail!(
            "whatsapp attachment too large: {} bytes > {} bytes",
            size,
            MAX_ATTACHMENT_BYTES
        );
    }
    Ok(())
}

macro_rules! uploaded_media_message {
    ($ty:path, $uploaded:expr $(, $field:ident = $value:expr )* $(,)?) => {{
        let uploaded = &$uploaded;
        $ty {
            url: Some(uploaded.url.clone()),
            direct_path: Some(uploaded.direct_path.clone()),
            media_key: Some(uploaded.media_key.clone()),
            file_sha256: Some(uploaded.file_sha256.clone()),
            file_enc_sha256: Some(uploaded.file_enc_sha256.clone()),
            file_length: Some(uploaded.file_length),
            mimetype: Some(uploaded.mimetype.clone()),
            context_info: uploaded.context_info.clone(),
            $($field: $value,)*
            ..Default::default()
        }
    }};
}

async fn build_attachment_message(
    client: &Client,
    attachment: &Attachment,
    context_info: Option<wa::ContextInfo>,
) -> anyhow::Result<wa::Message> {
    ensure_attachment_size_within_limit(attachment)?;

    let media_kind = outgoing_media_kind(&attachment.mime_type);
    let upload = client
        .upload(attachment.data.clone(), media_kind.media_type())
        .await
        .with_context(|| format!("upload whatsapp attachment failed: {}", attachment.filename))?;
    let uploaded = UploadedMediaFields::new(&upload, attachment, context_info);

    Ok(match media_kind {
        OutgoingMediaKind::Image => wa::Message {
            image_message: Some(Box::new(uploaded_media_message!(
                wa::message::ImageMessage,
                uploaded
            ))),
            ..Default::default()
        },
        OutgoingMediaKind::Video => wa::Message {
            video_message: Some(Box::new(uploaded_media_message!(
                wa::message::VideoMessage,
                uploaded
            ))),
            ..Default::default()
        },
        OutgoingMediaKind::Audio => wa::Message {
            audio_message: Some(Box::new(uploaded_media_message!(
                wa::message::AudioMessage,
                uploaded,
                ptt = Some(false)
            ))),
            ..Default::default()
        },
        OutgoingMediaKind::Document => wa::Message {
            document_message: Some(Box::new(uploaded_media_message!(
                wa::message::DocumentMessage,
                uploaded,
                file_name = Some(attachment.filename.clone())
            ))),
            ..Default::default()
        },
    })
}

struct UploadedMediaFields {
    url: String,
    direct_path: String,
    media_key: Vec<u8>,
    file_sha256: Vec<u8>,
    file_enc_sha256: Vec<u8>,
    file_length: u64,
    mimetype: String,
    context_info: Option<Box<wa::ContextInfo>>,
}

impl UploadedMediaFields {
    fn new(
        upload: &UploadResponse,
        attachment: &Attachment,
        context_info: Option<wa::ContextInfo>,
    ) -> Self {
        Self {
            url: upload.url.clone(),
            direct_path: upload.direct_path.clone(),
            media_key: upload.media_key.clone(),
            file_sha256: upload.file_sha256.clone(),
            file_enc_sha256: upload.file_enc_sha256.clone(),
            file_length: upload.file_length,
            mimetype: attachment.mime_type.clone(),
            context_info: context_info.map(Box::new),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutgoingMediaKind {
    Image,
    Video,
    Audio,
    Document,
}

impl OutgoingMediaKind {
    fn media_type(self) -> MediaType {
        match self {
            Self::Image => MediaType::Image,
            Self::Video => MediaType::Video,
            Self::Audio => MediaType::Audio,
            Self::Document => MediaType::Document,
        }
    }
}

fn outgoing_media_kind(mime_type: &str) -> OutgoingMediaKind {
    let mime = mime_type.to_ascii_lowercase();
    if mime.starts_with("image/") {
        return OutgoingMediaKind::Image;
    }
    if mime.starts_with("video/") {
        return OutgoingMediaKind::Video;
    }
    if mime.starts_with("audio/") {
        return OutgoingMediaKind::Audio;
    }
    OutgoingMediaKind::Document
}

async fn extract_incoming_attachments(
    client: &Client,
    info: &MessageInfo,
    message: &wa::Message,
) -> Vec<IncomingAttachment> {
    let base = message.get_base_message();
    let mut attachments = Vec::new();

    if let Some(image) = &base.image_message
        && let Some(att) = download_attachment(
            client,
            image.as_ref() as &dyn Downloadable,
            image.file_length,
            IncomingAttachmentKind::Image,
            image
                .mimetype
                .clone()
                .unwrap_or_else(|| "image/jpeg".to_string()),
            image.caption.clone(),
            format!(
                "image-{}.{}",
                info.id,
                infer_extension_from_mime(image.mimetype.as_deref()).unwrap_or("jpg")
            ),
        )
        .await
    {
        attachments.push(att);
    }

    if let Some(video) = &base.video_message
        && let Some(att) = download_attachment(
            client,
            video.as_ref() as &dyn Downloadable,
            video.file_length,
            IncomingAttachmentKind::File,
            video
                .mimetype
                .clone()
                .unwrap_or_else(|| "video/mp4".to_string()),
            video.caption.clone(),
            format!(
                "video-{}.{}",
                info.id,
                infer_extension_from_mime(video.mimetype.as_deref()).unwrap_or("mp4")
            ),
        )
        .await
    {
        attachments.push(att);
    }

    if let Some(audio) = &base.audio_message
        && let Some(att) = download_attachment(
            client,
            audio.as_ref() as &dyn Downloadable,
            audio.file_length,
            IncomingAttachmentKind::File,
            audio
                .mimetype
                .clone()
                .unwrap_or_else(|| "audio/mpeg".to_string()),
            None,
            format!(
                "audio-{}.{}",
                info.id,
                infer_extension_from_mime(audio.mimetype.as_deref()).unwrap_or("bin")
            ),
        )
        .await
    {
        attachments.push(att);
    }

    if let Some(document) = &base.document_message
        && let Some(att) = download_attachment(
            client,
            document.as_ref() as &dyn Downloadable,
            document.file_length,
            IncomingAttachmentKind::File,
            document
                .mimetype
                .clone()
                .unwrap_or_else(|| "application/octet-stream".to_string()),
            document.caption.clone(),
            document
                .file_name
                .clone()
                .unwrap_or_else(|| format!("file-{}", info.id)),
        )
        .await
    {
        attachments.push(att);
    }

    attachments
}

async fn download_attachment(
    client: &Client,
    downloadable: &dyn Downloadable,
    file_length: Option<u64>,
    kind: IncomingAttachmentKind,
    mime_type: String,
    caption: Option<String>,
    filename: String,
) -> Option<IncomingAttachment> {
    if file_length.is_some_and(|len| len > MAX_ATTACHMENT_BYTES) {
        tracing::warn!(
            filename = %filename,
            size = ?file_length,
            "skip whatsapp attachment larger than configured limit"
        );
        return None;
    }

    match client.download(downloadable).await {
        Ok(data) => Some(IncomingAttachment {
            kind,
            filename,
            mime_type: Some(mime_type),
            data: Arc::from(data.into_boxed_slice()),
            caption,
        }),
        Err(err) => {
            tracing::warn!(err = %err, "download whatsapp attachment failed");
            None
        }
    }
}

fn infer_extension_from_mime(mime: Option<&str>) -> Option<&'static str> {
    let mime = mime?;
    let mime = mime.to_ascii_lowercase();
    match mime.as_str() {
        "image/jpeg" | "image/jpg" => Some("jpg"),
        "image/png" => Some("png"),
        "image/webp" => Some("webp"),
        "image/gif" => Some("gif"),
        "application/pdf" => Some("pdf"),
        "audio/mpeg" | "audio/mp3" => Some("mp3"),
        "audio/ogg" | "audio/opus" => Some("ogg"),
        "video/mp4" => Some("mp4"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outgoing_media_kind_uses_mime_prefix() {
        assert_eq!(outgoing_media_kind("image/png"), OutgoingMediaKind::Image);
        assert_eq!(outgoing_media_kind("video/mp4"), OutgoingMediaKind::Video);
        assert_eq!(outgoing_media_kind("audio/mpeg"), OutgoingMediaKind::Audio);
        assert_eq!(
            outgoing_media_kind("application/pdf"),
            OutgoingMediaKind::Document
        );
    }

    #[test]
    fn infer_extension_from_mime_maps_known_types() {
        assert_eq!(infer_extension_from_mime(Some("image/jpeg")), Some("jpg"));
        assert_eq!(
            infer_extension_from_mime(Some("application/pdf")),
            Some("pdf")
        );
        assert_eq!(infer_extension_from_mime(Some("audio/ogg")), Some("ogg"));
        assert_eq!(infer_extension_from_mime(Some("unknown/type")), None);
    }

    #[test]
    fn infer_extension_from_mime_does_not_confuse_video_mpeg_with_audio() {
        assert_eq!(infer_extension_from_mime(Some("audio/mpeg")), Some("mp3"));
        assert_eq!(infer_extension_from_mime(Some("video/mpeg")), None);
    }

    #[test]
    fn recent_message_cache_reinserting_same_id_does_not_duplicate_order() {
        let mut cache = RecentMessageCache::default();
        let first = CachedMessage {
            info: MessageInfo::default(),
            message: Box::default(),
        };
        let second = CachedMessage {
            info: MessageInfo::default(),
            message: Box::default(),
        };

        cache.insert("abc".to_string(), first);
        cache.insert("abc".to_string(), second);

        assert_eq!(cache.order.len(), 1);
        assert_eq!(cache.order.front().map(String::as_str), Some("abc"));
        assert!(cache.entries.contains_key("abc"));
    }

    #[test]
    fn logout_target_jid_prefers_companion_device() {
        let primary = Jid::pn("15551234567").with_device(0);
        let companion = Jid::pn_device("15551234567", 33);

        assert_eq!(
            logout_target_jid(Some(primary.clone()), Some(companion.clone())),
            Some(companion.clone())
        );
        assert_eq!(logout_target_jid(Some(primary), None), None);
        assert_eq!(
            logout_target_jid(None, Some(companion.clone())),
            Some(companion)
        );
    }

    #[test]
    fn is_self_chat_matches_own_lid() {
        let own_lid: Jid = "100000000000001@lid".parse().expect("parse own lid");
        let own_pn: Jid = "15551234567@s.whatsapp.net".parse().expect("parse own pn");
        let mut info = MessageInfo::default();
        info.source.chat = own_lid.clone();

        assert!(is_self_chat(&info, Some(&own_pn), Some(&own_lid)));
    }

    #[test]
    fn is_self_chat_matches_own_phone_number() {
        let own_lid: Jid = "100000000000001@lid".parse().expect("parse own lid");
        let own_pn: Jid = "15551234567@s.whatsapp.net".parse().expect("parse own pn");
        let mut info = MessageInfo::default();
        info.source.chat = own_pn.clone();

        assert!(is_self_chat(&info, Some(&own_pn), Some(&own_lid)));
    }

    #[test]
    fn is_self_chat_rejects_other_direct_message() {
        let own_lid: Jid = "100000000000001@lid".parse().expect("parse own lid");
        let own_pn: Jid = "15551234567@s.whatsapp.net".parse().expect("parse own pn");
        let mut info = MessageInfo::default();
        info.source.chat = "16667778888@s.whatsapp.net"
            .parse()
            .expect("parse foreign chat");

        assert!(!is_self_chat(&info, Some(&own_pn), Some(&own_lid)));
    }
}
