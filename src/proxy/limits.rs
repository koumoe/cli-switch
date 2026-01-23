// Centralized limits for proxy request/response capture.
//
// Keep these in one place so we can tune behavior consistently across
// proxy.rs and proxy/stream.rs without hunting for scattered constants.

pub(super) const MAX_INBOUND_BODY_BYTES: usize = 64 * 1024 * 1024;
pub(super) const MAX_JSON_CAPTURE_BYTES: usize = 8 * 1024 * 1024;

// Used for capturing error bodies and error_detail in usage_events.
pub(super) const MAX_ERROR_DETAIL_BYTES: usize = 256 * 1024;

// Used for parsing/logging SSE streams.
pub(super) const MAX_SSE_BUF_BYTES: usize = 256 * 1024;
pub(super) const MAX_SSE_LOG_BUF_BYTES: usize = 1024 * 1024;
