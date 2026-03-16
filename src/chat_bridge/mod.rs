pub mod adapter;
mod auth;
pub mod cli;
mod output;
mod projects;
mod resolver;
mod router;

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::sync::{Mutex, mpsc, watch};

use self::adapter::telegram::TelegramAdapter;
use self::adapter::{ChatAdapter, IncomingMessage};
#[cfg(test)]
use self::adapter::{OutgoingMessage, SentMessage, StreamingMessage};
use self::auth::is_chat_session_not_found;
use self::cli::{
    LEGACY_GEMINI_UNTRACKED_SESSION_REF, ValidateResult, adapter_for, cli_type_label,
    permission_mode_label,
};
#[cfg(test)]
use self::output::StreamingReply;
#[cfg(test)]
use self::output::extract_display_text;
use self::output::format_projects_list;
#[cfg(test)]
use self::projects::AggregatedProject;
use self::projects::ProjectStore;
use self::router::{Command, format_session_label, format_sessions_list, help_text};
use crate::cli_tools::CliToolId;
use crate::storage::{self, BridgeSessionStatus, ChatPlatform, StorageError};

const TELEGRAM_RESTART_DELAY: Duration = Duration::from_secs(5);
const TELEGRAM_POLL_ERROR_DELAY: Duration = Duration::from_secs(3);
const STREAM_UPDATE_INTERVAL: Duration = Duration::from_millis(1200);
const TYPING_INTERVAL: Duration = Duration::from_secs(4);
const TURN_EXECUTION_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const MESSAGE_CHAR_LIMIT: usize = 3900;
const MAX_DISPLAY_JSON_DEPTH: usize = 24;

pub async fn run_supervisor(
    db_path: PathBuf,
    http_client: reqwest::Client,
    mut settings_rx: watch::Receiver<Arc<storage::AppSettings>>,
) {
    let runtime = ChatBridgeRuntime {
        db_path,
        settings_rx: settings_rx.clone(),
        project_store: Arc::new(ProjectStore::new()),
        busy_sessions: Arc::new(Mutex::new(HashSet::new())),
    };

    let mut telegram_task: Option<(String, tokio::task::JoinHandle<()>)> = None;

    loop {
        let token = desired_telegram_token(settings_rx.borrow().as_ref());
        match (&token, telegram_task.as_ref()) {
            (Some(desired), Some((running, _))) if desired == running => {}
            (Some(_), Some((_running, handle))) => {
                handle.abort();
                telegram_task = None;
            }
            (None, Some((_running, handle))) => {
                handle.abort();
                telegram_task = None;
            }
            _ => {}
        }

        if let Some(token) = token
            && telegram_task.is_none()
        {
            let runtime = runtime.clone();
            let client = http_client.clone();
            let token_copy = token.clone();
            let handle = tokio::spawn(async move {
                run_telegram_bridge(runtime, client, token_copy).await;
            });
            telegram_task = Some((token, handle));
        }

        if let Some((_token, handle)) = telegram_task.as_mut() {
            tokio::select! {
                changed = settings_rx.changed() => {
                    if changed.is_err() {
                        break;
                    }
                }
                _ = handle => {
                    telegram_task = None;
                    tokio::time::sleep(TELEGRAM_RESTART_DELAY).await;
                }
            }
        } else if settings_rx.changed().await.is_err() {
            break;
        }
    }

    if let Some((_token, handle)) = telegram_task {
        handle.abort();
        let _ = handle.await;
    }
}

#[derive(Clone)]
struct ChatBridgeRuntime {
    db_path: PathBuf,
    settings_rx: watch::Receiver<Arc<storage::AppSettings>>,
    project_store: Arc<ProjectStore>,
    busy_sessions: Arc<Mutex<HashSet<i64>>>,
}

impl ChatBridgeRuntime {
    fn settings_snapshot(&self) -> Arc<storage::AppSettings> {
        self.settings_rx.borrow().clone()
    }

    async fn handle_bound_command(
        &self,
        adapter: Arc<dyn ChatAdapter>,
        msg: IncomingMessage,
        platform: ChatPlatform,
        command: Command,
    ) -> anyhow::Result<()> {
        match command {
            Command::Projects => {
                self.record_audit_log(&msg, "command", "/projects", None)
                    .await;
                let key = self.project_cache_key(&msg);
                let items = self
                    .project_store
                    .list_all_projects(self.db_path.clone())
                    .await?;
                self.project_store
                    .remember_snapshot(key, items.clone())
                    .await;
                let content = format_projects_list(&items);
                self.send_text(adapter, &msg.chat_id, &content, msg.message_id.as_deref())
                    .await?;
            }
            Command::Start {
                tool,
                project_ref,
                alias,
                permission_mode,
            } => {
                self.record_audit_log(&msg, "command", &msg.text, None)
                    .await;
                let settings = self.settings_snapshot();
                let key = self.project_cache_key(&msg);
                let project = self
                    .project_store
                    .resolve_project_ref(
                        self.db_path.clone(),
                        &key,
                        &project_ref,
                        settings.chat_bridge_allow_new_projects,
                    )
                    .await?;
                let session = storage::create_bridge_session(
                    self.db_path.clone(),
                    storage::CreateBridgeSessionInput {
                        platform,
                        alias,
                        cli_type: tool,
                        cli_session_ref: None,
                        project_id: Some(project.path.clone()),
                        project_name: project.display_name.clone(),
                        working_dir: project.path.clone(),
                        permission_mode,
                    },
                )
                .await?;
                let message = format!(
                    "✅ {} 会话已启动\n{}\n路径: {}\n模式: {}\n⭐ 默认",
                    cli_type_label(tool),
                    format_session_label(&session),
                    session.working_dir,
                    permission_mode_label(permission_mode)
                );
                self.send_text(adapter, &msg.chat_id, &message, msg.message_id.as_deref())
                    .await?;
            }
            Command::Chat { target, message } => {
                let Some(session) = self
                    .resolve_active_session_or_reply(adapter.clone(), &msg, platform, &target)
                    .await?
                else {
                    return Ok(());
                };
                self.record_audit_log(&msg, "chat", &message, Some(session.id))
                    .await;
                self.run_turn_for_session(adapter, msg, session, message)
                    .await?;
            }
            Command::Switch { target } => {
                self.record_audit_log(&msg, "command", &msg.text, None)
                    .await;
                let Some(session) = self
                    .resolve_active_session_or_reply(adapter.clone(), &msg, platform, &target)
                    .await?
                else {
                    return Ok(());
                };
                storage::set_default_bridge_session_for_platform(
                    self.db_path.clone(),
                    platform,
                    session.id,
                )
                .await?;
                self.send_text(
                    adapter,
                    &msg.chat_id,
                    &format!("✅ 默认会话已切换到 {}", format_session_label(&session)),
                    msg.message_id.as_deref(),
                )
                .await?;
            }
            Command::Sessions => {
                self.record_audit_log(&msg, "command", "/sessions", None)
                    .await;
                let sessions = storage::list_bridge_sessions_for_platform(
                    self.db_path.clone(),
                    platform,
                    true,
                )
                .await?;
                let content = format_sessions_list(&sessions, storage::now_ms());
                self.send_text(adapter, &msg.chat_id, &content, msg.message_id.as_deref())
                    .await?;
            }
            Command::Stop { target } => {
                self.record_audit_log(&msg, "command", &msg.text, None)
                    .await;
                let Some(session) = self
                    .resolve_active_session_or_reply(adapter.clone(), &msg, platform, &target)
                    .await?
                else {
                    return Ok(());
                };
                let stopped =
                    storage::stop_bridge_session(self.db_path.clone(), session.id).await?;
                self.send_text(
                    adapter,
                    &msg.chat_id,
                    &format!("✅ 已停止 {}", format_session_label(&stopped)),
                    msg.message_id.as_deref(),
                )
                .await?;
            }
            Command::StopAll => {
                self.record_audit_log(&msg, "command", "/stop all", None)
                    .await;
                let count =
                    storage::stop_all_bridge_sessions_for_platform(self.db_path.clone(), platform)
                        .await?;
                self.send_text(
                    adapter,
                    &msg.chat_id,
                    &format!("✅ 已停止 {count} 个会话"),
                    msg.message_id.as_deref(),
                )
                .await?;
            }
            Command::Help => {
                self.record_audit_log(&msg, "command", "/help", None).await;
                self.send_text(
                    adapter,
                    &msg.chat_id,
                    &help_text(),
                    msg.message_id.as_deref(),
                )
                .await?;
            }
            Command::Bind { .. } => {
                tracing::error!(
                    platform = %platform.as_str(),
                    sender_id = %msg.sender_id,
                    chat_id = %msg.chat_id,
                    "bound user /bind reached handle_bound_command unexpectedly"
                );
                self.send_text(
                    adapter,
                    &msg.chat_id,
                    "当前账号已经绑定。如需重新绑定，请先在设置中解绑后再生成新的配对 Token。",
                    msg.message_id.as_deref(),
                )
                .await?;
            }
        }

        Ok(())
    }

    async fn route_to_default(
        &self,
        adapter: Arc<dyn ChatAdapter>,
        msg: IncomingMessage,
        platform: ChatPlatform,
        text: String,
    ) -> anyhow::Result<()> {
        let session =
            storage::get_default_bridge_session_for_platform(self.db_path.clone(), platform)
                .await?;
        let Some(session) = session else {
            self.send_text(
                adapter,
                &msg.chat_id,
                "还没有启动任何代理，使用 /codex、/claude 或 /gemini 启动一个。",
                msg.message_id.as_deref(),
            )
            .await?;
            return Ok(());
        };

        self.run_turn_for_session(adapter, msg, session, text).await
    }

    async fn run_turn_for_session(
        &self,
        adapter: Arc<dyn ChatAdapter>,
        msg: IncomingMessage,
        session: storage::BridgeSession,
        input: String,
    ) -> anyhow::Result<()> {
        if !self.try_mark_busy(session.id).await {
            self.send_text(
                adapter,
                &msg.chat_id,
                &format!(
                    "{} 当前已有任务在运行，请稍后再试。",
                    format_session_label(&session)
                ),
                msg.message_id.as_deref(),
            )
            .await?;
            return Ok(());
        }

        let result = self
            .run_turn_for_session_inner(adapter.clone(), &msg, session.clone(), input)
            .await;
        self.clear_busy(session.id).await;
        result
    }

    async fn run_turn_for_session_inner(
        &self,
        adapter: Arc<dyn ChatAdapter>,
        msg: &IncomingMessage,
        mut session: storage::BridgeSession,
        input: String,
    ) -> anyhow::Result<()> {
        let settings = self.settings_snapshot();
        let active_session_count = storage::count_active_bridge_sessions_for_platform(
            self.db_path.clone(),
            session.platform,
        )
        .await?;
        let use_streaming = should_stream_live_output(session.platform, active_session_count);

        let start_ms = storage::now_ms();
        storage::update_bridge_session(
            self.db_path.clone(),
            session.id,
            storage::UpdateBridgeSessionInput {
                status: Some(BridgeSessionStatus::Running),
                last_active_ms: Some(start_ms),
                ..Default::default()
            },
        )
        .await?;

        let cli_adapter = adapter_for(session.cli_type);
        let mut resume_existing = session.cli_session_ref.is_some();
        let mut corrected_session_ref = None::<String>;
        let mut generated_session_ref = None::<String>;

        if session.cli_type == CliToolId::Claude && session.cli_session_ref.is_none() {
            let generated = uuid::Uuid::new_v4().to_string();
            session.cli_session_ref = Some(generated.clone());
            generated_session_ref = Some(generated);
            resume_existing = false;
        }

        if resume_existing {
            match cli_adapter
                .validate_session_ref(&session, settings.as_ref())
                .await?
            {
                ValidateResult::Valid => {}
                ValidateResult::Corrected(corrected) => {
                    corrected_session_ref = Some(corrected.clone());
                    session.cli_session_ref = Some(corrected);
                }
                ValidateResult::Invalid(reason) => {
                    self.restore_session_after_turn(session.id, None).await;
                    self.send_text(
                        adapter,
                        &msg.chat_id,
                        &format!("❌ {reason}"),
                        msg.message_id.as_deref(),
                    )
                    .await?;
                    return Ok(());
                }
            }
        }

        let invocation = cli_adapter.build_invocation(
            &input,
            &session,
            settings.as_ref(),
            use_streaming,
            resume_existing,
        )?;

        let execution = self
            .execute_turn_process(adapter.clone(), msg, &session, invocation, use_streaming)
            .await;

        let maybe_session_ref = match &execution {
            Ok(result) if result.success => generated_session_ref
                .or(corrected_session_ref.clone())
                .or_else(|| cli_adapter.extract_session_ref(&result.stdout))
                .or_else(|| {
                    if session.cli_type == CliToolId::Gemini && session.cli_session_ref.is_none() {
                        Some(LEGACY_GEMINI_UNTRACKED_SESSION_REF.to_string())
                    } else {
                        None
                    }
                }),
            _ => corrected_session_ref,
        };

        self.restore_session_after_turn(session.id, maybe_session_ref)
            .await;
        execution.map(|_| ())
    }

    async fn restore_session_after_turn(&self, session_id: i64, session_ref: Option<String>) {
        let status = match storage::get_bridge_session(self.db_path.clone(), session_id).await {
            Ok(session) if session.status != BridgeSessionStatus::Stopped => {
                Some(BridgeSessionStatus::Idle)
            }
            Ok(_) => None,
            Err(err) if is_chat_session_not_found(&err) => None,
            Err(err) => {
                tracing::warn!(
                    session_id,
                    err = %err,
                    "read bridge session before restore failed; forcing idle status update"
                );
                Some(BridgeSessionStatus::Idle)
            }
        };
        let patch = storage::UpdateBridgeSessionInput {
            cli_session_ref: session_ref.map(Some),
            status,
            last_active_ms: Some(storage::now_ms()),
            ..Default::default()
        };
        if let Err(err) =
            storage::update_bridge_session(self.db_path.clone(), session_id, patch).await
        {
            tracing::warn!(session_id, err = %err, "restore bridge session after turn failed");
        }
    }

    async fn try_mark_busy(&self, session_id: i64) -> bool {
        let mut busy = self.busy_sessions.lock().await;
        if busy.contains(&session_id) {
            return false;
        }
        busy.insert(session_id);
        true
    }

    async fn clear_busy(&self, session_id: i64) {
        self.busy_sessions.lock().await.remove(&session_id);
    }
}

#[derive(Clone, Copy)]
enum StreamKind {
    Stdout,
    Stderr,
}

struct StreamChunk {
    kind: StreamKind,
    content: String,
}

async fn run_telegram_bridge(runtime: ChatBridgeRuntime, client: reqwest::Client, token: String) {
    let adapter = TelegramAdapter::new(client, token);
    let mut poller = adapter.poller();
    loop {
        match poller.prepare_for_polling().await {
            Ok(Some(message)) => {
                let runtime = runtime.clone();
                let sender: Arc<dyn ChatAdapter> = Arc::new(adapter.clone());
                tokio::spawn(async move {
                    runtime.handle_message(sender, message).await;
                });
                break;
            }
            Ok(None) => break,
            Err(err) => {
                tracing::warn!(err = %err, "telegram poller prepare failed");
                tokio::time::sleep(TELEGRAM_POLL_ERROR_DELAY).await;
            }
        }
    }

    loop {
        match poller.poll_updates().await {
            Ok(messages) => {
                for message in messages {
                    let runtime = runtime.clone();
                    let sender: Arc<dyn ChatAdapter> = Arc::new(adapter.clone());
                    tokio::spawn(async move {
                        runtime.handle_message(sender, message).await;
                    });
                }
            }
            Err(err) => {
                tracing::warn!(err = %err, "telegram poll loop failed");
                tokio::time::sleep(TELEGRAM_POLL_ERROR_DELAY).await;
            }
        }
    }
}

async fn read_stream<R>(reader: R, kind: StreamKind, tx: mpsc::UnboundedSender<StreamChunk>)
where
    R: AsyncRead + Unpin,
{
    let mut lines = BufReader::new(reader).lines();
    loop {
        match lines.next_line().await {
            Ok(Some(line)) => {
                let _ = tx.send(StreamChunk {
                    kind,
                    content: format!("{line}\n"),
                });
            }
            Ok(None) => break,
            Err(err) => {
                tracing::warn!(err = %err, "read child process stream failed");
                break;
            }
        }
    }
}

fn should_stream_live_output(platform: ChatPlatform, active_session_count: i64) -> bool {
    platform != ChatPlatform::WhatsApp && active_session_count <= 1
}

fn desired_telegram_token(settings: &storage::AppSettings) -> Option<String> {
    if !settings.chat_bridge_enabled || !settings.chat_bridge_telegram_enabled {
        return None;
    }
    let token = settings.chat_bridge_telegram_bot_token.as_deref()?.trim();
    (!token.is_empty()).then_some(token.to_string())
}

fn render_user_error(err: &anyhow::Error) -> String {
    match err.downcast_ref::<StorageError>() {
        Some(StorageError::ChatPairingTokenInvalid) => "配对 Token 无效".to_string(),
        Some(StorageError::ChatPairingTokenExpired) => "配对 Token 已过期".to_string(),
        Some(StorageError::ChatPairingTokenUsed) => "配对 Token 已被使用".to_string(),
        Some(StorageError::ChatPairingTokenPlatformMismatch {
            expected_platform,
            actual_platform,
        }) => format!("该配对 Token 仅能用于 {expected_platform}，当前消息来自 {actual_platform}"),
        Some(StorageError::ChatBindingAlreadyExists { .. }) => {
            "当前平台已绑定其他账号，请先在设置中解绑后再重新绑定".to_string()
        }
        Some(StorageError::ChatSessionAliasExists { alias, .. }) => {
            format!("会话别名已存在：{alias}")
        }
        Some(StorageError::ChatSessionNotFound { session_id }) => {
            format!("会话不存在：#{session_id}")
        }
        Some(StorageError::ChatProjectPathNotFound { path }) => format!("项目路径不存在：{path}"),
        _ => err.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc as StdArc, Mutex as StdMutex};

    #[derive(Clone)]
    struct FakeAdapter {
        native_streaming: bool,
        calls: StdArc<StdMutex<Vec<String>>>,
    }

    #[test]
    fn format_projects_list_uses_bracketed_index_and_path_only() {
        let rendered = format_projects_list(&[AggregatedProject {
            path: "/tmp/demo".to_string(),
            display_name: "DemoProject".to_string(),
            updated_at_ms: 0,
        }]);

        assert!(rendered.contains("[1001] DemoProject (/tmp/demo)"));
        assert!(!rendered.contains("claude"));
        assert!(!rendered.contains("codex"));
    }

    #[test]
    fn extract_display_text_parses_pretty_json_payloads() {
        let raw = r#"
{
  "response": {
    "candidates": [
      {
        "content": {
          "parts": [
            {
              "text": "Gemini final answer"
            }
          ]
        }
      }
    ]
  }
}
"#;

        assert_eq!(extract_display_text(raw, true), "Gemini final answer");
    }

    #[test]
    fn extract_display_text_ignores_message_history_when_response_exists() {
        let raw = r#"
{
  "messages": [
    {"role": "user", "content": "user prompt"},
    {"role": "assistant", "content": "old answer"}
  ],
  "response": {
    "candidates": [
      {
        "content": {
          "parts": [
            {
              "text": "Newest answer"
            }
          ]
        }
      }
    ]
  }
}
"#;

        assert_eq!(extract_display_text(raw, true), "Newest answer");
    }

    #[test]
    fn streaming_policy_matches_p1_rules() {
        assert!(should_stream_live_output(ChatPlatform::Telegram, 1));
        assert!(!should_stream_live_output(ChatPlatform::Telegram, 2));
        assert!(!should_stream_live_output(ChatPlatform::WhatsApp, 1));
    }

    impl FakeAdapter {
        fn new(native_streaming: bool) -> Self {
            Self {
                native_streaming,
                calls: StdArc::new(StdMutex::new(Vec::new())),
            }
        }

        fn record(&self, item: impl Into<String>) {
            self.calls.lock().expect("lock calls").push(item.into());
        }

        fn calls(&self) -> Vec<String> {
            self.calls.lock().expect("lock calls").clone()
        }
    }

    #[async_trait::async_trait]
    impl ChatAdapter for FakeAdapter {
        async fn send_message(&self, msg: OutgoingMessage) -> anyhow::Result<SentMessage> {
            self.record(format!("send:{}", msg.content));
            Ok(SentMessage {
                message_id: "sent-message".to_string(),
            })
        }

        async fn edit_message(
            &self,
            _chat_id: &str,
            _message_id: &str,
            content: &str,
        ) -> anyhow::Result<()> {
            self.record(format!("edit:{content}"));
            Ok(())
        }

        async fn begin_streaming_message(
            &self,
            msg: OutgoingMessage,
        ) -> anyhow::Result<Option<StreamingMessage>> {
            if !self.native_streaming {
                return Ok(None);
            }
            self.record(format!("draft:{}", msg.content));
            Ok(Some(StreamingMessage {
                id: "draft-1".to_string(),
            }))
        }

        async fn update_streaming_message(
            &self,
            _chat_id: &str,
            _stream: &StreamingMessage,
            content: &str,
        ) -> anyhow::Result<()> {
            self.record(format!("draft-update:{content}"));
            Ok(())
        }

        async fn finalize_streaming_message(
            &self,
            _stream: StreamingMessage,
            msg: OutgoingMessage,
        ) -> anyhow::Result<SentMessage> {
            self.record(format!("draft-finish:{}", msg.content));
            Ok(SentMessage {
                message_id: "final-message".to_string(),
            })
        }

        async fn send_typing(&self, _chat_id: &str) -> anyhow::Result<()> {
            Ok(())
        }

        fn platform(&self) -> ChatPlatform {
            ChatPlatform::Telegram
        }
    }

    #[tokio::test]
    async fn streaming_reply_uses_native_draft_streaming_when_available() {
        let adapter = FakeAdapter::new(true);
        let adapter_trait: Arc<dyn ChatAdapter> = Arc::new(adapter.clone());
        let mut reply = StreamingReply::new(
            adapter_trait,
            "chat-1".to_string(),
            Some("reply-1".to_string()),
            "label".to_string(),
        );

        reply.update("partial one").await.expect("draft update");
        reply.update("partial two").await.expect("draft update");
        reply.finish("final answer").await.expect("draft finish");

        assert_eq!(
            adapter.calls(),
            vec![
                "draft:label\npartial one".to_string(),
                "draft-update:label\npartial two".to_string(),
                "draft-finish:label\nfinal answer".to_string(),
            ]
        );
    }

    #[tokio::test]
    async fn streaming_reply_falls_back_to_send_and_edit_without_native_streaming() {
        let adapter = FakeAdapter::new(false);
        let adapter_trait: Arc<dyn ChatAdapter> = Arc::new(adapter.clone());
        let mut reply = StreamingReply::new(
            adapter_trait,
            "chat-1".to_string(),
            Some("reply-1".to_string()),
            "label".to_string(),
        );

        reply.update("partial one").await.expect("send");
        reply.update("partial two").await.expect("edit");
        reply.finish("final answer").await.expect("finish");

        assert_eq!(
            adapter.calls(),
            vec![
                "send:label\npartial one".to_string(),
                "edit:label\npartial two".to_string(),
                "edit:label\nfinal answer".to_string(),
            ]
        );
    }
}
