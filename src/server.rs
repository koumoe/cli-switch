use axum::Router;
use axum::routing::{any, get, post, put};
use http::Method;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::watch;

#[cfg(not(feature = "embed-ui"))]
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tower_http::trace::{DefaultOnFailure, DefaultOnResponse};

use crate::events::AppEvent;
use crate::update;
use crate::{events, storage};

mod error;
mod handlers;
mod state;
mod tasks;
mod ui;

pub use state::AppState;

fn request_endpoint_template(method: &Method, path: &str) -> Option<&'static str> {
    match (method.as_str(), path) {
        ("GET", "/api/health") => Some("/api/health"),
        ("GET", "/api/settings") => Some("/api/settings"),
        ("PUT", "/api/settings") => Some("/api/settings"),
        ("POST", "/api/maintenance/records/clear") => Some("/api/maintenance/records/clear"),
        ("POST", "/api/maintenance/logs/clear") => Some("/api/maintenance/logs/clear"),
        ("GET", "/api/maintenance/logs/size") => Some("/api/maintenance/logs/size"),
        ("GET", "/api/maintenance/db_size") => Some("/api/maintenance/db_size"),
        ("POST", "/api/logs/ingest") => Some("/api/logs/ingest"),
        ("GET", "/api/update/status") => Some("/api/update/status"),
        ("POST", "/api/update/check") => Some("/api/update/check"),
        ("POST", "/api/update/download") => Some("/api/update/download"),
        ("GET", "/api/channels") => Some("/api/channels"),
        ("POST", "/api/channels") => Some("/api/channels"),
        ("POST", "/api/channels/reorder") => Some("/api/channels/reorder"),
        ("GET", "/api/routes") => Some("/api/routes"),
        ("POST", "/api/routes") => Some("/api/routes"),
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
                ["api", "channels", _] if method == Method::PUT => Some("/api/channels/{id}"),
                ["api", "channels", _] if method == Method::DELETE => Some("/api/channels/{id}"),
                ["api", "routes", _] if method == Method::PUT => Some("/api/routes/{id}"),
                ["api", "routes", _] if method == Method::DELETE => Some("/api/routes/{id}"),
                ["api", "routes", _, "channels"] if method == Method::GET => {
                    Some("/api/routes/{id}/channels")
                }
                ["api", "routes", _, "channels", "reorder"] if method == Method::POST => {
                    Some("/api/routes/{id}/channels/reorder")
                }
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
        ("POST", "/api/maintenance/records/clear") => "handlers::records_clear",
        ("POST", "/api/maintenance/logs/clear") => "handlers::logs_clear",
        ("GET", "/api/maintenance/logs/size") => "handlers::logs_size",
        ("GET", "/api/maintenance/db_size") => "handlers::db_size",
        ("POST", "/api/logs/ingest") => "handlers::frontend_log_ingest",
        ("GET", "/api/update/status") => "handlers::update_status",
        ("POST", "/api/update/check") => "handlers::update_check",
        ("POST", "/api/update/download") => "handlers::update_download",
        ("GET", "/api/channels") => "handlers::list_channels",
        ("POST", "/api/channels") => "handlers::create_channel",
        ("POST", "/api/channels/reorder") => "handlers::reorder_channels",
        ("GET", "/api/routes") => "handlers::list_routes",
        ("POST", "/api/routes") => "handlers::create_route",
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
                ["api", "channels", _] if method == Method::PUT => "handlers::update_channel",
                ["api", "channels", _] if method == Method::DELETE => "handlers::delete_channel",
                ["api", "routes", _] if method == Method::PUT => "handlers::update_route",
                ["api", "routes", _] if method == Method::DELETE => "handlers::delete_route",
                ["api", "routes", _, "channels"] if method == Method::GET => {
                    "handlers::list_route_channels"
                }
                ["api", "routes", _, "channels", "reorder"] if method == Method::POST => {
                    "handlers::reorder_route_channels"
                }
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

            tracing::span!(
                tracing::Level::DEBUG,
                "http.request",
                method = %method,
                uri = %uri,
                path = %path,
                endpoint = endpoint,
                purpose = purpose
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
            "/api/maintenance/records/clear",
            post(handlers::records_clear),
        )
        .route("/api/maintenance/logs/clear", post(handlers::logs_clear))
        .route("/api/maintenance/logs/size", get(handlers::logs_size))
        .route("/api/maintenance/db_size", get(handlers::db_size))
        .route("/api/logs/ingest", post(handlers::frontend_log_ingest))
        .route("/api/update/status", get(handlers::update_status))
        .route("/api/update/check", post(handlers::update_check))
        .route("/api/update/download", post(handlers::update_download))
        .route(
            "/api/channels",
            get(handlers::list_channels).post(handlers::create_channel),
        )
        .route("/api/channels/reorder", post(handlers::reorder_channels))
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
            "/api/routes",
            get(handlers::list_routes).post(handlers::create_route),
        )
        .route(
            "/api/routes/{id}",
            put(handlers::update_route).delete(handlers::delete_route),
        )
        .route(
            "/api/routes/{id}/channels",
            get(handlers::list_route_channels),
        )
        .route(
            "/api/routes/{id}/channels/reorder",
            post(handlers::reorder_route_channels),
        )
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
    let http_client = reqwest::Client::builder().build()?;
    let db_path = Arc::new(db_path);
    let update_runtime = Arc::new(tokio::sync::Mutex::new(update::UpdateRuntime::default()));
    let state = AppState {
        listen_addr: addr,
        db_path: db_path.clone(),
        http_client: http_client.clone(),
        settings_notify,
        update_runtime: update_runtime.clone(),
    };

    tracing::info!(addr = %addr, open_browser, "backend server starting");

    let app = build_app(state);

    {
        let db_path = (*db_path).clone();
        let http_runtime = update_runtime.clone();
        tokio::spawn(async move {
            let settings = storage::get_app_settings(db_path.clone())
                .await
                .unwrap_or_default();
            let data_dir = crate::server::state::data_dir_from_db_path(&db_path);
            let status =
                update::get_status(http_runtime, &data_dir, settings.app_auto_update_enabled).await;
            events::publish(AppEvent::UpdateStatus(status));
        });
    }

    let settings_rx2 = settings_rx.clone();
    let settings_rx3 = settings_rx.clone();
    tokio::spawn(tasks::pricing_auto_update_loop(
        (*db_path).clone(),
        http_client.clone(),
        settings_rx,
    ));

    tokio::spawn(tasks::app_update_auto_loop(
        (*db_path).clone(),
        http_client.clone(),
        settings_rx2,
        update_runtime,
    ));

    tokio::spawn(tasks::logs_retention_cleanup_loop(
        (*db_path).clone(),
        settings_rx3,
    ));

    tokio::spawn(tasks::apply_autostart_setting((*db_path).clone()));

    if open_browser {
        let url = format!("http://{addr}");
        if let Err(e) = open_in_browser(&url) {
            tracing::warn!(url = %url, err = %e, "open browser failed");
        }
    }

    axum::serve(listener, app).await?;
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
        std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn()?;
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
