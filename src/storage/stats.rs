use rusqlite::params;
use serde::Serialize;
use std::path::PathBuf;

use super::{Protocol, with_conn};

#[derive(Debug, Clone, Serialize)]
pub struct StatsSummary {
    pub start_ms: i64,
    pub requests: i64,
    pub success: i64,
    pub failed: i64,
    pub avg_latency_ms: Option<f64>,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
    pub estimated_cost_usd: Option<String>,
}

pub async fn stats_summary(db_path: PathBuf, start_ms: i64) -> anyhow::Result<StatsSummary> {
    with_conn(db_path, move |conn| {
        let mut stmt = conn.prepare(
            r#"
            WITH per_req AS (
              SELECT
                COALESCE(request_id, id) AS rid,
                MAX(success) AS any_success
              FROM usage_events
              WHERE ts_ms >= ?1
              GROUP BY rid
            ),
            req_agg AS (
              SELECT
                COUNT(*) AS requests,
                SUM(any_success) AS success
              FROM per_req
            )
            SELECT
              (SELECT requests FROM req_agg) AS requests,
              (SELECT success FROM req_agg) AS success,
              (SELECT requests - success FROM req_agg) AS failed,
              AVG(CASE WHEN latency_ms > 0 THEN latency_ms ELSE NULL END) AS avg_latency_ms,
              SUM(COALESCE(prompt_tokens, 0)) AS prompt_tokens,
              SUM(COALESCE(completion_tokens, 0)) AS completion_tokens,
              SUM(COALESCE(total_tokens, 0)) AS total_tokens,
              SUM(COALESCE(CAST(estimated_cost_usd AS REAL), 0.0)) AS estimated_cost
            FROM usage_events
            WHERE ts_ms >= ?1
            "#,
        )?;
        let row = stmt.query_row(params![start_ms], |row| {
            let requests: i64 = row.get(0)?;
            let success: Option<i64> = row.get(1)?;
            let failed: Option<i64> = row.get(2)?;
            let avg_latency_ms: Option<f64> = row.get(3)?;
            let prompt_tokens: Option<i64> = row.get(4)?;
            let completion_tokens: Option<i64> = row.get(5)?;
            let total_tokens: Option<i64> = row.get(6)?;
            let estimated_cost: Option<f64> = row.get(7)?;

            Ok(StatsSummary {
                start_ms,
                requests,
                success: success.unwrap_or(0),
                failed: failed.unwrap_or(0),
                avg_latency_ms,
                prompt_tokens: prompt_tokens.unwrap_or(0),
                completion_tokens: completion_tokens.unwrap_or(0),
                total_tokens: total_tokens.unwrap_or(0),
                estimated_cost_usd: estimated_cost
                    .filter(|v| *v > 0.0)
                    .map(|v| format!("{v:.6}")),
            })
        })?;
        Ok(row)
    })
    .await
}

#[derive(Debug, Clone, Serialize)]
pub struct TrendPoint {
    pub bucket_start_ms: i64,
    pub channel_id: String,
    pub name: String,
    pub success: i64,
}

pub async fn stats_trend_by_day_channel(
    db_path: PathBuf,
    start_ms: i64,
    offset_ms: i64,
) -> anyhow::Result<Vec<TrendPoint>> {
    with_conn(db_path, move |conn| {
        let mut stmt = conn.prepare(
            r#"
            SELECT
              ((u.ts_ms + ?2) / 86400000) AS bucket_day,
              c.id,
              c.name,
              SUM(CASE WHEN u.success = 1 THEN 1 ELSE 0 END) AS success
            FROM usage_events u
            JOIN channels c ON c.id = u.channel_id
            WHERE u.ts_ms >= ?1
            GROUP BY bucket_day, c.id, c.name
            HAVING SUM(CASE WHEN u.success = 1 THEN 1 ELSE 0 END) > 0
            ORDER BY bucket_day ASC, c.name ASC
            "#,
        )?;

        let rows = stmt.query_map(params![start_ms, offset_ms], |row| {
            let bucket_day: i64 = row.get(0)?;
            let bucket_start_ms = bucket_day
                .saturating_mul(86_400_000)
                .saturating_sub(offset_ms);
            Ok(TrendPoint {
                bucket_start_ms,
                channel_id: row.get(1)?,
                name: row.get(2)?,
                success: row.get::<_, Option<i64>>(3)?.unwrap_or(0),
            })
        })?;

        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    })
    .await
}

#[derive(Debug, Clone, Serialize)]
pub struct ChannelStats {
    pub channel_id: String,
    pub name: String,
    pub protocol: Protocol,
    pub requests: i64,
    pub success: i64,
    pub failed: i64,
    pub avg_latency_ms: Option<f64>,
    pub total_tokens: i64,
    pub estimated_cost_usd: Option<String>,
}

pub async fn stats_channels(db_path: PathBuf, start_ms: i64) -> anyhow::Result<Vec<ChannelStats>> {
    with_conn(db_path, move |conn| {
        let mut stmt = conn.prepare(
            r#"
            SELECT
              c.id,
              c.name,
              c.protocol,
              COUNT(u.id) AS requests,
              SUM(CASE WHEN u.success = 1 THEN 1 ELSE 0 END) AS success,
              SUM(CASE WHEN u.success = 0 THEN 1 ELSE 0 END) AS failed,
              AVG(u.latency_ms) AS avg_latency_ms,
              SUM(COALESCE(u.total_tokens, 0)) AS total_tokens,
              SUM(COALESCE(CAST(u.estimated_cost_usd AS REAL), 0.0)) AS estimated_cost
            FROM channels c
            LEFT JOIN usage_events u
              ON u.channel_id = c.id
             AND u.ts_ms >= ?1
            GROUP BY c.id, c.name, c.protocol
            ORDER BY c.name ASC
            "#,
        )?;
        let rows = stmt.query_map(params![start_ms], |row| {
            Ok(ChannelStats {
                channel_id: row.get(0)?,
                name: row.get(1)?,
                protocol: row.get(2)?,
                requests: row.get(3)?,
                success: row.get::<_, Option<i64>>(4)?.unwrap_or(0),
                failed: row.get::<_, Option<i64>>(5)?.unwrap_or(0),
                avg_latency_ms: row.get(6)?,
                total_tokens: row.get::<_, Option<i64>>(7)?.unwrap_or(0),
                estimated_cost_usd: row
                    .get::<_, Option<f64>>(8)?
                    .filter(|v| *v > 0.0)
                    .map(|v| format!("{v:.6}")),
            })
        })?;

        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    })
    .await
}
