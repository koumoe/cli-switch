pub mod adapter;
mod auth;
pub mod cli;
mod i18n;
mod node_bridge;
mod output;
mod projects;
mod resolver;
mod router;
pub(crate) mod weixin;
pub(crate) mod whatsapp_web;

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
use self::router::{
    Command, StatsRange as CommandStatsRange, format_session_label, format_sessions_list, help_text,
};
use crate::chat_bridge::weixin::{
    WeixinControl, WeixinState, WeixinStatus,
    logout_by_clearing_auth_state as logout_weixin_by_clearing_auth_state, run_weixin_bridge,
};
use crate::chat_bridge::whatsapp_web::{
    WhatsAppBridgeCommand, WhatsAppWebControl, WhatsAppWebState, WhatsAppWebStatus,
    logout_by_clearing_auth_state, run_whatsapp_web_bridge,
};
use crate::cli_tools::{
    CLI_TOOLS, CliExecEnv, CliToolId, detect_cli_tool, normalize_version_string,
    try_get_cmd_version_at,
};
use crate::events;
use crate::i18n::{AppLocale, render_error};
use crate::storage::{self, BridgeSessionStatus, ChatPlatform, StorageError};

const STREAM_UPDATE_INTERVAL: Duration = Duration::from_millis(1200);
const TYPING_INTERVAL: Duration = Duration::from_secs(4);
#[allow(dead_code)]
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

pub struct SupervisorChannels {
    pub whatsapp_control_rx: mpsc::Receiver<WhatsAppWebControl>,
    pub whatsapp_status_tx: watch::Sender<WhatsAppWebStatus>,
    pub weixin_control_rx: mpsc::Receiver<WeixinControl>,
    pub weixin_status_tx: watch::Sender<WeixinStatus>,
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

    fn handle_expected_exit(
        &mut self,
        exit: Option<(Duration, Result<(), tokio::task::JoinError>)>,
    ) {
        let Some((_, result)) = exit else {
            return;
        };

        if let Err(err) = result {
            tracing::warn!(
                platform = self.platform_name,
                err = %err,
                "chat bridge task exited during expected shutdown"
            );
        }

        self.handle = None;
        self.started_at = None;
        self.token = None;
        self.restart_failures = 0;
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
    mut settings_rx: watch::Receiver<Arc<storage::AppSettings>>,
    channels_cache: Option<watch::Sender<Arc<Vec<storage::Channel>>>>,
    supervisor_channels: SupervisorChannels,
) {
    let SupervisorChannels {
        mut whatsapp_control_rx,
        whatsapp_status_tx,
        mut weixin_control_rx,
        weixin_status_tx,
    } = supervisor_channels;

    let runtime = ChatBridgeRuntime {
        db_path,
        settings_rx: settings_rx.clone(),
        whatsapp_status_rx: whatsapp_status_tx.subscribe(),
        weixin_status_rx: weixin_status_tx.subscribe(),
        channels_cache,
        project_store: Arc::new(ProjectStore::new()),
        busy_sessions: Arc::new(Mutex::new(HashSet::new())),
        active_turns: Arc::new(Mutex::new(HashMap::new())),
    };

    let mut telegram_task = ManagedBridgeTask::new("telegram");
    let mut discord_task = ManagedBridgeTask::new("discord");
    let mut whatsapp_task = ManagedBridgeTask::new("whatsapp");
    let mut weixin_task = ManagedBridgeTask::new("weixin");
    let mut whatsapp_nonce: u64 = 0;
    let mut weixin_nonce: u64 = 0;
    let mut whatsapp_bridge_control_tx: Option<mpsc::UnboundedSender<WhatsAppBridgeCommand>> = None;
    let mut whatsapp_skip_restart_backoff = false;

    loop {
        let telegram_token = desired_telegram_token(settings_rx.borrow().as_ref());
        let discord_token = desired_discord_token(settings_rx.borrow().as_ref());
        let whatsapp_enabled = desired_whatsapp_enabled(settings_rx.borrow().as_ref());
        let weixin_enabled = desired_weixin_enabled(settings_rx.borrow().as_ref());
        let whatsapp_key = whatsapp_enabled.then_some(format!("whatsapp-web:{whatsapp_nonce}"));
        let weixin_key = weixin_enabled.then_some(format!("weixin:{weixin_nonce}"));

        telegram_task.sync_token(telegram_token.as_deref());
        discord_task.sync_token(discord_token.as_deref());
        whatsapp_task.sync_token(whatsapp_key.as_deref());
        weixin_task.sync_token(weixin_key.as_deref());
        if !whatsapp_task.is_running() {
            whatsapp_bridge_control_tx = None;
        }

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

        if let Some(key) = whatsapp_key.clone() {
            if !whatsapp_task.is_running() {
                let runtime = runtime.clone();
                let client = http_client.clone();
                let status_tx = whatsapp_status_tx.clone();
                let (bridge_control_tx, bridge_control_rx) = mpsc::unbounded_channel();
                whatsapp_bridge_control_tx = Some(bridge_control_tx);
                whatsapp_task.spawn_if_needed(Some(key), move |_token| {
                    tokio::spawn(async move {
                        run_whatsapp_web_bridge(runtime, client, status_tx, bridge_control_rx)
                            .await;
                    })
                });
            }
        } else {
            whatsapp_bridge_control_tx = None;
            let _ = whatsapp_status_tx.send(WhatsAppWebStatus::disabled());
        }

        if let Some(key) = weixin_key.clone() {
            let runtime = runtime.clone();
            let client = http_client.clone();
            let status_tx = weixin_status_tx.clone();
            weixin_task.spawn_if_needed(Some(key), move |_token| {
                tokio::spawn(async move {
                    run_weixin_bridge(runtime, client, status_tx).await;
                })
            });
        } else {
            let _ = weixin_status_tx.send(WeixinStatus::disabled());
        }

        let telegram_running = telegram_task.is_running();
        let discord_running = discord_task.is_running();
        let whatsapp_running = whatsapp_task.is_running();
        let weixin_running = weixin_task.is_running();
        tokio::select! {
            changed = settings_rx.changed() => {
                if changed.is_err() {
                    break;
                }
            }
            cmd = whatsapp_control_rx.recv() => {
                let Some(cmd) = cmd else {
                    // channel closed, ignore
                    continue;
                };
                match cmd {
                    WhatsAppWebControl::StartLogin => {
                        if whatsapp_enabled {
                            whatsapp_nonce = whatsapp_nonce.wrapping_add(1);
                        }
                    }
                    WhatsAppWebControl::Logout => {
                        let mut sent_to_runtime = false;
                        if let Some(tx) = whatsapp_bridge_control_tx.as_ref() {
                            match tx.send(WhatsAppBridgeCommand::Logout) {
                                Ok(_) => {
                                    sent_to_runtime = true;
                                    whatsapp_skip_restart_backoff = true;
                                }
                                Err(err) => {
                                    tracing::warn!(err = %err, "send whatsapp bridge logout command failed");
                                }
                            }
                        }
                        if !sent_to_runtime {
                            if let Err(err) = logout_by_clearing_auth_state(&runtime.data_dir()) {
                                tracing::warn!(err = %err, "clear whatsapp auth state failed");
                            }
                            whatsapp_nonce = whatsapp_nonce.wrapping_add(1);
                        }
                    }
                }
            }
            cmd = weixin_control_rx.recv() => {
                let Some(cmd) = cmd else {
                    continue;
                };
                match cmd {
                    WeixinControl::StartLogin => {
                        if weixin_enabled {
                            weixin_nonce = weixin_nonce.wrapping_add(1);
                        }
                    }
                    WeixinControl::Logout => {
                        if let Err(err) = logout_weixin_by_clearing_auth_state(&runtime.data_dir()) {
                            tracing::warn!(err = %err, "clear weixin auth state failed");
                        }
                        weixin_nonce = weixin_nonce.wrapping_add(1);
                    }
                }
            }
            exit = telegram_task.wait_for_exit(), if telegram_running => {
                telegram_task.handle_exit(exit).await;
            }
            exit = discord_task.wait_for_exit(), if discord_running => {
                discord_task.handle_exit(exit).await;
            }
            exit = whatsapp_task.wait_for_exit(), if whatsapp_running => {
                whatsapp_bridge_control_tx = None;
                if whatsapp_skip_restart_backoff {
                    whatsapp_skip_restart_backoff = false;
                    whatsapp_task.handle_expected_exit(exit);
                } else {
                    whatsapp_task.handle_exit(exit).await;
                }
            }
            exit = weixin_task.wait_for_exit(), if weixin_running => {
                weixin_task.handle_exit(exit).await;
            }
        }
    }

    telegram_task.abort_and_join().await;
    discord_task.abort_and_join().await;
    whatsapp_task.abort_and_join().await;
    weixin_task.abort_and_join().await;
    let _ = whatsapp_status_tx.send(WhatsAppWebStatus::disabled());
    let _ = weixin_status_tx.send(WeixinStatus::disabled());
}

#[derive(Clone)]
struct ChatBridgeRuntime {
    db_path: PathBuf,
    settings_rx: watch::Receiver<Arc<storage::AppSettings>>,
    whatsapp_status_rx: watch::Receiver<WhatsAppWebStatus>,
    weixin_status_rx: watch::Receiver<WeixinStatus>,
    channels_cache: Option<watch::Sender<Arc<Vec<storage::Channel>>>>,
    project_store: Arc<ProjectStore>,
    busy_sessions: Arc<Mutex<HashSet<i64>>>,
    active_turns: Arc<Mutex<HashMap<i64, ActiveTurnHandle>>>,
}

#[derive(Debug, Clone)]
struct CliToolSnapshot {
    name: &'static str,
    installed: bool,
    version: Option<String>,
}

#[derive(Debug, Clone)]
enum ResolveChannelTargetResult {
    Exact(Box<storage::Channel>),
    Ambiguous(Vec<storage::Channel>),
    NotFound,
}

impl ChatBridgeRuntime {
    fn settings_snapshot(&self) -> Arc<storage::AppSettings> {
        self.settings_rx.borrow().clone()
    }

    fn publish_channels_cache(&self, channels: Vec<storage::Channel>) {
        if let Some(cache) = self.channels_cache.as_ref() {
            let _ = cache.send(Arc::new(channels));
        }
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
            Command::Channels => {
                let channels = storage::list_channels(self.db_path.clone()).await?;
                self.publish_channels_cache(channels.clone());
                let content = format_channels_list(&channels, storage::now_ms(), locale);
                self.send_text(
                    adapter,
                    &msg.chat_id,
                    &content,
                    msg.message_id.as_deref(),
                    locale,
                )
                .await?;
            }
            Command::ChannelSetEnabled { target, enabled } => {
                let channels = storage::list_channels(self.db_path.clone()).await?;
                let Some(channel) = self
                    .resolve_channel_or_reply(adapter.clone(), &msg, &target, &channels, locale)
                    .await?
                else {
                    return Ok(());
                };
                storage::set_channel_enabled(self.db_path.clone(), channel.id.clone(), enabled)
                    .await?;
                let channels = storage::list_channels(self.db_path.clone()).await?;
                self.publish_channels_cache(channels.clone());
                let updated = channels
                    .into_iter()
                    .find(|item| item.id == channel.id)
                    .unwrap_or(channel);
                let message = if enabled {
                    t_args(
                        locale,
                        "channel.enabled",
                        &args([("name", updated.name.clone())]),
                    )
                } else {
                    t_args(
                        locale,
                        "channel.disabled",
                        &args([("name", updated.name.clone())]),
                    )
                };
                self.send_text(
                    adapter,
                    &msg.chat_id,
                    &message,
                    msg.message_id.as_deref(),
                    locale,
                )
                .await?;
            }
            Command::Routes => {
                let routes = storage::list_routes(self.db_path.clone()).await?;
                let channels = storage::list_channels(self.db_path.clone()).await?;
                let channel_names = channels
                    .iter()
                    .map(|item| (item.id.clone(), item.name.clone()))
                    .collect::<HashMap<_, _>>();
                let mut route_channels = HashMap::<String, Vec<storage::RouteChannel>>::new();
                for route in &routes {
                    let items =
                        storage::list_route_channels(self.db_path.clone(), route.id.clone())
                            .await?;
                    route_channels.insert(route.id.clone(), items);
                }
                let content = format_routes_list(
                    &routes,
                    &route_channels,
                    &channel_names,
                    storage::now_ms(),
                    locale,
                );
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
            Command::Usage { range } => {
                let (start_ms, end_ms) = stats_window_ms(range);
                let summary =
                    storage::stats_summary(self.db_path.clone(), start_ms, end_ms).await?;
                let channel_stats =
                    storage::stats_channels(self.db_path.clone(), start_ms, end_ms).await?;
                let content = format_usage_report(range, &summary, &channel_stats, locale);
                self.send_text(
                    adapter,
                    &msg.chat_id,
                    &content,
                    msg.message_id.as_deref(),
                    locale,
                )
                .await?;
            }
            Command::Costs { range } => {
                let (start_ms, end_ms) = stats_window_ms(range);
                let summary =
                    storage::stats_summary(self.db_path.clone(), start_ms, end_ms).await?;
                let channel_stats =
                    storage::stats_channels(self.db_path.clone(), start_ms, end_ms).await?;
                let pricing = storage::pricing_status(self.db_path.clone()).await?;
                let content =
                    format_costs_report(range, &summary, &channel_stats, &pricing, locale);
                self.send_text(
                    adapter,
                    &msg.chat_id,
                    &content,
                    msg.message_id.as_deref(),
                    locale,
                )
                .await?;
            }
            Command::Status => {
                let now_ms = storage::now_ms();
                let settings = self.settings_snapshot();
                let channels = storage::list_channels(self.db_path.clone()).await?;
                let routes = storage::list_routes(self.db_path.clone()).await?;
                let pricing = storage::pricing_status(self.db_path.clone()).await?;
                let telegram_sessions = storage::count_active_bridge_sessions_for_platform(
                    self.db_path.clone(),
                    ChatPlatform::Telegram,
                )
                .await?;
                let discord_sessions = storage::count_active_bridge_sessions_for_platform(
                    self.db_path.clone(),
                    ChatPlatform::Discord,
                )
                .await?;
                let whatsapp_sessions = storage::count_active_bridge_sessions_for_platform(
                    self.db_path.clone(),
                    ChatPlatform::WhatsApp,
                )
                .await?;
                let weixin_sessions = storage::count_active_bridge_sessions_for_platform(
                    self.db_path.clone(),
                    ChatPlatform::Weixin,
                )
                .await?;
                let mut whatsapp_status = self.whatsapp_status_rx.borrow().clone();
                if !desired_whatsapp_enabled(settings.as_ref()) {
                    whatsapp_status = WhatsAppWebStatus::disabled();
                }
                let mut weixin_status = self.weixin_status_rx.borrow().clone();
                if !desired_weixin_enabled(settings.as_ref()) {
                    weixin_status = WeixinStatus::disabled();
                }
                let tool_statuses =
                    detect_cli_tool_statuses(settings.as_ref(), self.data_dir()).await?;
                let update_status = events::last_update_status();
                let content = format_status_report(StatusReportContext {
                    now_ms,
                    settings: settings.as_ref(),
                    channels: &channels,
                    routes: &routes,
                    tool_statuses: &tool_statuses,
                    telegram_sessions,
                    discord_sessions,
                    whatsapp_sessions,
                    weixin_sessions,
                    whatsapp_status: &whatsapp_status,
                    weixin_status: &weixin_status,
                    pricing: &pricing,
                    update_status: update_status.as_ref(),
                    locale,
                });
                self.send_text(
                    adapter,
                    &msg.chat_id,
                    &content,
                    msg.message_id.as_deref(),
                    locale,
                )
                .await?;
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
            // Active sessions should normally always have a default. Keep a separate
            // message for the inconsistent case so users only see /switch when it helps.
            let message_key = if storage::count_active_bridge_sessions_for_platform(
                self.db_path.clone(),
                platform,
            )
            .await?
                == 0
            {
                "session.no_active_for_message"
            } else {
                "session.no_default"
            };
            self.send_text(
                adapter,
                &msg.chat_id,
                &t(locale, message_key),
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

    async fn resolve_channel_or_reply(
        &self,
        adapter: Arc<dyn ChatAdapter>,
        msg: &IncomingMessage,
        target: &str,
        channels: &[storage::Channel],
        locale: AppLocale,
    ) -> anyhow::Result<Option<storage::Channel>> {
        match resolve_channel_target(channels, target) {
            ResolveChannelTargetResult::Exact(channel) => Ok(Some(*channel)),
            ResolveChannelTargetResult::Ambiguous(items) => {
                self.send_text(
                    adapter,
                    &msg.chat_id,
                    &format_ambiguous_channel_target(target, &items, locale),
                    msg.message_id.as_deref(),
                    locale,
                )
                .await?;
                Ok(None)
            }
            ResolveChannelTargetResult::NotFound => {
                self.send_text(
                    adapter,
                    &msg.chat_id,
                    &t_args(
                        locale,
                        "channel.not_found",
                        &args([("target", target.to_string())]),
                    ),
                    msg.message_id.as_deref(),
                    locale,
                )
                .await?;
                Ok(None)
            }
        }
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

fn resolve_channel_target(
    channels: &[storage::Channel],
    target: &str,
) -> ResolveChannelTargetResult {
    let trimmed = target.trim();
    if trimmed.is_empty() {
        return ResolveChannelTargetResult::NotFound;
    }

    if let Some(channel) = channels.iter().find(|item| item.id == trimmed) {
        return ResolveChannelTargetResult::Exact(Box::new(channel.clone()));
    }

    let lowered = trimmed.to_ascii_lowercase();
    let by_name = channels
        .iter()
        .filter(|item| item.name.to_ascii_lowercase() == lowered)
        .cloned()
        .collect::<Vec<_>>();
    if by_name.len() == 1 {
        return ResolveChannelTargetResult::Exact(Box::new(
            by_name.into_iter().next().unwrap_or_else(|| unreachable!()),
        ));
    }
    if by_name.len() > 1 {
        return ResolveChannelTargetResult::Ambiguous(by_name);
    }

    let by_id_prefix = channels
        .iter()
        .filter(|item| item.id.to_ascii_lowercase().starts_with(&lowered))
        .cloned()
        .collect::<Vec<_>>();
    match by_id_prefix.len() {
        0 => ResolveChannelTargetResult::NotFound,
        1 => ResolveChannelTargetResult::Exact(Box::new(
            by_id_prefix
                .into_iter()
                .next()
                .unwrap_or_else(|| unreachable!()),
        )),
        _ => ResolveChannelTargetResult::Ambiguous(by_id_prefix),
    }
}

fn format_ambiguous_channel_target(
    target: &str,
    items: &[storage::Channel],
    locale: AppLocale,
) -> String {
    let mut lines = vec![t_args(
        locale,
        "channel.ambiguous_title",
        &args([("target", target.to_string())]),
    )];
    for item in items {
        lines.push(format!(
            "- {} [{}] ({})",
            item.name,
            item.protocol.as_str(),
            short_id(&item.id)
        ));
    }
    lines.push(t(locale, "channel.ambiguous_hint"));
    lines.join("\n")
}

fn format_channels_list(channels: &[storage::Channel], now_ms: i64, locale: AppLocale) -> String {
    if channels.is_empty() {
        return t(locale, "channel.none");
    }

    let mut lines = vec![t(locale, "channel.list_title")];
    let mut current_protocol = None;
    let mut section_index = 0usize;

    for channel in channels {
        if current_protocol != Some(channel.protocol) {
            if current_protocol.is_some() {
                lines.push(String::new());
            }
            lines.push(format!(
                "【{}】",
                channel_protocol_display_label(channel.protocol)
            ));
            current_protocol = Some(channel.protocol);
            section_index = 0;
        }

        section_index += 1;
        lines.push(format!("{section_index}. {}", channel.name));
        lines.push(format!(
            "   {}: {} · {}: {} · {}: {}",
            t(locale, "channel.status_label"),
            channel_status_label(channel, now_ms, locale),
            t(locale, "channel.priority_label"),
            channel.priority,
            t(locale, "channel.id_label"),
            short_id(&channel.id)
        ));

        if storage::channel_is_auto_disabled(channel, now_ms) {
            lines.push(format!(
                "   {}: {}",
                t(locale, "channel.auto_disabled_until_label"),
                format_local_timestamp_ms(channel.auto_disabled_until_ms)
            ));
        }
    }

    lines.join("\n")
}

fn channel_protocol_display_label(protocol: storage::Protocol) -> &'static str {
    match protocol {
        storage::Protocol::Openai => "OpenAI",
        storage::Protocol::Anthropic => "Anthropic",
        storage::Protocol::Gemini => "Gemini",
    }
}

fn format_routes_list(
    routes: &[storage::Route],
    route_channels: &HashMap<String, Vec<storage::RouteChannel>>,
    channel_names: &HashMap<String, String>,
    now_ms: i64,
    locale: AppLocale,
) -> String {
    if routes.is_empty() {
        return t(locale, "route.none");
    }

    let mut lines = vec![t(locale, "route.list_title")];
    for route in routes {
        let match_model = route
            .match_model
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| t(locale, "route.match_all"));
        lines.push(format!(
            "{} [{}]  {}",
            route.name,
            route.protocol.as_str(),
            route_enabled_label(route.enabled, locale),
        ));
        lines.push(format!("    {}: {}", t(locale, "route.id_label"), route.id));
        lines.push(format!(
            "    {}: {}",
            t(locale, "route.match_model_label"),
            match_model
        ));

        let items = route_channels.get(&route.id).cloned().unwrap_or_default();
        if items.is_empty() {
            lines.push(format!(
                "    {}: {}",
                t(locale, "route.channels_label"),
                t(locale, "route.channels_empty")
            ));
            continue;
        }

        lines.push(format!("    {}:", t(locale, "route.channels_label")));
        for item in items {
            let name = channel_names
                .get(&item.channel_id)
                .cloned()
                .unwrap_or_else(|| item.channel_id.clone());
            let mut line = format!(
                "    {}. {} ({})",
                item.priority + 1,
                name,
                short_id(&item.channel_id)
            );
            if let Some(until_ms) = item.cooldown_until_ms.filter(|value| *value > now_ms) {
                line.push_str(&format!(
                    "  {} {}",
                    t(locale, "route.cooldown_until_label"),
                    format_local_timestamp_with_relative(now_ms, until_ms, locale)
                ));
            }
            lines.push(line);
        }
    }

    lines.join("\n")
}

fn format_usage_report(
    range: CommandStatsRange,
    summary: &storage::StatsSummary,
    channel_stats: &[storage::ChannelStats],
    locale: AppLocale,
) -> String {
    let mut lines = vec![t(locale, "stats.usage_title")];
    lines.extend(format_key_value_lines(vec![
        (
            t(locale, "stats.range_label"),
            stats_range_label(range, locale),
        ),
        (
            t(locale, "stats.requests_label"),
            summary.requests.to_string(),
        ),
        (
            t(locale, "stats.success_label"),
            summary.success.to_string(),
        ),
        (t(locale, "stats.failed_label"), summary.failed.to_string()),
        (
            t(locale, "stats.avg_latency_label"),
            format_avg_latency(summary.avg_latency_ms),
        ),
        (
            t(locale, "stats.prompt_tokens_label"),
            summary.prompt_tokens.to_string(),
        ),
        (
            t(locale, "stats.completion_tokens_label"),
            summary.completion_tokens.to_string(),
        ),
        (
            t(locale, "stats.total_tokens_label"),
            summary.total_tokens.to_string(),
        ),
        (
            t(locale, "stats.estimated_cost_label"),
            format_cost_usd(summary.estimated_cost_usd.as_deref()),
        ),
    ]));
    lines.push(String::new());
    lines.push(t(locale, "stats.by_channel_label"));

    let active_channels = active_channel_stats(channel_stats);
    if active_channels.is_empty() {
        lines.push(t(locale, "stats.by_channel_empty"));
        return lines.join("\n");
    }

    lines.extend(format_grouped_channel_stats(active_channels, |item| {
        vec![
            format!(
                "{}: {} · {}: {} · {}: {}",
                t(locale, "stats.requests_label"),
                item.requests,
                t(locale, "stats.success_label"),
                item.success,
                t(locale, "stats.failed_label"),
                item.failed
            ),
            format!(
                "{}: {} · {}: {}",
                t(locale, "stats.total_tokens_label"),
                item.total_tokens,
                t(locale, "stats.estimated_cost_short_label"),
                format_cost_usd(item.estimated_cost_usd.as_deref())
            ),
        ]
    }));

    lines.join("\n")
}

fn format_costs_report(
    range: CommandStatsRange,
    summary: &storage::StatsSummary,
    channel_stats: &[storage::ChannelStats],
    pricing: &storage::PricingStatus,
    locale: AppLocale,
) -> String {
    let mut lines = vec![t(locale, "stats.costs_title")];
    lines.extend(format_key_value_lines(vec![
        (
            t(locale, "stats.range_label"),
            stats_range_label(range, locale),
        ),
        (
            t(locale, "stats.estimated_cost_label"),
            format_cost_usd(summary.estimated_cost_usd.as_deref()),
        ),
        (
            t(locale, "stats.total_tokens_label"),
            summary.total_tokens.to_string(),
        ),
        (
            t(locale, "stats.pricing_models_label"),
            pricing.count.to_string(),
        ),
        (
            t(locale, "stats.last_sync_label"),
            pricing
                .last_sync_ms
                .map(|value| format_local_timestamp_with_relative(storage::now_ms(), value, locale))
                .unwrap_or_else(|| t(locale, "stats.not_synced")),
        ),
    ]));

    if pricing.count == 0 {
        lines.push(String::new());
        lines.push(t(locale, "stats.pricing_missing_hint"));
    }

    lines.push(String::new());
    lines.push(t(locale, "stats.by_channel_label"));

    let active_channels = active_channel_stats(channel_stats);
    if active_channels.is_empty() {
        lines.push(t(locale, "stats.by_channel_empty"));
        return lines.join("\n");
    }

    lines.extend(format_grouped_channel_stats(active_channels, |item| {
        vec![format!(
            "{}: {} · {}: {} · {}: {}",
            t(locale, "stats.estimated_cost_short_label"),
            format_cost_usd(item.estimated_cost_usd.as_deref()),
            t(locale, "stats.requests_label"),
            item.requests,
            t(locale, "stats.total_tokens_label"),
            item.total_tokens
        )]
    }));

    lines.join("\n")
}

fn format_key_value_lines(rows: Vec<(String, String)>) -> Vec<String> {
    rows.into_iter()
        .map(|(key, value)| format!("- {key}: {value}"))
        .collect()
}

fn active_channel_stats(channel_stats: &[storage::ChannelStats]) -> Vec<&storage::ChannelStats> {
    let mut items = channel_stats
        .iter()
        .filter(|item| item.requests > 0)
        .collect::<Vec<_>>();
    items.sort_by(|left, right| {
        protocol_sort_order(left.protocol)
            .cmp(&protocol_sort_order(right.protocol))
            .then_with(|| left.name.cmp(&right.name))
    });
    items
}

fn format_grouped_channel_stats<F>(
    channel_stats: Vec<&storage::ChannelStats>,
    detail_lines: F,
) -> Vec<String>
where
    F: Fn(&storage::ChannelStats) -> Vec<String>,
{
    let mut lines = Vec::new();
    let mut current_protocol = None;
    let mut section_index = 0usize;

    for item in channel_stats {
        if current_protocol != Some(item.protocol) {
            if current_protocol.is_some() {
                lines.push(String::new());
            }
            lines.push(format!(
                "【{}】",
                channel_protocol_display_label(item.protocol)
            ));
            current_protocol = Some(item.protocol);
            section_index = 0;
        }

        section_index += 1;
        lines.push(format!("{section_index}. {}", item.name));
        for detail in detail_lines(item) {
            lines.push(format!("   {detail}"));
        }
    }

    lines
}

fn protocol_sort_order(protocol: storage::Protocol) -> u8 {
    match protocol {
        storage::Protocol::Openai => 0,
        storage::Protocol::Anthropic => 1,
        storage::Protocol::Gemini => 2,
    }
}

struct StatusReportContext<'a> {
    now_ms: i64,
    settings: &'a storage::AppSettings,
    channels: &'a [storage::Channel],
    routes: &'a [storage::Route],
    tool_statuses: &'a [CliToolSnapshot],
    telegram_sessions: i64,
    discord_sessions: i64,
    whatsapp_sessions: i64,
    weixin_sessions: i64,
    whatsapp_status: &'a WhatsAppWebStatus,
    weixin_status: &'a WeixinStatus,
    pricing: &'a storage::PricingStatus,
    update_status: Option<&'a crate::update::UpdateStatus>,
    locale: AppLocale,
}

fn format_status_report(ctx: StatusReportContext<'_>) -> String {
    let StatusReportContext {
        now_ms,
        settings,
        channels,
        routes,
        tool_statuses,
        telegram_sessions,
        discord_sessions,
        whatsapp_sessions,
        weixin_sessions,
        whatsapp_status,
        weixin_status,
        pricing,
        update_status,
        locale,
    } = ctx;

    let auto_disabled = channels
        .iter()
        .filter(|item| storage::channel_is_auto_disabled(item, now_ms))
        .count();
    let enabled_channels = channels.iter().filter(|item| item.enabled).count();
    let enabled_routes = routes.iter().filter(|item| item.enabled).count();

    let mut lines = vec![t(locale, "status.title")];
    lines.push(format!(
        "{}: {}",
        t(locale, "status.generated_at_label"),
        format_local_timestamp_ms(now_ms)
    ));
    lines.push(format!(
        "{}: {}",
        t(locale, "status.version_label"),
        env!("CARGO_PKG_VERSION")
    ));
    lines.push(format!(
        "{}: {}",
        t(locale, "status.chat_bridge_label"),
        enabled_label(settings.chat_bridge_enabled, locale)
    ));
    lines.push(format!(
        "- Telegram: {} / {} / {}={}",
        enabled_label(settings.chat_bridge_telegram_enabled, locale),
        token_configured_label(settings.chat_bridge_telegram_bot_token_configured, locale),
        t(locale, "status.active_sessions_label"),
        telegram_sessions
    ));
    lines.push(format!(
        "- Discord: {} / {} / {}={}",
        enabled_label(settings.chat_bridge_discord_enabled, locale),
        token_configured_label(settings.chat_bridge_discord_bot_token_configured, locale),
        t(locale, "status.active_sessions_label"),
        discord_sessions
    ));
    lines.push(format!(
        "- WhatsApp: {} / state={} / connected={} / me={} / {}={}",
        enabled_label(settings.chat_bridge_whatsapp_enabled, locale),
        whatsapp_web_state_label(whatsapp_status.state, locale),
        configured_label(whatsapp_status.connected, locale),
        whatsapp_status
            .me
            .clone()
            .unwrap_or_else(|| t(locale, "status.unknown_label")),
        t(locale, "status.active_sessions_label"),
        whatsapp_sessions
    ));
    lines.push(format!(
        "- Weixin: {} / state={} / connected={} / me={} / {}={}",
        enabled_label(settings.chat_bridge_weixin_enabled, locale),
        weixin_state_label(weixin_status.state, locale),
        configured_label(weixin_status.connected, locale),
        weixin_status
            .me
            .clone()
            .unwrap_or_else(|| t(locale, "status.unknown_label")),
        t(locale, "status.active_sessions_label"),
        weixin_sessions
    ));
    lines.push(format!(
        "{}: {}={}  {}={}  {}={}",
        t(locale, "status.channels_label"),
        t(locale, "status.total_label"),
        channels.len(),
        t(locale, "status.enabled_label"),
        enabled_channels,
        t(locale, "status.auto_disabled_label"),
        auto_disabled
    ));
    lines.push(format!(
        "{}: {}={}  {}={}",
        t(locale, "status.routes_label"),
        t(locale, "status.total_label"),
        routes.len(),
        t(locale, "status.enabled_label"),
        enabled_routes
    ));
    lines.push(format!(
        "{}: {}={}  {}={}",
        t(locale, "status.pricing_label"),
        t(locale, "status.models_label"),
        pricing.count,
        t(locale, "status.last_sync_label"),
        pricing
            .last_sync_ms
            .map(|value| format_local_timestamp_with_relative(now_ms, value, locale))
            .unwrap_or_else(|| t(locale, "status.unknown_label"))
    ));
    lines.push(t(locale, "status.cli_tools_label"));
    for item in tool_statuses {
        lines.push(format!(
            "- {}: {}",
            item.name,
            if item.installed {
                item.version
                    .as_deref()
                    .map(|value| format!("{} ({value})", t(locale, "status.installed_label")))
                    .unwrap_or_else(|| t(locale, "status.installed_label"))
            } else {
                t(locale, "status.missing_label")
            }
        ));
    }

    let update_line = match update_status {
        Some(status) => {
            let latest = status.latest_version.as_deref().unwrap_or("-");
            format!(
                "{}: stage={}  latest={}  update_available={}",
                t(locale, "status.update_label"),
                status.stage,
                latest,
                if status.update_available {
                    t(locale, "status.yes_label")
                } else {
                    t(locale, "status.no_label")
                }
            )
        }
        None => format!(
            "{}: {}",
            t(locale, "status.update_label"),
            t(locale, "status.unknown_label")
        ),
    };
    lines.push(update_line);

    lines.join("\n")
}

async fn detect_cli_tool_statuses(
    settings: &storage::AppSettings,
    data_dir: PathBuf,
) -> anyhow::Result<Vec<CliToolSnapshot>> {
    let npm_path = settings.cli_tools_npm_path.clone();
    let node_path = settings.cli_tools_node_path.clone();
    tokio::task::spawn_blocking(move || {
        let env = CliExecEnv::new(npm_path.as_deref(), node_path.as_deref());
        let mut out = Vec::new();
        for def in CLI_TOOLS {
            let mut detected = detect_cli_tool(&env, &data_dir, def);
            if let Ok(shim_path) = crate::terminal::cli_tool_shim_path(def.bin)
                && shim_path.is_file()
            {
                let shim_version = try_get_cmd_version_at(&shim_path);
                if shim_version.is_some() {
                    if !detected.installed {
                        detected.installed = true;
                    }
                    if detected.version.is_none() {
                        detected.version = shim_version.as_deref().map(normalize_version_string);
                    }
                }
            }
            out.push(CliToolSnapshot {
                name: def.name,
                installed: detected.installed,
                version: detected.version,
            });
        }
        Ok(out)
    })
    .await
    .context("wait cli tool status task failed")?
}

fn channel_status_label(channel: &storage::Channel, now_ms: i64, locale: AppLocale) -> String {
    if !channel.enabled {
        return t(locale, "channel.status_disabled");
    }
    if storage::channel_is_auto_disabled(channel, now_ms) {
        return t(locale, "channel.status_auto_disabled");
    }
    t(locale, "channel.status_enabled")
}

fn route_enabled_label(enabled: bool, locale: AppLocale) -> String {
    if enabled {
        t(locale, "route.status_enabled")
    } else {
        t(locale, "route.status_disabled")
    }
}

fn enabled_label(enabled: bool, locale: AppLocale) -> String {
    if enabled {
        t(locale, "status.enabled_state")
    } else {
        t(locale, "status.disabled_state")
    }
}

fn token_configured_label(configured: bool, locale: AppLocale) -> String {
    if configured {
        t(locale, "status.token_configured")
    } else {
        t(locale, "status.token_missing")
    }
}

fn configured_label(configured: bool, locale: AppLocale) -> String {
    if configured {
        t(locale, "status.configured")
    } else {
        t(locale, "status.missing")
    }
}

fn whatsapp_web_state_label(state: WhatsAppWebState, locale: AppLocale) -> String {
    match state {
        WhatsAppWebState::Disabled => t(locale, "status.runtime_state_disabled"),
        WhatsAppWebState::Starting => t(locale, "status.runtime_state_starting"),
        WhatsAppWebState::AwaitingQr => t(locale, "status.runtime_state_awaiting_qr"),
        WhatsAppWebState::Connected => t(locale, "status.runtime_state_connected"),
        WhatsAppWebState::Error => t(locale, "status.runtime_state_error"),
    }
}

fn weixin_state_label(state: WeixinState, locale: AppLocale) -> String {
    match state {
        WeixinState::Disabled => t(locale, "status.runtime_state_disabled"),
        WeixinState::Starting => t(locale, "status.runtime_state_starting"),
        WeixinState::AwaitingQr => t(locale, "status.runtime_state_awaiting_qr"),
        WeixinState::Connected => t(locale, "status.runtime_state_connected"),
        WeixinState::Error => t(locale, "status.runtime_state_error"),
    }
}

fn stats_range_label(range: CommandStatsRange, locale: AppLocale) -> String {
    match range {
        CommandStatsRange::Today => t(locale, "stats.range_today"),
        CommandStatsRange::Yesterday => t(locale, "stats.range_yesterday"),
        CommandStatsRange::Week => t(locale, "stats.range_week"),
        CommandStatsRange::Month => t(locale, "stats.range_month"),
    }
}

fn stats_window_ms(range: CommandStatsRange) -> (i64, Option<i64>) {
    let now = time::OffsetDateTime::now_utc();
    let offset = time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);
    let local = now.to_offset(offset);

    let start_local = match range {
        CommandStatsRange::Today => local.replace_time(time::Time::MIDNIGHT),
        CommandStatsRange::Yesterday => {
            (local - time::Duration::days(1)).replace_time(time::Time::MIDNIGHT)
        }
        CommandStatsRange::Week => {
            let d = local.date();
            let days_since_monday = i64::from(d.weekday().number_from_monday().saturating_sub(1));
            let first = d - time::Duration::days(days_since_monday);
            local.replace_date(first).replace_time(time::Time::MIDNIGHT)
        }
        CommandStatsRange::Month => {
            let d = local.date();
            let first = time::Date::from_calendar_date(d.year(), d.month(), 1).unwrap_or(d);
            local.replace_date(first).replace_time(time::Time::MIDNIGHT)
        }
    };

    let start_ms = i64::try_from(
        start_local
            .to_offset(time::UtcOffset::UTC)
            .unix_timestamp_nanos()
            / 1_000_000,
    )
    .unwrap_or(0);
    let end_ms = match range {
        CommandStatsRange::Yesterday => {
            let start_today_local = local.replace_time(time::Time::MIDNIGHT);
            let start_today_ms = i64::try_from(
                start_today_local
                    .to_offset(time::UtcOffset::UTC)
                    .unix_timestamp_nanos()
                    / 1_000_000,
            )
            .unwrap_or(0);
            Some(start_today_ms.saturating_sub(1))
        }
        CommandStatsRange::Today | CommandStatsRange::Week | CommandStatsRange::Month => None,
    };
    (start_ms, end_ms)
}

fn format_avg_latency(value: Option<f64>) -> String {
    value
        .filter(|v| v.is_finite())
        .map(|v| format!("{v:.0} ms"))
        .unwrap_or_else(|| "-".to_string())
}

fn format_cost_usd(value: Option<&str>) -> String {
    value
        .map(|item| {
            let normalized = item
                .parse::<f64>()
                .ok()
                .map(|parsed| {
                    let mut s = format!("{parsed:.12}");
                    while s.contains('.') && s.ends_with('0') {
                        s.pop();
                    }
                    if s.ends_with('.') {
                        s.pop();
                    }
                    s
                })
                .filter(|parsed| !parsed.is_empty())
                .unwrap_or_else(|| item.to_string());
            format!("${normalized}")
        })
        .unwrap_or_else(|| "-".to_string())
}

fn format_local_timestamp_with_relative(now_ms: i64, at_ms: i64, locale: AppLocale) -> String {
    format!(
        "{} ({})",
        format_local_timestamp_ms(at_ms),
        format_relative_time_label(now_ms, at_ms, locale)
    )
}

fn format_relative_time_label(now_ms: i64, at_ms: i64, locale: AppLocale) -> String {
    let diff_ms = now_ms.saturating_sub(at_ms).max(0);
    let minutes = diff_ms / 60_000;
    if minutes <= 0 {
        return t(locale, "time.just_now");
    }
    if minutes < 60 {
        return t_args(
            locale,
            "time.minutes_ago",
            &args([("minutes", minutes.to_string())]),
        );
    }
    let hours = minutes / 60;
    if hours < 24 {
        return t_args(
            locale,
            "time.hours_ago",
            &args([("hours", hours.to_string())]),
        );
    }
    let days = hours / 24;
    t_args(locale, "time.days_ago", &args([("days", days.to_string())]))
}

fn format_local_timestamp_ms(ms: i64) -> String {
    let nanos = i128::from(ms).saturating_mul(1_000_000);
    let Ok(dt) = time::OffsetDateTime::from_unix_timestamp_nanos(nanos) else {
        return ms.to_string();
    };
    let offset = time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);
    let local = dt.to_offset(offset);
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        local.year(),
        u8::from(local.month()),
        local.day(),
        local.hour(),
        local.minute(),
        local.second()
    )
}

fn short_id(raw: &str) -> &str {
    raw.get(..8).unwrap_or(raw)
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
    !matches!(platform, ChatPlatform::Weixin) && active_session_count <= 1
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

fn desired_whatsapp_enabled(settings: &storage::AppSettings) -> bool {
    settings.chat_bridge_enabled && settings.chat_bridge_whatsapp_enabled
}

fn desired_weixin_enabled(settings: &storage::AppSettings) -> bool {
    settings.chat_bridge_enabled && settings.chat_bridge_weixin_enabled
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
        Some(StorageError::ChatBindingNotFound { .. }) => render_error(
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

    use crate::storage::Protocol;

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

    async fn bind_test_user(db_path: &Path, platform: ChatPlatform, platform_user_id: &str) {
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
        )
        .await
        .expect("consume pairing token");
    }

    async fn test_runtime(db_path: &Path) -> ChatBridgeRuntime {
        let settings = storage::get_app_settings(db_path.to_path_buf())
            .await
            .expect("load app settings");
        let (_settings_tx, settings_rx) = watch::channel(Arc::new(settings));
        let (_whatsapp_status_tx, whatsapp_status_rx) =
            watch::channel(WhatsAppWebStatus::disabled());
        let (_weixin_status_tx, weixin_status_rx) = watch::channel(WeixinStatus::disabled());
        ChatBridgeRuntime {
            db_path: db_path.to_path_buf(),
            settings_rx,
            whatsapp_status_rx,
            weixin_status_rx,
            channels_cache: None,
            project_store: Arc::new(ProjectStore::new()),
            busy_sessions: Arc::new(Mutex::new(HashSet::new())),
            active_turns: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    async fn create_test_channel(
        db_path: &Path,
        name: &str,
        protocol: Protocol,
    ) -> storage::Channel {
        storage::create_channel(
            db_path.to_path_buf(),
            storage::CreateChannel {
                name: name.to_string(),
                protocol,
                base_url: "https://example.com/v1".to_string(),
                auth_type: None,
                auth_ref: "env:TEST_KEY".to_string(),
                checkin_url: None,
                priority: 10,
                recharge_currency: None,
                real_multiplier: None,
                enabled: true,
                managed_by_newapi: None,
                newapi_account_id: None,
                newapi_channel_id: None,
                newapi_token_id: None,
                newapi_token_name: None,
                newapi_group: None,
            },
        )
        .await
        .expect("create test channel")
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
        assert!(should_stream_live_output(ChatPlatform::Discord, 1));
        assert!(!should_stream_live_output(ChatPlatform::Telegram, 2));
        assert!(should_stream_live_output(ChatPlatform::WhatsApp, 1));
        assert!(!should_stream_live_output(ChatPlatform::Weixin, 1));
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

        async fn delete_message(&self, _chat_id: &str, message_id: &str) -> anyhow::Result<()> {
            self.record(format!("delete:{message_id}"));
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
    async fn streaming_reply_keeps_pending_notice_until_native_streaming_finishes() {
        let adapter = FakeAdapter::new(true);
        let adapter_trait: Arc<dyn ChatAdapter> = Arc::new(adapter.clone());
        let mut reply = StreamingReply::with_pending_message(
            adapter_trait,
            "chat-1".to_string(),
            Some("reply-1".to_string()),
            "label".to_string(),
            AppLocale::ZhCN,
            Some(SentMessage {
                message_id: "pending-message".to_string(),
            }),
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
                "delete:pending-message".to_string(),
            ]
        );
    }

    #[tokio::test]
    async fn streaming_reply_reuses_pending_notice_for_send_edit_streaming() {
        let adapter = FakeAdapter::new(false);
        let adapter_trait: Arc<dyn ChatAdapter> = Arc::new(adapter.clone());
        let mut reply = StreamingReply::with_pending_message(
            adapter_trait,
            "chat-1".to_string(),
            Some("reply-1".to_string()),
            "label".to_string(),
            AppLocale::ZhCN,
            Some(SentMessage {
                message_id: "pending-message".to_string(),
            }),
        );

        reply.update("partial one").await.expect("send");
        reply.update("partial two").await.expect("edit");
        reply.finish("final answer").await.expect("finish");

        assert_eq!(
            adapter.calls(),
            vec![
                "edit:label\npartial one".to_string(),
                "edit:label\npartial two".to_string(),
                "edit:label\nfinal answer".to_string(),
            ]
        );
    }

    #[tokio::test]
    async fn streaming_reply_reuses_pending_notice_for_final_only_reply() {
        let adapter = FakeAdapter::new(false);
        let adapter_trait: Arc<dyn ChatAdapter> = Arc::new(adapter.clone());
        let mut reply = StreamingReply::with_pending_message(
            adapter_trait,
            "chat-1".to_string(),
            Some("reply-1".to_string()),
            "label".to_string(),
            AppLocale::ZhCN,
            Some(SentMessage {
                message_id: "pending-message".to_string(),
            }),
        );

        reply.finish("final answer").await.expect("finish");

        assert_eq!(
            adapter.calls(),
            vec!["edit:label\nfinal answer".to_string()]
        );
    }

    #[tokio::test]
    async fn clearing_progress_removes_partial_output_and_pending_notice() {
        let adapter = FakeAdapter::new(false);
        let adapter_trait: Arc<dyn ChatAdapter> = Arc::new(adapter.clone());
        let mut reply = StreamingReply::with_pending_message(
            adapter_trait,
            "chat-1".to_string(),
            Some("reply-1".to_string()),
            "label".to_string(),
            AppLocale::ZhCN,
            Some(SentMessage {
                message_id: "pending-message".to_string(),
            }),
        );

        reply.update("partial one").await.expect("update");
        reply
            .clear_progress_message()
            .await
            .expect("clear progress");

        assert_eq!(
            adapter.calls(),
            vec![
                "edit:label\npartial one".to_string(),
                "delete:pending-message".to_string(),
            ]
        );
    }

    #[tokio::test]
    async fn help_uses_global_ui_locale_for_bound_users() {
        let db_path = temp_db_path();
        remove_sqlite_artifacts(&db_path);
        storage::init_db(&db_path).expect("init db");
        bind_test_user(&db_path, ChatPlatform::Telegram, "tg-user-1").await;
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
    async fn plain_text_without_any_active_session_prompts_to_start_one() {
        let db_path = temp_db_path();
        remove_sqlite_artifacts(&db_path);
        storage::init_db(&db_path).expect("init db");
        bind_test_user(&db_path, ChatPlatform::Telegram, "tg-user-no-session").await;

        let runtime = test_runtime(&db_path).await;
        let adapter = FakeAdapter::new(false);
        runtime
            .handle_message(
                Arc::new(adapter.clone()),
                test_message(ChatPlatform::Telegram, "tg-user-no-session", "你好"),
            )
            .await;

        assert_eq!(
            adapter.calls(),
            vec![
                "send:当前没有活动会话，请先通过 /codex、/claude 或 /gemini 启动一个会话。"
                    .to_string()
            ]
        );
        remove_sqlite_artifacts(&db_path);
    }

    #[tokio::test]
    async fn plain_text_without_default_session_but_with_active_session_prompts_to_switch() {
        let db_path = temp_db_path();
        remove_sqlite_artifacts(&db_path);
        storage::init_db(&db_path).expect("init db");
        bind_test_user(&db_path, ChatPlatform::Telegram, "tg-user-no-default").await;
        let session = storage::create_bridge_session(
            db_path.clone(),
            storage::CreateBridgeSessionInput {
                platform: ChatPlatform::Telegram,
                alias: Some("alpha".to_string()),
                cli_type: CliToolId::Codex,
                cli_session_ref: None,
                project_id: Some("/tmp/demo".to_string()),
                project_name: "demo".to_string(),
                working_dir: "/tmp/demo".to_string(),
                permission_mode: storage::BridgePermissionMode::Safe,
            },
        )
        .await
        .expect("create bridge session");
        storage::update_bridge_session(
            db_path.clone(),
            session.id,
            storage::UpdateBridgeSessionInput {
                is_default: Some(false),
                ..Default::default()
            },
        )
        .await
        .expect("clear default session flag");

        let runtime = test_runtime(&db_path).await;
        let adapter = FakeAdapter::new(false);
        runtime
            .handle_message(
                Arc::new(adapter.clone()),
                test_message(ChatPlatform::Telegram, "tg-user-no-default", "hello"),
            )
            .await;

        assert_eq!(
            adapter.calls(),
            vec![
                "send:当前没有默认会话，请先通过 /switch 选择一个会话，或使用 /chat 指定目标。"
                    .to_string()
            ]
        );
        remove_sqlite_artifacts(&db_path);
    }

    #[tokio::test]
    async fn channels_disable_command_updates_channel_state() {
        let db_path = temp_db_path();
        remove_sqlite_artifacts(&db_path);
        storage::init_db(&db_path).expect("init db");
        bind_test_user(&db_path, ChatPlatform::Telegram, "tg-user-2").await;
        let channel = create_test_channel(&db_path, "openai-main", Protocol::Openai).await;

        let runtime = test_runtime(&db_path).await;
        let adapter = FakeAdapter::new(false);
        runtime
            .handle_message(
                Arc::new(adapter.clone()),
                test_message(
                    ChatPlatform::Telegram,
                    "tg-user-2",
                    "/channels disable openai-main",
                ),
            )
            .await;

        let updated = storage::get_channel(db_path.clone(), channel.id)
            .await
            .expect("get channel")
            .expect("channel exists");
        assert!(!updated.enabled);
        assert_eq!(
            adapter.calls(),
            vec!["send:已禁用渠道 openai-main。".to_string()]
        );
        remove_sqlite_artifacts(&db_path);
    }

    #[tokio::test]
    async fn channels_command_renders_grouped_list() {
        let db_path = temp_db_path();
        remove_sqlite_artifacts(&db_path);
        storage::init_db(&db_path).expect("init db");
        bind_test_user(&db_path, ChatPlatform::Telegram, "tg-user-2-list").await;
        create_test_channel(&db_path, "openai-main", Protocol::Openai).await;

        let runtime = test_runtime(&db_path).await;
        let adapter = FakeAdapter::new(false);
        runtime
            .handle_message(
                Arc::new(adapter.clone()),
                test_message(ChatPlatform::Telegram, "tg-user-2-list", "/channels"),
            )
            .await;

        let output = adapter.calls().join("\n");
        assert!(output.contains("渠道状态："), "{output}");
        assert!(output.contains("【OpenAI】"), "{output}");
        assert!(!output.contains("```text"), "{output}");
        assert!(output.contains("1. openai-main"), "{output}");
        assert!(output.contains("状态: 启用 · 优先级: 10 · ID:"), "{output}");
        remove_sqlite_artifacts(&db_path);
    }

    #[tokio::test]
    async fn channels_command_renders_grouped_list_for_weixin() {
        let db_path = temp_db_path();
        remove_sqlite_artifacts(&db_path);
        storage::init_db(&db_path).expect("init db");
        bind_test_user(&db_path, ChatPlatform::Weixin, "wx-user-2-list").await;
        create_test_channel(&db_path, "openai-main", Protocol::Openai).await;

        let runtime = test_runtime(&db_path).await;
        let adapter = FakeAdapter::new(false);
        runtime
            .handle_message(
                Arc::new(adapter.clone()),
                test_message(ChatPlatform::Weixin, "wx-user-2-list", "/channels"),
            )
            .await;

        let output = adapter.calls().join("\n");
        assert!(output.contains("渠道状态："), "{output}");
        assert!(output.contains("【OpenAI】"), "{output}");
        assert!(!output.contains("```text"), "{output}");
        assert!(output.contains("1. openai-main"), "{output}");
        remove_sqlite_artifacts(&db_path);
    }

    #[tokio::test]
    async fn channels_command_renders_grouped_list_for_discord() {
        let db_path = temp_db_path();
        remove_sqlite_artifacts(&db_path);
        storage::init_db(&db_path).expect("init db");
        bind_test_user(&db_path, ChatPlatform::Discord, "discord-user-2-list").await;
        create_test_channel(&db_path, "openai-main", Protocol::Openai).await;

        let runtime = test_runtime(&db_path).await;
        let adapter = FakeAdapter::new(false);
        runtime
            .handle_message(
                Arc::new(adapter.clone()),
                test_message(ChatPlatform::Discord, "discord-user-2-list", "/channels"),
            )
            .await;

        let output = adapter.calls().join("\n");
        assert!(output.contains("渠道状态："), "{output}");
        assert!(output.contains("【OpenAI】"), "{output}");
        assert!(!output.contains("```text"), "{output}");
        assert!(output.contains("1. openai-main"), "{output}");
        remove_sqlite_artifacts(&db_path);
    }

    #[tokio::test]
    async fn routes_command_lists_bound_channels() {
        let db_path = temp_db_path();
        remove_sqlite_artifacts(&db_path);
        storage::init_db(&db_path).expect("init db");
        bind_test_user(&db_path, ChatPlatform::Telegram, "tg-user-3").await;
        let channel = create_test_channel(&db_path, "openai-main", Protocol::Openai).await;
        let route = storage::create_route(
            db_path.clone(),
            storage::CreateRoute {
                name: "default".to_string(),
                protocol: Protocol::Openai,
                match_model: Some("gpt-4o".to_string()),
                enabled: true,
            },
        )
        .await
        .expect("create route");
        storage::set_route_channels(db_path.clone(), route.id, vec![channel.id])
            .await
            .expect("set route channels");

        let runtime = test_runtime(&db_path).await;
        let adapter = FakeAdapter::new(false);
        runtime
            .handle_message(
                Arc::new(adapter.clone()),
                test_message(ChatPlatform::Telegram, "tg-user-3", "/routes"),
            )
            .await;

        let output = adapter.calls().join("\n");
        assert!(output.contains("路由配置："), "{output}");
        assert!(output.contains("default [openai]"), "{output}");
        assert!(output.contains("openai-main"), "{output}");
        remove_sqlite_artifacts(&db_path);
    }

    #[tokio::test]
    async fn usage_command_reports_today_summary() {
        let db_path = temp_db_path();
        remove_sqlite_artifacts(&db_path);
        storage::init_db(&db_path).expect("init db");
        bind_test_user(&db_path, ChatPlatform::Telegram, "tg-user-4").await;
        let channel = create_test_channel(&db_path, "openai-main", Protocol::Openai).await;
        storage::insert_usage_event(
            db_path.clone(),
            storage::CreateUsageEvent {
                request_id: None,
                ts_ms: storage::now_ms(),
                protocol: Protocol::Openai,
                route_id: None,
                channel_id: channel.id,
                model: Some("gpt-4o".to_string()),
                success: true,
                http_status: Some(200),
                error_kind: None,
                error_detail: None,
                latency_ms: 120,
                ttft_ms: Some(30),
                prompt_tokens: Some(10),
                completion_tokens: Some(5),
                total_tokens: Some(15),
                cache_read_tokens: None,
                cache_write_tokens: None,
                estimated_cost_usd: Some("0.12".to_string()),
            },
        )
        .await
        .expect("insert usage event");

        let runtime = test_runtime(&db_path).await;
        let adapter = FakeAdapter::new(false);
        runtime
            .handle_message(
                Arc::new(adapter.clone()),
                test_message(ChatPlatform::Telegram, "tg-user-4", "/usage"),
            )
            .await;

        let output = adapter.calls().join("\n");
        assert!(output.contains("用量统计："), "{output}");
        assert!(!output.contains("```text"), "{output}");
        assert!(output.contains("- 请求数: 1"), "{output}");
        assert!(output.contains("- 预估成本: $0.12"), "{output}");
        assert!(output.contains("按渠道："), "{output}");
        assert!(output.contains("【OpenAI】"), "{output}");
        assert!(output.contains("1. openai-main"), "{output}");
        assert!(output.contains("请求数: 1 · 成功: 1 · 失败: 0"), "{output}");
        assert!(
            output.contains("Total Tokens: 15 · 成本: $0.12"),
            "{output}"
        );
        remove_sqlite_artifacts(&db_path);
    }

    #[tokio::test]
    async fn usage_command_reports_today_summary_for_weixin() {
        let db_path = temp_db_path();
        remove_sqlite_artifacts(&db_path);
        storage::init_db(&db_path).expect("init db");
        bind_test_user(&db_path, ChatPlatform::Weixin, "wx-user-4").await;
        let channel = create_test_channel(&db_path, "openai-main", Protocol::Openai).await;
        storage::insert_usage_event(
            db_path.clone(),
            storage::CreateUsageEvent {
                request_id: None,
                ts_ms: storage::now_ms(),
                protocol: Protocol::Openai,
                route_id: None,
                channel_id: channel.id,
                model: Some("gpt-4o".to_string()),
                success: true,
                http_status: Some(200),
                error_kind: None,
                error_detail: None,
                latency_ms: 120,
                ttft_ms: Some(30),
                prompt_tokens: Some(10),
                completion_tokens: Some(5),
                total_tokens: Some(15),
                cache_read_tokens: None,
                cache_write_tokens: None,
                estimated_cost_usd: Some("0.12".to_string()),
            },
        )
        .await
        .expect("insert usage event");

        let runtime = test_runtime(&db_path).await;
        let adapter = FakeAdapter::new(false);
        runtime
            .handle_message(
                Arc::new(adapter.clone()),
                test_message(ChatPlatform::Weixin, "wx-user-4", "/usage"),
            )
            .await;

        let output = adapter.calls().join("\n");
        assert!(output.contains("用量统计："), "{output}");
        assert!(!output.contains("```text"), "{output}");
        assert!(output.contains("【OpenAI】"), "{output}");
        assert!(output.contains("1. openai-main"), "{output}");
        remove_sqlite_artifacts(&db_path);
    }

    #[tokio::test]
    async fn usage_command_reports_today_summary_for_discord() {
        let db_path = temp_db_path();
        remove_sqlite_artifacts(&db_path);
        storage::init_db(&db_path).expect("init db");
        bind_test_user(&db_path, ChatPlatform::Discord, "discord-user-4").await;
        let channel = create_test_channel(&db_path, "openai-main", Protocol::Openai).await;
        storage::insert_usage_event(
            db_path.clone(),
            storage::CreateUsageEvent {
                request_id: None,
                ts_ms: storage::now_ms(),
                protocol: Protocol::Openai,
                route_id: None,
                channel_id: channel.id,
                model: Some("gpt-4o".to_string()),
                success: true,
                http_status: Some(200),
                error_kind: None,
                error_detail: None,
                latency_ms: 120,
                ttft_ms: Some(30),
                prompt_tokens: Some(10),
                completion_tokens: Some(5),
                total_tokens: Some(15),
                cache_read_tokens: None,
                cache_write_tokens: None,
                estimated_cost_usd: Some("0.12".to_string()),
            },
        )
        .await
        .expect("insert usage event");

        let runtime = test_runtime(&db_path).await;
        let adapter = FakeAdapter::new(false);
        runtime
            .handle_message(
                Arc::new(adapter.clone()),
                test_message(ChatPlatform::Discord, "discord-user-4", "/usage"),
            )
            .await;

        let output = adapter.calls().join("\n");
        assert!(output.contains("用量统计："), "{output}");
        assert!(!output.contains("```text"), "{output}");
        assert!(output.contains("【OpenAI】"), "{output}");
        assert!(output.contains("1. openai-main"), "{output}");
        remove_sqlite_artifacts(&db_path);
    }

    #[tokio::test]
    async fn costs_command_reports_summary() {
        let db_path = temp_db_path();
        remove_sqlite_artifacts(&db_path);
        storage::init_db(&db_path).expect("init db");
        bind_test_user(&db_path, ChatPlatform::Telegram, "tg-user-5").await;
        let channel = create_test_channel(&db_path, "openai-main", Protocol::Openai).await;
        storage::insert_usage_event(
            db_path.clone(),
            storage::CreateUsageEvent {
                request_id: None,
                ts_ms: storage::now_ms(),
                protocol: Protocol::Openai,
                route_id: None,
                channel_id: channel.id,
                model: Some("gpt-4o".to_string()),
                success: true,
                http_status: Some(200),
                error_kind: None,
                error_detail: None,
                latency_ms: 120,
                ttft_ms: Some(30),
                prompt_tokens: Some(10),
                completion_tokens: Some(5),
                total_tokens: Some(15),
                cache_read_tokens: None,
                cache_write_tokens: None,
                estimated_cost_usd: Some("0.12".to_string()),
            },
        )
        .await
        .expect("insert usage event");

        let runtime = test_runtime(&db_path).await;
        let adapter = FakeAdapter::new(false);
        runtime
            .handle_message(
                Arc::new(adapter.clone()),
                test_message(ChatPlatform::Telegram, "tg-user-5", "/costs"),
            )
            .await;

        let output = adapter.calls().join("\n");
        assert!(output.contains("费用报告："), "{output}");
        assert!(!output.contains("```text"), "{output}");
        assert!(output.contains("- 预估成本: $0.12"), "{output}");
        assert!(output.contains("- 价格模型数: 0"), "{output}");
        assert!(
            output.contains("当前没有价格模型数据，预估成本可能为空。"),
            "{output}"
        );
        assert!(output.contains("按渠道："), "{output}");
        assert!(output.contains("【OpenAI】"), "{output}");
        assert!(output.contains("1. openai-main"), "{output}");
        assert!(
            output.contains("成本: $0.12 · 请求数: 1 · Total Tokens: 15"),
            "{output}"
        );
        remove_sqlite_artifacts(&db_path);
    }

    #[tokio::test]
    async fn costs_command_reports_summary_for_weixin() {
        let db_path = temp_db_path();
        remove_sqlite_artifacts(&db_path);
        storage::init_db(&db_path).expect("init db");
        bind_test_user(&db_path, ChatPlatform::Weixin, "wx-user-5").await;
        let channel = create_test_channel(&db_path, "openai-main", Protocol::Openai).await;
        storage::insert_usage_event(
            db_path.clone(),
            storage::CreateUsageEvent {
                request_id: None,
                ts_ms: storage::now_ms(),
                protocol: Protocol::Openai,
                route_id: None,
                channel_id: channel.id,
                model: Some("gpt-4o".to_string()),
                success: true,
                http_status: Some(200),
                error_kind: None,
                error_detail: None,
                latency_ms: 120,
                ttft_ms: Some(30),
                prompt_tokens: Some(10),
                completion_tokens: Some(5),
                total_tokens: Some(15),
                cache_read_tokens: None,
                cache_write_tokens: None,
                estimated_cost_usd: Some("0.12".to_string()),
            },
        )
        .await
        .expect("insert usage event");

        let runtime = test_runtime(&db_path).await;
        let adapter = FakeAdapter::new(false);
        runtime
            .handle_message(
                Arc::new(adapter.clone()),
                test_message(ChatPlatform::Weixin, "wx-user-5", "/costs"),
            )
            .await;

        let output = adapter.calls().join("\n");
        assert!(output.contains("费用报告："), "{output}");
        assert!(!output.contains("```text"), "{output}");
        assert!(output.contains("【OpenAI】"), "{output}");
        assert!(output.contains("1. openai-main"), "{output}");
        remove_sqlite_artifacts(&db_path);
    }

    #[tokio::test]
    async fn costs_command_reports_summary_for_discord() {
        let db_path = temp_db_path();
        remove_sqlite_artifacts(&db_path);
        storage::init_db(&db_path).expect("init db");
        bind_test_user(&db_path, ChatPlatform::Discord, "discord-user-5").await;
        let channel = create_test_channel(&db_path, "openai-main", Protocol::Openai).await;
        storage::insert_usage_event(
            db_path.clone(),
            storage::CreateUsageEvent {
                request_id: None,
                ts_ms: storage::now_ms(),
                protocol: Protocol::Openai,
                route_id: None,
                channel_id: channel.id,
                model: Some("gpt-4o".to_string()),
                success: true,
                http_status: Some(200),
                error_kind: None,
                error_detail: None,
                latency_ms: 120,
                ttft_ms: Some(30),
                prompt_tokens: Some(10),
                completion_tokens: Some(5),
                total_tokens: Some(15),
                cache_read_tokens: None,
                cache_write_tokens: None,
                estimated_cost_usd: Some("0.12".to_string()),
            },
        )
        .await
        .expect("insert usage event");

        let runtime = test_runtime(&db_path).await;
        let adapter = FakeAdapter::new(false);
        runtime
            .handle_message(
                Arc::new(adapter.clone()),
                test_message(ChatPlatform::Discord, "discord-user-5", "/costs"),
            )
            .await;

        let output = adapter.calls().join("\n");
        assert!(output.contains("费用报告："), "{output}");
        assert!(!output.contains("```text"), "{output}");
        assert!(output.contains("【OpenAI】"), "{output}");
        assert!(output.contains("1. openai-main"), "{output}");
        remove_sqlite_artifacts(&db_path);
    }
}
