use async_trait::async_trait;
use std::sync::Arc;

use crate::storage::ChatPlatform;

pub mod discord;
pub mod telegram;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IncomingAttachmentKind {
    File,
    Image,
}

impl IncomingAttachmentKind {
    pub fn label(self) -> &'static str {
        match self {
            IncomingAttachmentKind::File => "file",
            IncomingAttachmentKind::Image => "image",
        }
    }
}

#[derive(Debug, Clone)]
pub struct IncomingAttachment {
    pub kind: IncomingAttachmentKind,
    pub filename: String,
    pub mime_type: Option<String>,
    pub data: Arc<[u8]>,
    pub caption: Option<String>,
}

#[derive(Debug, Clone)]
pub struct IncomingMessage {
    pub platform: ChatPlatform,
    pub sender_id: String,
    pub sender_display_name: Option<String>,
    pub chat_id: String,
    pub text: String,
    pub attachments: Vec<IncomingAttachment>,
    pub message_id: Option<String>,
    pub timestamp_ms: i64,
}

impl IncomingMessage {
    pub fn has_attachments(&self) -> bool {
        !self.attachments.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseMode {
    PlainText,
    Html,
}

impl ParseMode {
    pub fn as_telegram_str(self) -> Option<&'static str> {
        match self {
            ParseMode::PlainText => None,
            ParseMode::Html => Some("HTML"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Attachment {
    pub filename: String,
    pub mime_type: String,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct OutgoingMessage {
    pub chat_id: String,
    pub content: String,
    pub reply_to: Option<String>,
    pub parse_mode: ParseMode,
    pub attachments: Vec<Attachment>,
}

#[derive(Debug, Clone)]
pub struct SentMessage {
    pub message_id: String,
}

#[derive(Debug, Clone)]
pub struct StreamingMessage {
    pub id: String,
}

#[async_trait]
pub trait ChatAdapter: Send + Sync {
    async fn send_message(&self, msg: OutgoingMessage) -> anyhow::Result<SentMessage>;
    async fn edit_message(
        &self,
        chat_id: &str,
        message_id: &str,
        content: &str,
    ) -> anyhow::Result<()>;
    async fn delete_message(&self, _chat_id: &str, _message_id: &str) -> anyhow::Result<()> {
        Ok(())
    }
    async fn begin_streaming_message(
        &self,
        _msg: OutgoingMessage,
    ) -> anyhow::Result<Option<StreamingMessage>> {
        Ok(None)
    }
    async fn update_streaming_message(
        &self,
        _chat_id: &str,
        _stream: &StreamingMessage,
        _content: &str,
    ) -> anyhow::Result<()> {
        anyhow::bail!("streaming messages are not supported for this adapter")
    }
    async fn finalize_streaming_message(
        &self,
        _stream: StreamingMessage,
        msg: OutgoingMessage,
    ) -> anyhow::Result<SentMessage> {
        self.send_message(msg).await
    }
    async fn send_typing(&self, chat_id: &str) -> anyhow::Result<()>;
    fn platform(&self) -> ChatPlatform;
}
