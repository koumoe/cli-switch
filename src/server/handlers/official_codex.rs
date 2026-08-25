use crate::{
    server::{AppState, error::ApiError},
    storage,
};
use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::atomic::Ordering;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const REDIRECT_URI: &str = "http://localhost:1455/auth/callback";

#[derive(Debug, Clone)]
pub struct CodexOAuthSession {
    pub verifier: String,
    pub created_at_ms: i64,
}
#[derive(Serialize)]
pub(in crate::server) struct StartResponse {
    authorization_url: String,
    state: String,
}
#[derive(Deserialize)]
pub(in crate::server) struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
fn jwt_claim(token: &str, key: &str) -> Option<String> {
    let part = token.split('.').nth(1)?;
    let data = URL_SAFE_NO_PAD.decode(part).ok()?;
    serde_json::from_slice::<serde_json::Value>(&data)
        .ok()?
        .get(key)?
        .as_str()
        .map(str::to_owned)
}
fn account_id(token: &str) -> Option<String> {
    let p = token.split('.').nth(1)?;
    let d = URL_SAFE_NO_PAD.decode(p).ok()?;
    let v: serde_json::Value = serde_json::from_slice(&d).ok()?;
    v.get("https://api.openai.com/auth")
        .and_then(|x| x.get("chatgpt_account_id"))
        .and_then(|x| x.as_str())
        .map(str::to_owned)
        .or_else(|| v.get("sub").and_then(|x| x.as_str()).map(str::to_owned))
}

pub(crate) async fn valid_access_token(
    state: &AppState,
    id: String,
) -> anyhow::Result<(String, String)> {
    let account = storage::get_official_codex_account_secret(state.db_path(), id.clone())
        .await?
        .ok_or_else(|| anyhow::anyhow!("official codex account not found"))?;
    if account.expires_at_ms > now_ms() + 60_000 {
        return Ok((account.access_token, account.account_id));
    }
    #[derive(Deserialize)]
    struct Tokens {
        access_token: String,
        refresh_token: Option<String>,
        id_token: Option<String>,
        expires_in: i64,
    }
    let resp = state
        .http_client
        .post("https://auth.openai.com/oauth/token")
        .form(&[
            ("client_id", CLIENT_ID),
            ("grant_type", "refresh_token"),
            ("refresh_token", account.refresh_token.as_str()),
            ("scope", "openid profile email"),
        ])
        .send()
        .await?;
    let status = resp.status();
    let body = resp.bytes().await?;
    if !status.is_success() {
        return Err(anyhow::anyhow!(
            "codex token refresh failed: {status}: {}",
            String::from_utf8_lossy(&body)
        ));
    }
    let t: Tokens = serde_json::from_slice(&body)?;
    let refresh = t.refresh_token.unwrap_or(account.refresh_token);
    let id_token = t.id_token.unwrap_or(account.id_token);
    let expires = now_ms() + t.expires_in * 1000;
    storage::update_official_codex_tokens(
        state.db_path(),
        id,
        t.access_token.clone(),
        refresh,
        id_token,
        expires,
    )
    .await?;
    Ok((t.access_token, account.account_id))
}

pub(in crate::server) async fn start_official_codex_login(
    State(state): State<AppState>,
) -> Result<Json<StartResponse>, ApiError> {
    if !state.codex_callback_available.load(Ordering::Acquire) {
        return Err(ApiError::bad_request(
            "codex_callback_unavailable",
            "OAuth callback port 1455 is unavailable",
        ));
    }
    let state_code = Uuid::new_v4().simple().to_string();
    let verifier = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    state.codex_oauth_sessions.lock().await.insert(
        state_code.clone(),
        CodexOAuthSession {
            verifier,
            created_at_ms: now_ms(),
        },
    );
    let mut url = reqwest::Url::parse("https://auth.openai.com/oauth/authorize").unwrap();
    url.query_pairs_mut()
        .append_pair("client_id", CLIENT_ID)
        .append_pair("response_type", "code")
        .append_pair("redirect_uri", REDIRECT_URI)
        .append_pair("scope", "openid email profile offline_access")
        .append_pair("state", &state_code)
        .append_pair("code_challenge", &challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("prompt", "login")
        .append_pair("id_token_add_organizations", "true")
        .append_pair("codex_cli_simplified_flow", "true");
    Ok(Json(StartResponse {
        authorization_url: url.into(),
        state: state_code,
    }))
}

pub(in crate::server) async fn official_codex_callback(
    State(state): State<AppState>,
    Query(q): Query<CallbackQuery>,
) -> impl IntoResponse {
    let result = async {
        if let Some(e) = q.error {
            return Err(anyhow::anyhow!("OAuth denied: {e}"));
        }
        let st = q.state.ok_or_else(|| anyhow::anyhow!("missing state"))?;
        let code = q.code.ok_or_else(|| anyhow::anyhow!("missing code"))?;
        let session = state
            .codex_oauth_sessions
            .lock()
            .await
            .remove(&st)
            .ok_or_else(|| anyhow::anyhow!("invalid or expired state"))?;
        if now_ms() - session.created_at_ms > 600_000 {
            return Err(anyhow::anyhow!("OAuth session expired"));
        }
        #[derive(Deserialize)]
        struct Tokens {
            access_token: String,
            refresh_token: String,
            id_token: String,
            expires_in: i64,
        }
        let resp = state
            .http_client
            .post("https://auth.openai.com/oauth/token")
            .form(&[
                ("grant_type", "authorization_code"),
                ("client_id", CLIENT_ID),
                ("code", code.as_str()),
                ("redirect_uri", REDIRECT_URI),
                ("code_verifier", session.verifier.as_str()),
            ])
            .send()
            .await?;
        let status = resp.status();
        let body = resp.bytes().await?;
        if !status.is_success() {
            return Err(anyhow::anyhow!(
                "token exchange failed: {status}: {}",
                String::from_utf8_lossy(&body)
            ));
        }
        let t: Tokens = serde_json::from_slice(&body)?;
        let aid = account_id(&t.id_token)
            .or_else(|| account_id(&t.access_token))
            .ok_or_else(|| anyhow::anyhow!("token missing account id"))?;
        let email = jwt_claim(&t.id_token, "email");
        storage::upsert_official_codex_account(
            state.db_path(),
            aid,
            email,
            t.access_token,
            t.refresh_token,
            t.id_token,
            now_ms() + t.expires_in * 1000,
        )
        .await?;
        Ok::<_, anyhow::Error>(())
    }
    .await;
    match result {
        Ok(()) => (
            StatusCode::OK,
            Html("<html><body><h2>Codex 登录成功，可以关闭此窗口。</h2></body></html>"),
        ),
        Err(e) => {
            tracing::warn!(err=%e,"official codex oauth failed");
            (
                StatusCode::BAD_REQUEST,
                Html("<html><body><h2>Codex 登录失败，请返回 CliSwitch 重试。</h2></body></html>"),
            )
        }
    }
}

pub(in crate::server) async fn list_official_codex_accounts(
    State(state): State<AppState>,
) -> Result<Json<Vec<storage::OfficialCodexAccount>>, ApiError> {
    Ok(Json(
        storage::list_official_codex_accounts(state.db_path())
            .await
            .map_err(ApiError::Internal)?,
    ))
}
pub(in crate::server) async fn delete_official_codex_account(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<StatusCode, ApiError> {
    storage::delete_official_codex_account(state.db_path(), id)
        .await
        .map_err(ApiError::Internal)?;
    Ok(StatusCode::NO_CONTENT)
}

pub(in crate::server) async fn refresh_official_codex_account(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<StatusCode, ApiError> {
    valid_access_token(&state, id)
        .await
        .map_err(ApiError::Internal)?;
    Ok(StatusCode::NO_CONTENT)
}
