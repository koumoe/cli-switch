use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Serialize;

use crate::chat_bridge::whatsapp_web::{WhatsAppWebControl, WhatsAppWebState};
use crate::i18n::{UserFacingIssue, UserFacingIssuePayload, current_locale};
use crate::server::AppState;

#[derive(Debug, Clone, Serialize)]
pub(in crate::server) struct WhatsAppStatusResponse {
    pub state: WhatsAppWebState,
    pub connected: bool,
    pub me: Option<String>,
    pub qr: Option<String>,
    pub qr_image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issue: Option<UserFacingIssuePayload>,
}

pub(in crate::server) async fn get_chat_bridge_whatsapp_status(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let settings = state.settings_snapshot();
    let locale = current_locale().unwrap_or(settings.ui_locale);
    let mut status = state.whatsapp_status_rx.borrow().clone();
    if !settings.chat_bridge_enabled || !settings.chat_bridge_whatsapp_enabled {
        status.state = WhatsAppWebState::Disabled;
        status.connected = false;
        status.qr = None;
        status.qr_image = None;
        status.last_error = None;
    }

    let issue = status.last_error.as_ref().map(|_| {
        UserFacingIssue::new("chat_bridge_runtime_unavailable")
            .with_arg("platform", "WhatsApp")
            .to_payload(locale)
    });

    Json(WhatsAppStatusResponse {
        qr_image: status.qr_image,
        state: status.state,
        connected: status.connected,
        me: status.me,
        qr: status.qr,
        issue,
    })
}

pub(in crate::server) async fn start_chat_bridge_whatsapp_login(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let settings = state.settings_snapshot();
    if !settings.chat_bridge_enabled || !settings.chat_bridge_whatsapp_enabled {
        return StatusCode::BAD_REQUEST.into_response();
    }

    match state
        .whatsapp_control_tx
        .send(WhatsAppWebControl::StartLogin)
        .await
    {
        Ok(_) => StatusCode::ACCEPTED.into_response(),
        Err(err) => {
            tracing::warn!(err = %err, "send whatsapp login control failed");
            StatusCode::SERVICE_UNAVAILABLE.into_response()
        }
    }
}

pub(in crate::server) async fn logout_chat_bridge_whatsapp(
    State(state): State<AppState>,
) -> impl IntoResponse {
    match state
        .whatsapp_control_tx
        .send(WhatsAppWebControl::Logout)
        .await
    {
        Ok(_) => StatusCode::ACCEPTED.into_response(),
        Err(err) => {
            tracing::warn!(err = %err, "send whatsapp logout control failed");
            StatusCode::SERVICE_UNAVAILABLE.into_response()
        }
    }
}
