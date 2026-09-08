use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex as StdMutex};

use serde::Serialize;
use tokio::sync::{Mutex, RwLock};

use crate::cli_tools::{self, CLI_TOOLS, CliToolId, CliToolInstallMethod, DetectedCliTool};
use crate::storage;

use super::AppState;

pub(crate) const CHECK_INTERVAL: std::time::Duration = std::time::Duration::from_secs(6 * 3600);

#[derive(Debug, Clone, Serialize)]
pub(crate) struct CliToolStatus {
    pub(crate) id: CliToolId,
    pub(crate) name: &'static str,
    pub(crate) bin: &'static str,
    pub(crate) npm_package: &'static str,
    pub(crate) installed: bool,
    pub(crate) version: Option<String>,
    pub(crate) install_method: CliToolInstallMethod,
    pub(crate) install_path: Option<String>,
    pub(crate) installer_path: Option<String>,
    pub(crate) latest_version: Option<String>,
    pub(crate) update_available: bool,
    pub(crate) update_check_error: Option<String>,
    pub(crate) updating: bool,
}

impl CliToolStatus {
    pub(crate) fn from_detected(def: &cli_tools::CliToolDef, detected: DetectedCliTool) -> Self {
        Self {
            id: def.id,
            name: def.name,
            bin: def.bin,
            npm_package: def.npm_package,
            installed: detected.installed,
            version: detected.version,
            install_method: detected.install_method,
            install_path: detected
                .install_path
                .map(|p| p.to_string_lossy().into_owned()),
            installer_path: detected
                .installer_path
                .map(|p| p.to_string_lossy().into_owned()),
            latest_version: None,
            update_available: false,
            update_check_error: None,
            updating: false,
        }
    }

    pub(crate) fn apply_latest_version(&mut self, latest: anyhow::Result<String>) {
        self.latest_version = None;
        self.update_available = false;
        self.update_check_error = None;
        match latest {
            Ok(latest) => {
                if self.installed {
                    match self.version.as_deref().map_or_else(
                        || anyhow::bail!("installed CLI version is unavailable"),
                        |version| cli_tools::updates::is_update_available(version, &latest),
                    ) {
                        Ok(available) => self.update_available = available,
                        Err(err) => self.update_check_error = Some(err.to_string()),
                    }
                }
                self.latest_version = Some(latest);
            }
            Err(err) => self.update_check_error = Some(err.to_string()),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct CliToolsStatusResponse {
    pub(crate) os: &'static str,
    pub(crate) tools: Vec<CliToolStatus>,
    pub(crate) checked_at: Option<i64>,
}

#[derive(Default)]
pub(crate) struct CliToolsRuntime {
    // Detection and installation must not observe or mutate a half-installed executable.
    pub(crate) operation: Arc<Mutex<()>>,
    snapshot: RwLock<Option<CliToolsStatusResponse>>,
    active_tool: StdMutex<Option<CliToolId>>,
    refresh_pending: AtomicBool,
    pub(crate) updates_changed: tokio::sync::Notify,
}

impl CliToolsRuntime {
    pub(crate) async fn snapshot(&self) -> Option<CliToolsStatusResponse> {
        let mut snapshot = self.snapshot.read().await.clone()?;
        let active = *self.active_tool.lock().unwrap_or_else(|e| e.into_inner());
        for tool in &mut snapshot.tools {
            tool.updating = active == Some(tool.id);
        }
        Some(snapshot)
    }

    pub(crate) fn start_install(self: &Arc<Self>, tool: CliToolId) -> InstallActivity {
        *self.active_tool.lock().unwrap_or_else(|e| e.into_inner()) = Some(tool);
        InstallActivity(self.clone())
    }

    pub(crate) async fn record_checked_tool(&self, tool: &CliToolStatus) {
        if let Some(previous) = self
            .snapshot
            .write()
            .await
            .as_mut()
            .and_then(|snapshot| snapshot.tools.iter_mut().find(|item| item.id == tool.id))
        {
            *previous = tool.clone();
        }
    }

    pub(crate) async fn invalidate(&self) {
        *self.snapshot.write().await = None;
        self.updates_changed.notify_one();
    }

    pub(crate) async fn record_install(&self, tool: &mut CliToolStatus) {
        let mut cache = self.snapshot.write().await;
        if let Some(previous) = cache
            .as_mut()
            .and_then(|s| s.tools.iter_mut().find(|t| t.id == tool.id))
        {
            if let Some(latest) = previous.latest_version.clone() {
                tool.apply_latest_version(Ok(latest));
            } else {
                tool.update_check_error = previous.update_check_error.clone();
            }
            *previous = tool.clone();
        }
    }
}

pub(crate) struct InstallActivity(Arc<CliToolsRuntime>);

impl Drop for InstallActivity {
    fn drop(&mut self) {
        *self.0.active_tool.lock().unwrap_or_else(|e| e.into_inner()) = None;
        if self.0.refresh_pending.load(Ordering::SeqCst) {
            self.0.updates_changed.notify_one();
        }
    }
}

pub(crate) async fn status(
    state: &AppState,
    refresh: bool,
) -> anyhow::Result<CliToolsStatusResponse> {
    load_status(state, refresh, true).await
}

pub(crate) async fn background_status(
    state: &AppState,
    refresh: bool,
) -> anyhow::Result<CliToolsStatusResponse> {
    load_status(state, refresh, false).await
}

async fn load_status(
    state: &AppState,
    refresh: bool,
    notify_background: bool,
) -> anyhow::Result<CliToolsStatusResponse> {
    let runtime = &state.cli_tools_runtime;
    if refresh {
        runtime.refresh_pending.store(true, Ordering::SeqCst);
    }
    if let Some(snapshot) = runtime.snapshot().await
        && (!runtime.refresh_pending.load(Ordering::SeqCst)
            || snapshot.tools.iter().any(|t| t.updating))
    {
        return Ok(snapshot);
    }
    // Coalesce concurrent checks, including the initial page load and startup task.
    let operation = Arc::new(match runtime.operation.clone().try_lock_owned() {
        Ok(guard) => guard,
        Err(_) => {
            let guard = runtime.operation.clone().lock_owned().await;
            if let Some(snapshot) = runtime.snapshot().await
                && !runtime.refresh_pending.load(Ordering::SeqCst)
            {
                return Ok(snapshot);
            }
            guard
        }
    });
    runtime.refresh_pending.store(false, Ordering::SeqCst);
    let settings = storage::get_app_settings(state.db_path()).await?;
    let data_dir = state.data_dir();
    let detection_operation = operation.clone();
    let detected = tokio::task::spawn_blocking(move || {
        let _operation = detection_operation;
        let env = cli_tools::CliExecEnv::new(
            settings.cli_tools_npm_path.as_deref(),
            settings.cli_tools_node_path.as_deref(),
        );
        CLI_TOOLS
            .iter()
            .map(|def| {
                CliToolStatus::from_detected(
                    def,
                    cli_tools::detect_cli_tool_with_terminal_shim(&env, &data_dir, def),
                )
            })
            .collect::<Vec<_>>()
    })
    .await?;
    let registry = cli_tools::pick_cli_tools_npm_registry(&state.http_client).await;
    let tools = futures_util::future::join_all(detected.into_iter().zip(CLI_TOOLS).map(
        |(mut tool, def)| {
            let registry = &registry;
            async move {
                let latest = cli_tools::updates::latest_version(
                    &state.http_client,
                    def,
                    tool.install_method,
                    registry,
                )
                .await;
                if let Err(err) = &latest {
                    tracing::warn!(tool = tool.name, err = %err, "CLI update check failed");
                }
                tool.apply_latest_version(latest);
                tool
            }
        },
    ))
    .await;
    let snapshot = CliToolsStatusResponse {
        os: cli_tools::os_name(),
        tools,
        checked_at: Some(time::OffsetDateTime::now_utc().unix_timestamp()),
    };
    *runtime.snapshot.write().await = Some(snapshot.clone());
    if notify_background {
        runtime.updates_changed.notify_one();
    }
    Ok(snapshot)
}

pub(crate) fn should_auto_update(tool: &CliToolStatus, settings: &storage::AppSettings) -> bool {
    let enabled = match tool.id {
        CliToolId::Gemini => settings.gemini_cli_auto_update_enabled,
        CliToolId::Claude => settings.claude_code_auto_update_enabled,
        CliToolId::Codex => settings.codex_auto_update_enabled,
    };
    enabled
        && tool.installed
        && tool.update_available
        && tool.update_check_error.is_none()
        && !tool.updating
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool(id: CliToolId, version: Option<&str>, latest: &str) -> CliToolStatus {
        let def = CLI_TOOLS.iter().find(|def| def.id == id).unwrap();
        let mut tool = CliToolStatus::from_detected(
            def,
            DetectedCliTool {
                installed: version.is_some(),
                version: version.map(str::to_owned),
                install_method: CliToolInstallMethod::Npm,
                install_path: None,
                installer_path: None,
            },
        );
        tool.apply_latest_version(Ok(latest.to_string()));
        tool
    }

    #[test]
    fn only_newer_installed_versions_are_update_candidates() {
        for (current, available) in [
            (Some("1.0.0"), true),
            (Some("1.1.0"), false),
            (Some("2.0.0"), false),
            (None, false),
        ] {
            let tool = tool(CliToolId::Codex, current, "1.1.0");
            assert_eq!(tool.update_available, available);
            assert!(tool.update_check_error.is_none());
        }
    }

    #[test]
    fn failed_check_clears_stale_update_offer_and_can_recover() {
        let mut tool = tool(CliToolId::Codex, Some("1.0.0"), "1.1.0");
        tool.apply_latest_version(Err(anyhow::anyhow!("registry unavailable")));
        assert!(!tool.update_available);
        assert!(tool.latest_version.is_none());
        assert_eq!(
            tool.update_check_error.as_deref(),
            Some("registry unavailable")
        );
        tool.apply_latest_version(Ok("1.2.0".into()));
        assert!(tool.update_available);
        assert!(tool.update_check_error.is_none());
    }

    #[test]
    fn invalid_local_version_is_a_check_failure() {
        let tool = tool(CliToolId::Claude, Some("unknown"), "2.0.0");
        assert!(!tool.update_available);
        assert!(tool.update_check_error.is_some());
    }

    #[test]
    fn automatic_updates_require_individual_opt_in_and_an_installed_newer_release() {
        for def in CLI_TOOLS {
            let mut settings = storage::AppSettings {
                gemini_cli_auto_update_enabled: false,
                claude_code_auto_update_enabled: false,
                codex_auto_update_enabled: false,
                ..Default::default()
            };
            let mut tool = tool(def.id, Some("1.0.0"), "1.1.0");
            assert!(!should_auto_update(&tool, &settings));
            match def.id {
                CliToolId::Gemini => settings.gemini_cli_auto_update_enabled = true,
                CliToolId::Claude => settings.claude_code_auto_update_enabled = true,
                CliToolId::Codex => settings.codex_auto_update_enabled = true,
            }
            assert!(should_auto_update(&tool, &settings));
            tool.installed = false;
            assert!(!should_auto_update(&tool, &settings));
            tool.installed = true;
            tool.updating = true;
            assert!(!should_auto_update(&tool, &settings));
            tool.updating = false;
            tool.update_check_error = Some("offline".into());
            assert!(!should_auto_update(&tool, &settings));
            tool.apply_latest_version(Ok("1.0.0".into()));
            assert!(!should_auto_update(&tool, &settings));
        }
    }

    #[tokio::test]
    async fn polling_reflects_install_activity_and_post_install_version_without_network() {
        let runtime = Arc::new(CliToolsRuntime::default());
        let before = tool(CliToolId::Codex, Some("1.0.0"), "1.1.0");
        *runtime.snapshot.write().await = Some(CliToolsStatusResponse {
            os: "test",
            tools: vec![before],
            checked_at: Some(123),
        });
        let _operation = runtime.operation.lock().await;
        let activity = runtime.start_install(CliToolId::Codex);
        let during = tokio::time::timeout(std::time::Duration::from_secs(1), runtime.snapshot())
            .await
            .unwrap()
            .unwrap();
        assert!(during.tools[0].updating);
        let mut after = tool(CliToolId::Codex, Some("1.1.0"), "1.1.0");
        runtime.record_install(&mut after).await;
        drop(activity);
        let snapshot = runtime.snapshot().await.unwrap();
        assert_eq!(snapshot.checked_at, Some(123));
        assert_eq!(snapshot.tools[0].version.as_deref(), Some("1.1.0"));
        assert!(!snapshot.tools[0].updating);
        assert!(!snapshot.tools[0].update_available);
    }

    #[tokio::test]
    async fn deferred_check_wakes_after_the_installer_finishes() {
        use futures_util::FutureExt as _;
        let runtime = Arc::new(CliToolsRuntime::default());
        let activity = runtime.start_install(CliToolId::Codex);
        runtime.refresh_pending.store(true, Ordering::SeqCst);
        assert!(runtime.updates_changed.notified().now_or_never().is_none());
        drop(activity);
        assert!(runtime.refresh_pending.load(Ordering::SeqCst));
        assert!(runtime.updates_changed.notified().now_or_never().is_some());
    }

    #[tokio::test]
    async fn failed_install_preserves_offer_for_retry() {
        let runtime = CliToolsRuntime::default();
        *runtime.snapshot.write().await = Some(CliToolsStatusResponse {
            os: "test",
            tools: vec![tool(CliToolId::Gemini, Some("1.0.0"), "1.1.0")],
            checked_at: Some(123),
        });
        let mut after = tool(CliToolId::Gemini, Some("1.0.0"), "1.0.0");
        runtime.record_install(&mut after).await;
        assert!(after.update_available);
        assert_eq!(after.latest_version.as_deref(), Some("1.1.0"));
    }
}
