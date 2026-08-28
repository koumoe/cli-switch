// Centralized limits for proxy request/response capture.
//
// Keep these in one place so we can tune behavior consistently across
// proxy.rs and proxy/stream.rs without hunting for scattered constants.

pub(super) const MAX_INBOUND_BODY_BYTES: usize = 64 * 1024 * 1024;
pub(super) const MAX_JSON_CAPTURE_BYTES: usize = 8 * 1024 * 1024;

// Used for capturing error bodies and error_detail in usage_events.
pub(super) const MAX_ERROR_DETAIL_BYTES: usize = 256 * 1024;

// Used for parsing/logging SSE streams. Codex response.created/response.completed events may
// repeat large tool manifests in a single physical line, so keep this bounded but aligned with
// the OpenAI Responses bootstrap cap.
pub(super) const MAX_SSE_BUF_BYTES: usize = 2 * 1024 * 1024;
pub(super) const MAX_SSE_LOG_BUF_BYTES: usize = 1024 * 1024;

// Buffer OpenAI Responses handshake/metadata frames until the first semantic output or terminal
// event. This allows retrying a silent refusal or an immediate response.failed before downstream
// headers are committed. Large Codex tool manifests can make response.created exceed 100 KiB.
pub(super) const MAX_OPENAI_RESPONSES_BOOTSTRAP_BYTES: usize = MAX_SSE_BUF_BYTES;
