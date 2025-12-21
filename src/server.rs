use axum::Router;
use axum::routing::{any, get, post, put};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::watch;

#[cfg(not(feature = "embed-ui"))]
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tower_http::trace::{DefaultMakeSpan, DefaultOnFailure, DefaultOnResponse};

use crate::update;

mod error;
mod handlers;
mod state;
mod tasks;
mod ui;

pub use state::AppState;

fn build_app(state: AppState) -> Router {
    let app = Router::new()
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
        .with_state(state)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().level(tracing::Level::DEBUG))
                .on_response(DefaultOnResponse::new().level(tracing::Level::DEBUG))
                .on_failure(DefaultOnFailure::new().level(tracing::Level::WARN)),
        );

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

    let settings_rx2 = settings_rx.clone();
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
