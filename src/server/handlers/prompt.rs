use axum::Json;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Deserialize;
use std::path::Path;

use crate::cli_tools::CliToolId;
use crate::server::AppState;
use crate::server::error::{ApiError, map_storage_unit_no_content_err};
use crate::storage;

#[derive(Debug, Deserialize)]
pub(in crate::server) struct PromptProjectInput {
    name: String,
    path: String,
}

#[derive(Debug, Deserialize)]
pub(in crate::server) struct UpdatePromptProjectInput {
    name: Option<String>,
    path: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(in crate::server) struct PromptDocumentQuery {
    tool: Option<String>,
    scope: Option<String>,
    project_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(in crate::server) struct DeletePromptDocumentQuery {
    tool: Option<String>,
    scope: Option<String>,
    project_id: Option<String>,
    expected_updated_at_ms: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(in crate::server) struct SavePromptDocumentInput {
    tool: String,
    scope: String,
    project_id: Option<String>,
    content_md: String,
    expected_updated_at_ms: Option<i64>,
}

fn parse_tool(raw: Option<&str>) -> Result<CliToolId, ApiError> {
    match raw.map(str::trim).filter(|v| !v.is_empty()) {
        Some("gemini") => Ok(CliToolId::Gemini),
        Some("claude") => Ok(CliToolId::Claude),
        Some("codex") => Ok(CliToolId::Codex),
        Some(other) => Err(ApiError::bad_request(
            "prompt_tool_invalid",
            format!("Invalid prompt tool: {other}"),
        )),
        None => Err(ApiError::bad_request(
            "prompt_tool_required",
            "Prompt tool is required",
        )),
    }
}

fn parse_scope(raw: Option<&str>) -> Result<storage::PromptScope, ApiError> {
    match raw.map(str::trim).filter(|v| !v.is_empty()) {
        Some("global") => Ok(storage::PromptScope::Global),
        Some("project") => Ok(storage::PromptScope::Project),
        Some(other) => Err(ApiError::bad_request(
            "prompt_scope_invalid",
            format!("Invalid prompt scope: {other}"),
        )),
        None => Err(ApiError::bad_request(
            "prompt_scope_required",
            "Prompt scope is required",
        )),
    }
}

fn parse_optional_i64(
    name: &str,
    value: Option<String>,
    code: &'static str,
) -> Result<Option<i64>, ApiError> {
    let Some(value) = value else { return Ok(None) };
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    value
        .parse::<i64>()
        .map(Some)
        .map_err(|e| ApiError::bad_request(code, format!("Invalid {name}: {e}")))
}

fn validate_project_id(
    scope: storage::PromptScope,
    project_id: Option<String>,
) -> Result<Option<String>, ApiError> {
    let project_id = project_id
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty());

    match scope {
        storage::PromptScope::Global => {
            if project_id.is_some() {
                return Err(ApiError::bad_request(
                    "prompt_document_project_id_forbidden",
                    "Global prompt does not accept project_id",
                ));
            }
            Ok(None)
        }
        storage::PromptScope::Project => project_id.map(Some).ok_or_else(|| {
            ApiError::bad_request(
                "prompt_document_project_id_required",
                "project_id is required for project prompt",
            )
        }),
    }
}

fn validate_project_path(raw: &str) -> Result<String, ApiError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(ApiError::bad_request(
            "prompt_project_path_required",
            "Project path is required",
        ));
    }

    if !Path::new(trimmed).is_dir() {
        return Err(ApiError::bad_request(
            "prompt_project_path_invalid",
            "Project path must be an existing directory",
        ));
    }

    Ok(trimmed.to_string())
}

fn map_prompt_storage_error(e: &anyhow::Error) -> Option<ApiError> {
    match e.downcast_ref::<storage::StorageError>() {
        Some(storage::StorageError::PromptProjectNotFound { .. }) => Some(ApiError::not_found(
            "prompt_project_not_found",
            "Prompt project not found",
        )),
        Some(storage::StorageError::PromptProjectPathExists { .. }) => Some(ApiError::bad_request(
            "prompt_project_path_exists",
            "Project path already exists",
        )),
        Some(storage::StorageError::PromptProjectNameExists { .. }) => Some(ApiError::bad_request(
            "prompt_project_name_exists",
            "Project name already exists",
        )),
        Some(storage::StorageError::PromptDocumentNotFound) => Some(ApiError::not_found(
            "prompt_document_not_found",
            "Prompt document not found",
        )),
        Some(storage::StorageError::PromptDocumentTooLarge { .. }) => Some(ApiError::bad_request(
            "prompt_document_too_large",
            "Prompt document is too large",
        )),
        Some(storage::StorageError::PromptDocumentVersionConflict { .. }) => {
            Some(ApiError::conflict(
                "prompt_document_version_conflict",
                "Prompt document was updated by another editor",
            ))
        }
        _ => None,
    }
}

pub(in crate::server) async fn list_prompt_projects(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let items = storage::list_prompt_projects(state.db_path()).await?;
    Ok(Json(items))
}

pub(in crate::server) async fn create_prompt_project(
    State(state): State<AppState>,
    Json(input): Json<PromptProjectInput>,
) -> Result<impl IntoResponse, ApiError> {
    let path = validate_project_path(&input.path)?;
    let item = storage::create_prompt_project(
        state.db_path(),
        storage::CreatePromptProject {
            name: input.name,
            path,
        },
    )
    .await
    .map_err(|e| map_prompt_storage_error(&e).unwrap_or(ApiError::Internal(e)))?;
    Ok((StatusCode::CREATED, Json(item)))
}

pub(in crate::server) async fn update_prompt_project(
    State(state): State<AppState>,
    axum::extract::Path(project_id): axum::extract::Path<String>,
    Json(input): Json<UpdatePromptProjectInput>,
) -> Result<impl IntoResponse, ApiError> {
    let path = match input.path {
        Some(path) => Some(validate_project_path(&path)?),
        None => None,
    };

    let res = storage::update_prompt_project(
        state.db_path(),
        project_id,
        storage::UpdatePromptProject {
            name: input.name,
            path,
        },
    )
    .await;

    map_storage_unit_no_content_err(res, map_prompt_storage_error)
}

pub(in crate::server) async fn delete_prompt_project(
    State(state): State<AppState>,
    axum::extract::Path(project_id): axum::extract::Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let res = storage::delete_prompt_project(state.db_path(), project_id).await;
    map_storage_unit_no_content_err(res, map_prompt_storage_error)
}

pub(in crate::server) async fn get_prompt_document(
    State(state): State<AppState>,
    Query(query): Query<PromptDocumentQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let tool = parse_tool(query.tool.as_deref())?;
    let scope = parse_scope(query.scope.as_deref())?;
    let project_id = validate_project_id(scope, query.project_id)?;

    let doc = storage::get_prompt_document(state.db_path(), tool, scope, project_id)
        .await
        .map_err(|e| map_prompt_storage_error(&e).unwrap_or(ApiError::Internal(e)))?;
    Ok(Json(doc))
}

pub(in crate::server) async fn save_prompt_document(
    State(state): State<AppState>,
    Json(input): Json<SavePromptDocumentInput>,
) -> Result<impl IntoResponse, ApiError> {
    let tool = parse_tool(Some(&input.tool))?;
    let scope = parse_scope(Some(&input.scope))?;
    let project_id = validate_project_id(scope, input.project_id)?;

    let doc = storage::save_prompt_document(
        state.db_path(),
        storage::SavePromptDocument {
            tool,
            scope,
            project_id,
            content_md: input.content_md,
            expected_updated_at_ms: input.expected_updated_at_ms,
        },
    )
    .await
    .map_err(|e| map_prompt_storage_error(&e).unwrap_or(ApiError::Internal(e)))?;

    Ok(Json(doc))
}

pub(in crate::server) async fn delete_prompt_document(
    State(state): State<AppState>,
    Query(query): Query<DeletePromptDocumentQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let tool = parse_tool(query.tool.as_deref())?;
    let scope = parse_scope(query.scope.as_deref())?;
    let project_id = validate_project_id(scope, query.project_id)?;
    let expected_updated_at_ms = parse_optional_i64(
        "expected_updated_at_ms",
        query.expected_updated_at_ms,
        "prompt_document_expected_updated_at_ms_invalid",
    )?;

    let res = storage::delete_prompt_document(
        state.db_path(),
        storage::DeletePromptDocument {
            tool,
            scope,
            project_id,
            expected_updated_at_ms,
        },
    )
    .await;

    map_storage_unit_no_content_err(res, map_prompt_storage_error)
}
