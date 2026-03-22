use anyhow::Context as _;
use async_trait::async_trait;
use reqwest::multipart::{Form, Part};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;

use super::{
    Attachment, ChatAdapter, IncomingAttachment, IncomingAttachmentKind, IncomingMessage,
    OutgoingMessage, SentMessage,
};
use crate::storage::ChatPlatform;

// Keep this conservative; users can adjust later if Meta deprecates a version.
const WHATSAPP_GRAPH_API_VERSION: &str = "v20.0";
const WHATSAPP_GRAPH_API_BASE: &str = "https://graph.facebook.com";
const WHATSAPP_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Serialize)]
struct ReplyContext<'a> {
    message_id: &'a str,
}

#[derive(Deserialize)]
struct SendResponseMessage {
    id: String,
}

#[derive(Deserialize)]
struct SendResponse {
    messages: Option<Vec<SendResponseMessage>>,
}

impl SendResponse {
    fn into_message_id(self) -> String {
        self.messages
            .and_then(|items| items.into_iter().next())
            .map(|item| item.id)
            .unwrap_or_else(|| "unknown".to_string())
    }
}

#[derive(Debug, Clone)]
pub struct WhatsAppWebhookMessage {
    pub from: String,
    pub contact_name: Option<String>,
    pub message_id: String,
    pub timestamp_ms: i64,
    pub text: String,
    pub attachments: Vec<WhatsAppWebhookAttachment>,
}

#[derive(Debug, Clone)]
pub struct WhatsAppWebhookAttachment {
    pub kind: IncomingAttachmentKind,
    pub media_id: String,
    pub mime_type: Option<String>,
    pub filename: Option<String>,
    pub caption: Option<String>,
}

#[derive(Clone)]
pub struct WhatsAppAdapter {
    client: reqwest::Client,
    phone_number_id: String,
    access_token: String,
}

impl WhatsAppAdapter {
    pub fn new(client: reqwest::Client, phone_number_id: String, access_token: String) -> Self {
        Self {
            client,
            phone_number_id,
            access_token,
        }
    }

    pub async fn webhook_message_to_incoming(
        &self,
        msg: WhatsAppWebhookMessage,
    ) -> anyhow::Result<IncomingMessage> {
        let mut attachments = Vec::new();
        for item in msg.attachments {
            match self.download_media(&item.media_id).await {
                Ok(bytes) => attachments.push(IncomingAttachment {
                    kind: item.kind,
                    filename: infer_attachment_filename(&item, &msg.message_id),
                    mime_type: item.mime_type.clone(),
                    data: bytes,
                    caption: item.caption.clone(),
                }),
                Err(err) => {
                    tracing::warn!(
                        platform = "whatsapp",
                        media_id = %item.media_id,
                        err = %err,
                        "download whatsapp media failed; ignoring attachment"
                    );
                }
            }
        }

        Ok(IncomingMessage {
            platform: ChatPlatform::WhatsApp,
            sender_id: msg.from.clone(),
            sender_display_name: msg.contact_name.clone(),
            chat_id: msg.from,
            text: msg.text,
            attachments,
            message_id: Some(msg.message_id),
            timestamp_ms: msg.timestamp_ms,
        })
    }

    fn graph_url(&self, path: &str) -> String {
        format!("{WHATSAPP_GRAPH_API_BASE}/{WHATSAPP_GRAPH_API_VERSION}/{path}")
    }

    async fn get_json<TResp>(&self, path: &str) -> anyhow::Result<TResp>
    where
        TResp: DeserializeOwned,
    {
        let url = self.graph_url(path);
        let response = self
            .client
            .get(&url)
            .timeout(WHATSAPP_REQUEST_TIMEOUT)
            .bearer_auth(&self.access_token)
            .send()
            .await
            .with_context(|| format!("whatsapp graph api GET failed: {url}"))?;
        let status = response.status();
        let bytes = response
            .bytes()
            .await
            .with_context(|| format!("whatsapp graph api GET body read failed: {url}"))?;
        if !status.is_success() {
            anyhow::bail!(
                "whatsapp graph api GET failed: {url} status={status} body={}",
                body_preview(&bytes)
            );
        }
        serde_json::from_slice(&bytes).context("whatsapp graph api GET json decode failed")
    }

    async fn post_json<TReq, TResp>(&self, path: &str, body: &TReq) -> anyhow::Result<TResp>
    where
        TReq: Serialize + ?Sized,
        TResp: DeserializeOwned,
    {
        let url = self.graph_url(path);
        let response = self
            .client
            .post(&url)
            .timeout(WHATSAPP_REQUEST_TIMEOUT)
            .bearer_auth(&self.access_token)
            .json(body)
            .send()
            .await
            .with_context(|| format!("whatsapp graph api POST failed: {url}"))?;
        let status = response.status();
        let bytes = response
            .bytes()
            .await
            .with_context(|| format!("whatsapp graph api POST body read failed: {url}"))?;
        if !status.is_success() {
            anyhow::bail!(
                "whatsapp graph api POST failed: {url} status={status} body={}",
                body_preview(&bytes)
            );
        }
        serde_json::from_slice(&bytes).context("whatsapp graph api POST json decode failed")
    }

    async fn post_multipart<TResp>(&self, path: &str, form: Form) -> anyhow::Result<TResp>
    where
        TResp: DeserializeOwned,
    {
        let url = self.graph_url(path);
        let response = self
            .client
            .post(&url)
            .timeout(WHATSAPP_REQUEST_TIMEOUT)
            .bearer_auth(&self.access_token)
            .multipart(form)
            .send()
            .await
            .with_context(|| format!("whatsapp graph api multipart POST failed: {url}"))?;
        let status = response.status();
        let bytes = response
            .bytes()
            .await
            .with_context(|| format!("whatsapp graph api multipart body read failed: {url}"))?;
        if !status.is_success() {
            anyhow::bail!(
                "whatsapp graph api multipart POST failed: {url} status={status} body={}",
                body_preview(&bytes)
            );
        }
        serde_json::from_slice(&bytes).context("whatsapp graph api multipart json decode failed")
    }

    async fn send_text_message(
        &self,
        to: &str,
        reply_to: Option<&str>,
        content: &str,
    ) -> anyhow::Result<SentMessage> {
        #[derive(Serialize)]
        struct TextBody<'a> {
            body: &'a str,
            preview_url: bool,
        }

        #[derive(Serialize)]
        struct SendTextRequest<'a> {
            messaging_product: &'static str,
            to: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            context: Option<ReplyContext<'a>>,
            #[serde(rename = "type")]
            kind: &'static str,
            text: TextBody<'a>,
        }

        let resp: SendResponse = self
            .post_json(
                &format!("{}/messages", self.phone_number_id),
                &SendTextRequest {
                    messaging_product: "whatsapp",
                    to,
                    context: reply_to.map(|id| ReplyContext { message_id: id }),
                    kind: "text",
                    text: TextBody {
                        body: content,
                        preview_url: false,
                    },
                },
            )
            .await?;

        Ok(SentMessage {
            message_id: resp.into_message_id(),
        })
    }

    async fn upload_media(&self, attachment: &Attachment) -> anyhow::Result<String> {
        #[derive(Deserialize)]
        struct UploadResponse {
            id: String,
        }

        let part = Part::bytes(attachment.data.clone())
            .file_name(attachment.filename.clone())
            .mime_str(&attachment.mime_type)
            .context("whatsapp attachment mime type is invalid")?;

        let form = Form::new()
            .text("messaging_product", "whatsapp")
            .text("type", attachment.mime_type.clone())
            .part("file", part);

        let resp: UploadResponse = self
            .post_multipart(&format!("{}/media", self.phone_number_id), form)
            .await?;
        Ok(resp.id)
    }

    async fn send_document_message(
        &self,
        to: &str,
        reply_to: Option<&str>,
        media_id: &str,
        filename: Option<&str>,
        caption: Option<&str>,
    ) -> anyhow::Result<SentMessage> {
        #[derive(Serialize)]
        struct Document<'a> {
            id: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            filename: Option<&'a str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            caption: Option<&'a str>,
        }

        #[derive(Serialize)]
        struct SendRequest<'a> {
            messaging_product: &'static str,
            to: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            context: Option<ReplyContext<'a>>,
            #[serde(rename = "type")]
            kind: &'static str,
            document: Document<'a>,
        }

        let resp: SendResponse = self
            .post_json(
                &format!("{}/messages", self.phone_number_id),
                &SendRequest {
                    messaging_product: "whatsapp",
                    to,
                    context: reply_to.map(|id| ReplyContext { message_id: id }),
                    kind: "document",
                    document: Document {
                        id: media_id,
                        filename,
                        caption,
                    },
                },
            )
            .await?;

        Ok(SentMessage {
            message_id: resp.into_message_id(),
        })
    }

    async fn send_image_message(
        &self,
        to: &str,
        reply_to: Option<&str>,
        media_id: &str,
        caption: Option<&str>,
    ) -> anyhow::Result<SentMessage> {
        #[derive(Serialize)]
        struct Image<'a> {
            id: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            caption: Option<&'a str>,
        }

        #[derive(Serialize)]
        struct SendRequest<'a> {
            messaging_product: &'static str,
            to: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            context: Option<ReplyContext<'a>>,
            #[serde(rename = "type")]
            kind: &'static str,
            image: Image<'a>,
        }

        let resp: SendResponse = self
            .post_json(
                &format!("{}/messages", self.phone_number_id),
                &SendRequest {
                    messaging_product: "whatsapp",
                    to,
                    context: reply_to.map(|id| ReplyContext { message_id: id }),
                    kind: "image",
                    image: Image {
                        id: media_id,
                        caption,
                    },
                },
            )
            .await?;

        Ok(SentMessage {
            message_id: resp.into_message_id(),
        })
    }

    async fn download_media(&self, media_id: &str) -> anyhow::Result<Arc<[u8]>> {
        #[derive(Deserialize)]
        struct MediaInfo {
            url: String,
        }

        let info: MediaInfo = self.get_json(media_id).await?;
        let response = self
            .client
            .get(&info.url)
            .timeout(WHATSAPP_REQUEST_TIMEOUT)
            .bearer_auth(&self.access_token)
            .send()
            .await
            .with_context(|| format!("whatsapp media download failed: {}", info.url))?;
        let status = response.status();
        let bytes = response
            .bytes()
            .await
            .with_context(|| format!("whatsapp media download body read failed: {}", info.url))?;
        if !status.is_success() {
            anyhow::bail!(
                "whatsapp media download failed: {} status={status} body={}",
                info.url,
                body_preview(&bytes)
            );
        }
        Ok(bytes.to_vec().into())
    }
}

#[async_trait]
impl ChatAdapter for WhatsAppAdapter {
    async fn send_message(&self, msg: OutgoingMessage) -> anyhow::Result<SentMessage> {
        let mut first_sent = None;

        if !msg.content.trim().is_empty() {
            first_sent = Some(
                self.send_text_message(&msg.chat_id, msg.reply_to.as_deref(), &msg.content)
                    .await?,
            );
        }

        for (index, attachment) in msg.attachments.iter().enumerate() {
            let reply_to = if first_sent.is_none() && index == 0 {
                msg.reply_to.as_deref()
            } else {
                None
            };
            let media_id = self.upload_media(attachment).await?;
            let sent = if attachment.mime_type.starts_with("image/") {
                self.send_image_message(&msg.chat_id, reply_to, &media_id, None)
                    .await?
            } else {
                self.send_document_message(
                    &msg.chat_id,
                    reply_to,
                    &media_id,
                    Some(&attachment.filename),
                    None,
                )
                .await?
            };
            if first_sent.is_none() {
                first_sent = Some(sent);
            }
        }

        first_sent.ok_or_else(|| anyhow::anyhow!("whatsapp outgoing message is empty"))
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
        // WhatsApp Cloud API doesn't expose a typing indicator endpoint.
        Ok(())
    }

    fn platform(&self) -> ChatPlatform {
        ChatPlatform::WhatsApp
    }
}

fn body_preview(bytes: &[u8]) -> String {
    let preview = bytes
        .iter()
        .take(512)
        .copied()
        .map(|b| {
            if b.is_ascii_graphic() || b == b' ' {
                b as char
            } else {
                '.'
            }
        })
        .collect::<String>();
    if bytes.len() > 512 {
        format!("{preview}…")
    } else {
        preview
    }
}

fn infer_attachment_filename(item: &WhatsAppWebhookAttachment, message_id: &str) -> String {
    let base = item
        .filename
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|v| v.to_string());
    if let Some(name) = base.as_deref()
        && std::path::Path::new(name).extension().is_some()
    {
        return name.to_string();
    }

    let inferred_ext = item
        .mime_type
        .as_deref()
        .and_then(mime_guess::get_mime_extensions_str)
        .and_then(|items| items.first().copied());
    let stem = match item.kind {
        IncomingAttachmentKind::Image => "image",
        IncomingAttachmentKind::File => "file",
    };
    let sid = crate::chat_bridge::short_id(message_id);
    match (base, inferred_ext) {
        (Some(name), Some(ext)) => format!("{name}.{ext}"),
        (Some(name), None) => name,
        (None, Some(ext)) => format!("{stem}-{sid}.{ext}"),
        (None, None) => format!("{stem}-{sid}"),
    }
}
