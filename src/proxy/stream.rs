use bytes::Bytes;
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
    sse_last_event: Option<String>,
    sse_last_type: Option<String>,
    sse_seen_terminal: bool,
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
            sse_last_event: None,
            sse_last_type: None,
            sse_seen_terminal: false,
        }
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

    fn maybe_mark_terminal(&mut self, marker: &str) {
        if marker.is_empty() {
            return;
        }
        // Best-effort terminal markers for common upstreams.
        // Note: this is only for diagnosis; it doesn't change control flow.
        match self.ctx.protocol {
            Protocol::Openai => {
                if matches!(
                    marker,
                    "response.completed"
                        | "response.failed"
                        | "response.cancelled"
                        | "response.incomplete"
                ) {
                    self.sse_seen_terminal = true;
                }
            }
            Protocol::Anthropic => {
                if marker == "message_stop" {
                    self.sse_seen_terminal = true;
                }
            }
            Protocol::Gemini => {}
        }
    }

    fn consume_sse(&mut self, bytes: &Bytes) {
        if self.sse_buf.len() < super::limits::MAX_SSE_BUF_BYTES {
            let remain = super::limits::MAX_SSE_BUF_BYTES - self.sse_buf.len();
            self.sse_buf
                .extend_from_slice(&bytes[..bytes.len().min(remain)]);
        }

        while let Some(nl) = self.sse_buf.iter().position(|b| *b == b'\n') {
            let line = self.sse_buf.drain(..=nl).collect::<Vec<u8>>();
            let Ok(mut s) = std::str::from_utf8(&line) else {
                continue;
            };
            s = s.trim();
            if let Some(rest) = s.strip_prefix("event:") {
                let ev = rest.trim();
                if !ev.is_empty() {
                    self.sse_last_event = Some(super::truncate(ev, 120));
                    self.maybe_mark_terminal(ev);
                }
                continue;
            }
            if !s.starts_with("data:") {
                continue;
            }
            let data = s["data:".len()..].trim();
            if data.is_empty() {
                continue;
            }
            if data == "[DONE]" {
                self.sse_seen_terminal = true;
                continue;
            }
            let Ok(v) = serde_json::from_str::<serde_json::Value>(data) else {
                continue;
            };

            if let Some(t) = v.get("type").and_then(|v| v.as_str()) {
                self.sse_last_type = Some(super::truncate(t, 120));
                self.maybe_mark_terminal(t);
            }
            self.usage
                .merge(super::extract_usage_from_value(self.ctx.protocol, &v));
        }
    }

    fn finalize(&mut self) {
        if self.finalized {
            return;
        }
        self.finalized = true;
        if !self.ctx.record_usage {
            return;
        }

        let _guard = self.ctx.span.enter();

        let duration_ms = self.ctx.started.elapsed().as_millis() as i64;
        let (prompt_tokens, completion_tokens, total_tokens, cache_read_tokens, cache_write_tokens) =
            self.usage.as_event_fields();

        let success = self.ctx.status_is_success && self.stream_error.is_none();
        let error_kind = if success {
            None
        } else if !self.ctx.status_is_success {
            Some(format!("upstream_http:{}", self.ctx.http_status))
        } else if let Some(err) = self.stream_error.as_deref() {
            Some(format!("stream_error:{}", super::truncate(err, 240)))
        } else {
            Some("upstream_error".to_string())
        };
        let error_detail = if success {
            None
        } else if let Some(err) = self.stream_error.as_deref() {
            Some(super::truncate(err, 2000))
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
            sse_terminal = self.sse_seen_terminal,
            sse_last_event = self.sse_last_event.as_deref().unwrap_or("-"),
            sse_last_type = self.sse_last_type.as_deref().unwrap_or("-"),
            prompt_tokens = prompt_tokens.unwrap_or(-1),
            completion_tokens = completion_tokens.unwrap_or(-1),
            total_tokens = total_tokens.unwrap_or(-1),
            success,
            error_kind = error_kind.as_deref().unwrap_or("-"),
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
                sse_terminal = self.sse_seen_terminal,
                sse_last_event = self.sse_last_event.as_deref().unwrap_or("-"),
                sse_last_type = self.sse_last_type.as_deref().unwrap_or("-"),
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
                self.end_reason = Some("upstream_error");
                self.stream_error = Some(e.to_string());
                self.finalize();
                Poll::Ready(Some(Err(std::io::Error::other(e))))
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
            self.end_reason = Some("dropped");
        }
        // If the stream is dropped before completion (e.g. client disconnect), record it as a
        // stream error so it doesn't get counted as a successful request.
        if self.stream_error.is_none() {
            self.stream_error = Some("stream_dropped".to_string());
        }
        self.finalize();
    }
}
