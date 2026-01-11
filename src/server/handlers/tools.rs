use axum::Json;
use axum::extract::State;
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};

use crate::cli_tools::{CLI_TOOLS, CliToolId};
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
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct CliToolsStatusResponse {
    pub(crate) os: &'static str,
    pub(crate) npm_available: bool,
    pub(crate) npm_version: Option<String>,
    pub(crate) node_version: Option<String>,
    pub(crate) tools: Vec<CliToolStatus>,
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
                let installed = env.find_executable(d.bin).is_some();
                let version = if installed {
                    env.try_get_cmd_version(d.bin)
                        .map(|v| crate::cli_tools::normalize_version_string(&v))
                } else {
                    None
                };
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

    let res = tokio::task::spawn_blocking(move || {
        let env = crate::cli_tools::CliExecEnv::new(npm_path.as_deref(), node_path.as_deref());
        if !env.npm_available() {
            return Err(ApiError::bad_request(
                "tools_npm_missing",
                "npm not found in PATH",
            ));
        }

        let out = env
            .npm_install_global(def.npm_package)
            .map_err(ApiError::Internal)?;

        let env = crate::cli_tools::CliExecEnv::new(npm_path.as_deref(), node_path.as_deref());

        let installed = env.find_executable(def.bin).is_some();
        let version = if installed {
            env.try_get_cmd_version(def.bin)
                .map(|v| crate::cli_tools::normalize_version_string(&v))
        } else {
            None
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
