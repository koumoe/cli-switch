use std::str;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum SseLine {
    Event(String),
    Data(Vec<u8>),
    Blank,
    Other,
}

pub(super) struct SseLineParser {
    line_buf: Vec<u8>,
    max_line_bytes: usize,
    skip_oversized_line: bool,
}

pub(super) fn has_sse_field_prefix(bytes: &[u8]) -> bool {
    bytes
        .split(|byte| *byte == b'\n')
        .any(|line| line.starts_with(b"event:") || line.starts_with(b"data:"))
}

impl SseLineParser {
    pub(super) fn new(max_line_bytes: usize) -> Self {
        Self {
            line_buf: Vec::new(),
            max_line_bytes,
            skip_oversized_line: false,
        }
    }

    pub(super) fn feed<F>(&mut self, bytes: &[u8], mut on_line: F)
    where
        F: FnMut(SseLine),
    {
        for byte in bytes {
            if self.skip_oversized_line {
                if *byte == b'\n' {
                    self.skip_oversized_line = false;
                }
                continue;
            }
            if self.line_buf.len() >= self.max_line_bytes {
                self.line_buf.clear();
                self.skip_oversized_line = true;
                continue;
            }
            self.line_buf.push(*byte);
            if *byte == b'\n' {
                let line = std::mem::take(&mut self.line_buf);
                on_line(parse_line(&line));
            }
        }
    }

    pub(super) fn finish<F>(&mut self, mut on_line: F)
    where
        F: FnMut(SseLine),
    {
        if self.skip_oversized_line || self.line_buf.is_empty() {
            return;
        }
        let line = std::mem::take(&mut self.line_buf);
        on_line(parse_line(&line));
    }

    #[cfg(test)]
    fn has_pending_line(&self) -> bool {
        !self.line_buf.is_empty()
    }
}

fn parse_line(line: &[u8]) -> SseLine {
    let Ok(line) = str::from_utf8(line) else {
        return SseLine::Other;
    };
    let line = line.trim();
    if line.is_empty() {
        return SseLine::Blank;
    }
    if let Some(event) = line.strip_prefix("event:") {
        return SseLine::Event(event.trim().to_string());
    }
    if let Some(data) = line.strip_prefix("data:") {
        return SseLine::Data(data.trim().as_bytes().to_vec());
    }
    SseLine::Other
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_event_data_and_blank_lines_across_chunks() {
        let mut parser = SseLineParser::new(64);
        let mut lines = Vec::new();
        parser.feed(b"event: response.completed\nda", |line| lines.push(line));
        parser.feed(b"ta: {\"type\":\"response.completed\"}\n\n", |line| {
            lines.push(line)
        });

        assert_eq!(
            lines,
            vec![
                SseLine::Event("response.completed".to_string()),
                SseLine::Data(br#"{"type":"response.completed"}"#.to_vec()),
                SseLine::Blank,
            ]
        );
        assert!(!parser.has_pending_line());
    }

    #[test]
    fn skips_oversized_line_and_resumes_after_newline() {
        let mut parser = SseLineParser::new(10);
        let mut lines = Vec::new();
        parser.feed(b"data: too long\nevent: ok\n", |line| lines.push(line));

        assert_eq!(lines, vec![SseLine::Event("ok".to_string())]);
    }

    #[test]
    fn finish_parses_unterminated_final_line() {
        let mut parser = SseLineParser::new(64);
        let mut lines = Vec::new();
        parser.feed(b"data: [DONE]", |line| lines.push(line));
        assert!(lines.is_empty());
        parser.finish(|line| lines.push(line));
        assert_eq!(lines, vec![SseLine::Data(b"[DONE]".to_vec())]);
    }

    #[test]
    fn detects_sse_field_before_large_line_is_complete() {
        assert!(has_sse_field_prefix(b"data: partial JSON without newline"));
        assert!(has_sse_field_prefix(b"comment\nevent: response.created"));
        assert!(!has_sse_field_prefix(br#"{"data":"not an SSE field"}"#));
    }
}
