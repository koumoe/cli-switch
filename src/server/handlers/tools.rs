use axum::Json;
use axum::extract::State;
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::cli_tools::{CLI_TOOLS, CliToolId, CliToolInstallMethod};
use crate::nodejs;
use crate::server::AppState;
use crate::server::error::ApiError;
use crate::storage;

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ValidateProgram {
    Node,
    Npm,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ValidateProgramRequest {
    pub(crate) program: ValidateProgram,
    pub(crate) path: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct ValidateProgramResponse {
    pub(crate) ok: bool,
    pub(crate) version: String,
    pub(crate) resolved_path: String,
}

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
    pub(crate) npm_available: bool,
    pub(crate) npm_version: Option<String>,
    pub(crate) node_version: Option<String>,
    pub(crate) tools: Vec<CliToolStatus>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct InstallNpmEnvResponse {
    pub(crate) ok: bool,
    pub(crate) installed: bool,
    pub(crate) npm_path: Option<String>,
    pub(crate) node_path: Option<String>,
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

        let npm_available = env.npm_available();
        let npm_version = env.try_get_npm_version();
        let node_version = env.try_get_node_version();

        let tools = CLI_TOOLS
            .iter()
            .map(|d| {
                let mut detected = crate::cli_tools::detect_cli_tool(&env, &data_dir, d);

                // Prefer reporting the terminal shim if it is executable. The GUI process may not
                // inherit user's shell PATH, so relying on PATH-only detection is often flaky.
                if let Ok(shim_path) = crate::terminal::cli_tool_shim_path(d.bin)
                    && shim_path.is_file()
                {
                    let shim_version = crate::cli_tools::try_get_cmd_version_at(&shim_path);

                    // Best-effort cleanup: if the shim can't execute anymore, remove it so it
                    // doesn't shadow real resolution in user shells.
                    if shim_version.is_none() {
                        let _ = crate::terminal::remove_cli_tool_shim(d.bin);
                    } else {
                        if !detected.installed {
                            detected.installed = true;
                            detected.install_path = Some(shim_path);
                        }
                        if detected.version.is_none() {
                            detected.version = shim_version
                                .as_deref()
                                .map(crate::cli_tools::normalize_version_string);
                        }
                    }
                }

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
            npm_available,
            npm_version,
            node_version,
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

pub(in crate::server) async fn install_npm_env(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    // First, check current env (PATH + user settings).
    let settings = storage::get_app_settings(state.db_path()).await?;
    let npm_path0 = settings.cli_tools_npm_path.clone();
    let node_path0 = settings.cli_tools_node_path.clone();

    let already = tokio::task::spawn_blocking(move || {
        let env = crate::cli_tools::CliExecEnv::new(npm_path0.as_deref(), node_path0.as_deref());
        env.npm_available()
    })
    .await
    .map_err(|e| {
        ApiError::bad_request("tools_env_check_failed", format!("env check failed: {e}"))
    })?;

    if already {
        return Ok(Json(InstallNpmEnvResponse {
            ok: true,
            installed: false,
            npm_path: settings.cli_tools_npm_path,
            node_path: settings.cli_tools_node_path,
        }));
    }

    let data_dir = state.data_dir();
    let paths = nodejs::ensure_npm_env_installed(&state.http_client, &data_dir)
        .await
        .map_err(|e| ApiError::bad_request("tools_npm_env_install_failed", e.to_string()))?;

    let npm_path = paths.npm_path.to_string_lossy().to_string();
    let node_path = paths.node_path.to_string_lossy().to_string();

    // Persist as the manual paths so all existing code paths (status/install/auto-update) work.
    let updated_settings = storage::update_app_settings(
        state.db_path(),
        storage::AppSettingsPatch {
            cli_tools_npm_path: Some(npm_path.clone()),
            cli_tools_node_path: Some(node_path.clone()),
            ..Default::default()
        },
    )
    .await?;

    let _ = state.settings_cache.send(Arc::new(updated_settings));

    let next = *state.settings_notify.borrow() + 1;
    let _ = state.settings_notify.send(next);

    Ok(Json(InstallNpmEnvResponse {
        ok: true,
        installed: true,
        npm_path: Some(npm_path),
        node_path: Some(node_path),
    }))
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
    pub(crate) terminal_shim_error: Option<String>,
}

pub(in crate::server) async fn install_cli_tool(
    State(state): State<AppState>,
    Json(input): Json<InstallCliToolRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let def = CLI_TOOLS
        .iter()
        .find(|d| d.id == input.id)
        .ok_or_else(|| ApiError::bad_request("tools_unknown_id", "unknown tool id"))?;

    let settings = storage::get_app_settings(state.db_path()).await?;
    let npm_path = settings.cli_tools_npm_path.clone();
    let node_path = settings.cli_tools_node_path.clone();
    let npm_registry = crate::cli_tools::pick_cli_tools_npm_registry(&state.http_client).await;
    let data_dir = state.data_dir();
    let tools_prefix_dir = crate::cli_tools::cli_tools_npm_prefix_dir(&data_dir);

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
        let tool_path = detected.install_path.clone();

        let (terminal_shim_ok, terminal_shim_dir, terminal_shim_error) =
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
                (
                    false,
                    crate::terminal::cli_tools_shim_dir()
                        .ok()
                        .map(|p| p.to_string_lossy().to_string()),
                    None,
                )
            };

        Ok(InstallCliToolResponse {
            ok: out.status.success(),
            exit_code: out.status.code(),
            stdout: out.stdout,
            stderr: out.stderr,
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
            terminal_shim_error,
        })
    })
    .await;

    match res {
        Ok(Ok(v)) => Ok(Json(v)),
        Ok(Err(e)) => Err(e),
        Err(e) => Err(ApiError::Internal(anyhow::anyhow!(
            "cli tool install task join failed: {e}"
        ))),
    }
}

pub(in crate::server) async fn validate_program(
    Json(input): Json<ValidateProgramRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let raw = input.path.trim();
    if raw.is_empty() {
        return Err(ApiError::bad_request(
            "tools_path_required",
            "path is required",
        ));
    }

    let program = match input.program {
        ValidateProgram::Node => "node",
        ValidateProgram::Npm => "npm",
    };

    let raw = raw.to_string();
    let res = tokio::task::spawn_blocking(move || {
        let resolved =
            crate::cli_tools::resolve_program_from_user_path(program, &raw).ok_or_else(|| {
                ApiError::bad_request(
                    if program == "node" {
                        "tools_node_path_invalid"
                    } else {
                        "tools_npm_path_invalid"
                    },
                    format!("invalid {program} path"),
                )
            })?;

        let v = crate::cli_tools::try_get_cmd_version_at(&resolved).ok_or_else(|| {
            ApiError::bad_request(
                if program == "node" {
                    "tools_node_not_executable"
                } else {
                    "tools_npm_not_executable"
                },
                format!("{program} is not executable"),
            )
        })?;

        Ok::<_, ApiError>(ValidateProgramResponse {
            ok: true,
            version: crate::cli_tools::normalize_version_string(&v),
            resolved_path: resolved.to_string_lossy().to_string(),
        })
    })
    .await;

    match res {
        Ok(Ok(v)) => Ok(Json(v)),
        Ok(Err(e)) => Err(e),
        Err(e) => Err(ApiError::Internal(anyhow::anyhow!(
            "validate program task join failed: {e}"
        ))),
    }
}
