#[derive(thiserror::Error, Debug)]
pub enum StorageError {
    #[error("route not found: {route_id}")]
    RouteNotFound { route_id: String },

    #[error("channel not found: {channel_id}")]
    ChannelNotFound { channel_id: String },

    #[error("newapi account not found: {account_id}")]
    NewApiAccountNotFound { account_id: String },

    #[error("newapi account already exists: base_url={base_url} user_id={user_id}")]
    NewApiAccountAlreadyExists { base_url: String, user_id: String },

    #[error("remote account not found: {account_id}")]
    RemoteAccountNotFound { account_id: String },

    #[error("remote account already exists: provider={provider} base_url={base_url}")]
    RemoteAccountAlreadyExists { provider: String, base_url: String },

    #[error("channel protocol mismatch: route={route_protocol} channel={channel_protocol}")]
    ChannelProtocolMismatch {
        route_protocol: String,
        channel_protocol: String,
    },

    #[error("channel reorder mismatch: {reason}")]
    ChannelReorderMismatch { reason: &'static str },

    #[error("newapi account reorder mismatch: {reason}")]
    NewApiAccountReorderMismatch { reason: &'static str },

    #[error("remote account reorder mismatch: {reason}")]
    RemoteAccountReorderMismatch { reason: &'static str },

    #[error("prompt project not found: {project_id}")]
    PromptProjectNotFound { project_id: String },

    #[error("prompt document not found")]
    PromptDocumentNotFound,

    #[error("prompt document too large: actual={actual_bytes} max={max_bytes}")]
    PromptDocumentTooLarge {
        actual_bytes: usize,
        max_bytes: usize,
    },

    #[error("chat pairing token invalid")]
    ChatPairingTokenInvalid,

    #[error("chat pairing token expired")]
    ChatPairingTokenExpired,

    #[error("chat pairing token already used")]
    ChatPairingTokenUsed,

    #[error(
        "chat pairing token platform mismatch: expected={expected_platform} actual={actual_platform}"
    )]
    ChatPairingTokenPlatformMismatch {
        expected_platform: String,
        actual_platform: String,
    },

    #[error("chat binding already exists: platform={platform} sender={platform_user_id}")]
    ChatBindingAlreadyExists {
        platform: String,
        platform_user_id: String,
    },

    #[error("chat binding not found: {binding_id}")]
    ChatBindingNotFound { binding_id: i64 },

    #[error("chat session not found: {session_id}")]
    ChatSessionNotFound { session_id: i64 },

    #[error("chat session alias already exists: platform={platform} alias={alias}")]
    ChatSessionAliasExists { platform: String, alias: String },

    #[error("chat project path not found: {path}")]
    ChatProjectPathNotFound { path: String },

    #[error(
        "prompt document version conflict: expected={expected_updated_at_ms:?} current={current_updated_at_ms:?}"
    )]
    PromptDocumentVersionConflict {
        expected_updated_at_ms: Option<i64>,
        current_updated_at_ms: Option<i64>,
    },
}
