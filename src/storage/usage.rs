use rusqlite::{Connection, OptionalExtension as _, params};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

use super::{Protocol, with_conn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageEvent {
    pub id: String,
    pub request_id: Option<String>,
    pub ts_ms: i64,
    pub protocol: Protocol,
    pub route_id: Option<String>,
    pub channel_id: String,
    pub model: Option<String>,
    pub success: bool,
    pub http_status: Option<i64>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub latency_ms: i64,
    pub ttft_ms: Option<i64>,
    pub prompt_tokens: Option<i64>,
    pub completion_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
    pub cache_read_tokens: Option<i64>,
    pub cache_write_tokens: Option<i64>,
    pub estimated_cost_usd: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CreateUsageEvent {
    pub request_id: Option<Arc<str>>,
    pub ts_ms: i64,
    pub protocol: Protocol,
    pub route_id: Option<String>,
    pub channel_id: String,
    pub model: Option<String>,
    pub success: bool,
    pub http_status: Option<i64>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub latency_ms: i64,
    pub ttft_ms: Option<i64>,
    pub prompt_tokens: Option<i64>,
    pub completion_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
    pub cache_read_tokens: Option<i64>,
    pub cache_write_tokens: Option<i64>,
    pub estimated_cost_usd: Option<String>,
}

pub async fn insert_usage_event(db_path: PathBuf, input: CreateUsageEvent) -> anyhow::Result<()> {
    with_conn(db_path, move |conn| {
        let id = Uuid::new_v4().to_string();
        let CreateUsageEvent {
            request_id,
            ts_ms,
            protocol,
            route_id,
            channel_id,
            model,
            success,
            http_status,
            error_kind,
            error_detail,
            latency_ms,
            ttft_ms,
            prompt_tokens,
            completion_tokens,
            total_tokens,
            cache_read_tokens,
            cache_write_tokens,
            estimated_cost_usd,
        } = input;

        let estimated_cost_usd = estimated_cost_usd.or_else(|| {
            model.as_deref().and_then(|m| {
                estimate_cost_usd(
                    conn,
                    m,
                    success,
                    prompt_tokens,
                    completion_tokens,
                    cache_read_tokens,
                    cache_write_tokens,
                )
            })
        });

        conn.execute(
            r#"
            INSERT INTO usage_events (
              id, request_id, ts_ms, protocol, route_id, channel_id, model,
              success, http_status, error_kind, error_detail, latency_ms,
              ttft_ms, prompt_tokens, completion_tokens, total_tokens,
              cache_read_tokens, cache_write_tokens,
              estimated_cost_usd
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)
            "#,
            params![
                id,
                request_id.as_deref(),
                ts_ms,
                protocol.as_str(),
                route_id,
                channel_id,
                model,
                if success { 1 } else { 0 },
                http_status,
                error_kind,
                error_detail,
                latency_ms,
                ttft_ms,
                prompt_tokens,
                completion_tokens,
                total_tokens,
                cache_read_tokens,
                cache_write_tokens,
                estimated_cost_usd,
            ],
        )?;
        Ok(())
    })
    .await
}

fn parse_price_usd(s: &str) -> Option<f64> {
    let v = s.trim().parse::<f64>().ok()?;
    if v.is_finite() && v > 0.0 {
        Some(v)
    } else {
        None
    }
}

fn format_cost_usd(v: f64) -> Option<String> {
    if !v.is_finite() || v <= 0.0 {
        return None;
    }
    let mut s = format!("{v:.12}");
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

type PricingModelPrices = (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
);

fn find_pricing_for_model(
    conn: &Connection,
    model: &str,
) -> rusqlite::Result<Option<PricingModelPrices>> {
    let mut stmt = conn.prepare(
        r#"
        SELECT prompt_price, completion_price, request_price, cache_read_price, cache_write_price
        FROM pricing_models
        WHERE model_id = ?1
        "#,
    )?;
    if let Some(row) = stmt
        .query_row(params![model], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        })
        .optional()?
    {
        return Ok(Some(row));
    }

    let like = format!("%/{}", model.trim());
    let mut stmt = conn.prepare(
        r#"
        SELECT prompt_price, completion_price, request_price, cache_read_price, cache_write_price
        FROM pricing_models
        WHERE model_id LIKE ?1
        ORDER BY LENGTH(model_id) ASC
        LIMIT 1
        "#,
    )?;
    stmt.query_row(params![like], |row| {
        Ok((
            row.get(0)?,
            row.get(1)?,
            row.get(2)?,
            row.get(3)?,
            row.get(4)?,
        ))
    })
    .optional()
}

fn estimate_cost_usd(
    conn: &Connection,
    model: &str,
    success: bool,
    prompt_tokens: Option<i64>,
    completion_tokens: Option<i64>,
    cache_read_tokens: Option<i64>,
    cache_write_tokens: Option<i64>,
) -> Option<String> {
    let Ok(Some((
        prompt_price,
        completion_price,
        request_price,
        cache_read_price,
        cache_write_price,
    ))) = find_pricing_for_model(conn, model)
    else {
        return None;
    };

    let prompt_unit = prompt_price
        .as_deref()
        .and_then(parse_price_usd)
        .unwrap_or(0.0);
    let completion_unit = completion_price
        .as_deref()
        .and_then(parse_price_usd)
        .unwrap_or(0.0);
    let cache_read_unit = cache_read_price
        .as_deref()
        .and_then(parse_price_usd)
        .unwrap_or(0.0);
    let cache_write_unit = cache_write_price
        .as_deref()
        .and_then(parse_price_usd)
        .unwrap_or(0.0);
    let request_unit = if success {
        request_price
            .as_deref()
            .and_then(parse_price_usd)
            .unwrap_or(0.0)
    } else {
        0.0
    };

    let c = completion_tokens.unwrap_or(0).max(0) as f64;
    let cr = cache_read_tokens.unwrap_or(0).max(0);
    let cw = cache_write_tokens.unwrap_or(0).max(0);

    let mut regular_prompt_tokens = prompt_tokens.unwrap_or(0).max(0);
    if cr <= regular_prompt_tokens {
        regular_prompt_tokens -= cr;
    }
    if cw <= regular_prompt_tokens {
        regular_prompt_tokens -= cw;
    }
    let p = regular_prompt_tokens as f64;
    let cr = cr as f64;
    let cw = cw as f64;

    let cost = p * prompt_unit
        + c * completion_unit
        + cr * cache_read_unit
        + cw * cache_write_unit
        + request_unit;
    format_cost_usd(cost)
}

pub async fn backfill_usage_event_costs(db_path: PathBuf) -> anyhow::Result<i64> {
    with_conn(db_path, move |conn| {
        let mut stmt = conn.prepare(
            r#"
            SELECT id, model, success, prompt_tokens, completion_tokens, cache_read_tokens, cache_write_tokens
            FROM usage_events
            WHERE estimated_cost_usd IS NULL
              AND model IS NOT NULL
              AND (prompt_tokens IS NOT NULL OR completion_tokens IS NOT NULL OR success = 1)
            ORDER BY ts_ms DESC
            LIMIT 20000
            "#,
        )?;

        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)? != 0,
                row.get::<_, Option<i64>>(3)?,
                row.get::<_, Option<i64>>(4)?,
                row.get::<_, Option<i64>>(5)?,
                row.get::<_, Option<i64>>(6)?,
            ))
        })?;

        let mut updated = 0i64;
        for row in rows {
            let (id, model, success, prompt_tokens, completion_tokens, cache_read_tokens, cache_write_tokens) =
                row?;
            let Some(cost) = estimate_cost_usd(
                conn,
                &model,
                success,
                prompt_tokens,
                completion_tokens,
                cache_read_tokens,
                cache_write_tokens,
            ) else {
                continue;
            };
            let n = conn.execute(
                "UPDATE usage_events SET estimated_cost_usd = ?1 WHERE id = ?2 AND estimated_cost_usd IS NULL",
                params![cost, id],
            )?;
            updated += n as i64;
        }

        Ok(updated)
    })
    .await
}

pub async fn list_usage_events_recent(
    db_path: PathBuf,
    limit: i64,
) -> anyhow::Result<Vec<UsageEvent>> {
    with_conn(db_path, move |conn| {
        let mut stmt = conn.prepare(
            r#"
            SELECT id, request_id, ts_ms, protocol, route_id, channel_id, model,
                   success, http_status, error_kind, error_detail, latency_ms,
                   ttft_ms, prompt_tokens, completion_tokens, total_tokens,
                   cache_read_tokens, cache_write_tokens,
                   estimated_cost_usd
            FROM usage_events
            ORDER BY ts_ms DESC
            LIMIT ?1
            "#,
        )?;
        let rows = stmt.query_map(params![limit], |row| {
            Ok(UsageEvent {
                id: row.get(0)?,
                request_id: row.get(1)?,
                ts_ms: row.get(2)?,
                protocol: row.get(3)?,
                route_id: row.get(4)?,
                channel_id: row.get(5)?,
                model: row.get(6)?,
                success: row.get::<_, i64>(7)? != 0,
                http_status: row.get(8)?,
                error_kind: row.get(9)?,
                error_detail: row.get(10)?,
                latency_ms: row.get(11)?,
                ttft_ms: row.get(12)?,
                prompt_tokens: row.get(13)?,
                completion_tokens: row.get(14)?,
                total_tokens: row.get(15)?,
                cache_read_tokens: row.get(16)?,
                cache_write_tokens: row.get(17)?,
                estimated_cost_usd: row.get(18)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    })
    .await
}

#[derive(Debug, Clone, Default)]
pub struct UsageListQuery {
    pub start_ms: Option<i64>,
    pub end_ms: Option<i64>,
    pub protocol: Option<Protocol>,
    pub channel_id: Option<String>,
    pub model: Option<String>,
    pub request_id: Option<String>,
    pub success: Option<bool>,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct UsageListResult {
    pub total: i64,
    pub items: Vec<UsageEvent>,
}

pub async fn list_usage_events(
    db_path: PathBuf,
    q: UsageListQuery,
) -> anyhow::Result<UsageListResult> {
    with_conn(db_path, move |conn| {
        let mut where_sql = Vec::<String>::new();
        let mut params = Vec::<rusqlite::types::Value>::new();

        if let Some(start_ms) = q.start_ms {
            where_sql.push("ts_ms >= ?".to_string());
            params.push(start_ms.into());
        }
        if let Some(end_ms) = q.end_ms {
            where_sql.push("ts_ms <= ?".to_string());
            params.push(end_ms.into());
        }
        if let Some(protocol) = q.protocol {
            where_sql.push("protocol = ?".to_string());
            params.push(protocol.as_str().to_string().into());
        }
        if let Some(channel_id) = q.channel_id.filter(|s| !s.trim().is_empty()) {
            where_sql.push("channel_id = ?".to_string());
            params.push(channel_id.into());
        }
        if let Some(model) = q.model.filter(|s| !s.trim().is_empty()) {
            where_sql.push("model LIKE ?".to_string());
            params.push(format!("%{}%", model.trim()).into());
        }
        if let Some(request_id) = q.request_id.filter(|s| !s.trim().is_empty()) {
            where_sql.push("COALESCE(request_id, id) LIKE ?".to_string());
            params.push(format!("%{}%", request_id.trim()).into());
        }
        if let Some(success) = q.success {
            where_sql.push("success = ?".to_string());
            params.push((if success { 1i64 } else { 0i64 }).into());
        }

        let where_clause = if where_sql.is_empty() {
            "".to_string()
        } else {
            format!("WHERE {}", where_sql.join(" AND "))
        };

        let total: i64 = {
            let sql = format!("SELECT COUNT(*) FROM usage_events {where_clause}");
            let mut stmt = conn.prepare(&sql)?;
            stmt.query_row(rusqlite::params_from_iter(params.iter()), |row| row.get(0))?
        };

        let mut params_items = params;
        params_items.push(q.limit.into());
        params_items.push(q.offset.into());

        let sql = format!(
            r#"
            SELECT id, request_id, ts_ms, protocol, route_id, channel_id, model,
                   success, http_status, error_kind, error_detail, latency_ms,
                   ttft_ms, prompt_tokens, completion_tokens, total_tokens,
                   cache_read_tokens, cache_write_tokens,
                   estimated_cost_usd
            FROM usage_events
            {where_clause}
            ORDER BY ts_ms DESC
            LIMIT ? OFFSET ?
            "#
        );

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(params_items.iter()), |row| {
            Ok(UsageEvent {
                id: row.get(0)?,
                request_id: row.get(1)?,
                ts_ms: row.get(2)?,
                protocol: row.get(3)?,
                route_id: row.get(4)?,
                channel_id: row.get(5)?,
                model: row.get(6)?,
                success: row.get::<_, i64>(7)? != 0,
                http_status: row.get(8)?,
                error_kind: row.get(9)?,
                error_detail: row.get(10)?,
                latency_ms: row.get(11)?,
                ttft_ms: row.get(12)?,
                prompt_tokens: row.get(13)?,
                completion_tokens: row.get(14)?,
                total_tokens: row.get(15)?,
                cache_read_tokens: row.get(16)?,
                cache_write_tokens: row.get(17)?,
                estimated_cost_usd: row.get(18)?,
            })
        })?;

        let items = rows.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(UsageListResult { total, items })
    })
    .await
}
