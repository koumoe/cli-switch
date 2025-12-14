#[cfg(not(feature = "embed-ui"))]
use axum::response::Html;
use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{any, get, post, put},
};
use serde::Serialize;
use std::{net::SocketAddr, path::PathBuf, sync::Arc};
#[cfg(not(feature = "embed-ui"))]
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

use crate::storage;

#[derive(Clone)]
pub struct AppState {
    pub db_path: Arc<PathBuf>,
}

#[derive(Serialize)]
struct HealthResponse<'a> {
    status: &'a str,
}

async fn health() -> impl IntoResponse {
    Json(HealthResponse { status: "ok" })
}

#[cfg(not(feature = "embed-ui"))]
async fn ui_placeholder() -> impl IntoResponse {
    Html(
        r#"<!doctype html>
<html>
  <head><meta charset="utf-8"><title>CliSwitch</title></head>
  <body>
    <h1>CliSwitch</h1>
    <p>UI 尚未构建或未启用内嵌（feature <code>embed-ui</code>）。</p>
    <p>开发：先构建 <code>ui</code>，再启动后端，或直接用 Vite dev server。</p>
    <p>健康检查：<a href="/api/health">/api/health</a></p>
  </body>
</html>"#,
    )
}

async fn proxy_placeholder(State(_state): State<AppState>) -> impl IntoResponse {
    (StatusCode::NOT_IMPLEMENTED, "proxy not implemented yet")
}

#[cfg(feature = "embed-ui")]
#[derive(rust_embed::RustEmbed)]
#[folder = "ui/dist"]
struct UiDist;

#[cfg(feature = "embed-ui")]
async fn ui_fallback(uri: axum::extract::OriginalUri) -> impl IntoResponse {
    let mut path = uri.0.path().trim_start_matches('/').to_string();
    if path.is_empty() {
        path = "index.html".to_string();
    }

    if let Some(asset) = UiDist::get(&path) {
        let mime = mime_guess::from_path(&path).first_or_octet_stream();
        let body = axum::body::Bytes::from(asset.data.into_owned());
        return ([(axum::http::header::CONTENT_TYPE, mime.as_ref())], body).into_response();
    }

    StatusCode::NOT_FOUND.into_response()
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

#[derive(thiserror::Error, Debug)]
enum ApiError {
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    NotFound(String),
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, msg) = match &self {
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            ApiError::Internal(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal error".to_string(),
            ),
        };
        (status, Json(ErrorBody { error: msg })).into_response()
    }
}

async fn list_channels(State(state): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    let channels = storage::list_channels((*state.db_path).clone()).await?;
    Ok(Json(channels))
}

async fn create_channel(
    State(state): State<AppState>,
    Json(input): Json<storage::CreateChannel>,
) -> Result<impl IntoResponse, ApiError> {
    if input.name.trim().is_empty() {
        return Err(ApiError::BadRequest("name 不能为空".to_string()));
    }
    if input.base_url.trim().is_empty() {
        return Err(ApiError::BadRequest("base_url 不能为空".to_string()));
    }

    let channel = storage::create_channel((*state.db_path).clone(), input).await?;
    Ok((StatusCode::CREATED, Json(channel)))
}

async fn update_channel(
    State(state): State<AppState>,
    axum::extract::Path(channel_id): axum::extract::Path<String>,
    Json(input): Json<storage::UpdateChannel>,
) -> Result<impl IntoResponse, ApiError> {
    let res = storage::update_channel((*state.db_path).clone(), channel_id, input).await;
    match res {
        Ok(()) => Ok(StatusCode::NO_CONTENT),
        Err(e) if e.to_string().starts_with("channel not found") => {
            Err(ApiError::NotFound("channel not found".to_string()))
        }
        Err(e) => Err(ApiError::Internal(e)),
    }
}

async fn enable_channel(
    State(state): State<AppState>,
    axum::extract::Path(channel_id): axum::extract::Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    storage::set_channel_enabled((*state.db_path).clone(), channel_id, true).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn disable_channel(
    State(state): State<AppState>,
    axum::extract::Path(channel_id): axum::extract::Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    storage::set_channel_enabled((*state.db_path).clone(), channel_id, false).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_routes(State(state): State<AppState>) -> Result<impl IntoResponse, ApiError> {
    let routes = storage::list_routes((*state.db_path).clone()).await?;
    Ok(Json(routes))
}

async fn create_route(
    State(state): State<AppState>,
    Json(input): Json<storage::CreateRoute>,
) -> Result<impl IntoResponse, ApiError> {
    if input.name.trim().is_empty() {
        return Err(ApiError::BadRequest("name 不能为空".to_string()));
    }
    let route = storage::create_route((*state.db_path).clone(), input).await?;
    Ok((StatusCode::CREATED, Json(route)))
}

async fn update_route(
    State(state): State<AppState>,
    axum::extract::Path(route_id): axum::extract::Path<String>,
    Json(input): Json<storage::UpdateRoute>,
) -> Result<impl IntoResponse, ApiError> {
    let res = storage::update_route((*state.db_path).clone(), route_id, input).await;
    match res {
        Ok(()) => Ok(StatusCode::NO_CONTENT),
        Err(e) if e.to_string().starts_with("route not found") => {
            Err(ApiError::NotFound("route not found".to_string()))
        }
        Err(e) => Err(ApiError::Internal(e)),
    }
}

pub async fn serve(addr: SocketAddr, db_path: PathBuf) -> anyhow::Result<()> {
    let state = AppState {
        db_path: Arc::new(db_path),
    };

    let app = Router::new()
        .route("/api/health", get(health))
        .route("/api/channels", get(list_channels).post(create_channel))
        .route("/api/channels/:id", put(update_channel))
        .route("/api/channels/:id/enable", post(enable_channel))
        .route("/api/channels/:id/disable", post(disable_channel))
        .route("/api/routes", get(list_routes).post(create_route))
        .route("/api/routes/:id", put(update_route))
        .route("/v1/*path", any(proxy_placeholder))
        .route("/anthropic/*path", any(proxy_placeholder))
        .route("/gemini/*path", any(proxy_placeholder))
        .with_state(state)
        .layer(TraceLayer::new_for_http());

    #[cfg(feature = "embed-ui")]
    let app = app.fallback(any(ui_fallback));

    #[cfg(not(feature = "embed-ui"))]
    let app = {
        let dist = std::path::PathBuf::from("ui/dist");
        if dist.is_dir() {
            app.fallback_service(ServeDir::new(dist).append_index_html_on_directories(true))
        } else {
            app.route("/", get(ui_placeholder))
                .fallback(any(ui_placeholder))
        }
    };

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
