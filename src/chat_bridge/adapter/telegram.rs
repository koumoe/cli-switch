use anyhow::Context as _;
use async_trait::async_trait;
use bytes::Bytes;
use reqwest::StatusCode;
use reqwest::multipart::{Form, Part};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;

use super::{
    Attachment, ChatAdapter, IncomingAttachment, IncomingAttachmentKind, IncomingMessage,
    OutgoingMessage, ParseMode, SentMessage,
};
use crate::chat_bridge::output::render_chat_message;
use crate::storage::ChatPlatform;

const TELEGRAM_API_BASE: &str = "https://api.telegram.org";
const TELEGRAM_LONG_POLL_TIMEOUT_SECS: i64 = 30;
const TELEGRAM_RECENT_UPDATE_GRACE_MS: i64 = 15_000;
const TELEGRAM_REQUEST_TIMEOUT: Duration = Duration::from_secs(6);
const TELEGRAM_LONG_POLL_REQUEST_TIMEOUT: Duration =
    Duration::from_secs(TELEGRAM_LONG_POLL_TIMEOUT_SECS as u64 + 10);
const TELEGRAM_REQUEST_MAX_ATTEMPTS: usize = 3;
const TELEGRAM_RETRY_DELAY_BASE: Duration = Duration::from_millis(250);
const TELEGRAM_RETRY_DELAY_MAX: Duration = Duration::from_secs(2);

#[derive(Clone)]
pub struct TelegramAdapter {
    client: reqwest::Client,
    bot_token: String,
}

pub struct TelegramPoller {
    adapter: TelegramAdapter,
    next_offset: i64,
}

impl TelegramAdapter {
    pub fn new(client: reqwest::Client, bot_token: String) -> Self {
        Self { client, bot_token }
    }

    pub fn poller(&self) -> TelegramPoller {
        TelegramPoller {
            adapter: self.clone(),
            next_offset: 0,
        }
    }

    fn method_url(&self, method: &str) -> String {
        format!("{TELEGRAM_API_BASE}/bot{}/{method}", self.bot_token)
    }

    async fn post_json<TReq, TResp>(
        &self,
        method: &'static str,
        body: &TReq,
        timeout: Duration,
    ) -> anyhow::Result<TResp>
    where
        TReq: Serialize + ?Sized,
        TResp: DeserializeOwned,
    {
        let url = self.method_url(method);

        for attempt in 1..=TELEGRAM_REQUEST_MAX_ATTEMPTS {
            let response = match self
                .client
                .post(&url)
                .timeout(timeout)
                .json(body)
                .send()
                .await
            {
                Ok(response) => response,
                Err(err)
                    if attempt < TELEGRAM_REQUEST_MAX_ATTEMPTS
                        && is_retryable_transport_error(&err) =>
                {
                    tracing::warn!(
                        method,
                        attempt,
                        err = %err,
                        "telegram request transport failed; retrying"
                    );
                    tokio::time::sleep(request_retry_backoff_delay(attempt)).await;
                    continue;
                }
                Err(err) => {
                    return Err(err).context(format!("telegram {method} request failed"));
                }
            };

            let status = response.status();
            let bytes = response
                .bytes()
                .await
                .with_context(|| format!("telegram {method} response read failed"))?;

            if status.is_server_error() && attempt < TELEGRAM_REQUEST_MAX_ATTEMPTS {
                tracing::warn!(
                    method,
                    attempt,
                    status = %status,
                    body = %telegram_response_body_preview(&bytes),
                    "telegram api returned server error; retrying"
                );
                tokio::time::sleep(request_retry_backoff_delay(attempt)).await;
                continue;
            }

            return decode_telegram_response(method, status, &bytes);
        }

        tracing::error!(
            method,
            attempts = TELEGRAM_REQUEST_MAX_ATTEMPTS,
            "telegram request loop exhausted unexpectedly without returning"
        );
        anyhow::bail!("telegram {method} request loop exhausted unexpectedly");
    }

    async fn send_text_message(&self, msg: &OutgoingMessage) -> anyhow::Result<SentMessage> {
        #[derive(Serialize)]
        struct SendMessageRequest<'a> {
            chat_id: &'a str,
            text: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            reply_to_message_id: Option<i64>,
            #[serde(skip_serializing_if = "Option::is_none")]
            parse_mode: Option<&'static str>,
            disable_web_page_preview: bool,
        }

        let (content, parse_mode) = match msg.parse_mode {
            ParseMode::PlainText => {
                let rendered = render_chat_message(ChatPlatform::Telegram, &msg.content);
                (rendered.content, rendered.parse_mode)
            }
            _ => (msg.content.clone(), msg.parse_mode),
        };
        let reply_to_message_id = parse_reply_to_message_id(msg.reply_to.as_deref());
        let message: TelegramMessage = self
            .post_json(
                "sendMessage",
                &SendMessageRequest {
                    chat_id: &msg.chat_id,
                    text: &content,
                    reply_to_message_id,
                    parse_mode: parse_mode.as_telegram_str(),
                    disable_web_page_preview: true,
                },
                TELEGRAM_REQUEST_TIMEOUT,
            )
            .await?;

        Ok(SentMessage {
            message_id: message.message_id.to_string(),
        })
    }

    async fn send_document(
        &self,
        chat_id: &str,
        reply_to: Option<&str>,
        attachment: &Attachment,
    ) -> anyhow::Result<SentMessage> {
        let url = self.method_url("sendDocument");

        for attempt in 1..=TELEGRAM_REQUEST_MAX_ATTEMPTS {
            let part = Part::bytes(attachment.data.clone())
                .file_name(attachment.filename.clone())
                .mime_str(&attachment.mime_type)
                .with_context(|| {
                    format!(
                        "telegram attachment mime type is invalid: {}",
                        attachment.mime_type
                    )
                })?;

            let mut form = Form::new()
                .text("chat_id", chat_id.to_string())
                .part("document", part);
            if let Some(reply_to_message_id) = parse_reply_to_message_id(reply_to) {
                form = form.text("reply_to_message_id", reply_to_message_id.to_string());
            }

            let response = match self
                .client
                .post(&url)
                .timeout(TELEGRAM_REQUEST_TIMEOUT)
                .multipart(form)
                .send()
                .await
            {
                Ok(response) => response,
                Err(err)
                    if attempt < TELEGRAM_REQUEST_MAX_ATTEMPTS
                        && is_retryable_transport_error(&err) =>
                {
                    tracing::warn!(
                        attempt,
                        err = %err,
                        "telegram sendDocument transport failed; retrying"
                    );
                    tokio::time::sleep(request_retry_backoff_delay(attempt)).await;
                    continue;
                }
                Err(err) => return Err(err).context("telegram sendDocument request failed"),
            };

            let status = response.status();
            let bytes = response
                .bytes()
                .await
                .context("telegram sendDocument response read failed")?;

            if status.is_server_error() && attempt < TELEGRAM_REQUEST_MAX_ATTEMPTS {
                tracing::warn!(
                    attempt,
                    status = %status,
                    body = %telegram_response_body_preview(&bytes),
                    "telegram sendDocument server error; retrying"
                );
                tokio::time::sleep(request_retry_backoff_delay(attempt)).await;
                continue;
            }

            let message: TelegramMessage =
                decode_telegram_response("sendDocument", status, &bytes)?;
            return Ok(SentMessage {
                message_id: message.message_id.to_string(),
            });
        }

        anyhow::bail!("telegram sendDocument request loop exhausted unexpectedly");
    }

    async fn get_file_metadata(&self, file_id: &str) -> anyhow::Result<TelegramFile> {
        #[derive(Serialize)]
        struct GetFileRequest<'a> {
            file_id: &'a str,
        }

        self.post_json(
            "getFile",
            &GetFileRequest { file_id },
            TELEGRAM_REQUEST_TIMEOUT,
        )
        .await
    }

    async fn download_file(&self, file_path: &str) -> anyhow::Result<Arc<[u8]>> {
        let url = format!(
            "{TELEGRAM_API_BASE}/file/bot{}/{}",
            self.bot_token, file_path
        );
        let response = self
            .client
            .get(&url)
            .timeout(TELEGRAM_REQUEST_TIMEOUT)
            .send()
            .await
            .with_context(|| format!("telegram file download failed: {file_path}"))?;
        let status = response.status();
        let bytes = response
            .bytes()
            .await
            .with_context(|| format!("telegram file read failed: {file_path}"))?;
        if !status.is_success() {
            anyhow::bail!(
                "telegram file download failed with {}: {}",
                status,
                telegram_response_body_preview(&bytes)
            );
        }
        Ok(Arc::<[u8]>::from(bytes.to_vec()))
    }

    async fn download_telegram_document(
        &self,
        document: &TelegramDocument,
    ) -> anyhow::Result<IncomingAttachment> {
        let file = self.get_file_metadata(&document.file_id).await?;
        let data = self.download_file(&file.file_path).await?;
        let filename = document
            .file_name
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| format!("telegram-document-{}", document.file_id));
        let kind = classify_telegram_attachment(document.mime_type.as_deref(), &filename);
        Ok(IncomingAttachment {
            kind,
            filename,
            mime_type: document.mime_type.clone(),
            data,
            caption: None,
        })
    }

    async fn download_telegram_photo(
        &self,
        photo: &TelegramPhotoSize,
        fallback_name: &str,
        caption: Option<&str>,
    ) -> anyhow::Result<IncomingAttachment> {
        let file = self.get_file_metadata(&photo.file_id).await?;
        let data = self.download_file(&file.file_path).await?;
        let ext = file
            .file_path
            .rsplit('.')
            .next()
            .filter(|value| !value.contains('/'))
            .unwrap_or("jpg");
        let mime_type = mime_guess::from_path(&file.file_path)
            .first_raw()
            .unwrap_or("image/jpeg")
            .to_string();
        Ok(IncomingAttachment {
            kind: IncomingAttachmentKind::Image,
            filename: format!("{}.{}", fallback_name, ext),
            mime_type: Some(mime_type),
            data,
            caption: caption
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned),
        })
    }
}

impl TelegramPoller {
    pub async fn prepare_for_polling(&mut self) -> anyhow::Result<Option<IncomingMessage>> {
        self.delete_webhook().await?;
        self.sync_cursor_to_latest().await
    }

    pub async fn poll_updates(&mut self) -> anyhow::Result<Vec<IncomingMessage>> {
        let updates = self
            .get_updates_raw(self.next_offset, TELEGRAM_LONG_POLL_TIMEOUT_SECS, None)
            .await?;
        let mut messages = Vec::new();
        for update in updates {
            self.next_offset = self.next_offset.max(update.update_id.saturating_add(1));
            if let Some(message) = incoming_message_from_update(&self.adapter, update).await? {
                messages.push(message);
            }
        }
        Ok(messages)
    }

    async fn delete_webhook(&self) -> anyhow::Result<()> {
        #[derive(Serialize)]
        struct DeleteWebhookRequest {
            drop_pending_updates: bool,
        }

        let _: bool = self
            .adapter
            .post_json(
                "deleteWebhook",
                &DeleteWebhookRequest {
                    drop_pending_updates: false,
                },
                TELEGRAM_REQUEST_TIMEOUT,
            )
            .await?;
        Ok(())
    }

    async fn sync_cursor_to_latest(&mut self) -> anyhow::Result<Option<IncomingMessage>> {
        let updates = self.get_updates_raw(-1, 0, Some(1)).await?;
        let Some(update) = updates.into_iter().last() else {
            return Ok(None);
        };
        self.next_offset = update.update_id.saturating_add(1);
        let Some(message) = incoming_message_from_update(&self.adapter, update).await? else {
            return Ok(None);
        };
        let now_ms = crate::storage::now_ms();
        let age_ms = now_ms.saturating_sub(message.timestamp_ms);
        Ok((0..=TELEGRAM_RECENT_UPDATE_GRACE_MS)
            .contains(&age_ms)
            .then_some(message))
    }

    async fn get_updates_raw(
        &self,
        offset: i64,
        timeout: i64,
        limit: Option<i64>,
    ) -> anyhow::Result<Vec<TelegramUpdate>> {
        #[derive(Serialize)]
        struct GetUpdatesRequest {
            offset: i64,
            timeout: i64,
            #[serde(skip_serializing_if = "Option::is_none")]
            limit: Option<i64>,
            allowed_updates: Vec<&'static str>,
        }

        self.adapter
            .post_json(
                "getUpdates",
                &GetUpdatesRequest {
                    offset,
                    timeout,
                    limit,
                    allowed_updates: vec!["message"],
                },
                TELEGRAM_LONG_POLL_REQUEST_TIMEOUT,
            )
            .await
    }
}

#[async_trait]
impl ChatAdapter for TelegramAdapter {
    async fn send_message(&self, msg: OutgoingMessage) -> anyhow::Result<SentMessage> {
        let mut first_sent = None;

        if !msg.content.trim().is_empty() {
            first_sent = Some(self.send_text_message(&msg).await?);
        }

        for (index, attachment) in msg.attachments.iter().enumerate() {
            let reply_to = if first_sent.is_none() && index == 0 {
                msg.reply_to.as_deref()
            } else {
                None
            };
            let sent = self
                .send_document(&msg.chat_id, reply_to, attachment)
                .await?;
            if first_sent.is_none() {
                first_sent = Some(sent);
            }
        }

        first_sent.ok_or_else(|| anyhow::anyhow!("telegram outgoing message is empty"))
    }

    async fn edit_message(
        &self,
        chat_id: &str,
        message_id: &str,
        content: &str,
    ) -> anyhow::Result<()> {
        #[derive(Serialize)]
        struct EditMessageRequest<'a> {
            chat_id: &'a str,
            message_id: i64,
            text: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            parse_mode: Option<&'static str>,
            disable_web_page_preview: bool,
        }

        let rendered = render_chat_message(ChatPlatform::Telegram, content);
        let _: serde_json::Value = self
            .post_json(
                "editMessageText",
                &EditMessageRequest {
                    chat_id,
                    message_id: message_id
                        .parse()
                        .context("telegram message_id parse failed for editMessageText")?,
                    text: &rendered.content,
                    parse_mode: rendered.parse_mode.as_telegram_str(),
                    disable_web_page_preview: true,
                },
                TELEGRAM_REQUEST_TIMEOUT,
            )
            .await?;
        Ok(())
    }

    async fn delete_message(&self, chat_id: &str, message_id: &str) -> anyhow::Result<()> {
        #[derive(Serialize)]
        struct DeleteMessageRequest<'a> {
            chat_id: &'a str,
            message_id: i64,
        }

        let _: bool = self
            .post_json(
                "deleteMessage",
                &DeleteMessageRequest {
                    chat_id,
                    message_id: message_id
                        .parse()
                        .context("telegram message_id parse failed for deleteMessage")?,
                },
                TELEGRAM_REQUEST_TIMEOUT,
            )
            .await?;
        Ok(())
    }

    async fn send_typing(&self, chat_id: &str) -> anyhow::Result<()> {
        #[derive(Serialize)]
        struct ChatActionRequest<'a> {
            chat_id: &'a str,
            action: &'a str,
        }

        let _: bool = self
            .post_json(
                "sendChatAction",
                &ChatActionRequest {
                    chat_id,
                    action: "typing",
                },
                TELEGRAM_REQUEST_TIMEOUT,
            )
            .await?;
        Ok(())
    }

    fn platform(&self) -> ChatPlatform {
        ChatPlatform::Telegram
    }
}

fn parse_reply_to_message_id(reply_to: Option<&str>) -> Option<i64> {
    reply_to.and_then(|value| value.parse().ok())
}

fn request_retry_backoff_delay(attempt: usize) -> Duration {
    let exponent = attempt.saturating_sub(1).min(10);
    let factor = 1u32 << u32::try_from(exponent).unwrap_or(10);
    TELEGRAM_RETRY_DELAY_BASE
        .checked_mul(factor)
        .map(|delay| delay.min(TELEGRAM_RETRY_DELAY_MAX))
        .unwrap_or(TELEGRAM_RETRY_DELAY_MAX)
}

fn telegram_user_display_name(user: &TelegramUser) -> Option<String> {
    if let Some(username) = user.username.as_deref() {
        let trimmed = username.trim();
        if !trimmed.is_empty() {
            return Some(format!("@{trimmed}"));
        }
    }

    let first = user.first_name.as_deref().unwrap_or("").trim();
    let last = user.last_name.as_deref().unwrap_or("").trim();
    let full = format!("{first} {last}").trim().to_string();
    (!full.is_empty()).then_some(full)
}

#[derive(Debug, Deserialize)]
struct TelegramEnvelope<T> {
    ok: bool,
    result: Option<T>,
    description: Option<String>,
}

impl<T> TelegramEnvelope<T> {
    fn into_result(self, method: &str) -> anyhow::Result<T> {
        if self.ok {
            return self
                .result
                .ok_or_else(|| anyhow::anyhow!("telegram {method} returned ok but no result"));
        }
        Err(anyhow::anyhow!(
            "telegram {method} failed: {}",
            self.description
                .unwrap_or_else(|| "unknown telegram api error".to_string())
        ))
    }
}

fn is_retryable_transport_error(err: &reqwest::Error) -> bool {
    err.is_timeout() || err.is_connect() || (err.is_request() && err.status().is_none())
}

fn telegram_response_body_preview(bytes: &Bytes) -> String {
    let text = String::from_utf8_lossy(bytes);
    let trimmed = text.trim();
    let mut chars = trimmed.chars();
    let preview: String = chars.by_ref().take(240).collect();
    if chars.next().is_some() {
        format!("{preview}...")
    } else {
        preview
    }
}

fn decode_telegram_response<T>(method: &str, status: StatusCode, bytes: &Bytes) -> anyhow::Result<T>
where
    T: DeserializeOwned,
{
    match serde_json::from_slice::<TelegramEnvelope<T>>(bytes) {
        Ok(envelope) => envelope.into_result(method),
        Err(err) if status.is_success() => {
            Err(err).context(format!("telegram {method} response decode failed"))
        }
        Err(err) => Err(anyhow::anyhow!(
            "telegram {method} returned http {} with undecodable body ({}): {}",
            status,
            err,
            telegram_response_body_preview(bytes)
        )),
    }
}

#[derive(Debug, Deserialize)]
struct TelegramUpdate {
    update_id: i64,
    message: Option<TelegramMessage>,
}

#[derive(Debug, Deserialize)]
struct TelegramMessage {
    message_id: i64,
    date: i32,
    chat: TelegramChat,
    from: Option<TelegramUser>,
    caption: Option<String>,
    text: Option<String>,
    document: Option<TelegramDocument>,
    photo: Option<Vec<TelegramPhotoSize>>,
}

#[derive(Debug, Deserialize)]
struct TelegramChat {
    id: i64,
}

#[derive(Debug, Deserialize)]
struct TelegramUser {
    id: i64,
    username: Option<String>,
    first_name: Option<String>,
    last_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TelegramDocument {
    file_id: String,
    file_name: Option<String>,
    mime_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TelegramPhotoSize {
    file_id: String,
}

#[derive(Debug, Deserialize)]
struct TelegramFile {
    file_path: String,
}

async fn incoming_message_from_update(
    adapter: &TelegramAdapter,
    update: TelegramUpdate,
) -> anyhow::Result<Option<IncomingMessage>> {
    let Some(message) = update.message else {
        return Ok(None);
    };
    let mut attachments = Vec::<IncomingAttachment>::new();

    if let Some(document) = message.document.as_ref() {
        match adapter.download_telegram_document(document).await {
            Ok(attachment) => attachments.push(attachment),
            Err(err) => {
                tracing::warn!(
                    message_id = message.message_id,
                    err = %err,
                    "download telegram document failed; skipping attachment"
                );
            }
        }
    }

    if let Some(photo) = message.photo.as_ref().and_then(|items| items.last()) {
        let fallback_name = format!("telegram-photo-{}", message.message_id);
        match adapter
            .download_telegram_photo(photo, &fallback_name, message.caption.as_deref())
            .await
        {
            Ok(attachment) => attachments.push(attachment),
            Err(err) => {
                tracing::warn!(
                    message_id = message.message_id,
                    err = %err,
                    "download telegram photo failed; skipping attachment"
                );
            }
        }
    }

    let text = message
        .text
        .as_deref()
        .or(message.caption.as_deref())
        .map(str::trim)
        .unwrap_or("")
        .to_string();
    if text.is_empty() && attachments.is_empty() {
        return Ok(None);
    }
    let Some(from) = message.from else {
        return Ok(None);
    };

    Ok(Some(IncomingMessage {
        platform: ChatPlatform::Telegram,
        sender_id: from.id.to_string(),
        sender_display_name: telegram_user_display_name(&from),
        chat_id: message.chat.id.to_string(),
        text,
        attachments,
        message_id: Some(message.message_id.to_string()),
        timestamp_ms: i64::from(message.date).saturating_mul(1000),
    }))
}

fn classify_telegram_attachment(mime_type: Option<&str>, filename: &str) -> IncomingAttachmentKind {
    if mime_type
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_telegram_response_extracts_api_error_description() {
        let body =
            Bytes::from_static(br#"{"ok":false,"description":"Bad Request: chat not found"}"#);
        let err = decode_telegram_response::<TelegramMessage>(
            "sendMessage",
            StatusCode::BAD_REQUEST,
            &body,
        )
        .expect_err("should return telegram api error");
        assert!(
            err.to_string().contains("Bad Request: chat not found"),
            "{err}"
        );
    }

    #[test]
    fn decode_telegram_response_decodes_success_payload() {
        let body = Bytes::from_static(
            br#"{"ok":true,"result":{"message_id":42,"date":123,"chat":{"id":7},"from":{"id":9,"username":"demo","first_name":"Demo","last_name":null},"text":"hello"}}"#,
        );
        let message =
            decode_telegram_response::<TelegramMessage>("sendMessage", StatusCode::OK, &body)
                .expect("should decode telegram success payload");
        assert_eq!(message.message_id, 42);
        assert_eq!(message.text.as_deref(), Some("hello"));
    }

    #[test]
    fn request_retry_backoff_delay_grows_and_caps() {
        assert_eq!(request_retry_backoff_delay(1), Duration::from_millis(250));
        assert_eq!(request_retry_backoff_delay(2), Duration::from_millis(500));
        assert_eq!(request_retry_backoff_delay(3), Duration::from_millis(1000));
        assert_eq!(request_retry_backoff_delay(4), Duration::from_secs(2));
    }
}
