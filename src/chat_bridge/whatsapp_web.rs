use anyhow::Context as _;
use base64::Engine as _;
use image::{ImageBuffer, Luma};
use qrcode::QrCode;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc, watch};

use super::adapter::{
    Attachment, ChatAdapter, IncomingAttachment, IncomingAttachmentKind, OutgoingMessage,
    SentMessage,
};
use whatsapp_rust::bot::Bot;
use whatsapp_rust::download::{Downloadable, MediaType};
use whatsapp_rust::proto_helpers::{self, MessageExt as _};
use whatsapp_rust::store::SqliteStore;
use whatsapp_rust::types::events::{ConnectFailureReason, Event};
use whatsapp_rust::types::message::MessageInfo;
use whatsapp_rust::{Client, Jid, TokioRuntime, waproto::whatsapp as wa};
use whatsapp_rust_tokio_transport::TokioWebSocketTransportFactory;
use whatsapp_rust_ureq_http_client::UreqHttpClient;

const STORE_FILE_NAME: &str = "session.sqlite3";
const LOGOUT_MARKER_FILE_NAME: &str = "pending-logout";
const MAX_ATTACHMENT_BYTES: u64 = 20 * 1024 * 1024;
const RECENT_LIMIT: usize = 256;

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

#[derive(Clone)]
pub(crate) struct WhatsAppWebAdapter {
    client: Arc<Client>,
    recent_messages: Arc<Mutex<RecentMessageCache>>,
}

impl WhatsAppWebAdapter {
    fn new(client: Arc<Client>) -> Self {
        Self {
            client,
            recent_messages: Arc::new(Mutex::new(RecentMessageCache::default())),
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

    let base_dir = whatsapp_base_dir(data_dir);
    if base_dir.exists() {
        std::fs::remove_dir_all(&base_dir)
            .with_context(|| format!("remove whatsapp base dir failed: {}", base_dir.display()))?;
    }
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
    _http_client: reqwest::Client,
    status_tx: watch::Sender<WhatsAppWebStatus>,
) {
    let _ = status_tx.send(WhatsAppWebStatus::starting());

    let data_dir = runtime.data_dir();
    if let Err(err) = apply_pending_logout(&data_dir) {
        let _ = status_tx.send(WhatsAppWebStatus::error(err.to_string()));
        return;
    }

    let base_dir = whatsapp_base_dir(&data_dir);
    if let Err(err) = tokio::fs::create_dir_all(&base_dir).await {
        let _ = status_tx.send(WhatsAppWebStatus::error(format!(
            "create whatsapp base dir failed: {err}"
        )));
        return;
    }

    let store_path = whatsapp_store_path(&data_dir);
    let store = match SqliteStore::new(store_path.to_string_lossy().as_ref()).await {
        Ok(store) => Arc::new(store),
        Err(err) => {
            let _ = status_tx.send(WhatsAppWebStatus::error(format!(
                "open whatsapp sqlite store failed: {err}"
            )));
            return;
        }
    };

    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<Event>();
    let mut bot = match Bot::builder()
        .with_backend(store)
        .with_transport_factory(TokioWebSocketTransportFactory::new())
        .with_http_client(UreqHttpClient::new())
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
            return;
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
            return;
        }
    };
    tokio::pin!(bot_handle);

    loop {
        tokio::select! {
            maybe_event = event_rx.recv() => {
                let Some(event) = maybe_event else {
                    break;
                };
                handle_runtime_event(event, &runtime, &adapter, &client, &status_tx).await;
            }
            result = &mut bot_handle => {
                if let Err(err) = result {
                    tracing::warn!(err = %err, "whatsapp runtime task failed");
                }
                break;
            }
        }
    }

    let _ = status_tx.send(WhatsAppWebStatus::error("whatsapp bridge stopped"));
}

async fn handle_runtime_event(
    event: Event,
    runtime: &super::ChatBridgeRuntime,
    adapter: &Arc<WhatsAppWebAdapter>,
    client: &Arc<Client>,
    status_tx: &watch::Sender<WhatsAppWebStatus>,
) {
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
        }
        Event::Disconnected(_) => {
            let mut current = status_tx.borrow().clone();
            current.state = WhatsAppWebState::Starting;
            current.connected = false;
            current.qr = None;
            current.qr_image = None;
            let _ = status_tx.send(current);
        }
        Event::LoggedOut(info) => {
            let _ = status_tx.send(WhatsAppWebStatus::error(format!(
                "logged out: {}",
                connect_failure_reason_label(info.reason)
            )));
        }
        Event::ConnectFailure(failure) => {
            let _ = status_tx.send(WhatsAppWebStatus::error(format!(
                "connect failure: {} ({})",
                failure.message,
                connect_failure_reason_label(failure.reason)
            )));
        }
        Event::ClientOutdated(_) => {
            let _ = status_tx.send(WhatsAppWebStatus::error("client outdated"));
        }
        Event::StreamError(err) => {
            let _ = status_tx.send(WhatsAppWebStatus::error(format!(
                "stream error: {}",
                err.code
            )));
        }
        Event::Message(message, info) => {
            if info.source.is_from_me
                || (info.source.chat.server == "broadcast" && info.source.chat.user == "status")
                || info.source.chat.server == "newsletter"
            {
                return;
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
        }
        _ => {}
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

async fn build_attachment_message(
    client: &Client,
    attachment: &Attachment,
    context_info: Option<wa::ContextInfo>,
) -> anyhow::Result<wa::Message> {
    let media_kind = outgoing_media_kind(&attachment.mime_type);
    let upload = client
        .upload(attachment.data.clone(), media_kind.media_type())
        .await
        .with_context(|| format!("upload whatsapp attachment failed: {}", attachment.filename))?;

    Ok(match media_kind {
        OutgoingMediaKind::Image => wa::Message {
            image_message: Some(Box::new(wa::message::ImageMessage {
                url: Some(upload.url.clone()),
                direct_path: Some(upload.direct_path.clone()),
                media_key: Some(upload.media_key.clone()),
                file_sha256: Some(upload.file_sha256.clone()),
                file_enc_sha256: Some(upload.file_enc_sha256.clone()),
                file_length: Some(upload.file_length),
                mimetype: Some(attachment.mime_type.clone()),
                context_info: context_info.map(Box::new),
                ..Default::default()
            })),
            ..Default::default()
        },
        OutgoingMediaKind::Video => wa::Message {
            video_message: Some(Box::new(wa::message::VideoMessage {
                url: Some(upload.url.clone()),
                direct_path: Some(upload.direct_path.clone()),
                media_key: Some(upload.media_key.clone()),
                file_sha256: Some(upload.file_sha256.clone()),
                file_enc_sha256: Some(upload.file_enc_sha256.clone()),
                file_length: Some(upload.file_length),
                mimetype: Some(attachment.mime_type.clone()),
                context_info: context_info.map(Box::new),
                ..Default::default()
            })),
            ..Default::default()
        },
        OutgoingMediaKind::Audio => wa::Message {
            audio_message: Some(Box::new(wa::message::AudioMessage {
                url: Some(upload.url.clone()),
                direct_path: Some(upload.direct_path.clone()),
                media_key: Some(upload.media_key.clone()),
                file_sha256: Some(upload.file_sha256.clone()),
                file_enc_sha256: Some(upload.file_enc_sha256.clone()),
                file_length: Some(upload.file_length),
                mimetype: Some(attachment.mime_type.clone()),
                ptt: Some(false),
                context_info: context_info.map(Box::new),
                ..Default::default()
            })),
            ..Default::default()
        },
        OutgoingMediaKind::Document => wa::Message {
            document_message: Some(Box::new(wa::message::DocumentMessage {
                url: Some(upload.url.clone()),
                direct_path: Some(upload.direct_path.clone()),
                media_key: Some(upload.media_key.clone()),
                file_sha256: Some(upload.file_sha256.clone()),
                file_enc_sha256: Some(upload.file_enc_sha256.clone()),
                file_length: Some(upload.file_length),
                mimetype: Some(attachment.mime_type.clone()),
                file_name: Some(attachment.filename.clone()),
                context_info: context_info.map(Box::new),
                ..Default::default()
            })),
            ..Default::default()
        },
    })
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
    if mime.contains("jpeg") {
        return Some("jpg");
    }
    if mime.contains("png") {
        return Some("png");
    }
    if mime.contains("webp") {
        return Some("webp");
    }
    if mime.contains("gif") {
        return Some("gif");
    }
    if mime.contains("pdf") {
        return Some("pdf");
    }
    if mime.contains("mpeg") {
        return Some("mp3");
    }
    if mime.contains("ogg") {
        return Some("ogg");
    }
    if mime.contains("mp4") {
        return Some("mp4");
    }
    None
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
}
