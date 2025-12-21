use axum::Json;
use axum::extract::{Query, State};
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::server::AppState;
use crate::server::error::ApiError;
use crate::storage;

#[derive(Debug, Deserialize)]
pub(in crate::server) struct PricingModelsQuery {
    query: Option<String>,
    limit: Option<i64>,
}

pub(in crate::server) async fn pricing_status(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let status = storage::pricing_status(state.db_path()).await?;
    Ok(Json(status))
}

pub(in crate::server) async fn pricing_models(
    State(state): State<AppState>,
    Query(q): Query<PricingModelsQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let limit = q.limit.unwrap_or(200).clamp(1, 2000);
    let models = storage::search_pricing_models(state.db_path(), q.query, limit).await?;
    Ok(Json(models))
}

#[derive(Serialize)]
struct PricingSyncResponse {
    updated: usize,
    updated_at_ms: i64,
}

pub(in crate::server) async fn pricing_sync(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let (updated, updated_at_ms) = run_pricing_sync(&state.http_client, state.db_path()).await?;
    Ok(Json(PricingSyncResponse {
        updated,
        updated_at_ms,
    }))
}

pub(crate) async fn run_pricing_sync(
    http_client: &reqwest::Client,
    db_path: PathBuf,
) -> Result<(usize, i64), ApiError> {
    const PRICING_METADATA_URL: &str = "https://basellm.github.io/llm-metadata/api/all.json";
    const USD_PER_MILLION_DIVISOR: f64 = 1_000_000.0;

    fn json_value_to_f64(v: &serde_json::Value) -> Option<f64> {
        match v {
            serde_json::Value::Number(n) => n.as_f64(),
            serde_json::Value::String(s) => s.trim().parse::<f64>().ok(),
            _ => None,
        }
    }

    fn format_unit_price_usd_per_token(v: f64) -> Option<String> {
        if !v.is_finite() || v <= 0.0 {
            return None;
        }
        let mut s = format!("{v:.18}");
        while s.contains('.') && s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
        if s == "0" || s == "0.0" {
            None
        } else {
            Some(s)
        }
    }

    fn provider_priority(provider_id: &str) -> i32 {
        match provider_id {
            "openai" | "anthropic" | "google" | "deepseek" | "xai" => 0,
            _ => 10,
        }
    }

    let resp = http_client
        .get(PRICING_METADATA_URL)
        .send()
        .await
        .map_err(|e| ApiError::BadGateway(format!("请求 llm-metadata 失败：{e}")))?;

    let status = resp.status();
    let body = resp
        .bytes()
        .await
        .map_err(|e| ApiError::BadGateway(format!("读取 llm-metadata 响应失败：{e}")))?;

    if !status.is_success() {
        let snippet = String::from_utf8_lossy(&body);
        return Err(ApiError::BadGateway(format!(
            "llm-metadata 返回非成功状态：{status} body={snippet}"
        )));
    }

    let root: serde_json::Value = serde_json::from_slice(&body)
        .map_err(|e| ApiError::BadGateway(format!("解析 llm-metadata JSON 失败：{e}")))?;
    let providers = root.as_object().ok_or_else(|| {
        ApiError::BadGateway("llm-metadata JSON 顶层不是对象 (object)".to_string())
    })?;

    let updated_at_ms = storage::now_ms();
    let mut selected: std::collections::HashMap<String, (i32, storage::UpsertPricingModel)> =
        std::collections::HashMap::new();

    for (provider_id, provider) in providers {
        let Some(models_obj) = provider.get("models").and_then(|v| v.as_object()) else {
            continue;
        };
        let pri = provider_priority(provider_id);

        for (model_key, model) in models_obj {
            let model_id = model
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or(model_key)
                .trim();
            if model_id.is_empty() {
                continue;
            }

            let cost = model.get("cost").unwrap_or(&serde_json::Value::Null);
            let prompt_per_million = cost.get("input").and_then(json_value_to_f64);
            let completion_per_million = cost.get("output").and_then(json_value_to_f64);
            let cache_read_per_million = cost.get("cache_read").and_then(json_value_to_f64);
            let cache_write_per_million = cost.get("cache_write").and_then(json_value_to_f64);

            let prompt_price = prompt_per_million
                .map(|v| v / USD_PER_MILLION_DIVISOR)
                .and_then(format_unit_price_usd_per_token);
            let completion_price = completion_per_million
                .map(|v| v / USD_PER_MILLION_DIVISOR)
                .and_then(format_unit_price_usd_per_token);
            let cache_read_price = cache_read_per_million
                .map(|v| v / USD_PER_MILLION_DIVISOR)
                .and_then(format_unit_price_usd_per_token);
            let cache_write_price = cache_write_per_million
                .map(|v| v / USD_PER_MILLION_DIVISOR)
                .and_then(format_unit_price_usd_per_token);

            if prompt_price.is_none()
                && completion_price.is_none()
                && cache_read_price.is_none()
                && cache_write_price.is_none()
            {
                continue;
            }

            let raw_json = serde_json::to_string(&serde_json::json!({
                "provider": provider_id,
                "model_key": model_key,
                "model": model,
            }))
            .ok();

            let candidate = storage::UpsertPricingModel {
                model_id: model_id.to_string(),
                prompt_price,
                completion_price,
                request_price: None,
                cache_read_price,
                cache_write_price,
                raw_json,
            };

            match selected.get(model_id) {
                None => {
                    selected.insert(model_id.to_string(), (pri, candidate));
                }
                Some((existing_pri, existing)) => {
                    let candidate_has_more = (candidate.prompt_price.is_some()
                        && existing.prompt_price.is_none())
                        || (candidate.completion_price.is_some()
                            && existing.completion_price.is_none());
                    let candidate_has_more = candidate_has_more
                        || (candidate.cache_read_price.is_some()
                            && existing.cache_read_price.is_none())
                        || (candidate.cache_write_price.is_some()
                            && existing.cache_write_price.is_none());
                    if pri < *existing_pri || (pri == *existing_pri && candidate_has_more) {
                        selected.insert(model_id.to_string(), (pri, candidate));
                    }
                }
            }
        }
    }

    let models: Vec<storage::UpsertPricingModel> = selected.into_values().map(|(_, m)| m).collect();
    let updated = storage::upsert_pricing_models(db_path.clone(), models, updated_at_ms)
        .await
        .map_err(ApiError::Internal)?;

    if let Err(e) = storage::backfill_usage_event_costs(db_path).await {
        tracing::warn!(err = %e, "backfill usage event costs failed");
    }

    Ok((updated, updated_at_ms))
}
