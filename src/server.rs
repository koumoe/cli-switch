use axum::Router;
use axum::middleware::from_fn_with_state;
use axum::routing::{any, delete, get, post, put};
use http::Method;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, watch};

#[cfg(not(feature = "embed-ui"))]
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tower_http::trace::{DefaultOnFailure, DefaultOnResponse};

use crate::chat_bridge::weixin::WeixinStatus;
use crate::chat_bridge::whatsapp_web::WhatsAppWebStatus;
use crate::codex_upstream;
use crate::events::AppEvent;
use crate::i18n::locale_context_middleware;
use crate::update;
use crate::{chat_bridge, events, storage};

mod error;
mod handlers;
pub(crate) mod openai_auth;
mod scheduler;
mod state;
mod sub2api_auth;
mod tasks;
mod ui;

pub use state::AppState;

fn build_http_client() -> anyhow::Result<reqwest::Client> {
    // Some providers front their panel APIs with CloudFront/WAF and reject
    // requests without a User-Agent header.
    reqwest::Client::builder()
        .user_agent(format!("CliSwitch/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(Into::into)
}

fn build_proxy_http_client() -> anyhow::Result<reqwest::Client> {
    // Keep the proxy path transparent:
    // - do not inject a default User-Agent
    // - do not auto-negotiate compression when the inbound request omitted it
    reqwest::Client::builder()
        .no_gzip()
        .no_brotli()
        .no_deflate()
        .build()
        .map_err(Into::into)
}

fn build_openai_proxy_http_client() -> anyhow::Result<reqwest::Client> {
    // ChatGPT's Cloudflare edge can issue load-balancing cookies while
    // establishing an API route. This is one shared host-scoped cookie jar for
    // all managed OpenAI requests; cookies are routing state, not credentials.
    reqwest::Client::builder()
        .cookie_store(true)
        .no_gzip()
        .no_brotli()
        .no_deflate()
        .build()
        .map_err(Into::into)
}

fn request_endpoint_template(method: &Method, path: &str) -> Option<&'static str> {
    match (method.as_str(), path) {
        ("GET", "/api/health") => Some("/api/health"),
        ("GET", "/api/settings") => Some("/api/settings"),
        ("PUT", "/api/settings") => Some("/api/settings"),
        ("GET", "/api/currency/usd-cny") => Some("/api/currency/usd-cny"),
        ("POST", "/api/maintenance/records/clear") => Some("/api/maintenance/records/clear"),
        ("POST", "/api/maintenance/logs/clear") => Some("/api/maintenance/logs/clear"),
        ("GET", "/api/maintenance/logs/size") => Some("/api/maintenance/logs/size"),
        ("GET", "/api/maintenance/db_size") => Some("/api/maintenance/db_size"),
        ("POST", "/api/logs/ingest") => Some("/api/logs/ingest"),
        ("GET", "/api/update/status") => Some("/api/update/status"),
        ("POST", "/api/update/check") => Some("/api/update/check"),
        ("POST", "/api/update/download") => Some("/api/update/download"),
        ("GET", "/api/tools/status") => Some("/api/tools/status"),
        ("POST", "/api/tools/install") => Some("/api/tools/install"),
        ("GET", "/api/tools/config/status") => Some("/api/tools/config/status"),
        ("GET", "/api/chat_bridge/bindings") => Some("/api/chat_bridge/bindings"),
        ("GET", "/api/projects") => Some("/api/projects"),
        ("DELETE", "/api/projects") => Some("/api/projects"),
        ("GET", "/api/projects/document") => Some("/api/projects/document"),
        ("PUT", "/api/projects/document") => Some("/api/projects/document"),
        ("DELETE", "/api/projects/document") => Some("/api/projects/document"),
        ("POST", "/api/tools/config/apply") => Some("/api/tools/config/apply"),
        ("POST", "/api/chat_bridge/pairing_tokens") => Some("/api/chat_bridge/pairing_tokens"),
        ("GET", "/api/chat_bridge/whatsapp/status") => Some("/api/chat_bridge/whatsapp/status"),
        ("POST", "/api/chat_bridge/whatsapp/login") => Some("/api/chat_bridge/whatsapp/login"),
        ("POST", "/api/chat_bridge/whatsapp/logout") => Some("/api/chat_bridge/whatsapp/logout"),
        ("GET", "/api/chat_bridge/weixin/status") => Some("/api/chat_bridge/weixin/status"),
        ("POST", "/api/chat_bridge/weixin/login") => Some("/api/chat_bridge/weixin/login"),
        ("POST", "/api/chat_bridge/weixin/logout") => Some("/api/chat_bridge/weixin/logout"),
        ("GET", "/api/channels") => Some("/api/channels"),
        ("POST", "/api/channels") => Some("/api/channels"),
        ("POST", "/api/channels/reorder") => Some("/api/channels/reorder"),
        ("GET", "/api/channels/checkins/today") => Some("/api/channels/checkins/today"),
        ("GET", "/api/remote/accounts") => Some("/api/remote/accounts"),
        ("POST", "/api/remote/accounts") => Some("/api/remote/accounts"),
        ("POST", "/api/remote/accounts/detect") => Some("/api/remote/accounts/detect"),
        ("POST", "/api/remote/accounts/reorder") => Some("/api/remote/accounts/reorder"),
        ("POST", "/api/openai/oauth/start") => Some("/api/openai/oauth/start"),
        ("GET", "/api/remote/accounts/checkins/today") => {
            Some("/api/remote/accounts/checkins/today")
        }
        ("POST", "/api/remote/accounts/{id}/managed_channel") => {
            Some("/api/remote/accounts/{id}/managed_channel")
        }
        ("POST", "/api/system/open") => Some("/api/system/open"),
        ("POST", "/api/system/open_data_dir") => Some("/api/system/open_data_dir"),
        ("POST", "/api/system/pick_folder") => Some("/api/system/pick_folder"),
        ("GET", "/api/pricing/status") => Some("/api/pricing/status"),
        ("GET", "/api/pricing/models") => Some("/api/pricing/models"),
        ("POST", "/api/pricing/sync") => Some("/api/pricing/sync"),
        ("GET", "/api/stats/summary") => Some("/api/stats/summary"),
        ("GET", "/api/stats/channels") => Some("/api/stats/channels"),
        ("GET", "/api/stats/trend") => Some("/api/stats/trend"),
        ("GET", "/api/usage/list") => Some("/api/usage/list"),
        _ => {
            let segments: Vec<_> = path.split('/').filter(|s| !s.is_empty()).collect();
            match segments.as_slice() {
                ["api", "channels", _, "enable"] if method == Method::POST => {
                    Some("/api/channels/{id}/enable")
                }
                ["api", "channels", _, "disable"] if method == Method::POST => {
                    Some("/api/channels/{id}/disable")
                }
                ["api", "channels", _, "test"] if method == Method::POST => {
                    Some("/api/channels/{id}/test")
                }
                ["api", "channels", _, "checkins", "complete"] if method == Method::POST => {
                    Some("/api/channels/{id}/checkins/complete")
                }
                ["api", "remote", "accounts", _] if method == Method::PUT => {
                    Some("/api/remote/accounts/{id}")
                }
                ["api", "remote", "accounts", _] if method == Method::DELETE => {
                    Some("/api/remote/accounts/{id}")
                }
                ["api", "remote", "accounts", _, "refresh"] if method == Method::POST => {
                    Some("/api/remote/accounts/{id}/refresh")
                }
                ["api", "openai", "oauth", "sessions", _] if method == Method::GET => {
                    Some("/api/openai/oauth/sessions/{request_id}")
                }
                ["api", "openai", "accounts", _, "refresh"] if method == Method::POST => {
                    Some("/api/openai/accounts/{id}/refresh")
                }
                ["api", "remote", "accounts", _, "groups"] if method == Method::GET => {
                    Some("/api/remote/accounts/{id}/groups")
                }
                ["api", "remote", "accounts", _, "keys"] if method == Method::POST => {
                    Some("/api/remote/accounts/{id}/keys")
                }
                ["api", "remote", "accounts", _, "managed_channel"] if method == Method::POST => {
                    Some("/api/remote/accounts/{id}/managed_channel")
                }
                ["api", "remote", "accounts", _, "checkins", "complete"]
                    if method == Method::POST =>
                {
                    Some("/api/remote/accounts/{id}/checkins/complete")
                }
                ["api", "remote", "accounts", _, "checkins", "system"]
                    if method == Method::POST =>
                {
                    Some("/api/remote/accounts/{id}/checkins/system")
                }
                ["api", "chat_bridge", "bindings", _] if method == Method::DELETE => {
                    Some("/api/chat_bridge/bindings/{id}")
                }
                ["api", "channels", _] if method == Method::PUT => Some("/api/channels/{id}"),
                ["api", "channels", _] if method == Method::DELETE => Some("/api/channels/{id}"),
                ["v1", "messages", ..] => Some("/v1/messages/{*path}"),
                ["v1beta", ..] => Some("/v1beta/{*path}"),
                ["v1", ..] => Some("/v1/{*path}"),
                _ => None,
            }
        }
    }
}

fn request_purpose(method: &Method, path: &str) -> &'static str {
    match (method.as_str(), path) {
        ("GET", "/api/health") => "handlers::health",
        ("GET", "/api/settings") => "handlers::get_settings",
        ("PUT", "/api/settings") => "handlers::update_settings",
        ("GET", "/api/currency/usd-cny") => "handlers::usd_cny_exchange_rate",
        ("POST", "/api/maintenance/records/clear") => "handlers::records_clear",
        ("POST", "/api/maintenance/logs/clear") => "handlers::logs_clear",
        ("GET", "/api/maintenance/logs/size") => "handlers::logs_size",
        ("GET", "/api/maintenance/db_size") => "handlers::db_size",
        ("POST", "/api/logs/ingest") => "handlers::frontend_log_ingest",
        ("GET", "/api/update/status") => "handlers::update_status",
        ("POST", "/api/update/check") => "handlers::update_check",
        ("POST", "/api/update/download") => "handlers::update_download",
        ("GET", "/api/tools/status") => "handlers::cli_tools_status",
        ("POST", "/api/tools/install") => "handlers::install_cli_tool",
        ("GET", "/api/tools/config/status") => "handlers::cli_tools_proxy_config_status",
        ("POST", "/api/tools/config/apply") => "handlers::cli_tools_proxy_config_apply",
        ("GET", "/api/chat_bridge/bindings") => "handlers::list_chat_bridge_bindings",
        ("POST", "/api/chat_bridge/pairing_tokens") => "handlers::create_chat_bridge_pairing_token",
        ("GET", "/api/chat_bridge/whatsapp/status") => "handlers::get_chat_bridge_whatsapp_status",
        ("POST", "/api/chat_bridge/whatsapp/login") => "handlers::start_chat_bridge_whatsapp_login",
        ("POST", "/api/chat_bridge/whatsapp/logout") => "handlers::logout_chat_bridge_whatsapp",
        ("GET", "/api/chat_bridge/weixin/status") => "handlers::get_chat_bridge_weixin_status",
        ("POST", "/api/chat_bridge/weixin/login") => "handlers::start_chat_bridge_weixin_login",
        ("POST", "/api/chat_bridge/weixin/logout") => "handlers::logout_chat_bridge_weixin",
        ("GET", "/api/projects") => "handlers::list_projects",
        ("DELETE", "/api/projects") => "handlers::delete_project",
        ("GET", "/api/projects/document") => "handlers::get_project_document",
        ("PUT", "/api/projects/document") => "handlers::save_project_document",
        ("DELETE", "/api/projects/document") => "handlers::delete_project_document",
        ("GET", "/api/channels") => "handlers::list_channels",
        ("POST", "/api/channels") => "handlers::create_channel",
        ("POST", "/api/channels/reorder") => "handlers::reorder_channels",
        ("GET", "/api/channels/checkins/today") => "handlers::channel_checkins_today",
        ("GET", "/api/remote/accounts") => "handlers::list_remote_accounts",
        ("POST", "/api/remote/accounts") => "handlers::create_remote_account",
        ("POST", "/api/remote/accounts/detect") => "handlers::detect_remote_account",
        ("POST", "/api/remote/accounts/reorder") => "handlers::reorder_remote_accounts",
        ("POST", "/api/openai/oauth/start") => "handlers::start_openai_oauth",
        ("GET", "/api/remote/accounts/checkins/today") => "handlers::remote_account_checkins_today",
        ("POST", "/api/remote/accounts/{id}/managed_channel") => {
            "handlers::create_remote_managed_channel"
        }
        ("POST", "/api/system/open") => "handlers::open_in_browser",
        ("POST", "/api/system/open_data_dir") => "handlers::open_data_dir",
        ("POST", "/api/system/pick_folder") => "handlers::pick_folder",
        ("GET", "/api/pricing/status") => "handlers::pricing_status",
        ("GET", "/api/pricing/models") => "handlers::pricing_models",
        ("POST", "/api/pricing/sync") => "handlers::pricing_sync",
        ("GET", "/api/stats/summary") => "handlers::stats_summary",
        ("GET", "/api/stats/channels") => "handlers::stats_channels",
        ("GET", "/api/stats/trend") => "handlers::stats_trend",
        ("GET", "/api/usage/list") => "handlers::usage_list",
        _ => {
            let segments: Vec<_> = path.split('/').filter(|s| !s.is_empty()).collect();
            match segments.as_slice() {
                ["api", "channels", _, "enable"] if method == Method::POST => {
                    "handlers::enable_channel"
                }
                ["api", "channels", _, "disable"] if method == Method::POST => {
                    "handlers::disable_channel"
                }
                ["api", "channels", _, "test"] if method == Method::POST => {
                    "handlers::test_channel"
                }
                ["api", "channels", _, "checkins", "complete"] if method == Method::POST => {
                    "handlers::complete_channel_checkin_today"
                }
                ["api", "remote", "accounts", _] if method == Method::PUT => {
                    "handlers::update_remote_account"
                }
                ["api", "remote", "accounts", _] if method == Method::DELETE => {
                    "handlers::delete_remote_account"
                }
                ["api", "remote", "accounts", _, "refresh"] if method == Method::POST => {
                    "handlers::refresh_remote_account"
                }
                ["api", "openai", "oauth", "sessions", _] if method == Method::GET => {
                    "handlers::get_openai_oauth_status"
                }
                ["api", "openai", "accounts", _, "refresh"] if method == Method::POST => {
                    "handlers::refresh_openai_account"
                }
                ["api", "remote", "accounts", _, "groups"] if method == Method::GET => {
                    "handlers::list_remote_account_groups"
                }
                ["api", "remote", "accounts", _, "keys"] if method == Method::POST => {
                    "handlers::create_remote_account_key"
                }
                ["api", "remote", "accounts", _, "checkins", "complete"]
                    if method == Method::POST =>
                {
                    "handlers::complete_remote_account_checkin_today"
                }
                ["api", "remote", "accounts", _, "checkins", "system"]
                    if method == Method::POST =>
                {
                    "handlers::perform_remote_account_system_checkin"
                }
                ["api", "chat_bridge", "bindings", _] if method == Method::DELETE => {
                    "handlers::deactivate_chat_bridge_binding"
                }
                ["api", "channels", _] if method == Method::PUT => "handlers::update_channel",
                ["api", "channels", _] if method == Method::DELETE => "handlers::delete_channel",
                ["v1", "messages", ..] => "handlers::proxy_anthropic",
                ["v1beta", ..] => "handlers::proxy_gemini",
                ["v1", ..] => "handlers::proxy_openai",
                ["assets", ..] => "ServeDir",
                ["api", ..] => "unknown_api",
                _ => "ui",
            }
        }
    }
}

fn build_app(state: AppState) -> Router {
    let trace_layer = TraceLayer::new_for_http()
        .make_span_with(|request: &http::Request<_>| {
            let method = request.method();
            let uri = request.uri();
            let path = uri.path();
            let endpoint = request_endpoint_template(method, path).unwrap_or(path);
            let purpose = request_purpose(method, path);
            let user_agent = request
                .headers()
                .get(http::header::USER_AGENT)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("-");

            tracing::span!(
                tracing::Level::DEBUG,
                "http.request",
                method = %method,
                uri = %uri,
                path = %path,
                endpoint = endpoint,
                purpose = purpose,
                user_agent = %user_agent
            )
        })
        .on_response(DefaultOnResponse::new().level(tracing::Level::DEBUG))
        .on_failure(DefaultOnFailure::new().level(tracing::Level::WARN));

    let traced_api = Router::new()
        .route("/api/health", get(handlers::health))
        .route(
            "/api/settings",
            get(handlers::get_settings).put(handlers::update_settings),
        )
        .route(
            "/api/currency/usd-cny",
            get(handlers::usd_cny_exchange_rate),
        )
        .route(
            "/api/maintenance/records/clear",
            post(handlers::records_clear),
        )
        .route("/api/maintenance/logs/clear", post(handlers::logs_clear))
        .route("/api/maintenance/logs/size", get(handlers::logs_size))
        .route("/api/maintenance/db_size", get(handlers::db_size))
        .route("/api/logs/ingest", post(handlers::frontend_log_ingest))
        .route("/api/update/status", get(handlers::update_status))
        .route("/api/update/check", post(handlers::update_check))
        .route("/api/update/changelog", get(handlers::update_changelog))
        .route("/api/update/download", post(handlers::update_download))
        .route("/api/update/ignore", post(handlers::update_ignore))
        .route("/api/tools/status", get(handlers::cli_tools_status))
        .route("/api/tools/install", post(handlers::install_cli_tool))
        .route(
            "/api/tools/config/status",
            get(handlers::cli_tools_proxy_config_status),
        )
        .route(
            "/api/tools/config/apply",
            post(handlers::cli_tools_proxy_config_apply),
        )
        .route(
            "/api/chat_bridge/bindings",
            get(handlers::list_chat_bridge_bindings),
        )
        .route(
            "/api/chat_bridge/bindings/{id}",
            delete(handlers::deactivate_chat_bridge_binding),
        )
        .route(
            "/api/chat_bridge/pairing_tokens",
            post(handlers::create_chat_bridge_pairing_token),
        )
        .route(
            "/api/chat_bridge/whatsapp/status",
            get(handlers::get_chat_bridge_whatsapp_status),
        )
        .route(
            "/api/chat_bridge/whatsapp/login",
            post(handlers::start_chat_bridge_whatsapp_login),
        )
        .route(
            "/api/chat_bridge/whatsapp/logout",
            post(handlers::logout_chat_bridge_whatsapp),
        )
        .route(
            "/api/chat_bridge/weixin/status",
            get(handlers::get_chat_bridge_weixin_status),
        )
        .route(
            "/api/chat_bridge/weixin/login",
            post(handlers::start_chat_bridge_weixin_login),
        )
        .route(
            "/api/chat_bridge/weixin/logout",
            post(handlers::logout_chat_bridge_weixin),
        )
        .route(
            "/api/projects",
            get(handlers::list_projects).delete(handlers::delete_project),
        )
        .route(
            "/api/projects/document",
            get(handlers::get_project_document)
                .put(handlers::save_project_document)
                .delete(handlers::delete_project_document),
        )
        .route(
            "/api/channels",
            get(handlers::list_channels).post(handlers::create_channel),
        )
        .route("/api/channels/reorder", post(handlers::reorder_channels))
        .route(
            "/api/channels/checkins/today",
            get(handlers::channel_checkins_today),
        )
        .route(
            "/api/remote/accounts",
            get(handlers::list_remote_accounts).post(handlers::create_remote_account),
        )
        .route(
            "/api/remote/accounts/detect",
            post(handlers::detect_remote_account),
        )
        .route(
            "/api/remote/accounts/reorder",
            post(handlers::reorder_remote_accounts),
        )
        .route(
            "/api/openai/oauth/start",
            post(handlers::start_openai_oauth),
        )
        .route(
            "/api/openai/oauth/sessions/{request_id}",
            get(handlers::get_openai_oauth_status),
        )
        .route(
            "/api/openai/accounts/{id}/refresh",
            post(handlers::refresh_openai_account),
        )
        .route(
            "/api/remote/accounts/checkins/today",
            get(handlers::remote_account_checkins_today),
        )
        .route(
            "/api/remote/accounts/{id}",
            put(handlers::update_remote_account).delete(handlers::delete_remote_account),
        )
        .route(
            "/api/remote/accounts/{id}/refresh",
            post(handlers::refresh_remote_account),
        )
        .route(
            "/api/remote/accounts/{id}/groups",
            get(handlers::list_remote_account_groups),
        )
        .route(
            "/api/remote/accounts/{id}/keys",
            post(handlers::create_remote_account_key),
        )
        .route(
            "/api/remote/accounts/{id}/managed_channel",
            post(handlers::create_remote_managed_channel),
        )
        .route(
            "/api/remote/accounts/{id}/checkins/complete",
            post(handlers::complete_remote_account_checkin_today),
        )
        .route(
            "/api/remote/accounts/{id}/checkins/system",
            post(handlers::perform_remote_account_system_checkin),
        )
        .route(
            "/api/channels/{id}",
            put(handlers::update_channel).delete(handlers::delete_channel),
        )
        .route("/api/channels/{id}/enable", post(handlers::enable_channel))
        .route(
            "/api/channels/{id}/disable",
            post(handlers::disable_channel),
        )
        .route("/api/channels/{id}/test", post(handlers::test_channel))
        .route(
            "/api/channels/{id}/checkins/complete",
            post(handlers::complete_channel_checkin_today),
        )
        .route("/api/system/open", post(handlers::open_in_browser))
        .route("/api/system/open_data_dir", post(handlers::open_data_dir))
        .route("/api/system/pick_folder", post(handlers::pick_folder))
        .route("/api/pricing/status", get(handlers::pricing_status))
        .route("/api/pricing/models", get(handlers::pricing_models))
        .route("/api/pricing/sync", post(handlers::pricing_sync))
        .route("/api/stats/summary", get(handlers::stats_summary))
        .route("/api/stats/channels", get(handlers::stats_channels))
        .route("/api/stats/trend", get(handlers::stats_trend))
        .route("/api/usage/list", get(handlers::usage_list))
        .route("/v1/messages", any(handlers::proxy_anthropic))
        .route("/v1/messages/{*path}", any(handlers::proxy_anthropic))
        .route("/v1beta/{*path}", any(handlers::proxy_gemini))
        .route("/v1/{*path}", any(handlers::proxy_openai))
        .layer(from_fn_with_state(state.clone(), locale_context_middleware))
        .layer(trace_layer);

    let app = Router::new().merge(traced_api).with_state(state);

    #[cfg(feature = "embed-ui")]
    let app = app.fallback(any(ui::ui_fallback));

    #[cfg(not(feature = "embed-ui"))]
    let app = {
        let dist = std::path::PathBuf::from("ui/dist");
        if dist.is_dir() {
            app.fallback(any(ui::ui_fs_fallback)).nest_service(
                "/assets",
                ServeDir::new(dist.join("assets")).append_index_html_on_directories(false),
            )
        } else {
            app.route("/", get(ui::ui_placeholder))
                .fallback(any(ui::ui_placeholder))
        }
    };

    app
}

pub async fn serve_with_listener(
    listener: tokio::net::TcpListener,
    db_path: PathBuf,
    open_browser: bool,
) -> anyhow::Result<()> {
    let addr = listener.local_addr()?;
    let (settings_notify, settings_rx) = watch::channel(0u64);
    let http_client = build_http_client()?;
    let proxy_http_client = build_proxy_http_client()?;
    let openai_proxy_http_client = build_openai_proxy_http_client()?;
    let db_path = Arc::new(db_path);

    let settings0 = storage::get_app_settings((*db_path).clone()).await?;
    let initial_update_locale = settings0.ui_locale;
    let channels0 = storage::list_channels((*db_path).clone()).await?;
    let codex_version = {
        let npm_path = settings0.cli_tools_npm_path.clone();
        let node_path = settings0.cli_tools_node_path.clone();
        let data_dir = state::data_dir_from_db_path(db_path.as_path());
        tokio::task::spawn_blocking(move || {
            crate::cli_tools::detect_codex_version(
                npm_path.as_deref(),
                node_path.as_deref(),
                &data_dir,
            )
        })
        .await
        .unwrap_or(None)
    };
    let codex_identity = Arc::new(codex_upstream::identity_for_version(
        codex_version.as_deref(),
    ));
    tracing::info!(
        version = %codex_identity.version,
        detected_version = ?codex_version,
        "cached Codex client identity"
    );
    let (codex_identity_cache, codex_identity_cache_rx) = watch::channel(codex_identity);
    let (settings_cache, settings_cache_rx) = watch::channel(Arc::new(settings0));
    let (channels_cache, channels_cache_rx) = watch::channel(Arc::new(channels0));
    let (whatsapp_control_tx, whatsapp_control_rx) = mpsc::channel(32);
    let (whatsapp_status_tx, whatsapp_status_rx) = watch::channel(WhatsAppWebStatus::disabled());
    let (weixin_control_tx, weixin_control_rx) = mpsc::channel(32);
    let (weixin_status_tx, weixin_status_rx) = watch::channel(WeixinStatus::disabled());

    let update_runtime = Arc::new(tokio::sync::Mutex::new(update::UpdateRuntime {
        locale: initial_update_locale,
        ..Default::default()
    }));
    let state = AppState {
        listen_addr: addr,
        db_path: db_path.clone(),
        http_client: http_client.clone(),
        proxy_http_client,
        openai_proxy_http_client,
        codex_identity_cache,
        codex_identity_cache_rx,
        settings_notify,
        settings_cache,
        settings_cache_rx,
        channels_cache,
        channels_cache_rx,
        update_runtime: update_runtime.clone(),
        whatsapp_control_tx,
        whatsapp_status_rx,
        weixin_control_tx,
        weixin_status_rx,
    };

    tracing::info!(addr = %addr, open_browser, "backend server starting");

    let chat_bridge_settings_rx = state.settings_cache_rx.clone();
    let chat_bridge_channels_cache = state.channels_cache.clone();
    let codex_identity_cache = state.codex_identity_cache.clone();
    let app = build_app(state);

    let mut bg = tokio::task::JoinSet::<()>::new();

    {
        let db_path = (*db_path).clone();
        let http_runtime = update_runtime.clone();
        bg.spawn(async move {
            let settings = match storage::get_app_settings(db_path.clone()).await {
                Ok(s) => s,
                Err(e) => {
                    // Avoid guessing the update status when settings are unavailable.
                    tracing::warn!(
                        err = %e,
                        "load app settings failed; skip initial update status publish"
                    );
                    return;
                }
            };
            let data_dir = crate::server::state::data_dir_from_db_path(&db_path);
            let status = update::get_status(
                http_runtime,
                db_path,
                &data_dir,
                settings.app_auto_update_enabled,
            )
            .await;
            events::publish(AppEvent::UpdateStatus(status));
        });
    }

    let settings_rx2 = settings_rx.clone();
    let settings_rx3 = settings_rx.clone();
    let settings_rx4 = settings_rx.clone();
    let settings_rx5 = settings_rx.clone();
    let settings_rx6 = settings_rx.clone();
    bg.spawn(tasks::pricing_auto_update_loop(
        (*db_path).clone(),
        http_client.clone(),
        settings_rx,
    ));

    bg.spawn(tasks::app_update_auto_loop(
        (*db_path).clone(),
        http_client.clone(),
        settings_rx2,
        update_runtime,
    ));

    bg.spawn(tasks::cli_tools_auto_update_loop(
        (*db_path).clone(),
        settings_rx4,
        codex_identity_cache,
    ));

    bg.spawn(tasks::logs_retention_cleanup_loop(
        (*db_path).clone(),
        settings_rx3,
    ));

    bg.spawn(tasks::newapi_auto_checkin_loop(
        (*db_path).clone(),
        http_client.clone(),
        settings_rx5,
    ));

    bg.spawn(tasks::remote_accounts_maintenance_loop(
        (*db_path).clone(),
        http_client.clone(),
        settings_rx6,
    ));

    bg.spawn(tasks::apply_autostart_setting((*db_path).clone()));
    bg.spawn(chat_bridge::run_supervisor(
        (*db_path).clone(),
        http_client.clone(),
        chat_bridge_settings_rx,
        Some(chat_bridge_channels_cache),
        chat_bridge::SupervisorChannels {
            whatsapp_control_rx,
            whatsapp_status_tx,
            weixin_control_rx,
            weixin_status_tx,
        },
    ));

    if open_browser {
        let url = format!("http://{addr}");
        if let Err(e) = open_in_browser(&url) {
            tracing::warn!(url = %url, err = %e, "open browser failed");
        }
    }

    let res = axum::serve(listener, app).await;

    // Best-effort stop background tasks when the server ends.
    bg.abort_all();
    while let Some(joined) = bg.join_next().await {
        if let Err(e) = joined {
            tracing::debug!(err = %e, "background task ended");
        }
    }

    res?;
    Ok(())
}

pub async fn serve(addr: SocketAddr, db_path: PathBuf, open_browser: bool) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    serve_with_listener(listener, db_path, open_browser).await
}

fn open_in_browser(url: &str) -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(url).spawn()?;
        Ok(())
    }
    #[cfg(target_os = "windows")]
    {
        let mut cmd = std::process::Command::new("cmd");
        cmd.args(["/C", "start", "", url]);
        crate::process::command_silent(&mut cmd);
        cmd.spawn()?;
        Ok(())
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open").arg(url).spawn()?;
        Ok(())
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = url;
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "unsupported platform",
        ))
    }
}

fn open_path(path: &std::path::Path) -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(path).spawn()?;
        Ok(())
    }
    #[cfg(target_os = "windows")]
    {
        let mut cmd = std::process::Command::new("cmd");
        cmd.args(["/C", "start", "", &path.to_string_lossy().to_string()]);
        crate::process::command_silent(&mut cmd);
        cmd.spawn()?;
        Ok(())
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open").arg(path).spawn()?;
        Ok(())
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = path;
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "unsupported platform",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{build_http_client, build_openai_proxy_http_client, build_proxy_http_client};
    use axum::Router;
    use axum::extract::State;
    use axum::http::{
        HeaderMap, StatusCode,
        header::{ACCEPT_ENCODING, COOKIE, SET_COOKIE, USER_AGENT},
    };
    use axum::routing::get;

    #[tokio::test]
    async fn build_http_client_sets_user_agent_for_outbound_requests() {
        async fn handler(
            State(expected): State<String>,
            headers: HeaderMap,
        ) -> (StatusCode, String) {
            let actual = headers
                .get(USER_AGENT)
                .and_then(|value| value.to_str().ok())
                .unwrap_or("")
                .to_string();
            if actual == expected {
                (StatusCode::OK, actual)
            } else {
                (StatusCode::BAD_REQUEST, actual)
            }
        }

        let expected = format!("CliSwitch/{}", env!("CARGO_PKG_VERSION"));
        let app = Router::new()
            .route("/", get(handler))
            .with_state(expected.clone());
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("bind test server");
        let addr = listener.local_addr().expect("read local addr");
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        let response = build_http_client()
            .expect("build client")
            .get(format!("http://127.0.0.1:{}/", addr.port()))
            .send()
            .await
            .expect("send request");

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.text().await.expect("read body"), expected);
    }

    #[tokio::test]
    async fn build_proxy_http_client_does_not_inject_user_agent_or_accept_encoding() {
        async fn handler(headers: HeaderMap) -> String {
            let user_agent = headers
                .get(USER_AGENT)
                .and_then(|value| value.to_str().ok())
                .unwrap_or("-");
            let accept_encoding = headers
                .get(ACCEPT_ENCODING)
                .and_then(|value| value.to_str().ok())
                .unwrap_or("-");
            format!("ua={user_agent};ae={accept_encoding}")
        }

        let app = Router::new().route("/", get(handler));
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("bind test server");
        let addr = listener.local_addr().expect("read local addr");
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        let response = build_proxy_http_client()
            .expect("build proxy client")
            .get(format!("http://127.0.0.1:{}/", addr.port()))
            .send()
            .await
            .expect("send request");

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.text().await.expect("read body"), "ua=-;ae=-");
    }

    #[tokio::test]
    async fn build_openai_proxy_http_client_keeps_edge_cookies() {
        async fn set_cookie() -> (HeaderMap, &'static str) {
            let mut headers = HeaderMap::new();
            headers.insert(SET_COOKIE, "edge-route=ready; Path=/".parse().unwrap());
            (headers, "ok")
        }
        async fn read_cookie(headers: HeaderMap) -> String {
            headers
                .get(COOKIE)
                .and_then(|value| value.to_str().ok())
                .unwrap_or_default()
                .to_string()
        }

        let app = Router::new()
            .route("/set", get(set_cookie))
            .route("/read", get(read_cookie));
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("bind test server");
        let addr = listener.local_addr().expect("read local addr");
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        let client = build_openai_proxy_http_client().expect("build OpenAI proxy client");
        client
            .get(format!("http://127.0.0.1:{}/set", addr.port()))
            .send()
            .await
            .expect("set edge cookie");
        let cookie = client
            .get(format!("http://127.0.0.1:{}/read", addr.port()))
            .send()
            .await
            .expect("read edge cookie")
            .text()
            .await
            .expect("read response body");

        assert_eq!(cookie, "edge-route=ready");
    }
}
