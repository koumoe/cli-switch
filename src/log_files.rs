use anyhow::Context as _;
use serde::Serialize;
use std::path::{Path, PathBuf};
use time::{Date, OffsetDateTime};

#[derive(Debug, Clone, Copy)]
pub enum LogsClearKind {
    DateRange { start: Date, end: Date },
    All,
}

#[derive(Debug, Serialize)]
pub struct ClearLogsResult {
    pub deleted_files: u64,
    pub truncated_files: u64,
}

fn today_local() -> Date {
    OffsetDateTime::now_local()
        .unwrap_or_else(|_| OffsetDateTime::now_utc())
        .date()
}

fn parse_date_suffix(file_name: &str) -> Option<Date> {
    let s = file_name.trim();
    let fmt = time::format_description::parse("[year]-[month]-[day]").ok()?;

    // New format: "YYYY-MM-DD.log"
    if s.len() >= 10 {
        let prefix = s.get(0..10)?;
        let rest = s.get(10..).unwrap_or("");
        if rest == ".log" && let Ok(d) = Date::parse(prefix, &fmt) {
            return Some(d);
        }
    }

    // Legacy format: "cliswitch.jsonl.YYYY-MM-DD"
    if s.len() >= 11 && s.as_bytes().get(s.len() - 11) == Some(&b'.') {
        let suffix = s.get(s.len().saturating_sub(10)..)?;
        return Date::parse(suffix, &fmt).ok();
    }
    None
}

fn should_clear(kind: LogsClearKind, date: Date) -> bool {
    match kind {
        LogsClearKind::All => true,
        LogsClearKind::DateRange { start, end } => date >= start && date <= end,
    }
}

pub fn clear_logs(log_dir: &Path, kind: LogsClearKind) -> anyhow::Result<ClearLogsResult> {
    if !log_dir.is_dir() {
        return Ok(ClearLogsResult {
            deleted_files: 0,
            truncated_files: 0,
        });
    }

    let today = today_local();
    let mut deleted_files = 0_u64;
    let mut truncated_files = 0_u64;

    for entry in std::fs::read_dir(log_dir)
        .with_context(|| format!("读取日志目录失败：{}", log_dir.display()))?
    {
        let entry = entry?;
        let path: PathBuf = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(v) => v,
            None => continue,
        };
        let date = match parse_date_suffix(name) {
            Some(v) => v,
            None => continue,
        };
        if !should_clear(kind, date) {
            continue;
        }

        if date == today {
            let _ = std::fs::OpenOptions::new()
                .write(true)
                .truncate(true)
                .open(&path)?;
            truncated_files = truncated_files.saturating_add(1);
        } else if std::fs::remove_file(&path).is_ok() {
            deleted_files = deleted_files.saturating_add(1);
        }
    }

    Ok(ClearLogsResult {
        deleted_files,
        truncated_files,
    })
}
