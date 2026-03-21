use std::sync::Arc;

use super::ChatBridgeRuntime;
use super::adapter::{ChatAdapter, IncomingMessage};
use super::i18n::{args, t_args};
use super::router::{Command, ParsedInput, parse_input};
use crate::i18n::AppLocale;
use crate::storage::{self, StorageError};

impl ChatBridgeRuntime {
    pub(super) async fn handle_message(&self, adapter: Arc<dyn ChatAdapter>, msg: IncomingMessage) {
        let fallback_locale = self.settings_snapshot().ui_locale;
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
            let user_text = super::render_user_error(&err, fallback_locale);
            let _ = self
                .send_text(
                    adapter,
                    &msg.chat_id,
                    &format!("❌ {user_text}"),
                    msg.message_id.as_deref(),
                    fallback_locale,
                )
                .await;
        }
    }

    async fn handle_message_inner(
        &self,
        adapter: Arc<dyn ChatAdapter>,
        msg: IncomingMessage,
    ) -> anyhow::Result<()> {
        let settings_locale = self.settings_snapshot().ui_locale;
        let binding = storage::resolve_chat_binding(
            self.db_path.clone(),
            msg.platform,
            msg.sender_id.clone(),
        )
        .await?;

        let Some(binding) = binding else {
            let parsed = parse_input(&msg.text, settings_locale);
            if let Ok(ParsedInput::Command(Command::Bind { token })) = parsed {
                return self.handle_bind(adapter, msg, token, settings_locale).await;
            }
            return Ok(());
        };

        let locale = settings_locale;
        let parsed = parse_input(&msg.text, locale);
        let parsed = match parsed {
            Ok(value) => value,
            Err(err) => {
                self.send_text(
                    adapter,
                    &msg.chat_id,
                    &format!("❌ {err}"),
                    msg.message_id.as_deref(),
                    locale,
                )
                .await?;
                return Ok(());
            }
        };

        match parsed {
            ParsedInput::PlainText(text) => {
                self.route_to_default(adapter, msg, binding.platform, text, locale)
                    .await?;
            }
            ParsedInput::Command(command) => {
                if matches!(command, Command::Bind { .. }) {
                    self.send_text(
                        adapter,
                        &msg.chat_id,
                        &t_args(
                            locale,
                            "bind.already_bound",
                            &args([("platform", msg.platform.label().to_string())]),
                        ),
                        msg.message_id.as_deref(),
                        locale,
                    )
                    .await?;
                    return Ok(());
                }
                self.handle_bound_command(adapter, msg, binding.platform, command, locale)
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
        locale: AppLocale,
    ) -> anyhow::Result<()> {
        let binding = storage::consume_pairing_token(
            self.db_path.clone(),
            token,
            msg.platform,
            msg.sender_id.clone(),
            msg.sender_display_name.clone(),
            locale.as_str().to_string(),
        )
        .await?;
        let name = binding
            .display_name
            .unwrap_or_else(|| msg.sender_id.clone());
        self.send_text(
            adapter,
            &msg.chat_id,
            &t_args(
                locale,
                "bind.success",
                &args([
                    ("platform", binding.platform.label().to_string()),
                    ("account", name),
                ]),
            ),
            msg.message_id.as_deref(),
            locale,
        )
        .await?;
        Ok(())
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

pub(super) fn is_chat_session_not_found(err: &anyhow::Error) -> bool {
    matches!(
        err.downcast_ref::<StorageError>(),
        Some(StorageError::ChatSessionNotFound { .. })
    )
}
