use axum::Json;
use axum::extract::{Query, State};
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::cli_tools::{CLI_TOOLS, CliToolId, CliToolInstallMethod};
use crate::i18n::{UserFacingIssue, UserFacingIssuePayload, current_locale};
use crate::nodejs;
use crate::server::AppState;
use crate::server::cli_tool_updates::{self, CliToolStatus};
use crate::server::error::ApiError;
use crate::storage;

#[derive(Debug, Default, Deserialize)]
pub(crate) struct CliToolsStatusQuery {
    #[serde(default)]
    refresh: bool,
}

pub(in crate::server) async fn cli_tools_status(
    State(state): State<AppState>,
    Query(query): Query<CliToolsStatusQuery>,
) -> Result<impl IntoResponse, ApiError> {
    Ok(Json(cli_tool_updates::status(&state, query.refresh).await?))
}

#[derive(Debug, Deserialize)]
pub(crate) struct InstallCliToolRequest {
    pub(crate) id: CliToolId,
}

#[derive(Debug, Serialize)]
pub(crate) struct InstallCliToolResponse {
    pub(crate) ok: bool,
    pub(crate) exit_code: Option<i32>,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
    pub(crate) tool: CliToolStatus,
    pub(crate) terminal_shim_ok: bool,
    pub(crate) terminal_shim_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) terminal_shim_issue: Option<UserFacingIssuePayload>,
}

pub(in crate::server) async fn install_cli_tool(
    State(state): State<AppState>,
    Json(input): Json<InstallCliToolRequest>,
) -> Result<impl IntoResponse, ApiError> {
    // Keep ownership of the installation lock even if an HTTP client disconnects.
    let locale = current_locale().unwrap_or_default();
    let response = tokio::spawn(run_cli_tool_install(state, input.id, false, locale))
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("CLI install task join failed: {e}")))??;
    Ok(Json(response))
}

pub(crate) async fn run_cli_tool_install(
    state: AppState,
    requested_tool: CliToolId,
    automatic: bool,
    locale: crate::i18n::AppLocale,
) -> Result<InstallCliToolResponse, ApiError> {
    let runtime = state.cli_tools_runtime.clone();
    let operation = Arc::new(runtime.operation.clone().lock_owned().await);
    let def = CLI_TOOLS
        .iter()
        .find(|d| d.id == requested_tool)
        .ok_or_else(|| ApiError::bad_request("tools_unknown_id", "unknown tool id"))?;

    let settings = storage::get_app_settings(state.db_path()).await?;
    if automatic {
        let snapshot = runtime.snapshot().await;
        let candidate = snapshot
            .as_ref()
            .and_then(|s| s.tools.iter().find(|t| t.id == requested_tool));
        if !candidate.is_some_and(|tool| cli_tool_updates::should_auto_update(tool, &settings)) {
            return Err(ApiError::conflict(
                "tools_update_unavailable",
                "CLI update is no longer enabled or available",
            ));
        }
    }
    let activity = Arc::new(runtime.start_install(requested_tool));

    let mut npm_path = settings.cli_tools_npm_path.clone();
    let mut node_path = settings.cli_tools_node_path.clone();
    let npm_registry = crate::cli_tools::pick_cli_tools_npm_registry(&state.http_client).await;
    let data_dir = state.data_dir();
    let tools_prefix_dir = crate::cli_tools::cli_tools_npm_prefix_dir(&data_dir);

    // If the tool isn't managed by brew, we need a working npm. Keep it fully automatic and
    // invisible to users: install our bundled npm env on demand and persist it internally.
    let (detected0, npm_available0) = tokio::task::spawn_blocking({
        let npm_path = npm_path.clone();
        let node_path = node_path.clone();
        let data_dir = data_dir.clone();
        move || {
            let env = crate::cli_tools::CliExecEnv::new(npm_path.as_deref(), node_path.as_deref());
            let detected0 =
                crate::cli_tools::detect_cli_tool_with_terminal_shim(&env, &data_dir, def);
            (detected0, env.npm_available())
        }
    })
    .await
    .map_err(|e| {
        ApiError::Internal(anyhow::anyhow!(
            "cli tool install preflight task join failed: {e}"
        ))
    })?;

    // Recheck against the actual installer and pin npm installs to that release/source.
    // A mirror lag or an external CLI update must never cause an automatic downgrade.
    let mut current = CliToolStatus::from_detected(def, detected0.clone());
    let release = match crate::cli_tools::updates::latest_release(
        &state.http_client,
        def,
        detected0.install_method,
        &npm_registry,
    )
    .await
    {
        Ok(release) => release,
        Err(err) => {
            current.apply_latest_version(Err(anyhow::anyhow!(err.to_string())));
            runtime.record_checked_tool(&current).await;
            return Err(ApiError::Internal(err));
        }
    };
    current.apply_latest_version(Ok(release.version.clone()));
    runtime.record_checked_tool(&current).await;
    if (automatic
        && !cli_tool_updates::should_auto_update(
            &current,
            &storage::get_app_settings(state.db_path()).await?,
        ))
        || (current.installed && !current.update_available)
    {
        return Err(ApiError::conflict(
            "tools_update_unavailable",
            "CLI update is no longer enabled or available",
        ));
    }
    let target_version = release.version;
    let npm_package = format!("{}@{}", def.npm_package, target_version);
    let npm_registry = release.npm_registry.unwrap_or(npm_registry);

    if detected0.install_method != CliToolInstallMethod::Brew && !npm_available0 {
        let paths = nodejs::ensure_npm_env_installed(&state.http_client, &data_dir)
            .await
            .map_err(|e| ApiError::bad_request("tools_npm_env_install_failed", e.to_string()))?;

        let npm_path1 = paths.npm_path.to_string_lossy().to_string();
        let node_path1 = paths.node_path.to_string_lossy().to_string();

        let updated_settings = storage::update_app_settings(
            state.db_path(),
            storage::AppSettingsPatch {
                cli_tools_npm_path: Some(npm_path1.clone()),
                cli_tools_node_path: Some(node_path1.clone()),
                ..Default::default()
            },
        )
        .await?;

        let _ = state.settings_cache.send(Arc::new(updated_settings));
        let next = *state.settings_notify.borrow() + 1;
        let _ = state.settings_notify.send(next);

        npm_path = Some(npm_path1);
        node_path = Some(node_path1);
    }

    if automatic
        && !cli_tool_updates::should_auto_update(
            &current,
            &storage::get_app_settings(state.db_path()).await?,
        )
    {
        return Err(ApiError::conflict(
            "tools_update_unavailable",
            "CLI update is no longer enabled or available",
        ));
    }

    let expected_install_method = detected0.install_method;
    let expected_install_path = detected0.install_path;
    let install_operation = operation.clone();
    let install_activity = activity.clone();
    let res = tokio::task::spawn_blocking(move || {
        // The blocking installer retains these guards through timeout/HTTP cancellation.
        let _operation = install_operation;
        let _activity = install_activity;
        let env = crate::cli_tools::CliExecEnv::new(npm_path.as_deref(), node_path.as_deref());
        let detected0 = crate::cli_tools::detect_cli_tool_with_terminal_shim(&env, &data_dir, def);
        if detected0.install_method != expected_install_method
            || detected0.install_path != expected_install_path
            || (automatic && !detected0.installed)
            || (detected0.installed && !detected0.version.as_deref().is_some_and(|version| {
                crate::cli_tools::updates::is_update_available(version, &target_version).unwrap_or(false)
            }))
        {
            return Err(ApiError::conflict("tools_update_unavailable", "CLI installation changed or the target version is no longer newer; refresh and retry"));
        }

        // Decide update strategy without asking the user:
        // - If installed via brew, upgrade via brew (no npm required).
        // - If installed via global npm, update via global npm.
        // - Otherwise, install/update via CliSwitch-managed npm prefix.
        let out = match detected0.install_method {
            CliToolInstallMethod::Brew => {
                let Some(brew) = detected0.installer_path.clone() else {
                    return Err(ApiError::bad_request(
                        "tools_brew_missing",
                        "brew not found in PATH",
                    ));
                };
                crate::cli_tools::brew_upgrade_cli_tool(&brew, def.id)
            }
            CliToolInstallMethod::Npm => {
                if !env.npm_available() {
                    return Err(ApiError::bad_request(
                        "tools_npm_missing",
                        "npm not found in PATH",
                    ));
                }
                env.npm_install_global_with_registry(&npm_package, Some(npm_registry.as_str()))
            }
            CliToolInstallMethod::ManagedNpmPrefix | CliToolInstallMethod::Other => {
                if !env.npm_available() {
                    return Err(ApiError::bad_request(
                        "tools_npm_missing",
                        "npm not found in PATH",
                    ));
                }
                env.npm_install_global_to_prefix(
                    &npm_package,
                    &tools_prefix_dir,
                    Some(npm_registry.as_str()),
                )
            }
        };

        // Re-detect after install/update so we can report the latest version/method/path.
        let detected = crate::cli_tools::detect_cli_tool_with_terminal_shim(&env, &data_dir, def);
        let install_verified = detected.installed && detected.version.is_some();
        let target_verified = detected.version.as_deref().is_some_and(|version| {
            crate::cli_tools::updates::is_update_available(version, &target_version)
                .is_ok_and(|available| !available)
        });
        let tool_path = if install_verified {
            detected.install_path.clone()
        } else {
            None
        };

        let (command_ok, exit_code, stdout, mut stderr) = match out {
            Ok(out) => (
                out.status.success(),
                out.status.code(),
                out.stdout,
                out.stderr,
            ),
            Err(err) => (false, None, String::new(), err.to_string()),
        };
        if command_ok && (!install_verified || !target_verified) {
            if !stderr.is_empty() && !stderr.ends_with('\n') {
                stderr.push('\n');
            }
            stderr.push_str(&format!(
                "{} installation completed, but `{} --version` did not report the requested version {} or newer.",
                def.name, def.bin, target_version
            ));
        }

        let (terminal_shim_ok, terminal_shim_dir, terminal_shim_detail) =
            if let Some(tool_path) = tool_path.as_ref() {
                let node_bin_dir = env.node_bin_dir();
                let npm_global_bin_dir =
                    if detected.install_method == CliToolInstallMethod::ManagedNpmPrefix {
                        Some(crate::cli_tools::cli_tools_npm_prefix_bin_dir(
                            &tools_prefix_dir,
                        ))
                    } else {
                        None
                    };
                match crate::terminal::ensure_cli_tool_shim(
                    def.bin,
                    tool_path,
                    node_bin_dir.as_deref(),
                    npm_global_bin_dir.as_deref(),
                ) {
                    Ok(r) => (true, Some(r.shim_dir.to_string_lossy().to_string()), None),
                    Err(e) => (
                        false,
                        crate::terminal::cli_tools_shim_dir()
                            .ok()
                            .map(|p| p.to_string_lossy().to_string()),
                        Some(e.to_string()),
                    ),
                }
            } else {
                // Do not leave a generated shim pointing at a package-manager placeholder or
                // otherwise unusable executable.
                let _ = crate::terminal::remove_cli_tool_shim(def.bin);
                (
                    false,
                    crate::terminal::cli_tools_shim_dir()
                        .ok()
                        .map(|p| p.to_string_lossy().to_string()),
                    None,
                )
            };

        let terminal_shim_issue = terminal_shim_detail.as_ref().map(|detail| {
            UserFacingIssue::new("tools_terminal_shim_setup_failed")
                .with_arg("tool", def.id.as_str())
                .with_detail(detail.clone())
        });

        Ok(InstallCliToolResponse {
            ok: command_ok && install_verified && target_verified,
            exit_code,
            stdout,
            stderr,
            tool: CliToolStatus::from_detected(def, detected),
            terminal_shim_ok,
            terminal_shim_dir,
            terminal_shim_issue: terminal_shim_issue
                .as_ref()
                .map(|issue| issue.to_payload(locale)),
        })
    })
    .await;

    match res {
        Ok(Ok(mut v)) => {
            runtime.record_install(&mut v.tool).await;
            if requested_tool == CliToolId::Codex && v.ok {
                let identity =
                    crate::codex_upstream::identity_for_version(v.tool.version.as_deref());
                let _ = state.codex_identity_cache.send(Arc::new(identity));
            }
            Ok(v)
        }
        Ok(Err(e)) => {
            runtime.invalidate().await;
            Err(e)
        }
        Err(e) => {
            runtime.invalidate().await;
            Err(ApiError::Internal(anyhow::anyhow!(
                "cli tool install task join failed: {e}"
            )))
        }
    }
}

pub(in crate::server) async fn cli_tools_proxy_config_status(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let listen_addr = state.listen_addr;
    let res =
        tokio::task::spawn_blocking(move || crate::cli_tool_proxy_config::get_status(listen_addr))
            .await;

    match res {
        Ok(Ok(v)) => Ok(Json(v)),
        Ok(Err(e)) => Err(ApiError::Internal(e)),
        Err(e) => Err(ApiError::Internal(anyhow::anyhow!(
            "cli tools proxy config status task join failed: {e}"
        ))),
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct ApplyCliToolsProxyConfigRequest {
    /// Optional: apply only selected tools. Defaults to all supported CLI tools.
    pub(crate) tools: Option<Vec<CliToolId>>,
}

pub(in crate::server) async fn cli_tools_proxy_config_apply(
    State(state): State<AppState>,
    Json(input): Json<ApplyCliToolsProxyConfigRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let listen_addr = state.listen_addr;
    let locale = current_locale().unwrap_or_default();
    let tools = input
        .tools
        .unwrap_or_else(|| vec![CliToolId::Claude, CliToolId::Codex, CliToolId::Gemini]);

    // Avoid holding references across the blocking boundary.
    let tools2 = tools.clone();
    let res = tokio::task::spawn_blocking(move || {
        crate::cli_tool_proxy_config::apply(listen_addr, &tools2, locale)
    })
    .await;

    match res {
        Ok(Ok(v)) => Ok(Json(v)),
        Ok(Err(e)) => Err(ApiError::bad_request(
            "tools_proxy_config_apply_failed",
            e.to_string(),
        )),
        Err(e) => Err(ApiError::Internal(anyhow::anyhow!(
            "cli tools proxy config apply task join failed: {e}"
        ))),
    }
}
