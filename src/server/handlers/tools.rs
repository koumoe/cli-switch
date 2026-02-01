use axum::Json;
use axum::extract::State;
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::cli_tools::{CLI_TOOLS, CliToolId};
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

    let res = tokio::task::spawn_blocking(move || {
        let env = crate::cli_tools::CliExecEnv::new(npm_path.as_deref(), node_path.as_deref());

        let npm_available = env.npm_available();
        let npm_version = env.try_get_npm_version();
        let node_version = env.try_get_node_version();

        let tools = CLI_TOOLS
            .iter()
            .map(|d| {
                // Prefer reporting the terminal shim if it is executable. The GUI process may not
                // inherit user's shell PATH, so relying on PATH-only detection is often flaky.
                let mut shim_version: Option<String> = None;
                if let Ok(shim_path) = crate::terminal::cli_tool_shim_path(d.bin)
                    && shim_path.is_file()
                {
                    shim_version = crate::cli_tools::try_get_cmd_version_at(&shim_path);

                    // Best-effort cleanup: if the shim can't execute anymore, remove it so it
                    // doesn't shadow real resolution in user shells.
                    if shim_version.is_none() {
                        let _ = crate::terminal::remove_cli_tool_shim(d.bin);
                    }
                }

                let resolved = env.find_executable(d.bin);
                let installed = shim_version.is_some() || resolved.is_some();

                let version = shim_version
                    .or_else(|| {
                        resolved
                            .as_ref()
                            .and_then(|p| env.try_get_cmd_version_by_path(p))
                    })
                    .map(|v| crate::cli_tools::normalize_version_string(&v));
                CliToolStatus {
                    id: d.id,
                    name: d.name,
                    bin: d.bin,
                    npm_package: d.npm_package,
                    installed,
                    version,
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
    let npm_registry = crate::cli_tools::pick_cli_tools_npm_registry(
        &state.http_client,
        settings.cli_tools_npm_registry.as_deref(),
    )
    .await;
    let data_dir = state.data_dir();
    let tools_prefix_dir = crate::cli_tools::cli_tools_npm_prefix_dir(&data_dir);

    let res = tokio::task::spawn_blocking(move || {
        let env = crate::cli_tools::CliExecEnv::new(npm_path.as_deref(), node_path.as_deref());
        if !env.npm_available() {
            return Err(ApiError::bad_request(
                "tools_npm_missing",
                "npm not found in PATH",
            ));
        }

        // Install/update into our managed npm prefix to avoid touching user's system/global npm.
        let out = env
            .npm_install_global_to_prefix(
                def.npm_package,
                &tools_prefix_dir,
                npm_registry.as_deref(),
            )
            .map_err(ApiError::Internal)?;

        let prefix_bin_dir = env
            .npm_global_bin_dir_for_prefix(&tools_prefix_dir)
            .or_else(|| {
                #[cfg(windows)]
                {
                    Some(tools_prefix_dir.clone())
                }
                #[cfg(not(windows))]
                {
                    Some(tools_prefix_dir.join("bin"))
                }
            });

        // Prefer the managed prefix bin dir so we don't accidentally pick up a shim from user's PATH.
        let tool_path = prefix_bin_dir
            .as_ref()
            .and_then(|d| {
                crate::cli_tools::resolve_program_from_user_path(def.bin, &d.to_string_lossy())
            })
            .or_else(|| env.find_executable(def.bin));
        let installed = tool_path.is_some();
        let version = tool_path
            .as_ref()
            .and_then(|p| env.try_get_cmd_version_by_path(p))
            .map(|v| crate::cli_tools::normalize_version_string(&v));

        let (terminal_shim_ok, terminal_shim_dir, terminal_shim_error) =
            if let Some(tool_path) = tool_path.as_ref() {
                let node_bin_dir = env.node_bin_dir();
                let npm_global_bin_dir = prefix_bin_dir.as_deref();
                match crate::terminal::ensure_cli_tool_shim(
                    def.bin,
                    tool_path,
                    node_bin_dir.as_deref(),
                    npm_global_bin_dir,
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
                installed,
                version,
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
