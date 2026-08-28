use axum::http::{HeaderMap, HeaderValue, StatusCode};
use bytes::{Bytes, BytesMut};
use futures_util::StreamExt as _;
use futures_util::stream::BoxStream;

use super::{PreparedUpstreamResponse, Protocol, TokenUsage, extract_openai_usage, limits};

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

#[derive(Default)]
struct BootstrapState {
    line_buf: Vec<u8>,
    pending_event: Option<String>,
    skip_oversized_line: bool,
    semantic_output_seen: bool,
    usage: TokenUsage,
    terminal: Option<BootstrapTerminal>,
    pending_error: Option<PendingError>,
}

struct PendingError {
    error_kind: String,
    error_detail: String,
    retryable: bool,
}

impl BootstrapState {
    fn consume(&mut self, bytes: &[u8]) {
        for byte in bytes {
            if self.skip_oversized_line {
                if *byte == b'\n' {
                    self.skip_oversized_line = false;
                    self.pending_event = None;
                }
                continue;
            }
            if self.line_buf.len() >= limits::MAX_SSE_BUF_BYTES {
                self.line_buf.clear();
                self.skip_oversized_line = true;
                continue;
            }
            self.line_buf.push(*byte);
            if *byte == b'\n' {
                let line = std::mem::take(&mut self.line_buf);
                self.consume_line(&line);
                if self.semantic_output_seen || self.terminal.is_some() {
                    return;
                }
            }
        }
    }

    fn consume_line(&mut self, line: &[u8]) {
        let Ok(mut line) = std::str::from_utf8(line) else {
            return;
        };
        line = line.trim();
        if line.is_empty() {
            self.pending_event = None;
            return;
        }
        if let Some(event) = line.strip_prefix("event:") {
            let event = event.trim();
            if !event.is_empty() {
                self.pending_event = Some(super::truncate(event, 120));
            }
            return;
        }
        let Some(data) = line.strip_prefix("data:").map(str::trim) else {
            return;
        };
        if data.is_empty() {
            return;
        }
        if data == "[DONE]" {
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
        let Ok(value) = serde_json::from_str::<serde_json::Value>(data) else {
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
                    return OpenAiResponsesBootstrap::Ready(prepared_response_with_prefix(
                        status,
                        headers,
                        content_length,
                        prefix.freeze(),
                        stream,
                    ));
                }
                if let Some(terminal) = state.terminal.take() {
                    return bootstrap_terminal_result(
                        status,
                        headers,
                        content_length,
                        prefix.freeze(),
                        stream,
                        state.usage,
                        terminal,
                    );
                }
                if prefix.len() >= limits::MAX_OPENAI_RESPONSES_BOOTSTRAP_BYTES {
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
                    return OpenAiResponsesBootstrap::Ready(prepared_response_with_prefix(
                        status,
                        headers,
                        content_length,
                        prefix.freeze(),
                        stream,
                    ));
                }
            }
            Some(Err(error)) => {
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
                if !state.skip_oversized_line && !state.line_buf.is_empty() {
                    let line = std::mem::take(&mut state.line_buf);
                    state.consume_line(&line);
                }
                if let Some(terminal) = state.terminal.take() {
                    return bootstrap_terminal_result(
                        status,
                        headers,
                        content_length,
                        prefix.freeze(),
                        futures_util::stream::empty().boxed(),
                        state.usage,
                        terminal,
                    );
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
    status: StatusCode,
    headers: HeaderMap,
    content_length: Option<u64>,
    prefix: Bytes,
    remainder: BoxStream<'static, Result<Bytes, reqwest::Error>>,
    usage: TokenUsage,
    terminal: BootstrapTerminal,
) -> OpenAiResponsesBootstrap {
    let response =
        prepared_response_with_prefix(status, headers, content_length, prefix, remainder);
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
        stream,
    }
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
