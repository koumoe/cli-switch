use bytes::Bytes;
use std::error::Error as _;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Instant;

use crate::storage::Protocol;

#[derive(Clone)]
pub(super) struct StreamRecordContext {
    pub(super) db_path: std::path::PathBuf,
    pub(super) protocol: Protocol,
    pub(super) channel_id: String,
    pub(super) model: Option<String>,
    pub(super) request_id: Arc<str>,
    pub(super) http_status: i64,
    pub(super) status_is_success: bool,
    pub(super) started: Instant,
    pub(super) parse_sse: bool,
    pub(super) expected_sse: bool,
    pub(super) require_openai_responses_terminal: bool,
    pub(super) upstream_content_type: Option<String>,
    pub(super) content_type_corrected: bool,
    pub(super) record_usage: bool,
    /// Captured from the request handler so stream logs emitted from `Drop` can still be
    /// correlated with the request span (method/uri/endpoint/etc).
    pub(super) span: tracing::Span,
}

pub(super) struct InstrumentedStream {
    inner: futures_util::stream::BoxStream<'static, Result<Bytes, reqwest::Error>>,
    ctx: StreamRecordContext,
    finalized: bool,
    ttft_ms: Option<i64>,
    usage: super::TokenUsage,
    stream_bytes: usize,
    stream_chunks: u64,
    end_reason: Option<&'static str>,
    sse_buf: Vec<u8>,
    sse_log_buf: Vec<u8>,
    sse_log_truncated: bool,
    err_body_buf: Vec<u8>,
    stream_error: Option<String>,
    stream_error_detail: Option<String>,
    ignored_stream_error: Option<String>,
    sse_last_event: Option<String>,
    sse_last_type: Option<String>,
    sse_pending_event: Option<String>,
    sse_seen_terminal: bool,
    sse_seen_success_terminal: bool,
    sse_semantic_output_seen: bool,
    sse_terminal_error_kind: Option<String>,
    sse_terminal_error_detail: Option<String>,
    sse_skip_oversized_line: bool,
}

impl InstrumentedStream {
    pub(super) fn new(
        inner: futures_util::stream::BoxStream<'static, Result<Bytes, reqwest::Error>>,
        ctx: StreamRecordContext,
    ) -> Self {
        Self {
            inner,
            ctx,
            finalized: false,
            ttft_ms: None,
            usage: super::TokenUsage::default(),
            stream_bytes: 0,
            stream_chunks: 0,
            end_reason: None,
            sse_buf: Vec::new(),
            sse_log_buf: Vec::new(),
            sse_log_truncated: false,
            err_body_buf: Vec::new(),
            stream_error: None,
            stream_error_detail: None,
            ignored_stream_error: None,
            sse_last_event: None,
            sse_last_type: None,
            sse_pending_event: None,
            sse_seen_terminal: false,
            sse_seen_success_terminal: false,
            sse_semantic_output_seen: false,
            sse_terminal_error_kind: None,
            sse_terminal_error_detail: None,
            sse_skip_oversized_line: false,
        }
    }

    fn redact_key_query(mut s: String) -> String {
        // Best-effort redact `key=` query param (Gemini) if it ever appears in upstream errors.
        // Avoids leaking credentials into logs / usage events.
        let mut start = 0usize;
        while let Some(pos) = s[start..].find("key=") {
            let key_pos = start + pos;
            let val_start = key_pos + "key=".len();
            let mut val_end = val_start;
            while val_end < s.len() {
                let b = s.as_bytes()[val_end];
                if b == b'&' || b.is_ascii_whitespace() {
                    break;
                }
                val_end += 1;
            }
            if val_end > val_start {
                s.replace_range(val_start..val_end, "***");
                start = val_start + "***".len();
            } else {
                start = val_end;
            }
        }
        s
    }

    fn format_reqwest_error_detail(e: &reqwest::Error) -> String {
        let mut flags = Vec::<&'static str>::new();
        if e.is_timeout() {
            flags.push("timeout");
        }
        if e.is_connect() {
            flags.push("connect");
        }
        if e.is_decode() {
            flags.push("decode");
        }
        if e.is_body() {
            flags.push("body");
        }

        let mut out = String::new();
        if !flags.is_empty() {
            out.push_str("kind=");
            out.push_str(&flags.join("|"));
            out.push_str("; ");
        }
        out.push_str(&e.to_string());

        let mut chain = Vec::<String>::new();
        let mut cur = e.source();
        while let Some(err) = cur {
            chain.push(err.to_string());
            cur = err.source();
        }
        if !chain.is_empty() {
            out.push_str("; source=");
            out.push_str(&chain.join(" -> "));
        }

        Self::redact_key_query(out)
    }

    fn on_chunk(&mut self, bytes: &Bytes) {
        self.stream_chunks = self.stream_chunks.saturating_add(1);
        self.stream_bytes = self.stream_bytes.saturating_add(bytes.len());
        if self.ttft_ms.is_none() {
            self.ttft_ms = Some(self.ctx.started.elapsed().as_millis() as i64);
        }
        if !self.ctx.status_is_success
            && self.err_body_buf.len() < super::limits::MAX_ERROR_DETAIL_BYTES
        {
            let remain = super::limits::MAX_ERROR_DETAIL_BYTES - self.err_body_buf.len();
            self.err_body_buf
                .extend_from_slice(&bytes[..bytes.len().min(remain)]);
        }
        if self.ctx.parse_sse {
            if !self.sse_log_truncated
                && self.sse_log_buf.len() < super::limits::MAX_SSE_LOG_BUF_BYTES
            {
                let remain = super::limits::MAX_SSE_LOG_BUF_BYTES - self.sse_log_buf.len();
                self.sse_log_buf
                    .extend_from_slice(&bytes[..bytes.len().min(remain)]);
                if bytes.len() > remain {
                    self.sse_log_truncated = true;
                }
            } else if self.sse_log_buf.len() >= super::limits::MAX_SSE_LOG_BUF_BYTES {
                self.sse_log_truncated = true;
            }
            self.consume_sse(bytes);
        }
    }

    fn consume_sse(&mut self, bytes: &Bytes) {
        for byte in bytes {
            if self.sse_skip_oversized_line {
                if *byte == b'\n' {
                    self.sse_skip_oversized_line = false;
                    self.sse_pending_event = None;
                }
                continue;
            }

            if self.sse_buf.len() >= super::limits::MAX_SSE_BUF_BYTES {
                self.sse_buf.clear();
                self.sse_skip_oversized_line = true;
                continue;
            }

            self.sse_buf.push(*byte);
            if *byte == b'\n' {
                let line = std::mem::take(&mut self.sse_buf);
                self.consume_sse_line(&line);
            }
        }
    }

    fn consume_sse_line(&mut self, line: &[u8]) {
        let Ok(mut s) = std::str::from_utf8(line) else {
            return;
        };
        s = s.trim();
        if s.is_empty() {
            self.sse_pending_event = None;
            return;
        }
        if let Some(rest) = s.strip_prefix("event:") {
            let event = rest.trim();
            if !event.is_empty() {
                let event = super::truncate(event, 120);
                self.sse_last_event = Some(event.clone());
                self.sse_pending_event = Some(event);
            }
            return;
        }
        if !s.starts_with("data:") {
            return;
        }
        let data = s["data:".len()..].trim();
        if data.is_empty() {
            return;
        }
        if data == "[DONE]" {
            self.observe_terminal("[DONE]", None);
            return;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(data) else {
            return;
        };

        let marker = value
            .get("type")
            .and_then(|value| value.as_str())
            .map(str::to_string)
            .or_else(|| self.sse_pending_event.clone());
        if let Some(marker) = marker.as_deref() {
            self.sse_last_type = Some(super::truncate(marker, 120));
        }

        self.usage
            .merge(super::extract_usage_from_value(self.ctx.protocol, &value));

        if let Some(marker) = marker.as_deref() {
            if self.ctx.protocol == Protocol::Openai
                && super::openai_responses::event_has_semantic_output(marker, &value)
            {
                self.sse_semantic_output_seen = true;
            }
            self.observe_terminal(marker, Some(&value));
        }
    }

    fn observe_terminal(&mut self, marker: &str, value: Option<&serde_json::Value>) {
        match self.ctx.protocol {
            Protocol::Openai => match marker {
                "response.completed" | "response.done" | "response.incomplete" => {
                    self.sse_seen_terminal = true;
                    if let Some(value) = value
                        && let Some((kind, detail)) =
                            super::openai_responses::completed_failure(value)
                    {
                        self.set_terminal_failure(kind, detail);
                        return;
                    }

                    if self.ctx.require_openai_responses_terminal
                        && !self.sse_semantic_output_seen
                        && !self.usage.has_any_fields()
                        && value.is_none_or(|value| {
                            !super::openai_responses::completed_has_output(value)
                        })
                    {
                        self.set_terminal_failure(
                            "openai_silent_refusal".to_string(),
                            "OpenAI Responses stream completed without output or usage".to_string(),
                        );
                        return;
                    }
                    self.sse_seen_success_terminal = true;
                }
                "[DONE]" => {
                    self.sse_seen_terminal = true;
                    if self.ctx.require_openai_responses_terminal
                        && !self.sse_semantic_output_seen
                        && !self.usage.has_any_fields()
                    {
                        self.set_terminal_failure(
                            "openai_silent_refusal".to_string(),
                            "OpenAI Responses stream ended without output or usage".to_string(),
                        );
                    } else {
                        self.sse_seen_success_terminal = true;
                    }
                }
                "response.failed" | "response.cancelled" | "response.canceled"
                | "response.error" | "error" => {
                    self.sse_seen_terminal = true;
                    let detail = value
                        .and_then(super::openai_responses::terminal_error_detail)
                        .unwrap_or_else(|| format!("OpenAI Responses stream ended with {marker}"));
                    let kind = value
                        .and_then(super::openai_responses::internal_synthetic_error_kind)
                        .unwrap_or_else(|| format!("upstream_sse:{marker}"));
                    self.set_terminal_failure(kind, detail);
                }
                _ => {}
            },
            Protocol::Anthropic => {
                if marker == "message_stop" {
                    self.sse_seen_terminal = true;
                    self.sse_seen_success_terminal = true;
                }
            }
            Protocol::Gemini => {}
        }
    }

    fn set_terminal_failure(&mut self, kind: String, detail: String) {
        self.sse_seen_success_terminal = false;
        self.sse_terminal_error_kind = Some(kind);
        self.sse_terminal_error_detail = Some(super::truncate(&detail, 2000));
    }

    fn finish_sse_parser(&mut self) {
        if !self.ctx.parse_sse || self.sse_skip_oversized_line || self.sse_buf.is_empty() {
            return;
        }
        let line = std::mem::take(&mut self.sse_buf);
        self.consume_sse_line(&line);
    }

    fn finalize(&mut self) {
        if self.finalized {
            return;
        }
        self.finalized = true;
        self.finish_sse_parser();
        if !self.ctx.record_usage {
            return;
        }

        let _guard = self.ctx.span.enter();

        let duration_ms = self.ctx.started.elapsed().as_millis() as i64;
        let (prompt_tokens, completion_tokens, total_tokens, cache_read_tokens, cache_write_tokens) =
            self.usage.as_event_fields();

        let missing_required_terminal = self.ctx.require_openai_responses_terminal
            && self.ctx.status_is_success
            && self.stream_error.is_none()
            && !self.sse_seen_terminal;
        let success = self.ctx.status_is_success
            && self.stream_error.is_none()
            && self.sse_terminal_error_kind.is_none()
            && !missing_required_terminal;
        let error_kind = if success {
            None
        } else if !self.ctx.status_is_success {
            Some(format!("upstream_http:{}", self.ctx.http_status))
        } else if let Some(kind) = self.sse_terminal_error_kind.as_deref() {
            Some(kind.to_string())
        } else if let Some(err) = self.stream_error.as_deref() {
            Some(format!("stream_error:{}", super::truncate(err, 240)))
        } else if missing_required_terminal {
            Some("openai_responses_incomplete_stream".to_string())
        } else {
            Some("upstream_error".to_string())
        };
        let error_detail = if success {
            None
        } else if let Some(detail) = self.sse_terminal_error_detail.as_deref() {
            Some(detail.to_string())
        } else if let Some(detail) = self.stream_error_detail.as_deref() {
            Some(super::truncate(detail, 2000))
        } else if let Some(err) = self.stream_error.as_deref() {
            Some(super::truncate(err, 2000))
        } else if missing_required_terminal {
            Some("OpenAI Responses stream ended before a terminal event".to_string())
        } else if !self.ctx.status_is_success && !self.err_body_buf.is_empty() {
            let msg = super::parse_error_message(self.ctx.protocol, &self.err_body_buf)
                .unwrap_or_else(|| String::from_utf8_lossy(&self.err_body_buf).to_string());
            Some(super::truncate(&msg, 2000))
        } else {
            None
        };

        let response_sse = super::to_single_line(&String::from_utf8_lossy(&self.sse_log_buf));
        let response_sse_preview = super::truncate(&response_sse, 4096);

        tracing::debug!(
            protocol = self.ctx.protocol.as_str(),
            request_id = %self.ctx.request_id,
            channel_id = %self.ctx.channel_id,
            model = self.ctx.model.as_deref().unwrap_or("-"),
            http_status = self.ctx.http_status,
            ttft_ms = self.ttft_ms.unwrap_or(-1),
            duration_ms,
            stream_bytes = self.stream_bytes,
            stream_chunks = self.stream_chunks,
            stream_end = self.end_reason.unwrap_or("-"),
            upstream_content_type = self.ctx.upstream_content_type.as_deref().unwrap_or("-"),
            sse_expected = self.ctx.expected_sse,
            content_type_corrected = self.ctx.content_type_corrected,
            sse_terminal = self.sse_seen_terminal,
            sse_success_terminal = self.sse_seen_success_terminal,
            sse_semantic_output = self.sse_semantic_output_seen,
            sse_last_event = self.sse_last_event.as_deref().unwrap_or("-"),
            sse_last_type = self.sse_last_type.as_deref().unwrap_or("-"),
            prompt_tokens = prompt_tokens.unwrap_or(-1),
            completion_tokens = completion_tokens.unwrap_or(-1),
            total_tokens = total_tokens.unwrap_or(-1),
            success,
            error_kind = error_kind.as_deref().unwrap_or("-"),
            ignored_error = self.ignored_stream_error.as_deref().unwrap_or("-"),
            response_preview = %response_sse_preview,
            "proxy request result"
        );

        if self.ctx.parse_sse {
            tracing::debug!(
                target: "proxy_body",
                protocol = self.ctx.protocol.as_str(),
                request_id = %self.ctx.request_id,
                channel_id = %self.ctx.channel_id,
                model = self.ctx.model.as_deref().unwrap_or("-"),
                http_status = self.ctx.http_status,
                ttft_ms = self.ttft_ms.unwrap_or(-1),
                duration_ms,
                stream_end = self.end_reason.unwrap_or("-"),
                upstream_content_type = self.ctx.upstream_content_type.as_deref().unwrap_or("-"),
                sse_expected = self.ctx.expected_sse,
                content_type_corrected = self.ctx.content_type_corrected,
                sse_terminal = self.sse_seen_terminal,
                sse_success_terminal = self.sse_seen_success_terminal,
                sse_semantic_output = self.sse_semantic_output_seen,
                sse_last_event = self.sse_last_event.as_deref().unwrap_or("-"),
                sse_last_type = self.sse_last_type.as_deref().unwrap_or("-"),
                ignored_error = self.ignored_stream_error.as_deref().unwrap_or("-"),
                response_sse = %response_sse,
                response_sse_truncated = self.sse_log_truncated,
                body = true,
                "proxy request result"
            );
        }

        let event = super::build_usage_event(super::UsageEventParams {
            request_id: Some(self.ctx.request_id.clone()),
            protocol: self.ctx.protocol,
            channel_id: self.ctx.channel_id.clone(),
            model: self.ctx.model.clone(),
            success,
            http_status: Some(self.ctx.http_status),
            error_kind,
            error_detail,
            latency_ms: duration_ms,
            ttft_ms: self.ttft_ms,
            tokens: (
                prompt_tokens,
                completion_tokens,
                total_tokens,
                cache_read_tokens,
                cache_write_tokens,
            ),
        });
        super::spawn_usage_event(event, self.ctx.db_path.clone());
    }
}

impl futures_util::Stream for InstrumentedStream {
    type Item = Result<Bytes, std::io::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let polled = self.inner.as_mut().poll_next(cx);
        match polled {
            Poll::Ready(Some(Ok(bytes))) => {
                self.on_chunk(&bytes);
                Poll::Ready(Some(Ok(bytes)))
            }
            Poll::Ready(Some(Err(e))) => {
                // Some upstreams may close the connection abruptly right after sending the
                // terminal marker. If we already observed a successful terminal marker, treat
                // this as a clean end-of-stream to reduce noisy failures.
                if self.ctx.status_is_success && self.sse_seen_success_terminal {
                    self.end_reason = Some("upstream_error_after_terminal");
                    self.ignored_stream_error = Some(super::truncate(
                        &Self::format_reqwest_error_detail(&e),
                        2000,
                    ));
                    self.finalize();
                    Poll::Ready(None)
                } else {
                    self.end_reason = Some("upstream_error");
                    self.stream_error = Some(e.to_string());
                    self.stream_error_detail = Some(super::truncate(
                        &Self::format_reqwest_error_detail(&e),
                        2000,
                    ));
                    self.finalize();
                    Poll::Ready(Some(Err(std::io::Error::other(e))))
                }
            }
            Poll::Ready(None) => {
                self.end_reason = Some("upstream_eos");
                self.finalize();
                Poll::Ready(None)
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

impl Drop for InstrumentedStream {
    fn drop(&mut self) {
        if self.finalized || !self.ctx.record_usage {
            return;
        }
        if self.end_reason.is_none() {
            self.end_reason = Some(if self.sse_seen_success_terminal {
                "dropped_after_terminal"
            } else {
                "dropped"
            });
        }
        // If the stream is dropped before completion (e.g. client disconnect), record it as a
        // stream error so it doesn't get counted as a successful request.
        if self.stream_error.is_none() && !self.sse_seen_terminal {
            // If the client stops reading immediately after receiving a terminal marker
            // (e.g. OpenAI `response.completed` or Anthropic `message_stop`), treat it as success.
            if !(self.ctx.status_is_success && self.sse_seen_success_terminal) {
                self.stream_error = Some("stream_dropped".to_string());
            }
        }
        self.finalize();
    }
}
