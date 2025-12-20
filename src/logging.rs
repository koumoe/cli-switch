use anyhow::Context as _;
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    path::{Path, PathBuf},
    sync::OnceLock,
};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::fmt::writer::MakeWriterExt as _;
use tracing_subscriber::{
    EnvFilter, layer::SubscriberExt as _, reload, util::SubscriberInitExt as _,
};
use time::{Date, OffsetDateTime};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogLevel {
    None,
    Debug,
    Info,
    Warning,
    Error,
}

impl LogLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            LogLevel::None => "none",
            LogLevel::Debug => "debug",
            LogLevel::Info => "info",
            LogLevel::Warning => "warning",
            LogLevel::Error => "error",
        }
    }

    pub fn to_env_filter_directive(self) -> &'static str {
        match self {
            LogLevel::None => "off",
            LogLevel::Debug => "debug",
            LogLevel::Info => "info",
            LogLevel::Warning => "warn",
            LogLevel::Error => "error",
        }
    }

    pub fn to_tracing_level(self) -> Option<tracing::Level> {
        match self {
            LogLevel::None => None,
            LogLevel::Debug => Some(tracing::Level::DEBUG),
            LogLevel::Info => Some(tracing::Level::INFO),
            LogLevel::Warning => Some(tracing::Level::WARN),
            LogLevel::Error => Some(tracing::Level::ERROR),
        }
    }
}

struct LoggingRuntime {
    filter_handle: reload::Handle<EnvFilter, tracing_subscriber::Registry>,
    _file_guard: WorkerGuard,
    locked_by_env: bool,
    log_dir: PathBuf,
}

static LOGGING: OnceLock<LoggingRuntime> = OnceLock::new();

fn today_local() -> Date {
    OffsetDateTime::now_local()
        .unwrap_or_else(|_| OffsetDateTime::now_utc())
        .date()
}

fn format_ymd(date: Date) -> anyhow::Result<String> {
    static FMT: OnceLock<Vec<time::format_description::FormatItem<'static>>> = OnceLock::new();
    let fmt = FMT.get_or_init(|| {
        time::format_description::parse("[year]-[month]-[day]").expect("valid format")
    });
    Ok(date.format(fmt).context("格式化日期失败")?)
}

fn open_daily_log_file(log_dir: &Path, date: Date) -> anyhow::Result<File> {
    let name = format!("{}.log", format_ymd(date)?);
    let path = log_dir.join(name);
    Ok(std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("打开日志文件失败：{}", path.display()))?)
}

struct LocalDailyFileAppender {
    log_dir: PathBuf,
    current_date: Date,
    file: File,
}

impl LocalDailyFileAppender {
    fn new(log_dir: PathBuf) -> anyhow::Result<Self> {
        let current_date = today_local();
        let file = open_daily_log_file(&log_dir, current_date)?;
        Ok(Self {
            log_dir,
            current_date,
            file,
        })
    }

    fn maybe_rollover(&mut self) -> anyhow::Result<()> {
        let now = OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc());
        let date = now.date();
        if date != self.current_date {
            self.current_date = date;
            self.file = open_daily_log_file(&self.log_dir, date)?;
        }
        Ok(())
    }
}

impl std::io::Write for LocalDailyFileAppender {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if let Err(e) = self.maybe_rollover() {
            return Err(std::io::Error::new(std::io::ErrorKind::Other, e));
        }
        std::io::Write::write(&mut self.file, buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        std::io::Write::flush(&mut self.file)
    }
}

pub fn log_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("logs")
}

pub fn init(data_dir: &Path, settings_level: LogLevel) -> anyhow::Result<()> {
    let log_dir = log_dir(data_dir);
    std::fs::create_dir_all(&log_dir)
        .with_context(|| format!("创建日志目录失败：{}", log_dir.display()))?;

    let (env_filter, locked_by_env) = match std::env::var("CLISWITCH_LOG") {
        Ok(v) if !v.trim().is_empty() => (EnvFilter::new(v), true),
        _ => (
            EnvFilter::new(settings_level.to_env_filter_directive()),
            false,
        ),
    };

    let (filter_layer, filter_handle) = reload::Layer::new(env_filter);

    let file_appender = LocalDailyFileAppender::new(log_dir.clone()).context("初始化日志文件写入器失败")?;
    let (file_writer, file_guard) = tracing_appender::non_blocking(file_appender);

    let console_layer = tracing_subscriber::fmt::layer()
        .compact()
        .with_writer(std::io::stderr.with_filter(|meta| meta.target() != "proxy_body"))
        .with_target(false)
        .with_thread_ids(false)
        .with_thread_names(false)
        .with_file(false)
        .with_line_number(false);

    let file_layer = tracing_subscriber::fmt::layer()
        .json()
        .with_target(false)
        .with_current_span(false)
        .with_span_list(false)
        .with_thread_ids(false)
        .with_thread_names(false)
        .with_file(false)
        .with_line_number(false)
        .with_writer(file_writer);

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(console_layer)
        .with(file_layer)
        .init();

    LOGGING
        .set(LoggingRuntime {
            filter_handle,
            _file_guard: file_guard,
            locked_by_env,
            log_dir,
        })
        .map_err(|_| anyhow::anyhow!("日志系统已初始化"))?;

    Ok(())
}

pub fn set_level(level: LogLevel) -> anyhow::Result<()> {
    let rt = LOGGING.get().context("日志系统尚未初始化")?;
    if rt.locked_by_env {
        return Ok(());
    }
    rt.filter_handle
        .reload(EnvFilter::new(level.to_env_filter_directive()))
        .context("更新日志级别失败")?;
    Ok(())
}

pub fn current_log_dir() -> Option<&'static Path> {
    Some(LOGGING.get()?.log_dir.as_path())
}
