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
}
