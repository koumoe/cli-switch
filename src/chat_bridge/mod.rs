pub mod adapter;
mod auth;
pub mod cli;
mod i18n;
mod output;
mod projects;
mod resolver;
mod router;

use anyhow::Context as _;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::sync::{Mutex, mpsc, watch};

use self::adapter::discord::DiscordAdapter;
use self::adapter::telegram::TelegramAdapter;
use self::adapter::{ChatAdapter, IncomingAttachmentKind, IncomingMessage};
#[cfg(test)]
use self::adapter::{OutgoingMessage, SentMessage, StreamingMessage};
use self::auth::is_chat_session_not_found;
use self::cli::{
    LEGACY_GEMINI_UNTRACKED_SESSION_REF, ValidateResult, adapter_for, cli_type_label,
    permission_mode_label,
};
use self::i18n::{args, t, t_args};
#[cfg(test)]
use self::output::StreamingReply;
use self::output::TurnProcessContext;
#[cfg(test)]
use self::output::extract_display_text;
use self::output::format_projects_list;
#[cfg(test)]
use self::projects::AggregatedProject;
use self::projects::ProjectStore;
use self::router::{Command, format_session_label, format_sessions_list, help_text};
use crate::cli_tools::CliToolId;
use crate::i18n::{AppLocale, render_error};
use crate::storage::{self, BridgeSessionStatus, ChatPlatform, StorageError};

const STREAM_UPDATE_INTERVAL: Duration = Duration::from_millis(1200);
const TYPING_INTERVAL: Duration = Duration::from_secs(4);
const TURN_EXECUTION_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const MESSAGE_CHAR_LIMIT: usize = 3900;
const MAX_DISPLAY_JSON_DEPTH: usize = 24;

#[derive(Clone, Copy)]
struct RestartPolicy {
    base_delay: Duration,
    max_delay: Duration,
    reset_after: Duration,
}

#[derive(Clone, Copy)]
struct BackoffPolicy {
    base_delay: Duration,
    max_delay: Duration,
}

const CHAT_BRIDGE_RESTART_POLICY: RestartPolicy = RestartPolicy {
    base_delay: Duration::from_secs(5),
    max_delay: Duration::from_secs(60),
    reset_after: Duration::from_secs(60),
};

const CHAT_BRIDGE_POLL_ERROR_POLICY: BackoffPolicy = BackoffPolicy {
    base_delay: Duration::from_secs(3),
    max_delay: Duration::from_secs(30),
};

struct ManagedBridgeTask {
    platform_name: &'static str,
    token: Option<String>,
    started_at: Option<Instant>,
    handle: Option<tokio::task::JoinHandle<()>>,
    restart_failures: u32,
}

#[derive(Clone)]
struct ActiveTurnHandle {
    cancel_tx: watch::Sender<bool>,
    child_pid: Arc<AtomicU32>,
}

struct ActiveTurnRegistration {
    cancel_rx: watch::Receiver<bool>,
    child_pid: Arc<AtomicU32>,
}

impl ManagedBridgeTask {
    fn new(platform_name: &'static str) -> Self {
        Self {
            platform_name,
            token: None,
            started_at: None,
            handle: None,
            restart_failures: 0,
        }
    }

    fn is_running(&self) -> bool {
        self.handle.is_some()
    }

    fn sync_token(&mut self, desired: Option<&str>) {
        match (desired, self.token.as_deref()) {
            (Some(desired), Some(running)) if desired == running => {}
            (_, Some(_)) => self.abort_and_clear(),
            _ => {}
        }
    }

    fn spawn_if_needed(
        &mut self,
        desired_token: Option<String>,
        spawn: impl FnOnce(String) -> tokio::task::JoinHandle<()>,
    ) {
        if self.handle.is_some() {
            return;
        }
        let Some(token) = desired_token else {
            return;
        };
        let handle = spawn(token.clone());
        self.token = Some(token);
        self.started_at = Some(Instant::now());
        self.handle = Some(handle);
    }

    async fn wait_for_exit(&mut self) -> Option<(Duration, Result<(), tokio::task::JoinError>)> {
        let started_at = self.started_at?;
        let result = match self.handle.as_mut() {
            Some(handle) => handle.await,
            None => return None,
        };
        Some((started_at.elapsed(), result))
    }

    async fn handle_exit(&mut self, exit: Option<(Duration, Result<(), tokio::task::JoinError>)>) {
        let Some((uptime, result)) = exit else {
            return;
        };

        if let Err(err) = result {
            tracing::warn!(
                platform = self.platform_name,
                err = %err,
                "chat bridge task exited unexpectedly"
            );
        }

        self.handle = None;
        self.started_at = None;
        self.token = None;
        self.restart_failures = if uptime >= CHAT_BRIDGE_RESTART_POLICY.reset_after {
            1
        } else {
            self.restart_failures.saturating_add(1)
        };

        tokio::time::sleep(exponential_backoff_delay(
            self.restart_failures,
            CHAT_BRIDGE_RESTART_POLICY.base_delay,
            CHAT_BRIDGE_RESTART_POLICY.max_delay,
        ))
        .await;
    }

    fn abort_and_clear(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.abort();
        }
        self.token = None;
        self.started_at = None;
        self.restart_failures = 0;
    }

    async fn abort_and_join(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.abort();
            let _ = handle.await;
        }
        self.token = None;
        self.started_at = None;
    }
}

pub async fn run_supervisor(
    db_path: PathBuf,
    http_client: reqwest::Client,
    settings_tx: watch::Sender<Arc<storage::AppSettings>>,
    mut settings_rx: watch::Receiver<Arc<storage::AppSettings>>,
) {
    let runtime = ChatBridgeRuntime {
        db_path,
        settings_tx,
        settings_rx: settings_rx.clone(),
        project_store: Arc::new(ProjectStore::new()),
        busy_sessions: Arc::new(Mutex::new(HashSet::new())),
        active_turns: Arc::new(Mutex::new(HashMap::new())),
    };

    let mut telegram_task = ManagedBridgeTask::new("telegram");
    let mut discord_task = ManagedBridgeTask::new("discord");

    loop {
        let telegram_token = desired_telegram_token(settings_rx.borrow().as_ref());
        let discord_token = desired_discord_token(settings_rx.borrow().as_ref());

        telegram_task.sync_token(telegram_token.as_deref());
        discord_task.sync_token(discord_token.as_deref());

        if telegram_token.is_some() {
            let runtime = runtime.clone();
            let client = http_client.clone();
            telegram_task.spawn_if_needed(telegram_token.clone(), move |token| {
                tokio::spawn(async move {
                    run_telegram_bridge(runtime, client, token).await;
                })
            });
        }

        if discord_token.is_some() {
            let runtime = runtime.clone();
            let client = http_client.clone();
            discord_task.spawn_if_needed(discord_token.clone(), move |token| {
                tokio::spawn(async move {
                    run_discord_bridge(runtime, client, token).await;
                })
            });
        }

        let telegram_running = telegram_task.is_running();
        let discord_running = discord_task.is_running();
        if telegram_running || discord_running {
            tokio::select! {
                changed = settings_rx.changed() => {
                    if changed.is_err() {
                        break;
                    }
                }
                exit = telegram_task.wait_for_exit(), if telegram_running => {
                    telegram_task.handle_exit(exit).await;
                }
                exit = discord_task.wait_for_exit(), if discord_running => {
                    discord_task.handle_exit(exit).await;
                }
            }
        } else if settings_rx.changed().await.is_err() {
            break;
        }
    }

    telegram_task.abort_and_join().await;
    discord_task.abort_and_join().await;
}

#[derive(Clone)]
struct ChatBridgeRuntime {
    db_path: PathBuf,
    settings_tx: watch::Sender<Arc<storage::AppSettings>>,
    settings_rx: watch::Receiver<Arc<storage::AppSettings>>,
    project_store: Arc<ProjectStore>,
    busy_sessions: Arc<Mutex<HashSet<i64>>>,
    active_turns: Arc<Mutex<HashMap<i64, ActiveTurnHandle>>>,
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
        locale: AppLocale,
    ) -> anyhow::Result<()> {
        match command {
            Command::Projects => {
                let key = self.project_cache_key(&msg);
                let items = self
                    .project_store
                    .list_all_projects(self.db_path.clone())
                    .await?;
                self.project_store
                    .remember_snapshot(key, items.clone())
                    .await;
                let content = format_projects_list(&items, locale);
                self.send_text(
                    adapter,
                    &msg.chat_id,
                    &content,
                    msg.message_id.as_deref(),
                    locale,
                )
                .await?;
            }
            Command::Start {
                tool,
                project_ref,
                alias,
                permission_mode,
            } => {
                let settings = self.settings_snapshot();
                let key = self.project_cache_key(&msg);
                let project = self
                    .project_store
                    .resolve_project_ref(
                        self.db_path.clone(),
                        &key,
                        &project_ref,
                        settings.chat_bridge_allow_new_projects,
                        locale,
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
                let message = t_args(
                    locale,
                    "session.started",
                    &args([
                        ("cli_type", cli_type_label(tool).to_string()),
                        ("label", format_session_label(&session)),
                        ("path", session.working_dir.clone()),
                        ("mode", permission_mode_label(permission_mode).to_string()),
                    ]),
                );
                self.send_text(
                    adapter,
                    &msg.chat_id,
                    &message,
                    msg.message_id.as_deref(),
                    locale,
                )
                .await?;
            }
            Command::Chat { target, message } => {
                let Some(session) = self
                    .resolve_active_session_or_reply(
                        adapter.clone(),
                        &msg,
                        platform,
                        &target,
                        locale,
                    )
                    .await?
                else {
                    return Ok(());
                };
                self.run_turn_for_session(adapter, msg, session, message, locale)
                    .await?;
            }
            Command::Switch { target } => {
                let Some(session) = self
                    .resolve_active_session_or_reply(
                        adapter.clone(),
                        &msg,
                        platform,
                        &target,
                        locale,
                    )
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
                    &t_args(
                        locale,
                        "session.default_switched",
                        &args([("label", format_session_label(&session))]),
                    ),
                    msg.message_id.as_deref(),
                    locale,
                )
                .await?;
            }
            Command::Sessions => {
                let sessions = storage::list_bridge_sessions_for_platform(
                    self.db_path.clone(),
                    platform,
                    true,
                )
                .await?;
                let content = format_sessions_list(&sessions, storage::now_ms(), locale);
                self.send_text(
                    adapter,
                    &msg.chat_id,
                    &content,
                    msg.message_id.as_deref(),
                    locale,
                )
                .await?;
            }
            Command::Stop { target } => {
                let Some(session) = self
                    .resolve_active_session_or_reply(
                        adapter.clone(),
                        &msg,
                        platform,
                        &target,
                        locale,
                    )
                    .await?
                else {
                    return Ok(());
                };
                let cancelled = self.cancel_active_turn(session.id).await;
                let stopped =
                    storage::stop_bridge_session(self.db_path.clone(), session.id).await?;
                self.send_text(
                    adapter,
                    &msg.chat_id,
                    &if cancelled {
                        t_args(
                            locale,
                            "session.stopped_with_cancel",
                            &args([("label", format_session_label(&stopped))]),
                        )
                    } else {
                        t_args(
                            locale,
                            "session.stopped",
                            &args([("label", format_session_label(&stopped))]),
                        )
                    },
                    msg.message_id.as_deref(),
                    locale,
                )
                .await?;
            }
            Command::StopAll => {
                let sessions = storage::list_bridge_sessions_for_platform(
                    self.db_path.clone(),
                    platform,
                    true,
                )
                .await?;
                let mut cancelled = 0usize;
                for session in &sessions {
                    if self.cancel_active_turn(session.id).await {
                        cancelled += 1;
                    }
                }
                let count =
                    storage::stop_all_bridge_sessions_for_platform(self.db_path.clone(), platform)
                        .await?;
                self.send_text(
                    adapter,
                    &msg.chat_id,
                    &if cancelled > 0 {
                        t_args(
                            locale,
                            "session.stop_all_with_cancel",
                            &args([
                                ("count", count.to_string()),
                                ("cancelled", cancelled.to_string()),
                            ]),
                        )
                    } else {
                        t_args(
                            locale,
                            "session.stop_all",
                            &args([("count", count.to_string())]),
                        )
                    },
                    msg.message_id.as_deref(),
                    locale,
                )
                .await?;
            }
            Command::Lang { locale: next } => {
                if let Some(next) = next {
                    let next_locale = AppLocale::parse_or_default(&next);
                    let updated_settings = storage::update_app_settings(
                        self.db_path.clone(),
                        storage::AppSettingsPatch {
                            ui_locale: Some(next_locale),
                            ..Default::default()
                        },
                    )
                    .await?;
                    let _ = self.settings_tx.send(Arc::new(updated_settings.clone()));
                    self.send_text(
                        adapter,
                        &msg.chat_id,
                        &t_args(
                            updated_settings.ui_locale,
                            "lang.updated",
                            &args([("locale", updated_settings.ui_locale.as_str().to_string())]),
                        ),
                        msg.message_id.as_deref(),
                        updated_settings.ui_locale,
                    )
                    .await?;
                } else {
                    self.send_text(
                        adapter,
                        &msg.chat_id,
                        &t_args(
                            locale,
                            "lang.current",
                            &args([("locale", locale.as_str().to_string())]),
                        ),
                        msg.message_id.as_deref(),
                        locale,
                    )
                    .await?;
                }
            }
            Command::Help => {
                self.send_text(
                    adapter,
                    &msg.chat_id,
                    &help_text(locale),
                    msg.message_id.as_deref(),
                    locale,
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
                    &t(locale, "bind.rebind_required"),
                    msg.message_id.as_deref(),
                    locale,
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
        locale: AppLocale,
    ) -> anyhow::Result<()> {
        let session =
            storage::get_default_bridge_session_for_platform(self.db_path.clone(), platform)
                .await?;
        let Some(session) = session else {
            self.send_text(
                adapter,
                &msg.chat_id,
                &t(locale, "session.no_default"),
                msg.message_id.as_deref(),
                locale,
            )
            .await?;
            return Ok(());
        };

        self.run_turn_for_session(adapter, msg, session, text, locale)
            .await
    }

    async fn run_turn_for_session(
        &self,
        adapter: Arc<dyn ChatAdapter>,
        msg: IncomingMessage,
        session: storage::BridgeSession,
        input: String,
        locale: AppLocale,
    ) -> anyhow::Result<()> {
        if !self.try_mark_busy(session.id).await {
            self.send_text(
                adapter,
                &msg.chat_id,
                &t_args(
                    locale,
                    "session.busy",
                    &args([("label", format_session_label(&session))]),
                ),
                msg.message_id.as_deref(),
                locale,
            )
            .await?;
            return Ok(());
        }

        let active_turn = self.register_active_turn(session.id).await;
        let result = self
            .run_turn_for_session_inner(
                adapter.clone(),
                &msg,
                session.clone(),
                input,
                active_turn,
                locale,
            )
            .await;
        self.unregister_active_turn(session.id).await;
        self.clear_busy(session.id).await;
        result
    }

    async fn run_turn_for_session_inner(
        &self,
        adapter: Arc<dyn ChatAdapter>,
        msg: &IncomingMessage,
        mut session: storage::BridgeSession,
        input: String,
        active_turn: ActiveTurnRegistration,
        locale: AppLocale,
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
                .validate_session_ref(&session, settings.as_ref(), locale)
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
                        locale,
                    )
                    .await?;
                    return Ok(());
                }
            }
        }

        let prompt = self
            .build_turn_prompt(&session, msg, &input, locale)
            .await?;

        let invocation = cli_adapter.build_invocation(
            &prompt,
            &session,
            settings.as_ref(),
            use_streaming,
            resume_existing,
        )?;

        let execution = self
            .execute_turn_process(
                TurnProcessContext {
                    adapter: adapter.clone(),
                    msg,
                    use_streaming,
                    locale,
                },
                &session,
                invocation,
                active_turn,
            )
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

    async fn build_turn_prompt(
        &self,
        session: &storage::BridgeSession,
        msg: &IncomingMessage,
        input: &str,
        locale: AppLocale,
    ) -> anyhow::Result<String> {
        if !msg.has_attachments() {
            return Ok(input.to_string());
        }

        let materialized = self.materialize_incoming_attachments(session, msg).await?;
        if materialized.is_empty() {
            return Ok(input.to_string());
        }

        let mut lines = vec![
            t_args(
                locale,
                "turn.bridge_input_header",
                &args([("platform", msg.platform.label().to_string())]),
            ),
            String::new(),
        ];

        let trimmed = input.trim();
        if trimmed.is_empty() {
            lines.push(t(locale, "turn.attachments_only_notice"));
        } else {
            lines.push(t(locale, "turn.user_text_label"));
            lines.push(trimmed.to_string());
        }

        lines.push(String::new());
        lines.push(t(locale, "turn.attachments_label"));
        let normalized_user_text = normalized_prompt_text(trimmed);
        for (index, item) in materialized.iter().enumerate() {
            let caption = item
                .caption
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .filter(|value| should_include_attachment_caption(value, &normalized_user_text))
                .map(|value| format!(" | caption: {value}"))
                .unwrap_or_default();
            lines.push(format!(
                "{}. [{}] {} -> {}{}",
                index + 1,
                item.kind.label(),
                item.filename,
                item.path,
                caption
            ));
        }

        lines.push(String::new());
        lines.push(t(locale, "turn.attachments_instruction"));

        Ok(lines.join("\n"))
    }

    async fn materialize_incoming_attachments(
        &self,
        session: &storage::BridgeSession,
        msg: &IncomingMessage,
    ) -> anyhow::Result<Vec<MaterializedIncomingAttachment>> {
        let message_part = msg.message_id.as_deref().unwrap_or("no-message-id");
        let attachment_dir = self
            .data_dir()
            .join("chat-bridge")
            .join("inbox")
            .join(format!("session-{}", session.id))
            .join(format!(
                "{}-{}",
                msg.timestamp_ms.max(0),
                sanitize_fs_name(message_part)
            ));
        tokio::fs::create_dir_all(&attachment_dir)
            .await
            .with_context(|| {
                format!("create attachment dir failed: {}", attachment_dir.display())
            })?;

        let mut out = Vec::new();
        for (index, attachment) in msg.attachments.iter().enumerate() {
            let file_name = incoming_attachment_filename(index, attachment);
            let path = attachment_dir.join(&file_name);
            tokio::fs::write(&path, attachment.data.as_ref())
                .await
                .with_context(|| format!("write incoming attachment failed: {}", path.display()))?;
            out.push(MaterializedIncomingAttachment {
                kind: attachment.kind,
                filename: file_name,
                path: path.to_string_lossy().to_string(),
                caption: attachment.caption.clone(),
            });
        }
        Ok(out)
    }

    fn data_dir(&self) -> PathBuf {
        self.db_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."))
    }

    async fn register_active_turn(&self, session_id: i64) -> ActiveTurnRegistration {
        let (cancel_tx, cancel_rx) = watch::channel(false);
        let child_pid = Arc::new(AtomicU32::new(0));
        self.active_turns.lock().await.insert(
            session_id,
            ActiveTurnHandle {
                cancel_tx,
                child_pid: child_pid.clone(),
            },
        );
        ActiveTurnRegistration {
            cancel_rx,
            child_pid,
        }
    }

    async fn unregister_active_turn(&self, session_id: i64) {
        self.active_turns.lock().await.remove(&session_id);
    }

    async fn cancel_active_turn(&self, session_id: i64) -> bool {
        let handle = self.active_turns.lock().await.get(&session_id).cloned();
        let Some(handle) = handle else {
            return false;
        };

        let _ = handle.cancel_tx.send(true);
        let child_pid = handle.child_pid.load(Ordering::Relaxed);
        if child_pid != 0 {
            crate::process::kill_process_tree_best_effort(child_pid);
        }
        true
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

struct MaterializedIncomingAttachment {
    kind: IncomingAttachmentKind,
    filename: String,
    path: String,
    caption: Option<String>,
}

async fn run_telegram_bridge(runtime: ChatBridgeRuntime, client: reqwest::Client, token: String) {
    let adapter = TelegramAdapter::new(client, token);
    let mut poller = adapter.poller();
    let mut prepare_failures = 0u32;
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
                prepare_failures = prepare_failures.saturating_add(1);
                tracing::warn!(err = %err, "telegram poller prepare failed");
                tokio::time::sleep(exponential_backoff_delay(
                    prepare_failures,
                    CHAT_BRIDGE_POLL_ERROR_POLICY.base_delay,
                    CHAT_BRIDGE_POLL_ERROR_POLICY.max_delay,
                ))
                .await;
            }
        }
    }

    let mut poll_failures = 0u32;
    loop {
        match poller.poll_updates().await {
            Ok(messages) => {
                poll_failures = 0;
                for message in messages {
                    let runtime = runtime.clone();
                    let sender: Arc<dyn ChatAdapter> = Arc::new(adapter.clone());
                    tokio::spawn(async move {
                        runtime.handle_message(sender, message).await;
                    });
                }
            }
            Err(err) => {
                poll_failures = poll_failures.saturating_add(1);
                tracing::warn!(err = %err, "telegram poll loop failed");
                tokio::time::sleep(exponential_backoff_delay(
                    poll_failures,
                    CHAT_BRIDGE_POLL_ERROR_POLICY.base_delay,
                    CHAT_BRIDGE_POLL_ERROR_POLICY.max_delay,
                ))
                .await;
            }
        }
    }
}

async fn run_discord_bridge(runtime: ChatBridgeRuntime, client: reqwest::Client, token: String) {
    let adapter = DiscordAdapter::new(client, token);
    let mut poller = adapter.poller();
    let mut connect_failures = 0u32;

    loop {
        match poller.prepare_for_polling().await {
            Ok(()) => break,
            Err(err) => {
                connect_failures = connect_failures.saturating_add(1);
                tracing::warn!(err = %err, "discord poller prepare failed");
                tokio::time::sleep(exponential_backoff_delay(
                    connect_failures,
                    CHAT_BRIDGE_POLL_ERROR_POLICY.base_delay,
                    CHAT_BRIDGE_POLL_ERROR_POLICY.max_delay,
                ))
                .await;
            }
        }
    }

    let mut poll_failures = 0u32;
    loop {
        match poller.poll_updates().await {
            Ok(messages) => {
                poll_failures = 0;
                for message in messages {
                    let runtime = runtime.clone();
                    let sender: Arc<dyn ChatAdapter> = Arc::new(adapter.clone());
                    tokio::spawn(async move {
                        runtime.handle_message(sender, message).await;
                    });
                }
            }
            Err(err) => {
                poll_failures = poll_failures.saturating_add(1);
                tracing::warn!(err = %err, "discord poll loop failed");
                tokio::time::sleep(exponential_backoff_delay(
                    poll_failures,
                    CHAT_BRIDGE_POLL_ERROR_POLICY.base_delay,
                    CHAT_BRIDGE_POLL_ERROR_POLICY.max_delay,
                ))
                .await;
            }
        }
    }
}

fn exponential_backoff_delay(failures: u32, base: Duration, max: Duration) -> Duration {
    let exponent = failures.saturating_sub(1).min(10);
    let factor = 1u32 << exponent;
    base.checked_mul(factor)
        .map(|delay| delay.min(max))
        .unwrap_or(max)
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

fn desired_discord_token(settings: &storage::AppSettings) -> Option<String> {
    if !settings.chat_bridge_enabled || !settings.chat_bridge_discord_enabled {
        return None;
    }
    let token = settings.chat_bridge_discord_bot_token.as_deref()?.trim();
    (!token.is_empty()).then_some(token.to_string())
}

fn incoming_attachment_filename(
    index: usize,
    attachment: &self::adapter::IncomingAttachment,
) -> String {
    let sanitized = sanitize_fs_name(&attachment.filename);
    let base = sanitized.trim_matches('.');
    let inferred_ext = attachment
        .mime_type
        .as_deref()
        .and_then(mime_guess::get_mime_extensions_str)
        .and_then(|items| items.first().copied());
    if !base.is_empty() && Path::new(base).extension().is_some() {
        return format!("{:02}-{base}", index + 1);
    }
    let fallback_name = if base.is_empty() {
        format!("attachment-{}", index + 1)
    } else {
        base.to_string()
    };
    match inferred_ext {
        Some(ext) if !fallback_name.ends_with(&format!(".{ext}")) => {
            format!("{:02}-{}.{}", index + 1, fallback_name, ext)
        }
        _ => format!("{:02}-{fallback_name}", index + 1),
    }
}

fn sanitize_fs_name(raw: &str) -> String {
    let trimmed = raw.trim();
    let mut out = String::with_capacity(trimmed.len());
    for ch in trimmed.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_') {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    let collapsed = out.trim_matches('_').trim_matches('.');
    if collapsed.is_empty() {
        "attachment".to_string()
    } else {
        collapsed.to_string()
    }
}

fn should_include_attachment_caption(caption: &str, normalized_user_text: &str) -> bool {
    if normalized_user_text.is_empty() {
        return true;
    }
    normalized_prompt_text(caption) != normalized_user_text
}

fn normalized_prompt_text(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_lowercase()
}

fn render_user_error(err: &anyhow::Error, locale: AppLocale) -> String {
    match err.downcast_ref::<StorageError>() {
        Some(StorageError::ChatPairingTokenInvalid) => t(locale, "error.pairing_token_invalid"),
        Some(StorageError::ChatPairingTokenExpired) => t(locale, "error.pairing_token_expired"),
        Some(StorageError::ChatPairingTokenUsed) => t(locale, "error.pairing_token_used"),
        Some(StorageError::ChatPairingTokenPlatformMismatch {
            expected_platform,
            actual_platform,
        }) => t_args(
            locale,
            "error.pairing_token_platform_mismatch",
            &args([
                ("expected_platform", expected_platform.clone()),
                ("actual_platform", actual_platform.clone()),
            ]),
        ),
        Some(StorageError::ChatBindingAlreadyExists { .. }) => t(locale, "error.binding_exists"),
        Some(StorageError::ChatBindingNotFound { .. })
        | Some(StorageError::ChatBindingIdentityNotFound { .. }) => render_error(
            locale,
            "chat_bridge_binding_not_found",
            &std::collections::BTreeMap::new(),
            "Chat binding not found",
        ),
        Some(StorageError::ChatSessionAliasExists { alias, .. }) => t_args(
            locale,
            "error.session_alias_exists",
            &args([("alias", alias.clone())]),
        ),
        Some(StorageError::ChatSessionNotFound { session_id }) => t_args(
            locale,
            "error.session_not_found",
            &args([("session_id", session_id.to_string())]),
        ),
        Some(StorageError::ChatProjectPathNotFound { path }) => t_args(
            locale,
            "error.project_path_not_found",
            &args([("path", path.clone())]),
        ),
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

    fn temp_db_path() -> PathBuf {
        std::env::temp_dir().join(format!(
            "cliswitch-chat-bridge-runtime-test-{}.sqlite",
            uuid::Uuid::new_v4()
        ))
    }

    fn remove_sqlite_artifacts(path: &Path) {
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(PathBuf::from(format!("{}-wal", path.display())));
        let _ = std::fs::remove_file(PathBuf::from(format!("{}-shm", path.display())));
    }

    async fn bind_test_user(
        db_path: &Path,
        platform: ChatPlatform,
        platform_user_id: &str,
        locale: AppLocale,
    ) {
        let pairing = storage::create_pairing_token(
            db_path.to_path_buf(),
            storage::CreatePairingTokenInput {
                platform,
                expires_in_minutes: Some(5),
            },
        )
        .await
        .expect("create pairing token");

        storage::consume_pairing_token(
            db_path.to_path_buf(),
            pairing.token,
            platform,
            platform_user_id.to_string(),
            Some("@koumoe".to_string()),
            locale.as_str().to_string(),
        )
        .await
        .expect("consume pairing token");
    }

    async fn test_runtime(db_path: &Path) -> ChatBridgeRuntime {
        let settings = storage::get_app_settings(db_path.to_path_buf())
            .await
            .expect("load app settings");
        let (settings_tx, settings_rx) = watch::channel(Arc::new(settings));
        ChatBridgeRuntime {
            db_path: db_path.to_path_buf(),
            settings_tx,
            settings_rx,
            project_store: Arc::new(ProjectStore::new()),
            busy_sessions: Arc::new(Mutex::new(HashSet::new())),
            active_turns: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn test_message(platform: ChatPlatform, sender_id: &str, text: &str) -> IncomingMessage {
        IncomingMessage {
            platform,
            sender_id: sender_id.to_string(),
            sender_display_name: Some("@koumoe".to_string()),
            chat_id: "chat-1".to_string(),
            text: text.to_string(),
            attachments: Vec::new(),
            message_id: None,
            timestamp_ms: storage::now_ms(),
        }
    }

    #[test]
    fn format_projects_list_uses_bracketed_index_and_path_only() {
        let rendered = format_projects_list(
            &[AggregatedProject {
                path: "/tmp/demo".to_string(),
                display_name: "DemoProject".to_string(),
                updated_at_ms: 0,
            }],
            AppLocale::ZhCN,
        );

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
    fn attachment_caption_is_omitted_when_same_as_user_text() {
        let normalized = normalized_prompt_text("Fix   bug in   login");
        assert!(!should_include_attachment_caption(
            "  fix bug in login ",
            &normalized
        ));
    }

    #[test]
    fn attachment_caption_is_kept_when_different_from_user_text() {
        let normalized = normalized_prompt_text("Fix bug in login");
        assert!(should_include_attachment_caption(
            "screenshot: login error",
            &normalized
        ));
    }

    #[test]
    fn streaming_policy_matches_p1_rules() {
        assert!(should_stream_live_output(ChatPlatform::Telegram, 1));
        assert!(!should_stream_live_output(ChatPlatform::Telegram, 2));
        assert!(!should_stream_live_output(ChatPlatform::WhatsApp, 1));
    }

    #[test]
    fn exponential_backoff_delay_grows_and_caps() {
        assert_eq!(
            exponential_backoff_delay(1, Duration::from_secs(3), Duration::from_secs(30)),
            Duration::from_secs(3)
        );
        assert_eq!(
            exponential_backoff_delay(2, Duration::from_secs(3), Duration::from_secs(30)),
            Duration::from_secs(6)
        );
        assert_eq!(
            exponential_backoff_delay(10, Duration::from_secs(3), Duration::from_secs(30)),
            Duration::from_secs(30)
        );
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
            ChatPlatform::Discord
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
            AppLocale::ZhCN,
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
            AppLocale::ZhCN,
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

    #[tokio::test]
    async fn streaming_reply_skips_duplicate_final_edit_when_content_is_unchanged() {
        let adapter = FakeAdapter::new(false);
        let adapter_trait: Arc<dyn ChatAdapter> = Arc::new(adapter.clone());
        let mut reply = StreamingReply::new(
            adapter_trait,
            "chat-1".to_string(),
            Some("reply-1".to_string()),
            "label".to_string(),
            AppLocale::ZhCN,
        );

        reply.update("same answer").await.expect("send");
        reply.finish("same answer").await.expect("finish");

        assert_eq!(adapter.calls(), vec!["send:label\nsame answer".to_string()]);
    }

    #[tokio::test]
    async fn help_uses_global_ui_locale_even_if_binding_locale_is_zh_cn() {
        let db_path = temp_db_path();
        remove_sqlite_artifacts(&db_path);
        storage::init_db(&db_path).expect("init db");
        bind_test_user(
            &db_path,
            ChatPlatform::Telegram,
            "tg-user-1",
            AppLocale::ZhCN,
        )
        .await;
        storage::update_app_settings(
            db_path.clone(),
            storage::AppSettingsPatch {
                ui_locale: Some(AppLocale::EnUS),
                ..Default::default()
            },
        )
        .await
        .expect("update app locale");

        let runtime = test_runtime(&db_path).await;
        let adapter = FakeAdapter::new(false);
        runtime
            .handle_message(
                Arc::new(adapter.clone()),
                test_message(ChatPlatform::Telegram, "tg-user-1", "/help"),
            )
            .await;

        assert_eq!(
            adapter.calls(),
            vec![format!("send:{}", help_text(AppLocale::EnUS))]
        );
        remove_sqlite_artifacts(&db_path);
    }

    #[tokio::test]
    async fn lang_command_updates_global_ui_locale_and_binding_snapshot() {
        let db_path = temp_db_path();
        remove_sqlite_artifacts(&db_path);
        storage::init_db(&db_path).expect("init db");
        bind_test_user(
            &db_path,
            ChatPlatform::Telegram,
            "tg-user-1",
            AppLocale::ZhCN,
        )
        .await;

        let runtime = test_runtime(&db_path).await;
        let adapter = FakeAdapter::new(false);
        runtime
            .handle_message(
                Arc::new(adapter.clone()),
                test_message(ChatPlatform::Telegram, "tg-user-1", "/lang en"),
            )
            .await;

        let settings = storage::get_app_settings(db_path.clone())
            .await
            .expect("reload app settings");
        let bindings = storage::list_chat_bindings(db_path.clone())
            .await
            .expect("reload chat bindings");

        assert_eq!(settings.ui_locale, AppLocale::EnUS);
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].preferred_locale, AppLocale::EnUS.as_str());
        assert_eq!(
            adapter.calls(),
            vec![format!(
                "send:{}",
                t_args(
                    AppLocale::EnUS,
                    "lang.updated",
                    &args([("locale", AppLocale::EnUS.as_str().to_string())]),
                )
            )]
        );
        remove_sqlite_artifacts(&db_path);
    }
}
