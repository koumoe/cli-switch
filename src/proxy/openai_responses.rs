use axum::http::{HeaderMap, HeaderValue, StatusCode};
use bytes::{Bytes, BytesMut};
use futures_util::StreamExt as _;
use futures_util::stream::BoxStream;

use super::{
    PreparedUpstreamResponse, Protocol, TokenUsage, extract_openai_usage, limits,
    sse::{SseLine, SseLineParser, has_sse_field_prefix},
};

pub(super) enum OpenAiResponsesBootstrap {
    Ready(PreparedUpstreamResponse),
    Failure(OpenAiResponsesBootstrapFailure),
}

pub(super) struct OpenAiResponsesBootstrapFailure {
    pub(super) response: PreparedUpstreamResponse,
    pub(super) error_kind: String,
    pub(super) error_detail: String,
    pub(super) usage: TokenUsage,
    pub(super) retryable: bool,
    pub(super) synthetic_on_exhaustion: bool,
}

enum BootstrapTerminal {
    Success,
    Failure {
        error_kind: String,
        error_detail: String,
        retryable: bool,
        synthetic_on_exhaustion: bool,
    },
}

struct BootstrapState {
    parser: SseLineParser,
    pending_event: Option<String>,
    saw_sse_structure: bool,
    semantic_output_seen: bool,
    usage: TokenUsage,
    terminal: Option<BootstrapTerminal>,
    pending_error: Option<PendingError>,
}

impl Default for BootstrapState {
    fn default() -> Self {
        Self {
            parser: SseLineParser::new(limits::MAX_SSE_BUF_BYTES),
            pending_event: None,
            saw_sse_structure: false,
            semantic_output_seen: false,
            usage: TokenUsage::default(),
            terminal: None,
            pending_error: None,
        }
    }
}

struct PendingError {
    error_kind: String,
    error_detail: String,
    retryable: bool,
}

impl BootstrapState {
    fn consume(&mut self, bytes: &[u8]) {
        let mut lines = Vec::new();
        self.parser.feed(bytes, |line| lines.push(line));
        for line in lines {
            if matches!(line, SseLine::Event(_) | SseLine::Data(_)) {
                self.saw_sse_structure = true;
            }
            self.consume_line(line);
            if self.semantic_output_seen || self.terminal.is_some() {
                return;
            }
        }
    }

    fn consume_line(&mut self, line: SseLine) {
        match line {
            SseLine::Event(event) if !event.is_empty() => {
                self.pending_event = Some(super::truncate(&event, 120));
            }
            SseLine::Event(_) | SseLine::Blank => self.pending_event = None,
            SseLine::Data(data) => self.consume_data(&data),
            SseLine::Other => {}
        }
    }

    fn consume_data(&mut self, data: &[u8]) {
        if data.is_empty() {
            return;
        }
        if data == b"[DONE]" {
            self.terminal = Some(
                if self.semantic_output_seen || self.usage.has_any_fields() {
                    BootstrapTerminal::Success
                } else {
                    BootstrapTerminal::Failure {
                        error_kind: "openai_silent_refusal".to_string(),
                        error_detail: "OpenAI Responses stream ended without output or usage"
                            .to_string(),
                        retryable: true,
                        synthetic_on_exhaustion: true,
                    }
                },
            );
            return;
        }
        let Ok(value) = serde_json::from_slice::<serde_json::Value>(data) else {
            return;
        };
        let marker = value
            .get("type")
            .and_then(|value| value.as_str())
            .map(str::to_string)
            .or_else(|| self.pending_event.clone());

        self.usage.merge(extract_openai_usage(&value));
        let Some(marker) = marker else {
            return;
        };
        if event_has_semantic_output(&marker, &value) {
            self.semantic_output_seen = true;
            return;
        }

        match marker.as_str() {
            "response.completed" | "response.done" | "response.incomplete" => {
                if let Some((error_kind, error_detail)) = completed_failure(&value) {
                    self.terminal = Some(BootstrapTerminal::Failure {
                        error_kind,
                        error_detail,
                        retryable: terminal_failure_is_retryable(&value),
                        synthetic_on_exhaustion: false,
                    });
                } else if self.usage.has_any_fields() || completed_has_output(&value) {
                    self.terminal = Some(BootstrapTerminal::Success);
                } else {
                    self.terminal = Some(BootstrapTerminal::Failure {
                        error_kind: "openai_silent_refusal".to_string(),
                        error_detail: "OpenAI Responses stream completed without output or usage"
                            .to_string(),
                        retryable: true,
                        synthetic_on_exhaustion: true,
                    });
                }
            }
            "response.failed" | "response.cancelled" | "response.canceled" => {
                self.terminal = Some(BootstrapTerminal::Failure {
                    error_kind: format!("upstream_sse:{marker}"),
                    error_detail: terminal_error_detail(&value)
                        .unwrap_or_else(|| format!("OpenAI Responses stream ended with {marker}")),
                    retryable: terminal_failure_is_retryable(&value),
                    synthetic_on_exhaustion: false,
                });
            }
            "response.error" | "error" => {
                self.pending_error = Some(PendingError {
                    error_kind: format!("upstream_sse:{marker}"),
                    error_detail: terminal_error_detail(&value)
                        .unwrap_or_else(|| format!("OpenAI Responses stream ended with {marker}")),
                    retryable: terminal_failure_is_retryable(&value),
                });
            }
            _ => {}
        }
    }
}

pub(super) async fn inspect_openai_responses_bootstrap(
    response: PreparedUpstreamResponse,
) -> OpenAiResponsesBootstrap {
    let PreparedUpstreamResponse {
        status,
        headers,
        content_length,
        detected_sse: _,
        mut stream,
    } = response;
    let mut prefix = BytesMut::new();
    let mut state = BootstrapState::default();

    loop {
        match stream.next().await {
            Some(Ok(chunk)) => {
                prefix.extend_from_slice(&chunk);
                state.consume(&chunk);

                if state.semantic_output_seen {
                    return OpenAiResponsesBootstrap::Ready(set_detected_sse(
                        prepared_response_with_prefix(
                            status,
                            headers,
                            content_length,
                            prefix.freeze(),
                            stream,
                        ),
                        true,
                    ));
                }
                if let Some(terminal) = state.terminal.take() {
                    return bootstrap_terminal_result(
                        prepared_response_with_prefix(
                            status,
                            headers,
                            content_length,
                            prefix.freeze(),
                            stream,
                        ),
                        state.usage,
                        terminal,
                        state.saw_sse_structure,
                    );
                }
                if prefix.len() >= limits::MAX_OPENAI_RESPONSES_BOOTSTRAP_BYTES {
                    let saw_sse_structure =
                        state.saw_sse_structure || has_sse_field_prefix(&prefix);
                    if !saw_sse_structure {
                        return OpenAiResponsesBootstrap::Ready(set_detected_sse(
                            prepared_response_with_prefix(
                                status,
                                headers,
                                content_length,
                                prefix.freeze(),
                                stream,
                            ),
                            false,
                        ));
                    }
                    if let Some(pending_error) = state.pending_error.take() {
                        let response = prepared_response_with_prefix(
                            status,
                            headers,
                            content_length,
                            prefix.freeze(),
                            stream,
                        );
                        return OpenAiResponsesBootstrap::Failure(
                            OpenAiResponsesBootstrapFailure {
                                response,
                                error_kind: pending_error.error_kind,
                                error_detail: pending_error.error_detail,
                                usage: state.usage,
                                retryable: pending_error.retryable,
                                synthetic_on_exhaustion: true,
                            },
                        );
                    }
                    return OpenAiResponsesBootstrap::Ready(set_detected_sse(
                        prepared_response_with_prefix(
                            status,
                            headers,
                            content_length,
                            prefix.freeze(),
                            stream,
                        ),
                        saw_sse_structure,
                    ));
                }
            }
            Some(Err(error)) => {
                if !state.saw_sse_structure {
                    let response = prepared_response_with_prefix(
                        status,
                        headers,
                        content_length,
                        prefix.freeze(),
                        futures_util::stream::once(async move { Err(error) }).boxed(),
                    );
                    return OpenAiResponsesBootstrap::Ready(set_detected_sse(response, false));
                }
                let response = prepared_response_with_prefix(
                    status,
                    headers,
                    content_length,
                    prefix.freeze(),
                    futures_util::stream::empty().boxed(),
                );
                return OpenAiResponsesBootstrap::Failure(OpenAiResponsesBootstrapFailure {
                    response,
                    error_kind: format!(
                        "stream_error:{}",
                        super::truncate(&error.to_string(), 240)
                    ),
                    error_detail: super::truncate(&error.to_string(), 2000),
                    usage: state.usage,
                    retryable: true,
                    synthetic_on_exhaustion: true,
                });
            }
            None => {
                let mut lines = Vec::new();
                state.parser.finish(|line| lines.push(line));
                for line in lines {
                    if matches!(line, SseLine::Event(_) | SseLine::Data(_)) {
                        state.saw_sse_structure = true;
                    }
                    state.consume_line(line);
                }
                if let Some(terminal) = state.terminal.take() {
                    return bootstrap_terminal_result(
                        prepared_response_with_prefix(
                            status,
                            headers,
                            content_length,
                            prefix.freeze(),
                            futures_util::stream::empty().boxed(),
                        ),
                        state.usage,
                        terminal,
                        state.saw_sse_structure,
                    );
                }
                if !state.saw_sse_structure && !prefix.is_empty() {
                    if serde_json::from_slice::<serde_json::Value>(&prefix).is_ok() {
                        return OpenAiResponsesBootstrap::Ready(set_detected_sse(
                            prepared_response_with_prefix(
                                status,
                                headers,
                                content_length,
                                prefix.freeze(),
                                futures_util::stream::empty().boxed(),
                            ),
                            false,
                        ));
                    }
                    let response = prepared_response_with_prefix(
                        status,
                        headers,
                        content_length,
                        prefix.freeze(),
                        futures_util::stream::empty().boxed(),
                    );
                    return OpenAiResponsesBootstrap::Failure(OpenAiResponsesBootstrapFailure {
                        response,
                        error_kind: "openai_responses_incomplete_stream".to_string(),
                        error_detail:
                            "OpenAI Responses upstream returned neither SSE nor valid JSON"
                                .to_string(),
                        usage: state.usage,
                        retryable: true,
                        synthetic_on_exhaustion: true,
                    });
                }
                if let Some(pending_error) = state.pending_error.take() {
                    let response = prepared_response_with_prefix(
                        status,
                        headers,
                        content_length,
                        prefix.freeze(),
                        futures_util::stream::empty().boxed(),
                    );
                    return OpenAiResponsesBootstrap::Failure(OpenAiResponsesBootstrapFailure {
                        response,
                        error_kind: pending_error.error_kind,
                        error_detail: pending_error.error_detail,
                        usage: state.usage,
                        retryable: pending_error.retryable,
                        synthetic_on_exhaustion: true,
                    });
                }
                let response = prepared_response_with_prefix(
                    status,
                    headers,
                    content_length,
                    prefix.freeze(),
                    futures_util::stream::empty().boxed(),
                );
                return OpenAiResponsesBootstrap::Failure(OpenAiResponsesBootstrapFailure {
                    response,
                    error_kind: "openai_responses_incomplete_stream".to_string(),
                    error_detail: "OpenAI Responses stream ended before a terminal event"
                        .to_string(),
                    usage: state.usage,
                    retryable: true,
                    synthetic_on_exhaustion: true,
                });
            }
        }
    }
}

fn bootstrap_terminal_result(
    response: PreparedUpstreamResponse,
    usage: TokenUsage,
    terminal: BootstrapTerminal,
    saw_sse_structure: bool,
) -> OpenAiResponsesBootstrap {
    let response = set_detected_sse(response, saw_sse_structure);
    match terminal {
        BootstrapTerminal::Success => OpenAiResponsesBootstrap::Ready(response),
        BootstrapTerminal::Failure {
            error_kind,
            error_detail,
            retryable,
            synthetic_on_exhaustion,
        } => OpenAiResponsesBootstrap::Failure(OpenAiResponsesBootstrapFailure {
            response,
            error_kind,
            error_detail,
            usage,
            retryable,
            synthetic_on_exhaustion,
        }),
    }
}

fn prepared_response_with_prefix(
    status: StatusCode,
    headers: HeaderMap,
    content_length: Option<u64>,
    prefix: Bytes,
    remainder: BoxStream<'static, Result<Bytes, reqwest::Error>>,
) -> PreparedUpstreamResponse {
    let stream = futures_util::stream::once(async move { Ok::<Bytes, reqwest::Error>(prefix) })
        .chain(remainder)
        .boxed();
    PreparedUpstreamResponse {
        status,
        headers,
        content_length,
        detected_sse: None,
        stream,
    }
}

fn set_detected_sse(
    mut response: PreparedUpstreamResponse,
    detected_sse: bool,
) -> PreparedUpstreamResponse {
    response.detected_sse = Some(detected_sse);
    response
}

pub(super) fn synthetic_openai_responses_failure(
    response: PreparedUpstreamResponse,
    error_kind: &str,
    error_detail: &str,
) -> PreparedUpstreamResponse {
    let PreparedUpstreamResponse {
        status,
        mut headers,
        content_length: _,
        detected_sse: _,
        stream: _,
    } = response;
    let payload = serde_json::json!({
        "type": "response.failed",
        "response": {
            "status": "failed",
            "error": {
                "code": error_kind,
                "message": error_detail,
            }
        }
    });
    let body = Bytes::from(format!("event: response.failed\ndata: {}\n\n", payload));
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        HeaderValue::from_static("text/event-stream"),
    );
    headers.insert(
        axum::http::header::CACHE_CONTROL,
        HeaderValue::from_static("no-cache"),
    );
    let content_length = Some(body.len() as u64);
    let stream =
        futures_util::stream::once(async move { Ok::<Bytes, reqwest::Error>(body) }).boxed();
    PreparedUpstreamResponse {
        status,
        headers,
        content_length,
        detected_sse: Some(true),
        stream,
    }
}

pub(super) fn event_has_semantic_output(marker: &str, value: &serde_json::Value) -> bool {
    if completed_has_output(value) {
        return true;
    }
    marker.starts_with("response.")
        && !matches!(
            marker,
            "response.created"
                | "response.queued"
                | "response.in_progress"
                | "response.completed"
                | "response.done"
                | "response.failed"
                | "response.incomplete"
                | "response.cancelled"
                | "response.canceled"
                | "response.error"
        )
}

pub(super) fn completed_has_output(value: &serde_json::Value) -> bool {
    value
        .get("response")
        .and_then(|response| response.get("output"))
        .or_else(|| value.get("output"))
        .and_then(|output| output.as_array())
        .is_some_and(|output| !output.is_empty())
}

pub(super) fn completed_failure(value: &serde_json::Value) -> Option<(String, String)> {
    let response = value.get("response").unwrap_or(value);
    let status = response
        .get("status")
        .and_then(|status| status.as_str())
        .unwrap_or_default();
    let has_error = response.get("error").is_some_and(|error| !error.is_null())
        || value.get("error").is_some_and(|error| !error.is_null());
    if !has_error && !matches!(status, "failed" | "cancelled" | "canceled") {
        return None;
    }
    let marker = if status.is_empty() {
        "response.completed_with_error"
    } else {
        status
    };
    Some((
        format!("upstream_sse:{marker}"),
        terminal_error_detail(value)
            .unwrap_or_else(|| format!("OpenAI Responses completed with status {marker}")),
    ))
}

pub(super) fn terminal_error_detail(value: &serde_json::Value) -> Option<String> {
    super::parse_error_message_from_value(Protocol::Openai, value)
        .or_else(|| {
            value
                .get("response")
                .and_then(|response| response.get("incomplete_details"))
                .and_then(|details| details.get("reason"))
                .and_then(|reason| reason.as_str())
                .map(str::to_string)
        })
        .or_else(|| {
            value
                .get("incomplete_details")
                .and_then(|details| details.get("reason"))
                .and_then(|reason| reason.as_str())
                .map(str::to_string)
        })
}

pub(super) fn internal_synthetic_error_kind(value: &serde_json::Value) -> Option<String> {
    let code = value
        .get("response")
        .and_then(|response| response.get("error"))
        .and_then(|error| error.get("code"))
        .or_else(|| value.get("error").and_then(|error| error.get("code")))
        .and_then(|code| code.as_str())?;
    (matches!(
        code,
        "openai_silent_refusal" | "openai_responses_incomplete_stream"
    ) || code.starts_with("stream_error:"))
    .then(|| code.to_string())
}

fn terminal_failure_is_retryable(value: &serde_json::Value) -> bool {
    let error = value
        .get("response")
        .and_then(|response| response.get("error"))
        .or_else(|| value.get("error"));
    let code = error
        .and_then(|error| error.get("code"))
        .and_then(json_scalar_string)
        .unwrap_or_default()
        .to_ascii_lowercase();
    let error_type = error
        .and_then(|error| error.get("type"))
        .and_then(json_scalar_string)
        .unwrap_or_default()
        .to_ascii_lowercase();
    let status = error
        .and_then(|error| error.get("status"))
        .and_then(|status| status.as_u64())
        .or_else(|| value.get("status").and_then(|status| status.as_u64()));
    let message = terminal_error_detail(value)
        .unwrap_or_default()
        .to_ascii_lowercase();

    matches!(status, Some(408 | 409 | 429 | 500..=599))
        || [
            "server_error",
            "internal_error",
            "upstream_error",
            "rate_limit_exceeded",
            "rate_limit_error",
            "service_unavailable",
            "temporarily_unavailable",
            "overloaded",
            "account_disabled",
            "account_deactivated",
            "unauthorized",
            "invalid_authentication",
            "insufficient_quota",
        ]
        .iter()
        .any(|candidate| code == *candidate || error_type == *candidate)
        || [
            "overloaded",
            "over capacity",
            "rate limit",
            "temporarily unavailable",
            "service unavailable",
            "account disabled",
            "account deactivated",
        ]
        .iter()
        .any(|candidate| message.contains(candidate))
}

fn json_scalar_string(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(value) => Some(value.clone()),
        serde_json::Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn semantic_output_excludes_handshake_and_terminal_events() {
        assert!(!event_has_semantic_output(
            "response.created",
            &json!({"type":"response.created"})
        ));
        assert!(!event_has_semantic_output(
            "response.completed",
            &json!({"type":"response.completed","response":{"output":[]}})
        ));
        assert!(event_has_semantic_output(
            "response.output_text.delta",
            &json!({"type":"response.output_text.delta","delta":"hello"})
        ));
        assert!(event_has_semantic_output(
            "response.completed",
            &json!({"type":"response.completed","response":{"output":[{"type":"message"}]}})
        ));
    }

    #[test]
    fn completed_failure_distinguishes_valid_incomplete_from_errors() {
        assert!(
            completed_failure(&json!({
                "type":"response.completed",
                "response":{"status":"failed","error":{"message":"failed upstream"}}
            }))
            .is_some()
        );
        assert!(completed_failure(&json!({
            "type":"response.incomplete",
            "response":{"status":"incomplete","incomplete_details":{"reason":"max_output_tokens"}}
        }))
        .is_none());
        assert!(
            completed_failure(&json!({
                "type":"response.completed",
                "response":{"status":"completed","error":null}
            }))
            .is_none()
        );
    }

    #[test]
    fn terminal_error_detail_reads_nested_response_error() {
        assert_eq!(
            terminal_error_detail(&json!({
                "type":"response.failed",
                "response":{"error":{"message":"nested failure"}}
            }))
            .as_deref(),
            Some("nested failure")
        );
    }

    #[test]
    fn retryability_matches_cross_channel_policy() {
        for code in [
            "unauthorized",
            "invalid_authentication",
            "account_disabled",
            "insufficient_quota",
            "rate_limit_exceeded",
            "server_error",
        ] {
            assert!(
                terminal_failure_is_retryable(&json!({
                    "type":"response.failed",
                    "response":{"error":{"code":code,"message":"retry elsewhere"}}
                })),
                "expected {code} to be retryable across channels"
            );
        }

        for code in ["context_length_exceeded", "invalid_request_error"] {
            assert!(
                !terminal_failure_is_retryable(&json!({
                    "type":"response.failed",
                    "response":{"error":{"code":code,"message":"fix the request"}}
                })),
                "expected {code} to remain request-scoped"
            );
        }
    }

    #[test]
    fn numeric_retryable_status_is_recognized() {
        assert!(terminal_failure_is_retryable(&json!({
            "type":"response.failed",
            "response":{"error":{"status":429,"message":"slow down"}}
        })));
        assert!(!terminal_failure_is_retryable(&json!({
            "type":"response.failed",
            "response":{"error":{"status":400,"message":"bad request"}}
        })));
    }
}
