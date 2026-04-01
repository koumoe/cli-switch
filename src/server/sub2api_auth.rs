use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;

use crate::{storage, sub2api};

pub(crate) type Sub2ApiActionFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, sub2api::Sub2ApiRequestError>> + Send + 'a>>;

const RELOGIN_REQUIRED_MESSAGE: &str = "sub2api login expired, please sign in again";

#[derive(Debug, Clone)]
pub(crate) struct InMemorySub2ApiSession {
    pub access_token: String,
    pub refresh_token: Option<String>,
}

impl InMemorySub2ApiSession {
    pub(crate) fn from_account(account: &storage::RemoteAccount) -> anyhow::Result<Self> {
        let access_token = account
            .access_token
            .clone()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| anyhow::anyhow!("missing sub2api access token"))?;
        Ok(Self {
            access_token,
            refresh_token: account
                .refresh_token
                .clone()
                .filter(|value| !value.trim().is_empty()),
        })
    }
}

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub(crate) struct Sub2ApiReloginRequiredError {
    message: String,
}

impl Sub2ApiReloginRequiredError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub(crate) fn message(&self) -> &str {
        &self.message
    }
}

fn relogin_required_error() -> anyhow::Error {
    Sub2ApiReloginRequiredError::new(RELOGIN_REQUIRED_MESSAGE).into()
}

pub(crate) fn relogin_required_message(err: &anyhow::Error) -> Option<&str> {
    err.downcast_ref::<Sub2ApiReloginRequiredError>()
        .map(Sub2ApiReloginRequiredError::message)
}

fn map_refresh_error(err: sub2api::Sub2ApiRequestError) -> anyhow::Error {
    if err.is_refresh_token_invalid() {
        return relogin_required_error();
    }
    err.into()
}

fn map_retry_error(err: sub2api::Sub2ApiRequestError) -> anyhow::Error {
    if err.is_access_token_invalid() {
        return relogin_required_error();
    }
    err.into()
}

pub(crate) async fn run_with_inmemory_session<T, F>(
    http_client: &reqwest::Client,
    base_url: &str,
    session: &mut InMemorySub2ApiSession,
    action: F,
) -> anyhow::Result<T>
where
    F: for<'a> Fn(&'a reqwest::Client, &'a str, &'a str) -> Sub2ApiActionFuture<'a, T>,
{
    match action(http_client, base_url, &session.access_token).await {
        Ok(value) => Ok(value),
        Err(err) if err.is_access_token_invalid() => {
            let refresh_token = session
                .refresh_token
                .clone()
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(relogin_required_error)?;
            let refreshed = sub2api::refresh_access_token(http_client, base_url, &refresh_token)
                .await
                .map_err(map_refresh_error)?;
            session.access_token = refreshed.access_token;
            session.refresh_token = Some(refreshed.refresh_token);
            action(http_client, base_url, &session.access_token)
                .await
                .map_err(map_retry_error)
        }
        Err(err) => Err(err.into()),
    }
}

pub(crate) async fn run_with_persisted_session<T, F>(
    db_path: PathBuf,
    http_client: &reqwest::Client,
    account: &mut storage::RemoteAccount,
    action: F,
) -> anyhow::Result<T>
where
    F: for<'a> Fn(&'a reqwest::Client, &'a str, &'a str) -> Sub2ApiActionFuture<'a, T>,
{
    let mut session = InMemorySub2ApiSession::from_account(account)?;
    let original_access_token = account.access_token.clone();
    let original_refresh_token = account.refresh_token.clone();
    let result =
        run_with_inmemory_session(http_client, &account.base_url, &mut session, action).await;

    let session_changed = original_access_token.as_deref() != Some(session.access_token.as_str())
        || original_refresh_token != session.refresh_token;
    if session_changed || (result.is_ok() && account.reauth_required) {
        storage::update_remote_account_auth_session(
            db_path.clone(),
            account.id.clone(),
            Some(session.access_token.clone()),
            session.refresh_token.clone(),
        )
        .await?;
        account.access_token = Some(session.access_token.clone());
        account.refresh_token = session.refresh_token.clone();
        account.access_token_configured = true;
        account.reauth_required = false;
        account.last_sync_error = None;
    }

    if let Some(message) = result.as_ref().err().and_then(relogin_required_message) {
        let synced_at_ms = Some(storage::now_ms());
        if let Err(update_err) = storage::apply_remote_account_sync_failure(
            db_path,
            account.id.clone(),
            message.to_string(),
            synced_at_ms,
            true,
        )
        .await
        {
            tracing::warn!(account_id = %account.id, err = %update_err, "persist sub2api relogin-required state failed");
        }
        account.reauth_required = true;
        account.last_sync_error = Some(message.to_string());
        account.last_synced_at_ms = synced_at_ms;
    }

    result
}
