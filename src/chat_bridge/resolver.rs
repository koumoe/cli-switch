use std::sync::Arc;

use super::ChatBridgeRuntime;
use super::adapter::{ChatAdapter, IncomingMessage};
use super::router::{ResolveTargetResult, format_ambiguous_target, resolve_target};
use crate::storage::{self, ChatPlatform};

impl ChatBridgeRuntime {
    pub(super) async fn resolve_active_session_or_reply(
        &self,
        adapter: Arc<dyn ChatAdapter>,
        msg: &IncomingMessage,
        platform: ChatPlatform,
        target: &str,
    ) -> anyhow::Result<Option<storage::BridgeSession>> {
        let sessions =
            storage::list_bridge_sessions_for_platform(self.db_path.clone(), platform, true)
                .await?;
        match resolve_target(&sessions, target) {
            ResolveTargetResult::Exact(session) => Ok(Some(session)),
            ResolveTargetResult::Ambiguous(items) => {
                self.send_text(
                    adapter,
                    &msg.chat_id,
                    &format_ambiguous_target(target, &items),
                    msg.message_id.as_deref(),
                )
                .await?;
                Ok(None)
            }
            ResolveTargetResult::NotFound => {
                self.send_text(
                    adapter,
                    &msg.chat_id,
                    "未找到匹配的会话，使用 /sessions 查看可用会话。",
                    msg.message_id.as_deref(),
                )
                .await?;
                Ok(None)
            }
        }
    }
}
