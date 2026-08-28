use rusqlite::{OptionalExtension as _, params};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::with_conn;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExchangeRate {
    pub base_currency: String,
    pub quote_currency: String,
    pub rate: f64,
    pub effective_date: String,
    pub source: String,
    pub fetched_at_ms: i64,
}

pub async fn get_exchange_rate(
    db_path: PathBuf,
    base_currency: String,
    quote_currency: String,
) -> anyhow::Result<Option<ExchangeRate>> {
    with_conn(db_path, move |conn| {
        conn.query_row(
            r#"
            SELECT base_currency, quote_currency, rate, effective_date, source, fetched_at_ms
            FROM exchange_rates
            WHERE base_currency = ?1 AND quote_currency = ?2
            "#,
            params![base_currency, quote_currency],
            |row| {
                Ok(ExchangeRate {
                    base_currency: row.get(0)?,
                    quote_currency: row.get(1)?,
                    rate: row.get(2)?,
                    effective_date: row.get(3)?,
                    source: row.get(4)?,
                    fetched_at_ms: row.get(5)?,
                })
            },
        )
        .optional()
        .map_err(Into::into)
    })
    .await
}

pub async fn upsert_exchange_rate(db_path: PathBuf, rate: ExchangeRate) -> anyhow::Result<()> {
    if !rate.rate.is_finite() || rate.rate <= 0.0 {
        anyhow::bail!("exchange rate must be a finite number greater than zero");
    }

    with_conn(db_path, move |conn| {
        conn.execute(
            r#"
            INSERT INTO exchange_rates (
              base_currency, quote_currency, rate, effective_date, source, fetched_at_ms
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(base_currency, quote_currency) DO UPDATE SET
              rate = excluded.rate,
              effective_date = excluded.effective_date,
              source = excluded.source,
              fetched_at_ms = excluded.fetched_at_ms
            "#,
            params![
                rate.base_currency,
                rate.quote_currency,
                rate.rate,
                rate.effective_date,
                rate.source,
                rate.fetched_at_ms,
            ],
        )?;
        Ok(())
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cleanup_db(path: &std::path::Path) {
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(PathBuf::from(format!("{}-wal", path.display())));
        let _ = std::fs::remove_file(PathBuf::from(format!("{}-shm", path.display())));
    }

    #[tokio::test]
    async fn exchange_rate_round_trips_and_updates() {
        let db_path = std::env::temp_dir().join(format!(
            "cliswitch-exchange-rate-{}.db",
            uuid::Uuid::new_v4()
        ));
        crate::storage::init_db(&db_path).expect("initialize database");

        let initial = ExchangeRate {
            base_currency: "USD".to_string(),
            quote_currency: "CNY".to_string(),
            rate: 6.7,
            effective_date: "2026-08-27".to_string(),
            source: "Frankfurter".to_string(),
            fetched_at_ms: 100,
        };
        upsert_exchange_rate(db_path.clone(), initial)
            .await
            .expect("store exchange rate");

        let mut updated = get_exchange_rate(db_path.clone(), "USD".to_string(), "CNY".to_string())
            .await
            .expect("read exchange rate")
            .expect("exchange rate exists");
        assert_eq!(updated.rate, 6.7);

        updated.rate = 6.8;
        updated.fetched_at_ms = 200;
        upsert_exchange_rate(db_path.clone(), updated)
            .await
            .expect("update exchange rate");

        let stored = get_exchange_rate(db_path.clone(), "USD".to_string(), "CNY".to_string())
            .await
            .expect("read updated exchange rate")
            .expect("updated exchange rate exists");
        assert_eq!(stored.rate, 6.8);
        assert_eq!(stored.fetched_at_ms, 200);
        cleanup_db(&db_path);
    }
}
