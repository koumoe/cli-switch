use axum::Json;
use axum::extract::State;
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::cli_tools::{CLI_TOOLS, CliToolId, CliToolInstallMethod};
use crate::i18n::{UserFacingIssue, UserFacingIssuePayload, current_locale};
use crate::nodejs;
use crate::server::AppState;
use crate::server::error::ApiError;
use crate::storage;

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
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct CliToolsStatusResponse {
    pub(crate) os: &'static str,
    pub(crate) tools: Vec<CliToolStatus>,
}

pub(in crate::server) async fn cli_tools_status(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let settings = storage::get_app_settings(state.db_path()).await?;
    let npm_path = settings.cli_tools_npm_path.clone();
    let node_path = settings.cli_tools_node_path.clone();
    let data_dir = state.data_dir();

    let res = tokio::task::spawn_blocking(move || {
        let env = crate::cli_tools::CliExecEnv::new(npm_path.as_deref(), node_path.as_deref());

        let tools = CLI_TOOLS
            .iter()
            .map(|d| {
                let detected =
                    crate::cli_tools::detect_cli_tool_with_terminal_shim(&env, &data_dir, d);

                CliToolStatus {
                    id: d.id,
                    name: d.name,
                    bin: d.bin,
                    npm_package: d.npm_package,
                    installed: detected.installed,
                    version: detected.version,
                    install_method: detected.install_method,
                    install_path: detected
                        .install_path
                        .map(|p| p.to_string_lossy().to_string()),
                    installer_path: detected
                        .installer_path
                        .map(|p| p.to_string_lossy().to_string()),
                }
            })
            .collect::<Vec<_>>();

        CliToolsStatusResponse {
            os: crate::cli_tools::os_name(),
            tools,
        }
    })
    .await;

    match res {
        Ok(v) => Ok(Json(v)),
        Err(e) => Err(ApiError::Internal(anyhow::anyhow!(
            "cli tools status task join failed: {e}"
        ))),
    }
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
    let requested_tool = input.id;
    let def = CLI_TOOLS
        .iter()
        .find(|d| d.id == input.id)
        .ok_or_else(|| ApiError::bad_request("tools_unknown_id", "unknown tool id"))?;

    let settings = storage::get_app_settings(state.db_path()).await?;
    let mut npm_path = settings.cli_tools_npm_path.clone();
    let mut node_path = settings.cli_tools_node_path.clone();
    let npm_registry = crate::cli_tools::pick_cli_tools_npm_registry(&state.http_client).await;
    let data_dir = state.data_dir();
    let tools_prefix_dir = crate::cli_tools::cli_tools_npm_prefix_dir(&data_dir);

    // If the tool isn't managed by brew, we need a working npm. Keep it fully automatic and
    // invisible to users: install our bundled npm env on demand and persist it internally.
    let (method0, npm_available0) = tokio::task::spawn_blocking({
        let npm_path = npm_path.clone();
        let node_path = node_path.clone();
        let data_dir = data_dir.clone();
        move || {
            let env = crate::cli_tools::CliExecEnv::new(npm_path.as_deref(), node_path.as_deref());
            let detected0 = crate::cli_tools::detect_cli_tool(&env, &data_dir, def);
            (detected0.install_method, env.npm_available())
        }
    })
    .await
    .map_err(|e| {
        ApiError::Internal(anyhow::anyhow!(
            "cli tool install preflight task join failed: {e}"
        ))
    })?;

    if method0 != CliToolInstallMethod::Brew && !npm_available0 {
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

    let locale = current_locale().unwrap_or_default();
    let res = tokio::task::spawn_blocking(move || {
        let env = crate::cli_tools::CliExecEnv::new(npm_path.as_deref(), node_path.as_deref());
        let detected0 = crate::cli_tools::detect_cli_tool(&env, &data_dir, def);

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
                    .map_err(ApiError::Internal)?
            }
            CliToolInstallMethod::Npm => {
                if !env.npm_available() {
                    return Err(ApiError::bad_request(
                        "tools_npm_missing",
                        "npm not found in PATH",
                    ));
                }
                env.npm_install_global_with_registry(def.npm_package, Some(npm_registry.as_str()))
                    .map_err(ApiError::Internal)?
            }
            CliToolInstallMethod::ManagedNpmPrefix | CliToolInstallMethod::Other => {
                if !env.npm_available() {
                    return Err(ApiError::bad_request(
                        "tools_npm_missing",
                        "npm not found in PATH",
                    ));
                }
                env.npm_install_global_to_prefix(
                    def.npm_package,
                    &tools_prefix_dir,
                    Some(npm_registry.as_str()),
                )
                .map_err(ApiError::Internal)?
            }
        };

        // Re-detect after install/update so we can report the latest version/method/path.
        let detected = crate::cli_tools::detect_cli_tool(&env, &data_dir, def);
        let install_verified = detected.installed && detected.version.is_some();
        let tool_path = if install_verified {
            detected.install_path.clone()
        } else {
            None
        };

        let command_ok = out.status.success();
        let exit_code = out.status.code();
        let stdout = out.stdout;
        let mut stderr = out.stderr;
        if command_ok && !install_verified {
            if !stderr.is_empty() && !stderr.ends_with('\n') {
                stderr.push('\n');
            }
            stderr.push_str(&format!(
                "{} installation completed, but `{} --version` failed; the executable is unusable.",
                def.name, def.bin
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
            ok: command_ok && install_verified,
            exit_code,
            stdout,
            stderr,
            tool: CliToolStatus {
                id: def.id,
                name: def.name,
                bin: def.bin,
                npm_package: def.npm_package,
                installed: detected.installed,
                version: detected.version,
                install_method: detected.install_method,
                install_path: detected
                    .install_path
                    .map(|p| p.to_string_lossy().to_string()),
                installer_path: detected
                    .installer_path
                    .map(|p| p.to_string_lossy().to_string()),
            },
            terminal_shim_ok,
            terminal_shim_dir,
            terminal_shim_issue: terminal_shim_issue
                .as_ref()
                .map(|issue| issue.to_payload(locale)),
        })
    })
    .await;

    match res {
        Ok(Ok(v)) => {
            if requested_tool == CliToolId::Codex && v.ok {
                let identity =
                    crate::codex_upstream::identity_for_version(v.tool.version.as_deref());
                let _ = state.codex_identity_cache.send(Arc::new(identity));
            }
            Ok(Json(v))
        }
        Ok(Err(e)) => Err(e),
        Err(e) => Err(ApiError::Internal(anyhow::anyhow!(
            "cli tool install task join failed: {e}"
        ))),
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
