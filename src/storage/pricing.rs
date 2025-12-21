use rusqlite::{OptionalExtension as _, params};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::with_conn;

#[derive(Debug, Clone)]
pub struct UpsertPricingModel {
    pub model_id: String,
    pub prompt_price: Option<String>,
    pub completion_price: Option<String>,
    pub request_price: Option<String>,
    pub cache_read_price: Option<String>,
    pub cache_write_price: Option<String>,
    pub raw_json: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PricingModel {
    pub model_id: String,
    pub prompt_price: Option<String>,
    pub completion_price: Option<String>,
    pub request_price: Option<String>,
    pub updated_at_ms: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PricingStatus {
    pub count: i64,
    pub last_sync_ms: Option<i64>,
}

pub async fn pricing_status(db_path: PathBuf) -> anyhow::Result<PricingStatus> {
    with_conn(db_path, |conn| {
        let count: i64 = conn.query_row(r#"SELECT COUNT(*) FROM pricing_models"#, [], |row| {
            row.get(0)
        })?;
        let last_sync_ms: Option<i64> = conn
            .query_row(
                r#"SELECT MAX(updated_at_ms) FROM pricing_models"#,
                [],
                |row| row.get(0),
            )
            .optional()?
            .flatten();
        Ok(PricingStatus {
            count,
            last_sync_ms,
        })
    })
    .await
}

pub async fn upsert_pricing_models(
    db_path: PathBuf,
    models: Vec<UpsertPricingModel>,
    updated_at_ms: i64,
) -> anyhow::Result<usize> {
    with_conn(db_path, move |conn| {
        let tx = conn.unchecked_transaction()?;
        {
            let mut stmt = tx.prepare(
                r#"
                INSERT INTO pricing_models (
                  model_id, prompt_price, completion_price, request_price,
                  cache_read_price, cache_write_price,
                  raw_json, updated_at_ms
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                ON CONFLICT(model_id) DO UPDATE SET
                  prompt_price = excluded.prompt_price,
                  completion_price = excluded.completion_price,
                  request_price = excluded.request_price,
                  cache_read_price = excluded.cache_read_price,
                  cache_write_price = excluded.cache_write_price,
                  raw_json = excluded.raw_json,
                  updated_at_ms = excluded.updated_at_ms
                "#,
            )?;

            for m in &models {
                stmt.execute(params![
                    m.model_id,
                    m.prompt_price,
                    m.completion_price,
                    m.request_price,
                    m.cache_read_price,
                    m.cache_write_price,
                    m.raw_json,
                    updated_at_ms
                ])?;
            }
        }

        tx.commit()?;
        Ok(models.len())
    })
    .await
}

pub async fn search_pricing_models(
    db_path: PathBuf,
    query: Option<String>,
    limit: i64,
) -> anyhow::Result<Vec<PricingModel>> {
    with_conn(db_path, move |conn| {
        if let Some(q) = query.filter(|s| !s.trim().is_empty()) {
            let like = format!("%{}%", q.trim());
            let mut stmt = conn.prepare(
                r#"
                SELECT model_id, prompt_price, completion_price, request_price, updated_at_ms
                FROM pricing_models
                WHERE model_id LIKE ?1
                ORDER BY model_id ASC
                LIMIT ?2
                "#,
            )?;
            let rows = stmt.query_map(params![like, limit], |row| {
                Ok(PricingModel {
                    model_id: row.get(0)?,
                    prompt_price: row.get(1)?,
                    completion_price: row.get(2)?,
                    request_price: row.get(3)?,
                    updated_at_ms: row.get(4)?,
                })
            })?;
            return rows
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(Into::into);
        }

        let mut stmt = conn.prepare(
            r#"
            SELECT model_id, prompt_price, completion_price, request_price, updated_at_ms
            FROM pricing_models
            ORDER BY model_id ASC
            LIMIT ?1
            "#,
        )?;
        let rows = stmt.query_map(params![limit], |row| {
            Ok(PricingModel {
                model_id: row.get(0)?,
                prompt_price: row.get(1)?,
                completion_price: row.get(2)?,
                request_price: row.get(3)?,
                updated_at_ms: row.get(4)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    })
    .await
}
