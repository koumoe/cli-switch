use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Deserialize;

use crate::server::AppState;
use crate::server::error::{ApiError, map_storage_unit_no_content};
use crate::storage;

pub(in crate::server) async fn list_routes(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let routes = storage::list_routes(state.db_path()).await?;
    Ok(Json(routes))
}

pub(in crate::server) async fn create_route(
    State(state): State<AppState>,
    Json(input): Json<storage::CreateRoute>,
) -> Result<impl IntoResponse, ApiError> {
    if input.name.trim().is_empty() {
        return Err(ApiError::BadRequest("name 不能为空".to_string()));
    }
    let route = storage::create_route(state.db_path(), input).await?;
    Ok((StatusCode::CREATED, Json(route)))
}

pub(in crate::server) async fn update_route(
    State(state): State<AppState>,
    axum::extract::Path(route_id): axum::extract::Path<String>,
    Json(input): Json<storage::UpdateRoute>,
) -> Result<impl IntoResponse, ApiError> {
    let res = storage::update_route(state.db_path(), route_id, input).await;
    map_storage_unit_no_content(res, |msg| {
        msg.starts_with("route not found")
            .then(|| ApiError::NotFound("route not found".to_string()))
    })
}

pub(in crate::server) async fn delete_route(
    State(state): State<AppState>,
    axum::extract::Path(route_id): axum::extract::Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let res = storage::delete_route(state.db_path(), route_id).await;
    map_storage_unit_no_content(res, |msg| {
        msg.starts_with("route not found")
            .then(|| ApiError::NotFound("route not found".to_string()))
    })
}

pub(in crate::server) async fn list_route_channels(
    State(state): State<AppState>,
    axum::extract::Path(route_id): axum::extract::Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let db_path = state.db_path();
    let Some(_) = storage::get_route(db_path.clone(), route_id.clone()).await? else {
        return Err(ApiError::NotFound("route not found".to_string()));
    };
    let items = storage::list_route_channels(db_path, route_id).await?;
    Ok(Json(items))
}

#[derive(Debug, Deserialize)]
pub(in crate::server) struct ReorderRouteChannelsInput {
    channel_ids: Vec<String>,
}

pub(in crate::server) async fn reorder_route_channels(
    State(state): State<AppState>,
    axum::extract::Path(route_id): axum::extract::Path<String>,
    Json(input): Json<ReorderRouteChannelsInput>,
) -> Result<impl IntoResponse, ApiError> {
    let mut seen = std::collections::HashSet::<String>::new();
    for id in &input.channel_ids {
        if !seen.insert(id.clone()) {
            return Err(ApiError::BadRequest("channel_ids 存在重复项".to_string()));
        }
    }

    let res = storage::set_route_channels(state.db_path(), route_id, input.channel_ids).await;
    map_storage_unit_no_content(res, |msg| {
        if msg.starts_with("route not found") {
            Some(ApiError::NotFound("route not found".to_string()))
        } else if msg.starts_with("channel not found") {
            Some(ApiError::NotFound("channel not found".to_string()))
        } else if msg.starts_with("channel protocol mismatch") {
            Some(ApiError::BadRequest(msg.to_string()))
        } else {
            None
        }
    })
}
