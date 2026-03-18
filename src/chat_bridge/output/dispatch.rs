use anyhow::Context as _;
use std::sync::Arc;

use super::super::adapter::{
    Attachment, ChatAdapter, IncomingMessage, OutgoingMessage, SentMessage, StreamingMessage,
};
use super::super::cli::{self, cli_type_label};
use super::super::router::format_session_label;
use super::super::{
    ActiveTurnRegistration, ChatBridgeRuntime, MESSAGE_CHAR_LIMIT, STREAM_UPDATE_INTERVAL,
    StreamChunk, StreamKind, TURN_EXECUTION_TIMEOUT, TYPING_INTERVAL, read_stream,
};
use super::format::{
    RenderedMessage, build_live_output, chunk_text, compose_final_output, format_streaming_message,
    render_chat_message, render_redacted_chat_message,
};
use super::notice::build_safe_mode_notice;
use super::parse::append_display_chunks_from_raw;
use super::redact_sensitive_text;
use crate::storage;

const LONG_OUTPUT_PREVIEW_CHARS: usize = 2_200;
const PROCESSING_NOTICE_TEXT: &str = "正在输入中，请稍等……";
const ATTACHMENT_NOTICE_FOOTER: &str = "\n\n完整输出已作为附件发送。";

#[derive(Clone, Copy)]
struct PlatformOutputPolicy {
    message_char_limit: usize,
    attachment_threshold: Option<usize>,
}

const DEFAULT_OUTPUT_POLICY: PlatformOutputPolicy = PlatformOutputPolicy {
    message_char_limit: MESSAGE_CHAR_LIMIT,
    attachment_threshold: None,
};

const TELEGRAM_OUTPUT_POLICY: PlatformOutputPolicy = PlatformOutputPolicy {
    message_char_limit: 3200,
    attachment_threshold: Some(8_000),
};

const DISCORD_OUTPUT_POLICY: PlatformOutputPolicy = PlatformOutputPolicy {
    message_char_limit: 1900,
    attachment_threshold: Some(6_000),
};

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
    #[cfg(test)]
    pub(in crate::chat_bridge) fn new(
        adapter: Arc<dyn ChatAdapter>,
        chat_id: String,
        reply_to: Option<String>,
        initial_label: String,
    ) -> Self {
        Self::with_pending_message(adapter, chat_id, reply_to, initial_label, None)
    }

    pub(in crate::chat_bridge) fn with_pending_message(
        adapter: Arc<dyn ChatAdapter>,
        chat_id: String,
        reply_to: Option<String>,
        initial_label: String,
        pending_message: Option<SentMessage>,
    ) -> Self {
        Self {
            adapter,
            chat_id,
            reply_to,
            label: initial_label,
            message: pending_message,
            stream: None,
            last_sent: String::new(),
        }
    }

    fn outgoing_message(&self, content: String) -> OutgoingMessage {
        OutgoingMessage {
            chat_id: self.chat_id.clone(),
            content,
            reply_to: self.reply_to.clone(),
            parse_mode: super::super::adapter::ParseMode::PlainText,
            attachments: Vec::new(),
        }
    }

    fn render_content(&self, body: &str) -> String {
        format_streaming_message(
            &self.label,
            body,
            message_char_limit(self.adapter.platform()),
        )
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
        if let Some(sent) = self.message.take() {
            if let Some(content) =
                build_single_labeled_content(self.adapter.platform(), &self.label, body)
                && content == self.last_sent
            {
                self.message = Some(sent);
                return Ok(());
            }
            send_or_replace_labeled_text(
                &self.adapter,
                &self.chat_id,
                &self.label,
                body,
                self.reply_to.as_deref(),
                Some(&sent.message_id),
            )
            .await?;
            return Ok(());
        }

        if let Some(stream) = self.stream.take() {
            let rendered = self.render_content(body);
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
            .send_message(self.outgoing_message(self.render_content(body)))
            .await?;
        self.message = Some(sent);
        Ok(())
    }

    pub(in crate::chat_bridge) async fn clear_progress_message(&mut self) -> anyhow::Result<()> {
        if let Some(sent) = self.message.take() {
            self.adapter
                .delete_message(&self.chat_id, &sent.message_id)
                .await?;
        }
        self.stream = None;
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
        mut active_turn: ActiveTurnRegistration,
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
        active_turn
            .child_pid
            .store(child_pid, std::sync::atomic::Ordering::Relaxed);
        let mut wait_fut = Box::pin(child.wait());
        let mut exit_status = None;
        let mut stream_closed = false;
        let mut timed_out = false;
        let mut cancelled = *active_turn.cancel_rx.borrow();
        if cancelled && child_pid != 0 {
            crate::process::kill_process_tree_best_effort(child_pid);
        }

        let mut typing_interval = tokio::time::interval(TYPING_INTERVAL);
        typing_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut stream_interval = tokio::time::interval(STREAM_UPDATE_INTERVAL);
        stream_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut timeout_fut = Box::pin(tokio::time::sleep(TURN_EXECUTION_TIMEOUT));

        let mut pending_message = if cancelled {
            None
        } else {
            match send_processing_notice(&adapter, &msg.chat_id, msg.message_id.as_deref()).await {
                Ok(sent) => Some(sent),
                Err(err) => {
                    tracing::warn!(
                        chat_id = %msg.chat_id,
                        err = %err,
                        "send processing notice failed"
                    );
                    None
                }
            }
        };

        let mut live_reply = if use_streaming {
            Some(StreamingReply::with_pending_message(
                adapter.clone(),
                msg.chat_id.clone(),
                msg.message_id.clone(),
                label.clone(),
                pending_message.take(),
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
                changed = active_turn.cancel_rx.changed(), if exit_status.is_none() && !cancelled => {
                    if changed.is_ok() && *active_turn.cancel_rx.borrow() {
                        cancelled = true;
                        if child_pid != 0 {
                            tracing::info!(session_id = session.id, child_pid, "chat bridge turn cancelled");
                            crate::process::kill_process_tree_best_effort(child_pid);
                        }
                    }
                }
                _ = stream_interval.tick(), if use_streaming && !cancelled => {
                    if let Some(reply) = live_reply.as_mut()
                        && let Some(content) = build_live_output(&live_display_chunks)
                    {
                        let _ = reply.update(&content).await;
                    }
                }
                _ = &mut timeout_fut, if exit_status.is_none() && !timed_out && !cancelled => {
                    timed_out = true;
                    tracing::warn!(
                        session_id = session.id,
                        timeout_secs = TURN_EXECUTION_TIMEOUT.as_secs(),
                        "chat bridge turn timed out; killing child process"
                    );
                    crate::process::kill_process_tree_best_effort(child_pid);
                }
                _ = typing_interval.tick(), if !cancelled => {
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

        if cancelled {
            if let Some(reply) = live_reply.as_mut() {
                let _ = reply.clear_progress_message().await;
            } else if let Some(sent) = pending_message.as_ref() {
                delete_message_best_effort(&adapter, &msg.chat_id, &sent.message_id).await;
            }
            return Ok(TurnExecutionResult {
                success: false,
                stdout: stdout_buf,
            });
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
        for (index, chunk) in chunk_text(text, message_char_limit(adapter.platform()))
            .into_iter()
            .enumerate()
        {
            let _ = send_formatted_message(
                &adapter,
                chat_id,
                &chunk,
                if index == 0 { reply_to } else { None },
                Vec::new(),
            )
            .await?;
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
        send_or_replace_labeled_text(&adapter, chat_id, label, text, reply_to, None).await
    }
}

async fn send_processing_notice(
    adapter: &Arc<dyn ChatAdapter>,
    chat_id: &str,
    reply_to: Option<&str>,
) -> anyhow::Result<SentMessage> {
    send_formatted_message(
        adapter,
        chat_id,
        PROCESSING_NOTICE_TEXT,
        reply_to,
        Vec::new(),
    )
    .await
}

async fn delete_message_best_effort(
    adapter: &Arc<dyn ChatAdapter>,
    chat_id: &str,
    message_id: &str,
) {
    if let Err(err) = adapter.delete_message(chat_id, message_id).await {
        tracing::warn!(
            chat_id = %chat_id,
            message_id = %message_id,
            err = %err,
            "delete chat bridge message failed"
        );
    }
}

async fn send_or_replace_labeled_text(
    adapter: &Arc<dyn ChatAdapter>,
    chat_id: &str,
    label: &str,
    text: &str,
    reply_to: Option<&str>,
    replace_message_id: Option<&str>,
) -> anyhow::Result<()> {
    if let Some(content) = build_single_labeled_content(adapter.platform(), label, text) {
        if let Some(message_id) = replace_message_id {
            adapter.edit_message(chat_id, message_id, &content).await?;
        } else {
            let _ =
                send_formatted_message(adapter, chat_id, &content, reply_to, Vec::new()).await?;
        }
        return Ok(());
    }

    if let Some(message_id) = replace_message_id {
        delete_message_best_effort(adapter, chat_id, message_id).await;
    }

    if should_send_as_attachment(adapter.platform(), text) {
        let redacted = redact_sensitive_text(text);
        let preview_message = build_attachment_preview(adapter.platform(), label, &redacted);
        let file_name = output_attachment_filename(label);
        let rendered = render_redacted_chat_message(adapter.platform(), &preview_message);
        send_rendered_message(
            adapter,
            chat_id,
            rendered,
            reply_to,
            vec![Attachment {
                filename: file_name,
                mime_type: "text/plain; charset=utf-8".to_string(),
                data: format!("{label}\n{redacted}\n").into_bytes(),
            }],
        )
        .await?;
        return Ok(());
    }

    let limit = message_char_limit(adapter.platform());
    let max_body = limit.saturating_sub(label.chars().count() + 1);
    let chunks = chunk_text(text, max_body.max(256));
    for (index, chunk) in chunks.into_iter().enumerate() {
        let title = if index == 0 {
            label.to_string()
        } else {
            format!("{label} (续 {})", index + 1)
        };
        let content = format!("{title}\n{chunk}");
        let _ = send_formatted_message(
            adapter,
            chat_id,
            &content,
            if index == 0 { reply_to } else { None },
            Vec::new(),
        )
        .await?;
    }
    Ok(())
}

fn build_single_labeled_content(
    platform: storage::ChatPlatform,
    label: &str,
    text: &str,
) -> Option<String> {
    if should_send_as_attachment(platform, text) {
        return None;
    }
    let limit = message_char_limit(platform);
    let max_body = limit.saturating_sub(label.chars().count() + 1).max(256);
    let chunks = chunk_text(text, max_body);
    if chunks.len() != 1 {
        return None;
    }
    chunks
        .into_iter()
        .next()
        .map(|chunk| format!("{label}\n{chunk}"))
}

async fn send_formatted_message(
    adapter: &Arc<dyn ChatAdapter>,
    chat_id: &str,
    content: &str,
    reply_to: Option<&str>,
    attachments: Vec<Attachment>,
) -> anyhow::Result<SentMessage> {
    let rendered = render_chat_message(adapter.platform(), content);
    send_rendered_message(adapter, chat_id, rendered, reply_to, attachments).await
}

async fn send_rendered_message(
    adapter: &Arc<dyn ChatAdapter>,
    chat_id: &str,
    rendered: RenderedMessage,
    reply_to: Option<&str>,
    attachments: Vec<Attachment>,
) -> anyhow::Result<SentMessage> {
    adapter
        .send_message(OutgoingMessage {
            chat_id: chat_id.to_string(),
            content: rendered.content,
            reply_to: reply_to.map(str::to_string),
            parse_mode: rendered.parse_mode,
            attachments,
        })
        .await
}

fn message_char_limit(platform: storage::ChatPlatform) -> usize {
    output_policy(platform).message_char_limit
}

fn should_send_as_attachment(platform: storage::ChatPlatform, text: &str) -> bool {
    output_policy(platform)
        .attachment_threshold
        .is_some_and(|limit| text.chars().count() > limit)
}

fn output_policy(platform: storage::ChatPlatform) -> PlatformOutputPolicy {
    match platform {
        storage::ChatPlatform::Telegram => TELEGRAM_OUTPUT_POLICY,
        storage::ChatPlatform::Discord => DISCORD_OUTPUT_POLICY,
        storage::ChatPlatform::WhatsApp => DEFAULT_OUTPUT_POLICY,
    }
}

fn build_attachment_preview(platform: storage::ChatPlatform, label: &str, text: &str) -> String {
    let limit = message_char_limit(platform);
    let fixed_chars = label.chars().count() + 1 + ATTACHMENT_NOTICE_FOOTER.chars().count();
    let preview_budget = limit.saturating_sub(fixed_chars);
    let trimmed = text.trim();
    if preview_budget == 0 {
        return format!("{label}{ATTACHMENT_NOTICE_FOOTER}");
    }

    let total_chars = trimmed.chars().count();
    let mut preview_limit = LONG_OUTPUT_PREVIEW_CHARS.min(preview_budget);
    let mut truncated = total_chars > preview_limit;
    let ellipsis = "\n...\n";
    if truncated && preview_limit > ellipsis.chars().count() {
        preview_limit -= ellipsis.chars().count();
    } else if truncated {
        truncated = false;
        preview_limit = preview_budget;
    }

    let mut preview: String = trimmed.chars().take(preview_limit).collect();
    if truncated {
        preview.push_str(ellipsis);
    }

    format!("{label}\n{preview}{ATTACHMENT_NOTICE_FOOTER}")
}

fn output_attachment_filename(label: &str) -> String {
    let base = label
        .trim_matches(|ch| matches!(ch, '[' | ']'))
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>();
    let base = base.trim_matches('-');
    let name = if base.is_empty() {
        "chat-bridge-output"
    } else {
        base
    };
    let timestamp = storage::now_ms();
    format!("{name}-{timestamp}.txt")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attachment_preview_respects_platform_limit() {
        let label = "[#1|codex|demo]";
        let text = "a".repeat(20_000);
        let preview = build_attachment_preview(storage::ChatPlatform::Discord, label, &text);
        assert!(preview.chars().count() <= message_char_limit(storage::ChatPlatform::Discord));
    }
}
