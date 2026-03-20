use anyhow::Context as _;
use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use reqwest::StatusCode;
use reqwest::multipart::{Form, Part};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use tokio::net::TcpStream;
use tokio::time::Instant;
use tokio_tungstenite::MaybeTlsStream;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message as WsMessage;

use super::{
    ChatAdapter, IncomingAttachment, IncomingAttachmentKind, IncomingMessage, OutgoingMessage,
    SentMessage, StreamingMessage,
};
use crate::storage::ChatPlatform;

const DISCORD_API_BASE: &str = "https://discord.com/api/v10";
const DISCORD_FALLBACK_GATEWAY_URL: &str = "wss://gateway.discord.gg/";
const DISCORD_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const DISCORD_GATEWAY_INTENTS: u64 = (1 << 0) | (1 << 9) | (1 << 12) | (1 << 15);

type DiscordSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;

#[derive(Clone)]
pub struct DiscordAdapter {
    client: reqwest::Client,
    bot_token: String,
}

pub struct DiscordPoller {
    adapter: DiscordAdapter,
    ws: Option<DiscordSocket>,
    seq: Option<u64>,
    session_id: Option<String>,
    resume_gateway_url: Option<String>,
    heartbeat_interval: Duration,
    next_heartbeat_at: Option<Instant>,
    heartbeat_acked: bool,
}

impl DiscordAdapter {
    pub fn new(client: reqwest::Client, bot_token: String) -> Self {
        Self { client, bot_token }
    }

    pub fn poller(&self) -> DiscordPoller {
        DiscordPoller {
            adapter: self.clone(),
            ws: None,
            seq: None,
            session_id: None,
            resume_gateway_url: None,
            heartbeat_interval: Duration::from_secs(45),
            next_heartbeat_at: None,
            heartbeat_acked: true,
        }
    }

    fn auth_header_value(&self) -> String {
        format!("Bot {}", self.bot_token)
    }

    fn api_url(&self, path: &str) -> String {
        format!("{DISCORD_API_BASE}{path}")
    }

    async fn send_json<TReq, TResp>(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<&TReq>,
    ) -> anyhow::Result<TResp>
    where
        TReq: Serialize + ?Sized,
        TResp: for<'de> Deserialize<'de>,
    {
        let url = self.api_url(path);
        let mut request = self
            .client
            .request(method, &url)
            .timeout(DISCORD_REQUEST_TIMEOUT)
            .header(reqwest::header::AUTHORIZATION, self.auth_header_value());
        if let Some(body) = body {
            request = request.json(body);
        }
        let response = request
            .send()
            .await
            .with_context(|| format!("discord request failed: {path}"))?;
        let status = response.status();
        let bytes = response
            .bytes()
            .await
            .with_context(|| format!("discord response body read failed: {path}"))?;
        decode_discord_response(path, status, &bytes)
    }

    async fn send_empty(&self, method: reqwest::Method, path: &str) -> anyhow::Result<()> {
        let url = self.api_url(path);
        let response = self
            .client
            .request(method, &url)
            .timeout(DISCORD_REQUEST_TIMEOUT)
            .header(reqwest::header::AUTHORIZATION, self.auth_header_value())
            .send()
            .await
            .with_context(|| format!("discord request failed: {path}"))?;
        let status = response.status();
        let bytes = response
            .bytes()
            .await
            .with_context(|| format!("discord response body read failed: {path}"))?;
        if status.is_success() {
            return Ok(());
        }
        Err(discord_http_error(path, status, &bytes))
    }

    async fn send_multipart<TResp>(&self, path: &str, form: Form) -> anyhow::Result<TResp>
    where
        TResp: for<'de> Deserialize<'de>,
    {
        let url = self.api_url(path);
        let response = self
            .client
            .post(&url)
            .timeout(DISCORD_REQUEST_TIMEOUT)
            .header(reqwest::header::AUTHORIZATION, self.auth_header_value())
            .multipart(form)
            .send()
            .await
            .with_context(|| format!("discord multipart request failed: {path}"))?;
        let status = response.status();
        let bytes = response
            .bytes()
            .await
            .with_context(|| format!("discord response body read failed: {path}"))?;
        decode_discord_response(path, status, &bytes)
    }

    async fn fetch_gateway_url(&self) -> anyhow::Result<String> {
        #[derive(Debug, Deserialize)]
        struct GatewayBotInfo {
            url: String,
        }

        let info: GatewayBotInfo = self
            .send_json::<(), _>(reqwest::Method::GET, "/gateway/bot", None)
            .await?;
        Ok(info.url)
    }

    async fn create_message_with_attachments(
        &self,
        path: &str,
        msg: &OutgoingMessage,
    ) -> anyhow::Result<SentMessage> {
        let payload = DiscordCreateMessagePayload {
            content: &msg.content,
            allowed_mentions: DiscordAllowedMentions::none(),
            message_reference: DiscordMessageReference::from_reply_to(msg.reply_to.as_deref()),
            attachments: msg
                .attachments
                .iter()
                .enumerate()
                .map(|(index, attachment)| DiscordAttachmentMeta {
                    id: index,
                    filename: &attachment.filename,
                })
                .collect(),
        };

        let mut form = Form::new().text(
            "payload_json",
            serde_json::to_string(&payload).context("serialize discord payload_json failed")?,
        );
        for (index, attachment) in msg.attachments.iter().enumerate() {
            let part = Part::bytes(attachment.data.clone())
                .file_name(attachment.filename.clone())
                .mime_str(&attachment.mime_type)
                .with_context(|| {
                    format!(
                        "discord attachment mime type is invalid: {}",
                        attachment.mime_type
                    )
                })?;
            form = form.part(format!("files[{index}]"), part);
        }

        let sent: DiscordMessageResponse = self.send_multipart(path, form).await?;
        Ok(SentMessage {
            message_id: sent.id,
        })
    }

    async fn download_attachment(&self, url: &str, name: &str) -> anyhow::Result<Arc<[u8]>> {
        let response = self
            .client
            .get(url)
            .timeout(DISCORD_REQUEST_TIMEOUT)
            .send()
            .await
            .with_context(|| format!("discord attachment download failed: {name}"))?;
        let status = response.status();
        let bytes = response
            .bytes()
            .await
            .with_context(|| format!("discord attachment body read failed: {name}"))?;
        if !status.is_success() {
            return Err(discord_http_error("attachment download", status, &bytes));
        }
        Ok(Arc::<[u8]>::from(bytes.to_vec()))
    }
}

impl DiscordPoller {
    pub async fn prepare_for_polling(&mut self) -> anyhow::Result<()> {
        self.connect(false).await
    }

    pub async fn reconnect(&mut self) -> anyhow::Result<()> {
        self.disconnect();
        self.connect(true).await
    }

    pub fn disconnect(&mut self) {
        self.ws = None;
        self.next_heartbeat_at = None;
        self.heartbeat_acked = true;
    }

    pub async fn poll_updates(&mut self) -> anyhow::Result<Vec<IncomingMessage>> {
        loop {
            if self.ws.is_none() {
                self.connect(true).await?;
            }

            let deadline = self
                .next_heartbeat_at
                .unwrap_or_else(|| Instant::now() + self.heartbeat_interval);
            let next = {
                let ws = self
                    .ws
                    .as_mut()
                    .ok_or_else(|| anyhow::anyhow!("discord gateway is not connected"))?;
                tokio::time::timeout_at(deadline, ws.next()).await
            };

            match next {
                Err(_) => {
                    self.send_heartbeat().await?;
                }
                Ok(Some(Ok(message))) => {
                    if let Some(incoming) = self.handle_socket_message(message).await? {
                        return Ok(vec![incoming]);
                    }
                }
                Ok(Some(Err(err))) => {
                    self.disconnect();
                    return Err(err).context("discord gateway read failed");
                }
                Ok(None) => {
                    self.disconnect();
                    anyhow::bail!("discord gateway closed the websocket")
                }
            }
        }
    }

    async fn connect(&mut self, use_resume: bool) -> anyhow::Result<()> {
        let base_url = if use_resume {
            self.resume_gateway_url
                .clone()
                .or(Some(DISCORD_FALLBACK_GATEWAY_URL.to_string()))
        } else {
            None
        }
        .unwrap_or(self.adapter.fetch_gateway_url().await.unwrap_or_else(|err| {
            tracing::warn!(err = %err, "discord gateway bot url fetch failed; using fallback url");
            DISCORD_FALLBACK_GATEWAY_URL.to_string()
        }));

        let connect_url = discord_gateway_connect_url(&base_url);
        let (mut ws, _) = connect_async(&connect_url)
            .await
            .with_context(|| format!("connect discord gateway failed: {connect_url}"))?;
        let hello = match ws.next().await {
            Some(Ok(message)) => parse_discord_hello(message)?,
            Some(Err(err)) => return Err(err).context("read discord gateway hello failed"),
            None => anyhow::bail!("discord gateway closed before hello"),
        };

        self.heartbeat_interval = Duration::from_millis(hello.heartbeat_interval);
        self.next_heartbeat_at = Some(Instant::now() + self.heartbeat_interval);
        self.heartbeat_acked = true;

        let should_resume = use_resume && self.session_id.is_some() && self.seq.is_some();
        if should_resume {
            #[derive(Serialize)]
            struct ResumePayload<'a> {
                token: &'a str,
                session_id: &'a str,
                seq: u64,
            }

            self.send_gateway_message_with_socket(
                &mut ws,
                &GatewaySend {
                    op: 6,
                    d: serde_json::to_value(ResumePayload {
                        token: &self.adapter.bot_token,
                        session_id: self
                            .session_id
                            .as_deref()
                            .ok_or_else(|| anyhow::anyhow!("missing discord session_id"))?,
                        seq: self
                            .seq
                            .ok_or_else(|| anyhow::anyhow!("missing discord sequence"))?,
                    })
                    .context("serialize discord resume payload failed")?,
                },
            )
            .await?;
        } else {
            #[derive(Serialize)]
            struct IdentifyProperties {
                os: &'static str,
                browser: &'static str,
                device: &'static str,
            }

            #[derive(Serialize)]
            struct IdentifyPayload<'a> {
                token: &'a str,
                intents: u64,
                properties: IdentifyProperties,
            }

            self.send_gateway_message_with_socket(
                &mut ws,
                &GatewaySend {
                    op: 2,
                    d: serde_json::to_value(IdentifyPayload {
                        token: &self.adapter.bot_token,
                        intents: DISCORD_GATEWAY_INTENTS,
                        properties: IdentifyProperties {
                            os: std::env::consts::OS,
                            browser: "cliswitch",
                            device: "cliswitch",
                        },
                    })
                    .context("serialize discord identify payload failed")?,
                },
            )
            .await?;
        }

        self.ws = Some(ws);
        Ok(())
    }

    async fn send_gateway_message_with_socket(
        &self,
        ws: &mut DiscordSocket,
        payload: &GatewaySend,
    ) -> anyhow::Result<()> {
        let text =
            serde_json::to_string(payload).context("serialize discord gateway payload failed")?;
        ws.send(WsMessage::Text(text))
            .await
            .context("send discord gateway payload failed")
    }

    async fn send_gateway_message(&mut self, payload: &GatewaySend) -> anyhow::Result<()> {
        let text =
            serde_json::to_string(payload).context("serialize discord gateway payload failed")?;
        let ws = self
            .ws
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("discord gateway is not connected"))?;
        ws.send(WsMessage::Text(text))
            .await
            .context("send discord gateway payload failed")
    }

    async fn send_heartbeat(&mut self) -> anyhow::Result<()> {
        if !self.heartbeat_acked {
            self.disconnect();
            anyhow::bail!("discord gateway heartbeat was not acknowledged")
        }

        self.send_gateway_message(&GatewaySend {
            op: 1,
            d: self.seq.map(Value::from).unwrap_or(Value::Null),
        })
        .await?;
        self.heartbeat_acked = false;
        self.next_heartbeat_at = Some(Instant::now() + self.heartbeat_interval);
        Ok(())
    }

    async fn handle_socket_message(
        &mut self,
        message: WsMessage,
    ) -> anyhow::Result<Option<IncomingMessage>> {
        match message {
            WsMessage::Text(text) => self.handle_gateway_text(text.to_string()).await,
            WsMessage::Binary(bytes) => {
                self.handle_gateway_text(String::from_utf8_lossy(&bytes).into_owned())
                    .await
            }
            WsMessage::Ping(payload) => {
                let ws = self
                    .ws
                    .as_mut()
                    .ok_or_else(|| anyhow::anyhow!("discord gateway is not connected"))?;
                ws.send(WsMessage::Pong(payload))
                    .await
                    .context("send discord gateway pong failed")?;
                Ok(None)
            }
            WsMessage::Pong(_) => Ok(None),
            WsMessage::Close(frame) => {
                self.disconnect();
                anyhow::bail!(
                    "discord gateway closed: {}",
                    frame
                        .as_ref()
                        .map(|item| item.reason.to_string())
                        .filter(|item| !item.is_empty())
                        .unwrap_or_else(|| "no reason".to_string())
                )
            }
            _ => Ok(None),
        }
    }

    async fn handle_gateway_text(
        &mut self,
        text: String,
    ) -> anyhow::Result<Option<IncomingMessage>> {
        let packet: GatewayReceive =
            serde_json::from_str(&text).context("decode discord gateway packet failed")?;
        if let Some(seq) = packet.s {
            self.seq = Some(seq);
        }

        match packet.op {
            0 => self.handle_dispatch(packet).await,
            1 => {
                self.send_heartbeat().await?;
                Ok(None)
            }
            7 => {
                self.disconnect();
                anyhow::bail!("discord gateway requested reconnect")
            }
            9 => {
                let resumable = packet.d.as_bool().unwrap_or(false);
                if !resumable {
                    self.seq = None;
                    self.session_id = None;
                    self.resume_gateway_url = None;
                }
                self.disconnect();
                anyhow::bail!("discord gateway session is invalid")
            }
            10 => {
                let hello: GatewayHello = serde_json::from_value(packet.d)
                    .context("decode discord hello payload failed")?;
                self.heartbeat_interval = Duration::from_millis(hello.heartbeat_interval);
                self.next_heartbeat_at = Some(Instant::now() + self.heartbeat_interval);
                self.heartbeat_acked = true;
                Ok(None)
            }
            11 => {
                self.heartbeat_acked = true;
                Ok(None)
            }
            _ => Ok(None),
        }
    }

    async fn handle_dispatch(
        &mut self,
        packet: GatewayReceive,
    ) -> anyhow::Result<Option<IncomingMessage>> {
        match packet.t.as_deref() {
            Some("READY") => {
                let ready: GatewayReady = serde_json::from_value(packet.d)
                    .context("decode discord READY event failed")?;
                self.session_id = Some(ready.session_id);
                self.resume_gateway_url = Some(ready.resume_gateway_url);
                Ok(None)
            }
            Some("RESUMED") => Ok(None),
            Some("MESSAGE_CREATE") => {
                let event: DiscordMessageCreate = serde_json::from_value(packet.d)
                    .context("decode discord MESSAGE_CREATE failed")?;
                incoming_message_from_event(&self.adapter, event).await
            }
            _ => Ok(None),
        }
    }
}

#[async_trait]
impl ChatAdapter for DiscordAdapter {
    async fn send_message(&self, msg: OutgoingMessage) -> anyhow::Result<SentMessage> {
        let path = format!("/channels/{}/messages", msg.chat_id);
        if !msg.attachments.is_empty() {
            return self.create_message_with_attachments(&path, &msg).await;
        }

        let sent: DiscordMessageResponse = self
            .send_json(
                reqwest::Method::POST,
                &path,
                Some(&DiscordCreateMessageRequest {
                    content: &msg.content,
                    allowed_mentions: DiscordAllowedMentions::none(),
                    message_reference: DiscordMessageReference::from_reply_to(
                        msg.reply_to.as_deref(),
                    ),
                }),
            )
            .await?;

        Ok(SentMessage {
            message_id: sent.id,
        })
    }

    async fn edit_message(
        &self,
        chat_id: &str,
        message_id: &str,
        content: &str,
    ) -> anyhow::Result<()> {
        let path = format!("/channels/{chat_id}/messages/{message_id}");
        let _: DiscordMessageResponse = self
            .send_json(
                reqwest::Method::PATCH,
                &path,
                Some(&DiscordEditMessageRequest {
                    content,
                    allowed_mentions: DiscordAllowedMentions::none(),
                }),
            )
            .await?;
        Ok(())
    }

    async fn delete_message(&self, chat_id: &str, message_id: &str) -> anyhow::Result<()> {
        self.send_empty(
            reqwest::Method::DELETE,
            &format!("/channels/{chat_id}/messages/{message_id}"),
        )
        .await
    }

    async fn begin_streaming_message(
        &self,
        _msg: OutgoingMessage,
    ) -> anyhow::Result<Option<StreamingMessage>> {
        Ok(None)
    }

    async fn send_typing(&self, chat_id: &str) -> anyhow::Result<()> {
        self.send_empty(
            reqwest::Method::POST,
            &format!("/channels/{chat_id}/typing"),
        )
        .await
    }

    fn platform(&self) -> ChatPlatform {
        ChatPlatform::Discord
    }
}

#[derive(Debug, Serialize)]
struct GatewaySend {
    op: u8,
    d: Value,
}

#[derive(Debug, Deserialize)]
struct GatewayReceive {
    op: u8,
    d: Value,
    s: Option<u64>,
    t: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GatewayHello {
    heartbeat_interval: u64,
}

#[derive(Debug, Deserialize)]
struct GatewayReady {
    session_id: String,
    resume_gateway_url: String,
}

#[derive(Debug, Deserialize)]
struct DiscordMessageResponse {
    id: String,
}

#[derive(Debug, Serialize)]
struct DiscordAllowedMentions {
    parse: Vec<&'static str>,
    replied_user: bool,
}

impl DiscordAllowedMentions {
    fn none() -> Self {
        Self {
            parse: Vec::new(),
            replied_user: false,
        }
    }
}

#[derive(Debug, Serialize)]
struct DiscordMessageReference<'a> {
    message_id: &'a str,
    fail_if_not_exists: bool,
}

impl<'a> DiscordMessageReference<'a> {
    fn from_reply_to(reply_to: Option<&'a str>) -> Option<Self> {
        reply_to.map(|message_id| Self {
            message_id,
            fail_if_not_exists: false,
        })
    }
}

#[derive(Debug, Serialize)]
struct DiscordAttachmentMeta<'a> {
    id: usize,
    filename: &'a str,
}

#[derive(Debug, Serialize)]
struct DiscordCreateMessagePayload<'a> {
    content: &'a str,
    allowed_mentions: DiscordAllowedMentions,
    #[serde(skip_serializing_if = "Option::is_none")]
    message_reference: Option<DiscordMessageReference<'a>>,
    attachments: Vec<DiscordAttachmentMeta<'a>>,
}

#[derive(Debug, Serialize)]
struct DiscordCreateMessageRequest<'a> {
    content: &'a str,
    allowed_mentions: DiscordAllowedMentions,
    #[serde(skip_serializing_if = "Option::is_none")]
    message_reference: Option<DiscordMessageReference<'a>>,
}

#[derive(Debug, Serialize)]
struct DiscordEditMessageRequest<'a> {
    content: &'a str,
    allowed_mentions: DiscordAllowedMentions,
}

#[derive(Debug, Deserialize)]
struct DiscordMessageCreate {
    id: String,
    channel_id: String,
    #[serde(default)]
    content: String,
    timestamp: String,
    author: DiscordAuthor,
    #[serde(default)]
    attachments: Vec<DiscordAttachmentPayload>,
    member: Option<DiscordMember>,
}

#[derive(Debug, Deserialize)]
struct DiscordAuthor {
    id: String,
    username: String,
    global_name: Option<String>,
    bot: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct DiscordMember {
    nick: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DiscordAttachmentPayload {
    filename: String,
    url: String,
    content_type: Option<String>,
}

async fn incoming_message_from_event(
    adapter: &DiscordAdapter,
    event: DiscordMessageCreate,
) -> anyhow::Result<Option<IncomingMessage>> {
    if event.author.bot.unwrap_or(false) {
        return Ok(None);
    }

    let mut attachments = Vec::new();
    for attachment in event.attachments {
        match adapter
            .download_attachment(&attachment.url, &attachment.filename)
            .await
        {
            Ok(data) => attachments.push(IncomingAttachment {
                kind: classify_incoming_attachment(
                    attachment.content_type.as_deref(),
                    &attachment.filename,
                ),
                filename: attachment.filename,
                mime_type: attachment.content_type,
                data,
                caption: None,
            }),
            Err(err) => {
                tracing::warn!(
                    filename = %attachment.filename,
                    err = %err,
                    "download discord incoming attachment failed; skipping attachment"
                );
            }
        }
    }

    let text = event.content.trim().to_string();
    if text.is_empty() && attachments.is_empty() {
        return Ok(None);
    }

    Ok(Some(IncomingMessage {
        platform: ChatPlatform::Discord,
        sender_id: event.author.id,
        sender_display_name: event
            .member
            .and_then(|member| member.nick)
            .or(event.author.global_name)
            .or(Some(event.author.username)),
        chat_id: event.channel_id,
        text,
        attachments,
        message_id: Some(event.id),
        timestamp_ms: parse_discord_timestamp_ms(&event.timestamp),
    }))
}

fn parse_discord_hello(message: WsMessage) -> anyhow::Result<GatewayHello> {
    let text = match message {
        WsMessage::Text(text) => text.to_string(),
        WsMessage::Binary(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
        other => anyhow::bail!("unexpected discord gateway hello frame: {other:?}"),
    };
    let packet: GatewayReceive =
        serde_json::from_str(&text).context("decode discord gateway hello packet failed")?;
    if packet.op != 10 {
        anyhow::bail!(
            "expected discord gateway hello opcode 10, got {}",
            packet.op
        );
    }
    serde_json::from_value(packet.d).context("decode discord gateway hello payload failed")
}

fn classify_incoming_attachment(
    content_type: Option<&str>,
    filename: &str,
) -> IncomingAttachmentKind {
    if content_type
        .map(|value| value.starts_with("image/"))
        .unwrap_or(false)
    {
        return IncomingAttachmentKind::Image;
    }

    let guessed = mime_guess::from_path(filename).first_raw().unwrap_or("");
    if guessed.starts_with("image/") {
        IncomingAttachmentKind::Image
    } else {
        IncomingAttachmentKind::File
    }
}

fn parse_discord_timestamp_ms(raw: &str) -> i64 {
    OffsetDateTime::parse(raw, &Rfc3339)
        .ok()
        .and_then(|value| {
            let millis = value.unix_timestamp_nanos() / 1_000_000;
            i64::try_from(millis).ok()
        })
        .unwrap_or_else(crate::storage::now_ms)
}

fn discord_gateway_connect_url(base: &str) -> String {
    let trimmed = base.trim_end_matches('/');
    if trimmed.contains('?') {
        format!("{trimmed}&v=10&encoding=json")
    } else {
        format!("{trimmed}/?v=10&encoding=json")
    }
}

fn decode_discord_response<T>(path: &str, status: StatusCode, bytes: &[u8]) -> anyhow::Result<T>
where
    T: for<'de> Deserialize<'de>,
{
    if !status.is_success() {
        return Err(discord_http_error(path, status, bytes));
    }
    serde_json::from_slice(bytes).with_context(|| format!("decode discord response failed: {path}"))
}

fn discord_http_error(path: &str, status: StatusCode, bytes: &[u8]) -> anyhow::Error {
    let preview = String::from_utf8_lossy(bytes);
    anyhow::anyhow!(
        "discord request failed: {} {} {}",
        path,
        status,
        preview.trim()
    )
}
