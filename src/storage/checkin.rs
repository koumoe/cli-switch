use rusqlite::OptionalExtension as _;
use rusqlite::params;
use serde::Serialize;
use std::path::PathBuf;

use super::{now_ms, with_conn};

#[derive(Debug, Clone, Serialize)]
pub struct ChannelCheckinsToday {
    pub date: String,
    pub completed_channel_ids: Vec<String>,
}

fn local_today_ymd() -> anyhow::Result<String> {
    let offset = time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);
    let today = time::OffsetDateTime::now_utc().to_offset(offset).date();
    let fmt = time::format_description::parse("[year]-[month]-[day]")?;
    Ok(today.format(&fmt)?)
}

pub async fn get_channel_checkins_today(db_path: PathBuf) -> anyhow::Result<ChannelCheckinsToday> {
    let date = local_today_ymd()?;
    let completed_channel_ids = with_conn(db_path, {
        let date = date.clone();
        move |conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT channel_id
                FROM channel_checkins
                WHERE date = ?1
                "#,
            )?;
            let mut rows = stmt.query([date])?;
            let mut ids = Vec::new();
            while let Some(row) = rows.next()? {
                ids.push(row.get::<_, String>(0)?);
            }
            Ok(ids)
        }
    })
    .await?;

    Ok(ChannelCheckinsToday {
        date,
        completed_channel_ids,
    })
}

pub async fn complete_channel_checkin_today(
    db_path: PathBuf,
    channel_id: String,
) -> anyhow::Result<()> {
    let date = local_today_ymd()?;
    let ts = now_ms();
    with_conn(db_path, move |conn| {
        // 确保渠道存在，避免写入脏数据
        let exists: Option<i64> = conn
            .query_row(
                r#"SELECT 1 FROM channels WHERE id = ?1"#,
                params![channel_id],
                |row| row.get(0),
            )
            .optional()?;
        if exists.is_none() {
            return Err(anyhow::anyhow!("channel not found"));
        }

        conn.execute(
            r#"
            INSERT INTO channel_checkins (channel_id, date, completed_at_ms)
            VALUES (?1, ?2, ?3)
            ON CONFLICT(channel_id, date) DO UPDATE SET completed_at_ms = excluded.completed_at_ms
            "#,
            params![channel_id, date, ts],
        )?;
        Ok(())
    })
    .await
}
