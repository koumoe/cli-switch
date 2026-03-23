use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Serialize;

use crate::chat_bridge::weixin::{WeixinControl, WeixinState};
use crate::server::AppState;

#[derive(Debug, Clone, Serialize)]
pub(in crate::server) struct WeixinStatusResponse {
    pub state: WeixinState,
    pub connected: bool,
    pub me: Option<String>,
    pub qr: Option<String>,
    pub qr_image: Option<String>,
    pub last_error: Option<String>,
}

pub(in crate::server) async fn get_chat_bridge_weixin_status(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let settings = state.settings_snapshot();
    let mut status = state.weixin_status_rx.borrow().clone();
    if !settings.chat_bridge_enabled || !settings.chat_bridge_weixin_enabled {
        status.state = WeixinState::Disabled;
        status.connected = false;
        status.qr = None;
        status.qr_image = None;
    }

    Json(WeixinStatusResponse {
        state: status.state,
        connected: status.connected,
        me: status.me,
        qr: status.qr,
        qr_image: status.qr_image,
        last_error: status.last_error,
    })
}

pub(in crate::server) async fn start_chat_bridge_weixin_login(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let settings = state.settings_snapshot();
    if !settings.chat_bridge_enabled || !settings.chat_bridge_weixin_enabled {
        return StatusCode::BAD_REQUEST.into_response();
    }

    match state
        .weixin_control_tx
        .send(WeixinControl::StartLogin)
        .await
    {
        Ok(_) => StatusCode::ACCEPTED.into_response(),
        Err(err) => {
            tracing::warn!(err = %err, "send weixin login control failed");
            StatusCode::SERVICE_UNAVAILABLE.into_response()
        }
    }
}

pub(in crate::server) async fn logout_chat_bridge_weixin(
    State(state): State<AppState>,
) -> impl IntoResponse {
    match state.weixin_control_tx.send(WeixinControl::Logout).await {
        Ok(_) => StatusCode::ACCEPTED.into_response(),
        Err(err) => {
            tracing::warn!(err = %err, "send weixin logout control failed");
            StatusCode::SERVICE_UNAVAILABLE.into_response()
        }
    }
}
