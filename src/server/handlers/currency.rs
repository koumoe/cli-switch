use anyhow::Context as _;
use axum::Json;
use axum::extract::State;
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;
use std::time::Duration;

use crate::server::AppState;
use crate::server::error::ApiError;
use crate::storage;

const BASE_CURRENCY: &str = "USD";
const QUOTE_CURRENCY: &str = "CNY";
const SOURCE: &str = "Frankfurter";
const FRANKFURTER_USD_CNY_URL: &str = "https://api.frankfurter.dev/v2/rate/USD/CNY";
const REFRESH_INTERVAL_MS: i64 = 12 * 60 * 60 * 1_000;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

static REFRESH_LOCK: LazyLock<tokio::sync::Mutex<()>> =
    LazyLock::new(|| tokio::sync::Mutex::new(()));

#[derive(Debug, Deserialize)]
struct FrankfurterRateResponse {
    date: String,
    base: String,
    quote: String,
    rate: f64,
}

#[derive(Debug, Serialize)]
pub(in crate::server) struct ExchangeRateResponse {
    base_currency: String,
    quote_currency: String,
    rate: f64,
    effective_date: String,
    source: String,
    fetched_at_ms: i64,
    stale: bool,
}

impl ExchangeRateResponse {
    fn from_stored(rate: storage::ExchangeRate, stale: bool) -> Self {
        Self {
            base_currency: rate.base_currency,
            quote_currency: rate.quote_currency,
            rate: rate.rate,
            effective_date: rate.effective_date,
            source: rate.source,
            fetched_at_ms: rate.fetched_at_ms,
            stale,
        }
    }
}

fn is_fresh(rate: &storage::ExchangeRate, now_ms: i64) -> bool {
    now_ms.saturating_sub(rate.fetched_at_ms) < REFRESH_INTERVAL_MS
}

async fn fetch_usd_cny_rate(
    client: &reqwest::Client,
    url: &str,
    fetched_at_ms: i64,
) -> anyhow::Result<storage::ExchangeRate> {
    let response = client
        .get(url)
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
        .context("request Frankfurter exchange rate")?
        .error_for_status()
        .context("Frankfurter exchange rate returned non-success status")?;
    let payload = response
        .json::<FrankfurterRateResponse>()
        .await
        .context("parse Frankfurter exchange rate response")?;

    if payload.base != BASE_CURRENCY || payload.quote != QUOTE_CURRENCY {
        anyhow::bail!(
            "unexpected exchange rate pair: {}/{}",
            payload.base,
            payload.quote
        );
    }
    if !payload.rate.is_finite() || payload.rate <= 0.0 {
        anyhow::bail!("invalid USD/CNY exchange rate: {}", payload.rate);
    }
    if payload.date.trim().is_empty() {
        anyhow::bail!("exchange rate effective date is empty");
    }

    Ok(storage::ExchangeRate {
        base_currency: BASE_CURRENCY.to_string(),
        quote_currency: QUOTE_CURRENCY.to_string(),
        rate: payload.rate,
        effective_date: payload.date,
        source: SOURCE.to_string(),
        fetched_at_ms,
    })
}

async fn load_cached_rate(state: &AppState) -> Result<Option<storage::ExchangeRate>, ApiError> {
    storage::get_exchange_rate(
        state.db_path(),
        BASE_CURRENCY.to_string(),
        QUOTE_CURRENCY.to_string(),
    )
    .await
    .map_err(Into::into)
}

pub(in crate::server) async fn usd_cny_exchange_rate(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let now_ms = storage::now_ms();
    let cached = load_cached_rate(&state).await?;
    if let Some(rate) = cached.as_ref().filter(|rate| is_fresh(rate, now_ms)) {
        return Ok(Json(ExchangeRateResponse::from_stored(rate.clone(), false)));
    }

    let _guard = REFRESH_LOCK.lock().await;

    let now_ms = storage::now_ms();
    let cached = load_cached_rate(&state).await?;
    if let Some(rate) = cached.as_ref().filter(|rate| is_fresh(rate, now_ms)) {
        return Ok(Json(ExchangeRateResponse::from_stored(rate.clone(), false)));
    }

    match fetch_usd_cny_rate(&state.http_client, FRANKFURTER_USD_CNY_URL, now_ms).await {
        Ok(rate) => {
            storage::upsert_exchange_rate(state.db_path(), rate.clone()).await?;
            Ok(Json(ExchangeRateResponse::from_stored(rate, false)))
        }
        Err(error) => {
            tracing::warn!(err = %error, "refresh USD/CNY exchange rate failed");
            if let Some(rate) = cached {
                return Ok(Json(ExchangeRateResponse::from_stored(rate, true)));
            }
            Err(ApiError::bad_gateway(
                "currency_exchange_rate_unavailable",
                "USD/CNY exchange rate is unavailable and no cached rate exists",
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::Router;
    use axum::routing::get;

    #[test]
    fn freshness_uses_the_refresh_interval() {
        let rate = storage::ExchangeRate {
            base_currency: BASE_CURRENCY.to_string(),
            quote_currency: QUOTE_CURRENCY.to_string(),
            rate: 6.7,
            effective_date: "2026-08-27".to_string(),
            source: SOURCE.to_string(),
            fetched_at_ms: 1_000,
        };

        assert!(is_fresh(&rate, 1_000 + REFRESH_INTERVAL_MS - 1));
        assert!(!is_fresh(&rate, 1_000 + REFRESH_INTERVAL_MS));
    }

    #[tokio::test]
    async fn fetches_and_validates_frankfurter_response() {
        let app = Router::new().route(
            "/rate",
            get(|| async {
                Json(serde_json::json!({
                    "date": "2026-08-28",
                    "base": "USD",
                    "quote": "CNY",
                    "rate": 6.7175
                }))
            }),
        );
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("bind test server");
        let addr = listener.local_addr().expect("read test server address");
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        let rate = fetch_usd_cny_rate(&reqwest::Client::new(), &format!("http://{addr}/rate"), 123)
            .await
            .expect("fetch exchange rate");

        assert_eq!(rate.rate, 6.7175);
        assert_eq!(rate.effective_date, "2026-08-28");
        assert_eq!(rate.fetched_at_ms, 123);
    }
}
