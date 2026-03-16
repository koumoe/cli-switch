use anyhow::Context as _;
use std::sync::Arc;

use super::super::adapter::{
    ChatAdapter, IncomingMessage, OutgoingMessage, SentMessage, StreamingMessage,
};
use super::super::cli::{self, cli_type_label};
use super::super::router::format_session_label;
use super::super::{
    ChatBridgeRuntime, MESSAGE_CHAR_LIMIT, STREAM_UPDATE_INTERVAL, StreamChunk, StreamKind,
    TURN_EXECUTION_TIMEOUT, TYPING_INTERVAL, read_stream,
};
use super::format::{
    build_live_output, chunk_text, compose_final_output, format_streaming_message,
};
use super::notice::build_safe_mode_notice;
use super::parse::append_display_chunks_from_raw;
use crate::storage;

pub(in crate::chat_bridge) struct TurnExecutionResult {
    pub(in crate::chat_bridge) success: bool,
    pub(in crate::chat_bridge) stdout: String,
}

pub(in crate::chat_bridge) struct StreamingReply {
    adapter: Arc<dyn ChatAdapter>,
    chat_id: String,
    reply_to: Option<String>,
    label: String,
    message: Option<SentMessage>,
    stream: Option<StreamingMessage>,
    last_sent: String,
}

impl StreamingReply {
    pub(in crate::chat_bridge) fn new(
        adapter: Arc<dyn ChatAdapter>,
        chat_id: String,
        reply_to: Option<String>,
        initial_label: String,
    ) -> Self {
        Self {
            adapter,
            chat_id,
            reply_to,
            label: initial_label,
            message: None,
            stream: None,
            last_sent: String::new(),
        }
    }

    fn outgoing_message(&self, content: String) -> OutgoingMessage {
        OutgoingMessage {
            chat_id: self.chat_id.clone(),
            content,
            reply_to: self.reply_to.clone(),
        }
    }

    fn render_content(&self, body: &str) -> String {
        format_streaming_message(&self.label, body, MESSAGE_CHAR_LIMIT)
    }

    pub(in crate::chat_bridge) async fn update(&mut self, body: &str) -> anyhow::Result<()> {
        let rendered = self.render_content(body);
        if rendered == self.last_sent {
            return Ok(());
        }

        if let Some(sent) = self.message.as_ref() {
            self.adapter
                .edit_message(&self.chat_id, &sent.message_id, &rendered)
                .await?;
        } else if let Some(stream) = self.stream.as_ref() {
            if let Err(err) = self
                .adapter
                .update_streaming_message(&self.chat_id, stream, &rendered)
                .await
            {
                tracing::warn!(
                    chat_id = %self.chat_id,
                    err = %err,
                    "chat adapter native streaming update failed; falling back to send/edit"
                );
                let sent = self
                    .adapter
                    .send_message(self.outgoing_message(rendered.clone()))
                    .await?;
                self.message = Some(sent);
                self.stream = None;
            }
        } else {
            match self
                .adapter
                .begin_streaming_message(self.outgoing_message(rendered.clone()))
                .await
            {
                Ok(Some(stream)) => {
                    self.stream = Some(stream);
                }
                Ok(None) => {
                    let sent = self
                        .adapter
                        .send_message(self.outgoing_message(rendered.clone()))
                        .await?;
                    self.message = Some(sent);
                }
                Err(err) => {
                    tracing::warn!(
                        chat_id = %self.chat_id,
                        err = %err,
                        "chat adapter native streaming setup failed; falling back to send/edit"
                    );
                    let sent = self
                        .adapter
                        .send_message(self.outgoing_message(rendered.clone()))
                        .await?;
                    self.message = Some(sent);
                }
            }
        }
        self.last_sent = rendered;
        Ok(())
    }

    pub(in crate::chat_bridge) async fn finish(&mut self, body: &str) -> anyhow::Result<()> {
        let rendered = self.render_content(body);

        if let Some(sent) = self.message.as_ref() {
            if rendered != self.last_sent {
                self.adapter
                    .edit_message(&self.chat_id, &sent.message_id, &rendered)
                    .await?;
                self.last_sent = rendered;
            }
            return Ok(());
        }

        if let Some(stream) = self.stream.take() {
            let sent = match self
                .adapter
                .finalize_streaming_message(stream, self.outgoing_message(rendered.clone()))
                .await
            {
                Ok(sent) => sent,
                Err(err) => {
                    tracing::warn!(
                        chat_id = %self.chat_id,
                        err = %err,
                        "chat adapter native streaming finalize failed; falling back to sendMessage"
                    );
                    self.adapter
                        .send_message(self.outgoing_message(rendered.clone()))
                        .await?
                }
            };
            self.message = Some(sent);
            self.last_sent = rendered;
            return Ok(());
        }

        let sent = self
            .adapter
            .send_message(self.outgoing_message(rendered.clone()))
            .await?;
        self.message = Some(sent);
        self.last_sent = rendered;
        Ok(())
    }
}

impl ChatBridgeRuntime {
    pub(in crate::chat_bridge) async fn execute_turn_process(
        &self,
        adapter: Arc<dyn ChatAdapter>,
        msg: &IncomingMessage,
        session: &storage::BridgeSession,
        mut invocation: cli::CliInvocation,
        use_streaming: bool,
    ) -> anyhow::Result<TurnExecutionResult> {
        let label = format_session_label(session);
        let mut child = invocation
            .command
            .spawn()
            .with_context(|| format!("启动 {} 失败", cli_type_label(session.cli_type)))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("failed to capture child stdout"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| anyhow::anyhow!("failed to capture child stderr"))?;

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<StreamChunk>();
        let stdout_task = tokio::spawn(read_stream(stdout, StreamKind::Stdout, tx.clone()));
        let stderr_task = tokio::spawn(read_stream(stderr, StreamKind::Stderr, tx));

        let mut stdout_buf = String::new();
        let mut stderr_buf = String::new();
        let mut live_display_chunks = Vec::<String>::new();
        let child_pid = child.id().unwrap_or_default();
        let mut wait_fut = Box::pin(child.wait());
        let mut exit_status = None;
        let mut stream_closed = false;
        let mut timed_out = false;

        let mut typing_interval = tokio::time::interval(TYPING_INTERVAL);
        typing_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut stream_interval = tokio::time::interval(STREAM_UPDATE_INTERVAL);
        stream_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut timeout_fut = Box::pin(tokio::time::sleep(TURN_EXECUTION_TIMEOUT));

        let mut live_reply = if use_streaming {
            Some(StreamingReply::new(
                adapter.clone(),
                msg.chat_id.clone(),
                msg.message_id.clone(),
                label.clone(),
            ))
        } else {
            None
        };

        loop {
            tokio::select! {
                maybe_chunk = rx.recv(), if !stream_closed => {
                    match maybe_chunk {
                        Some(chunk) => {
                            match chunk.kind {
                                StreamKind::Stdout => {
                                    stdout_buf.push_str(&chunk.content);
                                    if use_streaming {
                                        append_display_chunks_from_raw(
                                            &chunk.content,
                                            false,
                                            &mut live_display_chunks,
                                        );
                                    }
                                }
                                StreamKind::Stderr => stderr_buf.push_str(&chunk.content),
                            }
                        }
                        None => {
                            stream_closed = true;
                            if exit_status.is_some() {
                                break;
                            }
                        }
                    }
                }
                status = &mut wait_fut, if exit_status.is_none() => {
                    exit_status = Some(status.context("wait child process failed")?);
                    if stream_closed {
                        break;
                    }
                }
                _ = stream_interval.tick(), if use_streaming => {
                    if let Some(reply) = live_reply.as_mut()
                        && let Some(content) = build_live_output(&live_display_chunks)
                    {
                        let _ = reply.update(&content).await;
                    }
                }
                _ = &mut timeout_fut, if exit_status.is_none() && !timed_out => {
                    timed_out = true;
                    tracing::warn!(
                        session_id = session.id,
                        timeout_secs = TURN_EXECUTION_TIMEOUT.as_secs(),
                        "chat bridge turn timed out; killing child process"
                    );
                    crate::process::kill_process_tree_best_effort(child_pid);
                }
                _ = typing_interval.tick() => {
                    let _ = adapter.send_typing(&msg.chat_id).await;
                }
            }
        }

        let _ = stdout_task.await;
        let _ = stderr_task.await;
        let status =
            exit_status.ok_or_else(|| anyhow::anyhow!("child process exited without status"))?;

        let file_output = if let Some(path) = invocation.final_output_path.as_ref() {
            tokio::fs::read_to_string(path).await.ok()
        } else {
            None
        };
        if let Some(path) = invocation.final_output_path.take() {
            let _ = tokio::fs::remove_file(path).await;
        }

        let final_text = compose_final_output(
            status.success(),
            timed_out,
            file_output.as_deref(),
            &stdout_buf,
            &stderr_buf,
        );
        if let Some(reply) = live_reply.as_mut() {
            reply.finish(&final_text).await?;
        } else {
            self.send_labeled_text(
                adapter.clone(),
                &msg.chat_id,
                &label,
                &final_text,
                msg.message_id.as_deref(),
            )
            .await?;
        }

        if matches!(session.permission_mode, storage::BridgePermissionMode::Safe)
            && let Some(notice) = build_safe_mode_notice(&stdout_buf, &stderr_buf)
        {
            self.send_labeled_text(
                adapter,
                &msg.chat_id,
                &format!("⚠️ {label}"),
                &notice,
                msg.message_id.as_deref(),
            )
            .await?;
        }

        Ok(TurnExecutionResult {
            success: status.success() && !timed_out,
            stdout: stdout_buf,
        })
    }

    pub(in crate::chat_bridge) async fn send_text(
        &self,
        adapter: Arc<dyn ChatAdapter>,
        chat_id: &str,
        text: &str,
        reply_to: Option<&str>,
    ) -> anyhow::Result<()> {
        for (index, chunk) in chunk_text(text, MESSAGE_CHAR_LIMIT).into_iter().enumerate() {
            let sent = OutgoingMessage {
                chat_id: chat_id.to_string(),
                content: chunk,
                reply_to: if index == 0 {
                    reply_to.map(str::to_string)
                } else {
                    None
                },
            };
            let _ = adapter.send_message(sent).await?;
        }
        Ok(())
    }

    pub(in crate::chat_bridge) async fn send_labeled_text(
        &self,
        adapter: Arc<dyn ChatAdapter>,
        chat_id: &str,
        label: &str,
        text: &str,
        reply_to: Option<&str>,
    ) -> anyhow::Result<()> {
        let max_body = MESSAGE_CHAR_LIMIT.saturating_sub(label.chars().count() + 1);
        let chunks = chunk_text(text, max_body.max(256));
        for (index, chunk) in chunks.into_iter().enumerate() {
            let title = if index == 0 {
                label.to_string()
            } else {
                format!("{label} (续 {})", index + 1)
            };
            let content = format!("{title}\n{chunk}");
            adapter
                .send_message(OutgoingMessage {
                    chat_id: chat_id.to_string(),
                    content,
                    reply_to: if index == 0 {
                        reply_to.map(str::to_string)
                    } else {
                        None
                    },
                })
                .await?;
        }
        Ok(())
    }
}
