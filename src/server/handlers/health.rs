use axum::Json;
use axum::extract::State;
use axum::response::IntoResponse;
use serde::Serialize;

use crate::server::AppState;

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    version: &'static str,
    listen_addr: String,
    data_dir: String,
    db_path: String,
}

pub(in crate::server) async fn health(State(state): State<AppState>) -> impl IntoResponse {
    let data_dir = state
        .db_path
        .parent()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "-".to_string());
    Json(HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
        listen_addr: state.listen_addr.to_string(),
        data_dir,
        db_path: state.db_path.display().to_string(),
    })
}
