use axum::body::Bytes;
use axum::extract::OriginalUri;
use axum::http::StatusCode;
use axum::response::IntoResponse;

#[cfg(not(feature = "embed-ui"))]
use axum::response::Html;

#[cfg(not(feature = "embed-ui"))]
pub(crate) async fn ui_placeholder() -> impl IntoResponse {
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

#[cfg(not(feature = "embed-ui"))]
pub(crate) async fn ui_fs_fallback(uri: OriginalUri) -> impl IntoResponse {
    let dist = std::path::PathBuf::from("ui/dist");
    if !dist.is_dir() {
        return ui_placeholder().await.into_response();
    }

    let mut path = uri.0.path().trim_start_matches('/').to_string();
    if path.is_empty() {
        path = "index.html".to_string();
    }

    if path.contains("..") || path.contains('\\') {
        return StatusCode::NOT_FOUND.into_response();
    }

    let candidate = dist.join(&path);
    if candidate.is_file() {
        match tokio::fs::read(&candidate).await {
            Ok(bytes) => {
                let mime = mime_guess::from_path(&path).first_or_octet_stream();
                return (
                    [(axum::http::header::CONTENT_TYPE, mime.as_ref())],
                    Bytes::from(bytes),
                )
                    .into_response();
            }
            Err(_) => return StatusCode::NOT_FOUND.into_response(),
        }
    }

    let is_asset_like = path.starts_with("assets/") || path.contains('.');
    if is_asset_like {
        return StatusCode::NOT_FOUND.into_response();
    }

    let index = dist.join("index.html");
    match tokio::fs::read(&index).await {
        Ok(bytes) => (
            [(axum::http::header::CONTENT_TYPE, "text/html")],
            Bytes::from(bytes),
        )
            .into_response(),
        Err(_) => ui_placeholder().await.into_response(),
    }
}

#[cfg(feature = "embed-ui")]
#[derive(rust_embed::RustEmbed)]
#[folder = "ui/dist"]
struct UiDist;

#[cfg(feature = "embed-ui")]
pub(crate) async fn ui_fallback(uri: OriginalUri) -> impl IntoResponse {
    let mut path = uri.0.path().trim_start_matches('/').to_string();
    if path.is_empty() {
        path = "index.html".to_string();
    }

    if let Some(asset) = UiDist::get(&path) {
        let mime = mime_guess::from_path(&path).first_or_octet_stream();
        let body = Bytes::from(asset.data.into_owned());
        return ([(axum::http::header::CONTENT_TYPE, mime.as_ref())], body).into_response();
    }

    let is_asset_like = path.starts_with("assets/") || path.contains('.');
    if is_asset_like {
        return StatusCode::NOT_FOUND.into_response();
    }

    if let Some(index) = UiDist::get("index.html") {
        let body = Bytes::from(index.data.into_owned());
        return ([(axum::http::header::CONTENT_TYPE, "text/html")], body).into_response();
    }

    StatusCode::NOT_FOUND.into_response()
}
