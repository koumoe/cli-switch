use axum::Json;
use axum::extract::State;
use axum::response::IntoResponse;
use serde::Serialize;
use std::collections::HashMap;

use crate::cli_tools::CliToolId;
use crate::server::AppState;
use crate::server::error::ApiError;
use crate::storage;

#[derive(Debug, Serialize)]
pub(in crate::server) struct OpenAiCodexTicketStatusResponse {
    version: Option<String>,
    version_source: Option<&'static str>,
    issue: Option<&'static str>,
    tickets: Vec<OpenAiCodexTicketStatusItem>,
}

#[derive(Debug, Serialize)]
struct OpenAiCodexTicketStatusItem {
    account_id: String,
    account_name: String,
    model: String,
    ready: bool,
    length: Option<usize>,
    remaining_seconds: Option<i64>,
    expires_at_ms: Option<i64>,
    last_attempt_at_ms: Option<i64>,
    last_error: Option<&'static str>,
}

pub(in crate::server) async fn openai_codex_ticket_status(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let settings = storage::get_app_settings(state.db_path()).await?;
    let issue = if !settings.openai_codex_ticket_enabled {
        Some("disabled")
    } else if settings
        .openai_codex_ticket_harvest_proxy_url
        .as_deref()
        .is_none_or(|proxy| proxy.trim().is_empty())
    {
        Some("missing_proxy")
    } else {
        None
    };

    let tool_snapshot = if issue.is_none() {
        state.cli_tools_runtime.snapshot().await
    } else {
        None
    };
    let (version, version_source, issue) = if issue.is_some() {
        (None, None, issue)
    } else {
        let local_version = tool_snapshot.as_ref().and_then(|snapshot| {
            snapshot
                .tools
                .iter()
                .find(|tool| tool.id == CliToolId::Codex)
                .and_then(|tool| tool.version.clone())
                .filter(|version| crate::openai_codex_ticket::is_supported_codex_version(version))
        });
        if let Some(version) = local_version {
            (Some(version), Some("local"), None)
        } else if let Some(version) = settings
            .openai_codex_ticket_version_override
            .clone()
            .filter(|version| crate::openai_codex_ticket::is_supported_codex_version(version))
        {
            (Some(version), Some("configured"), None)
        } else if tool_snapshot.is_none() {
            (None, None, Some("checking_version"))
        } else {
            (None, None, Some("missing_version"))
        }
    };

    let now = storage::now_ms();
    let channels = storage::list_channels(state.db_path()).await?;
    let oauth_account_ids = crate::server::openai_oauth_ticket_account_ids(&channels);
    let accounts = storage::list_openai_accounts(state.db_path()).await?;
    let mut tickets = Vec::new();
    for account in accounts {
        if !oauth_account_ids.contains(&account.id) {
            continue;
        }
        let tickets_by_model =
            storage::list_openai_codex_tickets(state.db_path(), account.id.clone())
                .await?
                .into_iter()
                .map(|ticket| (ticket.model.clone(), ticket))
                .collect::<HashMap<_, _>>();
        for model in &settings.openai_codex_ticket_models {
            let status = tickets_by_model
                .get(model)
                .cloned()
                .map(|ticket| storage::ticket_status(ticket, now));
            tickets.push(OpenAiCodexTicketStatusItem {
                account_id: account.id.clone(),
                account_name: account.name.clone(),
                model: model.clone(),
                ready: status.as_ref().is_some_and(|status| status.ready),
                length: status.as_ref().map(|status| status.length),
                remaining_seconds: status.as_ref().map(|status| status.remaining_seconds),
                expires_at_ms: status.as_ref().and_then(|status| status.expires_at_ms),
                last_attempt_at_ms: status.as_ref().and_then(|status| status.last_attempt_at_ms),
                last_error: status
                    .as_ref()
                    .and_then(|status| status.last_error.as_deref())
                    .map(safe_ticket_error),
            });
        }
    }

    Ok(Json(OpenAiCodexTicketStatusResponse {
        version,
        version_source,
        issue,
        tickets,
    }))
}

fn safe_ticket_error(error: &str) -> &'static str {
    if error.contains("invalid shape") {
        "invalid_shape"
    } else if error.contains("did not include") {
        "missing_header"
    } else if error.contains("HTTP ") {
        "upstream_http"
    } else if error.contains("access token") {
        "missing_access_token"
    } else {
        "harvest_failed"
    }
}
