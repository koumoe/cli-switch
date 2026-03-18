use std::sync::Arc;

use super::ChatBridgeRuntime;
use super::adapter::{ChatAdapter, IncomingMessage};
use super::output::redact_sensitive_text;
use super::router::{Command, ParsedInput, parse_input};
use crate::storage::{self, StorageError};

impl ChatBridgeRuntime {
    pub(super) async fn handle_message(&self, adapter: Arc<dyn ChatAdapter>, msg: IncomingMessage) {
        if let Err(err) = self
            .handle_message_inner(adapter.clone(), msg.clone())
            .await
        {
            tracing::warn!(
                platform = %msg.platform.as_str(),
                sender_id = %msg.sender_id,
                chat_id = %msg.chat_id,
                err = %format!("{err:#}"),
                "chat bridge message handling failed"
            );
            let user_text = super::render_user_error(&err);
            let _ = self
                .send_text(
                    adapter,
                    &msg.chat_id,
                    &format!("❌ {user_text}"),
                    msg.message_id.as_deref(),
                )
                .await;
        }
    }

    async fn handle_message_inner(
        &self,
        adapter: Arc<dyn ChatAdapter>,
        msg: IncomingMessage,
    ) -> anyhow::Result<()> {
        let parsed = parse_input(&msg.text);
        let binding = storage::resolve_chat_binding(
            self.db_path.clone(),
            msg.platform,
            msg.sender_id.clone(),
        )
        .await?;

        let Some(binding) = binding else {
            if let Ok(ParsedInput::Command(Command::Bind { token })) = parsed {
                return self.handle_bind(adapter, msg, token).await;
            }
            return Ok(());
        };

        let parsed = match parsed {
            Ok(value) => value,
            Err(err) => {
                self.send_text(
                    adapter,
                    &msg.chat_id,
                    &format!("❌ {err}"),
                    msg.message_id.as_deref(),
                )
                .await?;
                return Ok(());
            }
        };

        match parsed {
            ParsedInput::PlainText(text) => {
                self.record_audit_log(&msg, "chat", &text, None).await;
                self.route_to_default(adapter, msg, binding.platform, text)
                    .await?;
            }
            ParsedInput::Command(command) => {
                if matches!(command, Command::Bind { .. }) {
                    self.send_text(
                        adapter,
                        &msg.chat_id,
                        &format!("当前 {} 账号已绑定，无需重复 /bind。", msg.platform.label()),
                        msg.message_id.as_deref(),
                    )
                    .await?;
                    return Ok(());
                }
                self.handle_bound_command(adapter, msg, binding.platform, command)
                    .await?;
            }
        }

        Ok(())
    }

    async fn handle_bind(
        &self,
        adapter: Arc<dyn ChatAdapter>,
        msg: IncomingMessage,
        token: String,
    ) -> anyhow::Result<()> {
        let binding = storage::consume_pairing_token(
            self.db_path.clone(),
            token,
            msg.platform,
            msg.sender_id.clone(),
            msg.sender_display_name.clone(),
        )
        .await?;
        self.record_audit_log(&msg, "command", "/bind", None).await;
        let name = binding
            .display_name
            .unwrap_or_else(|| msg.sender_id.clone());
        self.send_text(
            adapter,
            &msg.chat_id,
            &format!(
                "✅ 绑定成功\n平台: {}\n账号: {name}\n\n输入 /projects 查看项目列表\n输入 /help 查看所有命令",
                binding.platform.label(),
            ),
            msg.message_id.as_deref(),
        )
        .await?;
        Ok(())
    }

    pub(super) async fn record_audit_log(
        &self,
        msg: &IncomingMessage,
        message_type: &str,
        content: &str,
        session_id: Option<i64>,
    ) {
        if let Err(err) = storage::create_chat_audit_log(
            self.db_path.clone(),
            storage::CreateChatAuditLogInput {
                platform: msg.platform,
                sender_id: msg.sender_id.clone(),
                chat_id: msg.chat_id.clone(),
                message_type: message_type.to_string(),
                content: redact_sensitive_text(&format_audit_log_content(msg, content)),
                session_id,
            },
        )
        .await
        {
            tracing::warn!(
                platform = %msg.platform.as_str(),
                err = %err,
                "create chat audit log failed"
            );
        }
    }

    pub(super) fn project_cache_key(
        &self,
        msg: &IncomingMessage,
    ) -> super::projects::ProjectSelectionCacheKey {
        super::projects::ProjectSelectionCacheKey {
            platform: msg.platform.as_str().to_string(),
            chat_id: msg.chat_id.clone(),
        }
    }
}

fn format_audit_log_content(msg: &IncomingMessage, content: &str) -> String {
    let mut lines = Vec::<String>::new();
    let trimmed = content.trim();
    if !trimmed.is_empty() {
        lines.push(trimmed.to_string());
    }
    if !msg.attachments.is_empty() {
        lines.push(format!(
            "attachments: {}",
            msg.attachments
                .iter()
                .map(|item| format!("[{}] {}", item.kind.label(), item.filename))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if lines.is_empty() {
        "(empty message)".to_string()
    } else {
        lines.join("\n")
    }
}

pub(super) fn is_chat_session_not_found(err: &anyhow::Error) -> bool {
    matches!(
        err.downcast_ref::<StorageError>(),
        Some(StorageError::ChatSessionNotFound { .. })
    )
}
