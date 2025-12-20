use anyhow::Context as _;
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::Rotation;
use tracing_subscriber::fmt::writer::MakeWriterExt as _;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt as _, reload, util::SubscriberInitExt as _};

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

    let file_appender = tracing_appender::rolling::RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("")
        .filename_suffix("log")
        .build(&log_dir)
        .context("初始化日志轮转失败")?;
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
