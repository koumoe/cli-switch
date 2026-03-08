#[derive(thiserror::Error, Debug)]
pub enum StorageError {
    #[error("route not found: {route_id}")]
    RouteNotFound { route_id: String },

    #[error("channel not found: {channel_id}")]
    ChannelNotFound { channel_id: String },

    #[error("channel protocol mismatch: route={route_protocol} channel={channel_protocol}")]
    ChannelProtocolMismatch {
        route_protocol: String,
        channel_protocol: String,
    },

    #[error("channel reorder mismatch: {reason}")]
    ChannelReorderMismatch { reason: &'static str },

    #[error("prompt project not found: {project_id}")]
    PromptProjectNotFound { project_id: String },

    #[error("prompt project path already exists: {path}")]
    PromptProjectPathExists { path: String },

    #[error("prompt project name already exists: {name}")]
    PromptProjectNameExists { name: String },

    #[error("prompt document not found")]
    PromptDocumentNotFound,

    #[error("prompt document too large: actual={actual_bytes} max={max_bytes}")]
    PromptDocumentTooLarge {
        actual_bytes: usize,
        max_bytes: usize,
    },

    #[error(
        "prompt document version conflict: expected={expected_updated_at_ms:?} current={current_updated_at_ms:?}"
    )]
    PromptDocumentVersionConflict {
        expected_updated_at_ms: Option<i64>,
        current_updated_at_ms: Option<i64>,
    },
}
