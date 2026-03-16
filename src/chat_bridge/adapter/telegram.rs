use anyhow::Context as _;
use async_trait::async_trait;
use bytes::Bytes;
use reqwest::StatusCode;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;

use super::{ChatAdapter, IncomingMessage, OutgoingMessage, SentMessage, StreamingMessage};
use crate::storage::ChatPlatform;

const TELEGRAM_API_BASE: &str = "https://api.telegram.org";
const TELEGRAM_LONG_POLL_TIMEOUT_SECS: i64 = 30;
const TELEGRAM_RECENT_UPDATE_GRACE_MS: i64 = 15_000;
const TELEGRAM_REQUEST_TIMEOUT: Duration = Duration::from_secs(6);
const TELEGRAM_LONG_POLL_REQUEST_TIMEOUT: Duration =
    Duration::from_secs(TELEGRAM_LONG_POLL_TIMEOUT_SECS as u64 + 10);
const TELEGRAM_REQUEST_MAX_ATTEMPTS: usize = 2;
const TELEGRAM_RETRY_DELAY: Duration = Duration::from_millis(250);
static NEXT_TELEGRAM_DRAFT_ID: AtomicI64 = AtomicI64::new(1);

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
                    tokio::time::sleep(TELEGRAM_RETRY_DELAY).await;
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
                tokio::time::sleep(TELEGRAM_RETRY_DELAY).await;
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

    async fn send_message_draft(
        &self,
        chat_id: &str,
        draft_id: i64,
        text: &str,
    ) -> anyhow::Result<()> {
        #[derive(Serialize)]
        struct SendMessageDraftRequest<'a> {
            chat_id: i64,
            draft_id: i64,
            text: &'a str,
        }

        let _: bool = self
            .post_json(
                "sendMessageDraft",
                &SendMessageDraftRequest {
                    chat_id: parse_chat_id(chat_id)?,
                    draft_id,
                    text,
                },
                TELEGRAM_REQUEST_TIMEOUT,
            )
            .await?;
        Ok(())
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
            if let Some(message) = incoming_message_from_update(update) {
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
        let Some(message) = incoming_message_from_update(update) else {
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
        #[derive(Serialize)]
        struct SendMessageRequest<'a> {
            chat_id: &'a str,
            text: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            reply_to_message_id: Option<i64>,
            disable_web_page_preview: bool,
        }

        let reply_to_message_id = parse_reply_to_message_id(msg.reply_to.as_deref());
        let message: TelegramMessage = self
            .post_json(
                "sendMessage",
                &SendMessageRequest {
                    chat_id: &msg.chat_id,
                    text: &msg.content,
                    reply_to_message_id,
                    disable_web_page_preview: true,
                },
                TELEGRAM_REQUEST_TIMEOUT,
            )
            .await?;

        Ok(SentMessage {
            message_id: message.message_id.to_string(),
        })
    }

    async fn begin_streaming_message(
        &self,
        msg: OutgoingMessage,
    ) -> anyhow::Result<Option<StreamingMessage>> {
        let draft_id = next_draft_id();
        self.send_message_draft(&msg.chat_id, draft_id, &msg.content)
            .await?;
        Ok(Some(StreamingMessage {
            id: draft_id.to_string(),
        }))
    }

    async fn update_streaming_message(
        &self,
        chat_id: &str,
        stream: &StreamingMessage,
        content: &str,
    ) -> anyhow::Result<()> {
        self.send_message_draft(chat_id, parse_draft_id(&stream.id)?, content)
            .await
    }

    async fn finalize_streaming_message(
        &self,
        stream: StreamingMessage,
        msg: OutgoingMessage,
    ) -> anyhow::Result<SentMessage> {
        let draft_id = parse_draft_id(&stream.id)?;
        let sent = self.send_message(msg.clone()).await?;
        if let Err(err) = self.send_message_draft(&msg.chat_id, draft_id, "").await {
            tracing::warn!(
                chat_id = %msg.chat_id,
                draft_id,
                err = %err,
                "clear telegram message draft failed after finalize"
            );
        }
        Ok(sent)
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
            disable_web_page_preview: bool,
        }

        let _: serde_json::Value = self
            .post_json(
                "editMessageText",
                &EditMessageRequest {
                    chat_id,
                    message_id: message_id
                        .parse()
                        .context("telegram message_id parse failed for editMessageText")?,
                    text: content,
                    disable_web_page_preview: true,
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

fn parse_chat_id(chat_id: &str) -> anyhow::Result<i64> {
    chat_id
        .parse()
        .with_context(|| format!("telegram chat_id parse failed for sendMessageDraft: {chat_id}"))
}

fn parse_draft_id(draft_id: &str) -> anyhow::Result<i64> {
    draft_id
        .parse()
        .with_context(|| format!("telegram draft_id parse failed for sendMessageDraft: {draft_id}"))
}

fn next_draft_id() -> i64 {
    let id = NEXT_TELEGRAM_DRAFT_ID.fetch_add(1, Ordering::Relaxed);
    if id >= i64::MAX - 1 {
        NEXT_TELEGRAM_DRAFT_ID.store(1, Ordering::Relaxed);
    }
    id.max(1)
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
    text: Option<String>,
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

fn incoming_message_from_update(update: TelegramUpdate) -> Option<IncomingMessage> {
    let message = update.message?;
    let text = message.text.map(|value| value.trim().to_string())?;
    if text.is_empty() {
        return None;
    }
    let from = message.from?;

    Some(IncomingMessage {
        platform: ChatPlatform::Telegram,
        sender_id: from.id.to_string(),
        sender_display_name: telegram_user_display_name(&from),
        chat_id: message.chat.id.to_string(),
        text,
        message_id: Some(message.message_id.to_string()),
        timestamp_ms: i64::from(message.date).saturating_mul(1000),
    })
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
}
