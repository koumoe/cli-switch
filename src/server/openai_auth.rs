use anyhow::Context as _;
use axum::extract::Query;
use axum::response::{Html, IntoResponse, Response};
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use http::StatusCode;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{OnceLock, RwLock};
use std::time::Duration;
use tokio::sync::{Mutex, OnceCell};
use uuid::Uuid;

use crate::storage;

const AUTH_URL: &str = "https://auth.openai.com/oauth/authorize";
const TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const REDIRECT_URI: &str = "http://localhost:1455/auth/callback";
const CALLBACK_ADDR: &str = "127.0.0.1:1455";
const SESSION_TTL: Duration = Duration::from_secs(5 * 60);

static CALLBACK_SERVER: OnceCell<()> = OnceCell::const_new();
static SESSIONS: OnceLock<RwLock<HashMap<String, OAuthSession>>> = OnceLock::new();
static REFRESH_LOCKS: OnceLock<RwLock<HashMap<String, std::sync::Arc<Mutex<()>>>>> =
    OnceLock::new();

fn sessions() -> &'static RwLock<HashMap<String, OAuthSession>> {
    SESSIONS.get_or_init(|| RwLock::new(HashMap::new()))
}

fn refresh_lock(account_id: &str) -> anyhow::Result<std::sync::Arc<Mutex<()>>> {
    let locks = REFRESH_LOCKS.get_or_init(|| RwLock::new(HashMap::new()));
    if let Some(lock) = locks
        .read()
        .map_err(|_| anyhow::anyhow!("OpenAI refresh lock map poisoned"))?
        .get(account_id)
        .cloned()
    {
        return Ok(lock);
    }
    let mut locks = locks
        .write()
        .map_err(|_| anyhow::anyhow!("OpenAI refresh lock map poisoned"))?;
    Ok(locks
        .entry(account_id.to_string())
        .or_insert_with(|| std::sync::Arc::new(Mutex::new(())))
        .clone())
}

#[derive(Debug, Clone)]
struct OAuthSession {
    request_id: String,
    state: String,
    code_verifier: String,
    requested_name: Option<String>,
    db_path: PathBuf,
    http_client: reqwest::Client,
    expires_at_ms: i64,
    status: OAuthSessionState,
}

#[derive(Debug, Clone)]
enum OAuthSessionState {
    Pending,
    Exchanging,
    Completed { account_id: String },
    Failed { error: String },
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct OAuthStartResponse {
    pub request_id: String,
    pub authorization_url: String,
    pub expires_at_ms: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum OAuthPublicStatus {
    Pending,
    Exchanging,
    Completed,
    Failed,
    Expired,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct OAuthStatusResponse {
    pub request_id: String,
    pub status: OAuthPublicStatus,
    pub account: Option<storage::OpenAiAccount>,
    pub error: Option<String>,
    pub expires_at_ms: i64,
}

#[derive(Debug, Deserialize)]
struct OAuthCallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    id_token: Option<String>,
    #[serde(default)]
    expires_in: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
struct JwtClaims {
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default, rename = "https://api.openai.com/auth")]
    openai_auth: OpenAiAuthClaims,
}

#[derive(Debug, Default, Deserialize)]
struct OpenAiAuthClaims {
    #[serde(default)]
    chatgpt_account_id: Option<String>,
    #[serde(default)]
    chatgpt_plan_type: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct RefreshedOpenAiTokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub id_token: Option<String>,
    pub token_expires_at_ms: Option<i64>,
    pub account_id: Option<String>,
    pub email: Option<String>,
    pub display_name: Option<String>,
    pub plan_type: Option<String>,
}

fn random_urlsafe() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

fn pkce_challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

fn authorization_url(state: &str, challenge: &str) -> anyhow::Result<String> {
    let mut url = reqwest::Url::parse(AUTH_URL)?;
    url.query_pairs_mut()
        .append_pair("client_id", CLIENT_ID)
        .append_pair("response_type", "code")
        .append_pair("redirect_uri", REDIRECT_URI)
        .append_pair("scope", "openid email profile offline_access")
        .append_pair("state", state)
        .append_pair("code_challenge", challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("prompt", "login")
        .append_pair("id_token_add_organizations", "true")
        .append_pair("codex_cli_simplified_flow", "true");
    Ok(url.into())
}

async fn ensure_callback_server() -> anyhow::Result<()> {
    CALLBACK_SERVER
        .get_or_try_init(|| async {
            let listener = tokio::net::TcpListener::bind(CALLBACK_ADDR)
                .await
                .with_context(|| format!("OpenAI OAuth callback port {CALLBACK_ADDR} is in use"))?;
            let app = axum::Router::new().route("/auth/callback", axum::routing::get(callback));
            tokio::spawn(async move {
                if let Err(error) = axum::serve(listener, app).await {
                    tracing::error!(%error, "OpenAI OAuth callback server stopped");
                }
            });
            Ok::<(), anyhow::Error>(())
        })
        .await
        .map(|_| ())
}

fn cleanup_expired_sessions(now: i64) {
    let Ok(mut sessions) = sessions().write() else {
        return;
    };
    sessions.retain(|_, session| session.expires_at_ms.saturating_add(600_000) > now);
}

pub(crate) async fn start_oauth(
    db_path: PathBuf,
    http_client: reqwest::Client,
    requested_name: Option<String>,
) -> anyhow::Result<OAuthStartResponse> {
    ensure_callback_server().await?;
    let now = storage::now_ms();
    cleanup_expired_sessions(now);
    let request_id = Uuid::new_v4().to_string();
    let state = random_urlsafe();
    let code_verifier = random_urlsafe();
    let expires_at_ms = now + i64::try_from(SESSION_TTL.as_millis()).unwrap_or(300_000);
    let authorization_url = authorization_url(&state, &pkce_challenge(&code_verifier))?;
    let session = OAuthSession {
        request_id: request_id.clone(),
        state,
        code_verifier,
        requested_name: requested_name
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        db_path,
        http_client,
        expires_at_ms,
        status: OAuthSessionState::Pending,
    };
    sessions()
        .write()
        .map_err(|_| anyhow::anyhow!("OpenAI OAuth session lock poisoned"))?
        .insert(request_id.clone(), session);
    Ok(OAuthStartResponse {
        request_id,
        authorization_url,
        expires_at_ms,
    })
}

pub(crate) async fn oauth_status(request_id: &str) -> anyhow::Result<Option<OAuthStatusResponse>> {
    let now = storage::now_ms();
    let snapshot = sessions()
        .read()
        .map_err(|_| anyhow::anyhow!("OpenAI OAuth session lock poisoned"))?
        .get(request_id)
        .cloned();
    let Some(session) = snapshot else {
        return Ok(None);
    };
    let expired = session.expires_at_ms <= now
        && matches!(
            session.status,
            OAuthSessionState::Pending | OAuthSessionState::Exchanging
        );
    if expired {
        if let Ok(mut all) = sessions().write()
            && let Some(stored) = all.get_mut(request_id)
        {
            stored.status = OAuthSessionState::Failed {
                error: "OpenAI OAuth login timed out".to_string(),
            };
        }
        return Ok(Some(OAuthStatusResponse {
            request_id: session.request_id,
            status: OAuthPublicStatus::Expired,
            account: None,
            error: Some("OpenAI OAuth login timed out".to_string()),
            expires_at_ms: session.expires_at_ms,
        }));
    }
    let (status, account_id, error) = match session.status {
        OAuthSessionState::Pending => (OAuthPublicStatus::Pending, None, None),
        OAuthSessionState::Exchanging => (OAuthPublicStatus::Exchanging, None, None),
        OAuthSessionState::Completed { account_id } => {
            (OAuthPublicStatus::Completed, Some(account_id), None)
        }
        OAuthSessionState::Failed { error } => (OAuthPublicStatus::Failed, None, Some(error)),
    };
    let account = match account_id {
        Some(account_id) => {
            storage::get_openai_account_without_secret_optional(session.db_path, account_id).await?
        }
        None => None,
    };
    Ok(Some(OAuthStatusResponse {
        request_id: session.request_id,
        status,
        account,
        error,
        expires_at_ms: session.expires_at_ms,
    }))
}

async fn callback(Query(query): Query<OAuthCallbackQuery>) -> Response {
    let Some(state) = query
        .state
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return (
            StatusCode::BAD_REQUEST,
            Html(callback_html(false, "Missing OAuth state")),
        )
            .into_response();
    };
    let session = {
        let Ok(mut all) = sessions().write() else {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(callback_html(false, "OAuth session unavailable")),
            )
                .into_response();
        };
        let Some(session) = all.values_mut().find(|session| session.state == state) else {
            return (
                StatusCode::BAD_REQUEST,
                Html(callback_html(false, "Invalid OAuth state")),
            )
                .into_response();
        };
        if session.expires_at_ms <= storage::now_ms() {
            session.status = OAuthSessionState::Failed {
                error: "OpenAI OAuth login timed out".to_string(),
            };
            return (
                StatusCode::GONE,
                Html(callback_html(false, "OAuth login timed out")),
            )
                .into_response();
        }
        if let Some(error) = query.error.as_deref() {
            let description = query.error_description.as_deref().unwrap_or(error);
            session.status = OAuthSessionState::Failed {
                error: format!("OpenAI rejected OAuth login: {description}"),
            };
            return (
                StatusCode::BAD_REQUEST,
                Html(callback_html(false, "OpenAI login was not completed")),
            )
                .into_response();
        }
        let Some(code) = query
            .code
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            session.status = OAuthSessionState::Failed {
                error: "OpenAI OAuth callback did not include a code".to_string(),
            };
            return (
                StatusCode::BAD_REQUEST,
                Html(callback_html(false, "Missing OAuth code")),
            )
                .into_response();
        };
        if !matches!(session.status, OAuthSessionState::Pending) {
            return (
                StatusCode::CONFLICT,
                Html(callback_html(false, "OAuth login already handled")),
            )
                .into_response();
        }
        session.status = OAuthSessionState::Exchanging;
        (session.clone(), code.to_string())
    };

    let (session, code) = session;
    let result = match exchange_code(&session.http_client, &code, &session.code_verifier).await {
        Ok(response) => match token_response_to_storage(response) {
            Ok(tokens) => {
                storage::upsert_openai_account_tokens(
                    session.db_path.clone(),
                    session.requested_name.clone(),
                    tokens,
                )
                .await
            }
            Err(error) => Err(error),
        },
        Err(error) => Err(error),
    };

    match result {
        Ok(account) => {
            if let Ok(secret) =
                storage::get_openai_account_with_secret(session.db_path.clone(), account.id.clone())
                    .await
                && let Ok(quota) = crate::openai_quota::fetch(&session.http_client, &secret).await
            {
                let _ = storage::update_openai_account_quota(
                    session.db_path.clone(),
                    account.id.clone(),
                    quota,
                )
                .await;
            }
            if let Ok(mut all) = sessions().write()
                && let Some(stored) = all.get_mut(&session.request_id)
            {
                stored.status = OAuthSessionState::Completed {
                    account_id: account.id,
                };
            }
            (
                StatusCode::OK,
                Html(callback_html(true, "OpenAI login completed")),
            )
                .into_response()
        }
        Err(error) => {
            tracing::warn!(%error, "OpenAI OAuth token exchange failed");
            if let Ok(mut all) = sessions().write()
                && let Some(stored) = all.get_mut(&session.request_id)
            {
                stored.status = OAuthSessionState::Failed {
                    error: "Failed to exchange OpenAI OAuth code".to_string(),
                };
            }
            (
                StatusCode::BAD_GATEWAY,
                Html(callback_html(false, "OpenAI login failed")),
            )
                .into_response()
        }
    }
}

fn callback_html(success: bool, message: &str) -> String {
    let title = if success {
        "CliSwitch OpenAI"
    } else {
        "CliSwitch OAuth Error"
    };
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>{title}</title></head><body><main><h1>{message}</h1><p>You can close this window and return to CliSwitch.</p></main></body></html>"
    )
}

async fn exchange_code(
    client: &reqwest::Client,
    code: &str,
    code_verifier: &str,
) -> anyhow::Result<TokenResponse> {
    post_token_form(
        client,
        TOKEN_URL,
        &[
            ("grant_type", "authorization_code"),
            ("client_id", CLIENT_ID),
            ("code", code),
            ("redirect_uri", REDIRECT_URI),
            ("code_verifier", code_verifier),
        ],
    )
    .await
}

async fn post_token_form(
    client: &reqwest::Client,
    token_url: &str,
    form: &[(&str, &str)],
) -> anyhow::Result<TokenResponse> {
    let response = client
        .post(token_url)
        .header(reqwest::header::ACCEPT, "application/json")
        .form(form)
        .send()
        .await
        .context("OpenAI token request failed")?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!(
            "OpenAI token endpoint returned HTTP {}: {}",
            status.as_u16(),
            body
        ));
    }
    response
        .json::<TokenResponse>()
        .await
        .context("invalid OpenAI token response")
}

fn parse_claims(token: Option<&str>) -> Option<JwtClaims> {
    let payload = token?.split('.').nth(1)?;
    let bytes = URL_SAFE_NO_PAD.decode(payload).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn identity_from_tokens(token: &TokenResponse) -> JwtClaims {
    let id_claims = parse_claims(token.id_token.as_deref()).unwrap_or_default();
    let access_claims = parse_claims(Some(&token.access_token)).unwrap_or_default();
    JwtClaims {
        email: id_claims.email.or(access_claims.email),
        name: id_claims.name.or(access_claims.name),
        openai_auth: OpenAiAuthClaims {
            chatgpt_account_id: id_claims
                .openai_auth
                .chatgpt_account_id
                .or(access_claims.openai_auth.chatgpt_account_id),
            chatgpt_plan_type: id_claims
                .openai_auth
                .chatgpt_plan_type
                .or(access_claims.openai_auth.chatgpt_plan_type),
        },
    }
}

fn token_response_to_storage(
    response: TokenResponse,
) -> anyhow::Result<storage::OpenAiAccountTokens> {
    let claims = identity_from_tokens(&response);
    let account_id = claims
        .openai_auth
        .chatgpt_account_id
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("OpenAI token did not contain chatgpt_account_id"))?;
    Ok(storage::OpenAiAccountTokens {
        access_token: response.access_token,
        refresh_token: response.refresh_token,
        id_token: response.id_token,
        token_expires_at_ms: response
            .expires_in
            .map(|seconds| storage::now_ms().saturating_add(seconds.saturating_mul(1000))),
        account_id,
        email: claims.email,
        display_name: claims.name,
        plan_type: claims.openai_auth.chatgpt_plan_type,
    })
}

async fn refresh_tokens_at(
    client: &reqwest::Client,
    token_url: &str,
    refresh_token: &str,
) -> anyhow::Result<RefreshedOpenAiTokens> {
    let response = post_token_form(
        client,
        token_url,
        &[
            ("client_id", CLIENT_ID),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("scope", "openid profile email"),
        ],
    )
    .await?;
    let claims = identity_from_tokens(&response);
    Ok(RefreshedOpenAiTokens {
        access_token: response.access_token,
        refresh_token: response.refresh_token,
        id_token: response.id_token,
        token_expires_at_ms: response
            .expires_in
            .map(|seconds| storage::now_ms().saturating_add(seconds.saturating_mul(1000))),
        account_id: claims.openai_auth.chatgpt_account_id,
        email: claims.email,
        display_name: claims.name,
        plan_type: claims.openai_auth.chatgpt_plan_type,
    })
}

pub(crate) async fn refresh_persisted_account(
    client: &reqwest::Client,
    db_path: PathBuf,
    account_id: String,
) -> anyhow::Result<storage::OpenAiAccount> {
    let expected_access_token =
        storage::get_openai_account_with_secret(db_path.clone(), account_id.clone())
            .await?
            .access_token;
    refresh_persisted_account_if_current(
        client,
        db_path,
        account_id,
        expected_access_token.as_deref(),
    )
    .await
}

pub(crate) async fn refresh_persisted_account_if_current(
    client: &reqwest::Client,
    db_path: PathBuf,
    account_id: String,
    expected_access_token: Option<&str>,
) -> anyhow::Result<storage::OpenAiAccount> {
    refresh_persisted_account_if_current_at(
        client,
        TOKEN_URL,
        db_path,
        account_id,
        expected_access_token,
    )
    .await
}

async fn refresh_persisted_account_if_current_at(
    client: &reqwest::Client,
    token_url: &str,
    db_path: PathBuf,
    account_id: String,
    expected_access_token: Option<&str>,
) -> anyhow::Result<storage::OpenAiAccount> {
    let lock = refresh_lock(&account_id)?;
    let _guard = lock.lock().await;
    let account =
        storage::get_openai_account_with_secret(db_path.clone(), account_id.clone()).await?;
    if let Some(expected) = expected_access_token
        && account.access_token.as_deref() != Some(expected)
    {
        return Ok(account);
    }
    let refresh_token = account
        .refresh_token
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("OpenAI refresh token is missing"))?;
    match refresh_tokens_at(client, token_url, refresh_token).await {
        Ok(refreshed) => {
            if let Some(refreshed_account_id) = refreshed
                .account_id
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                && refreshed_account_id != account.remote_user_id
            {
                anyhow::bail!("OpenAI refresh returned a different account identity");
            }
            let tokens = storage::OpenAiAccountTokens {
                access_token: refreshed.access_token,
                refresh_token: refreshed.refresh_token,
                id_token: refreshed.id_token,
                token_expires_at_ms: refreshed.token_expires_at_ms,
                account_id: account.remote_user_id,
                email: refreshed.email.or(account.remote_username),
                display_name: refreshed.display_name.or(account.remote_display_name),
                plan_type: refreshed.plan_type.or(account.plan_type),
            };
            let updated =
                storage::upsert_openai_account_tokens(db_path.clone(), Some(account.name), tokens)
                    .await?;
            storage::get_openai_account_with_secret(db_path, updated.id).await
        }
        Err(error) => {
            let message = error.to_string();
            let reauth_required = message.contains("invalid_grant")
                || message.contains("invalid refresh token")
                || message.contains("HTTP 401");
            let _ = storage::mark_openai_account_auth_failure(
                db_path,
                account_id,
                message.clone(),
                reauth_required,
            )
            .await;
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Json, Router, routing::post};
    use serde_json::json;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    fn jwt(payload: serde_json::Value) -> String {
        format!(
            "header.{}.signature",
            URL_SAFE_NO_PAD.encode(serde_json::to_vec(&payload).unwrap())
        )
    }

    #[test]
    fn pkce_is_s256_and_url_safe() {
        assert_eq!(
            pkce_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn authorization_url_contains_codex_parameters() {
        let url = reqwest::Url::parse(&authorization_url("state", "challenge").unwrap()).unwrap();
        let query = url.query_pairs().collect::<HashMap<_, _>>();
        assert_eq!(
            query.get("client_id").map(|value| value.as_ref()),
            Some(CLIENT_ID)
        );
        assert_eq!(
            query.get("redirect_uri").map(|value| value.as_ref()),
            Some(REDIRECT_URI)
        );
        assert_eq!(
            query
                .get("code_challenge_method")
                .map(|value| value.as_ref()),
            Some("S256")
        );
        assert_eq!(
            query
                .get("codex_cli_simplified_flow")
                .map(|value| value.as_ref()),
            Some("true")
        );
    }

    #[test]
    fn reads_openai_identity_from_id_token() {
        let response = TokenResponse {
            access_token: jwt(json!({})),
            refresh_token: Some("refresh".to_string()),
            id_token: Some(jwt(json!({
                "email": "user@example.com",
                "name": "Codex User",
                "https://api.openai.com/auth": {
                    "chatgpt_account_id": "acct-1",
                    "chatgpt_plan_type": "plus"
                }
            }))),
            expires_in: Some(3600),
        };
        let tokens = token_response_to_storage(response).unwrap();
        assert_eq!(tokens.account_id, "acct-1");
        assert_eq!(tokens.email.as_deref(), Some("user@example.com"));
        assert_eq!(tokens.plan_type.as_deref(), Some("plus"));
    }

    #[tokio::test]
    async fn concurrent_refresh_is_singleflight_per_account() {
        let calls = Arc::new(AtomicUsize::new(0));
        let handler_calls = calls.clone();
        let app = Router::new().route(
            "/token",
            post(move || {
                let calls = handler_calls.clone();
                async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    tokio::time::sleep(Duration::from_millis(30)).await;
                    Json(json!({
                        "access_token": "new-access",
                        "refresh_token": "new-refresh",
                        "expires_in": 3600
                    }))
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        let db_path = std::env::temp_dir().join(format!(
            "cliswitch-openai-refresh-singleflight-{}.db",
            Uuid::new_v4()
        ));
        storage::init_db(&db_path).unwrap();
        let account = storage::upsert_openai_account_tokens(
            db_path.clone(),
            None,
            storage::OpenAiAccountTokens {
                access_token: "old-access".to_string(),
                refresh_token: Some("old-refresh".to_string()),
                id_token: None,
                token_expires_at_ms: Some(0),
                account_id: "account-1".to_string(),
                email: None,
                display_name: None,
                plan_type: None,
            },
        )
        .await
        .unwrap();
        let token_url = format!("http://{address}/token");
        let client = reqwest::Client::new();
        let first = refresh_persisted_account_if_current_at(
            &client,
            &token_url,
            db_path.clone(),
            account.id.clone(),
            Some("old-access"),
        );
        let second = refresh_persisted_account_if_current_at(
            &client,
            &token_url,
            db_path.clone(),
            account.id,
            Some("old-access"),
        );
        let (first, second) = tokio::join!(first, second);
        assert_eq!(first.unwrap().access_token.as_deref(), Some("new-access"));
        assert_eq!(second.unwrap().access_token.as_deref(), Some("new-access"));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        let _ = std::fs::remove_file(db_path);
    }
}
