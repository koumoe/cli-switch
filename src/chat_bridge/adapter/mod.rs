use async_trait::async_trait;

use crate::storage::ChatPlatform;

pub mod telegram;

#[derive(Debug, Clone)]
pub struct IncomingMessage {
    pub platform: ChatPlatform,
    pub sender_id: String,
    pub sender_display_name: Option<String>,
    pub chat_id: String,
    pub text: String,
    pub message_id: Option<String>,
    pub timestamp_ms: i64,
}

#[derive(Debug, Clone)]
pub struct OutgoingMessage {
    pub chat_id: String,
    pub content: String,
    pub reply_to: Option<String>,
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
