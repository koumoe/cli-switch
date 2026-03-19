use super::parse::extract_display_text;
use super::security::redact_sensitive_text;
use crate::chat_bridge::adapter::ParseMode;
use crate::chat_bridge::projects::{AggregatedProject, display_project_index};
use crate::storage::ChatPlatform;

pub(in crate::chat_bridge) struct RenderedMessage {
    pub(in crate::chat_bridge) content: String,
    pub(in crate::chat_bridge) parse_mode: ParseMode,
}

#[derive(Debug, Clone)]
struct SourceSegment {
    text: String,
    is_fenced_code: bool,
}

pub(in crate::chat_bridge) fn format_projects_list(items: &[AggregatedProject]) -> String {
    if items.is_empty() {
        return "没有发现可用项目。先在本机运行对应 CLI，或在设置里开启“允许新项目”后直接使用绝对路径。".to_string();
    }

    let mut lines = vec!["📂 项目列表:".to_string()];
    for (index, item) in items.iter().enumerate() {
        lines.push(format!(
            "[{}] {} ({})",
            display_project_index(index),
            item.display_name,
            display_path(&item.path),
        ));
    }
    lines.join("\n")
}

fn display_path(path: &str) -> String {
    let trimmed = path.trim();
    if let Some(home) = directories::UserDirs::new().map(|item| item.home_dir().to_path_buf()) {
        let home_str = home.to_string_lossy().to_string();
        if trimmed == home_str {
            return "~".to_string();
        }
        let prefix = format!("{home_str}/");
        if let Some(rest) = trimmed.strip_prefix(&prefix) {
            return format!("~/{rest}");
        }
    }
    trimmed.to_string()
}

pub(super) fn build_live_output(display_chunks: &[String]) -> Option<String> {
    if display_chunks.is_empty() {
        return None;
    }
    Some(display_chunks.join("\n"))
}

pub(in crate::chat_bridge) fn render_chat_message(
    platform: ChatPlatform,
    content: &str,
) -> RenderedMessage {
    let redacted = redact_sensitive_text(content);
    render_redacted_chat_message(platform, &redacted)
}

pub(super) fn render_chat_message_chunks(
    platform: ChatPlatform,
    content: &str,
    max_chars: usize,
) -> Vec<RenderedMessage> {
    match platform {
        ChatPlatform::Telegram => {
            let redacted = redact_sensitive_text(content);
            let raw_chunks = chunk_telegram_source_by_rendered_limit(&redacted, max_chars.max(1));
            raw_chunks
                .into_iter()
                .map(|chunk| RenderedMessage {
                    content: format_telegram_message(&chunk),
                    parse_mode: ParseMode::Html,
                })
                .collect()
        }
        _ => {
            let redacted = redact_sensitive_text(content);
            chunk_text(&redacted, max_chars.max(1))
                .into_iter()
                .map(|chunk| RenderedMessage {
                    content: chunk,
                    parse_mode: ParseMode::PlainText,
                })
                .collect()
        }
    }
}

pub(super) fn render_redacted_chat_message(
    platform: ChatPlatform,
    redacted_content: &str,
) -> RenderedMessage {
    match platform {
        ChatPlatform::Telegram => RenderedMessage {
            content: format_telegram_message(redacted_content),
            parse_mode: ParseMode::Html,
        },
        _ => RenderedMessage {
            content: redacted_content.to_string(),
            parse_mode: ParseMode::PlainText,
        },
    }
}

pub(super) fn format_streaming_message(label: &str, body: &str, max_chars: usize) -> String {
    let normalized_body = body.trim();
    if normalized_body.is_empty() {
        return label.to_string();
    }

    let max_body_chars = max_chars
        .saturating_sub(label.chars().count())
        .saturating_sub(1)
        .max(256);
    let capped_body = cap_for_streaming(normalized_body, max_body_chars);
    format!("{label}\n{capped_body}")
}

pub(super) fn compose_final_output(
    success: bool,
    timed_out: bool,
    file_output: Option<&str>,
    stdout: &str,
    stderr: &str,
) -> String {
    let mut content = file_output
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| {
            let extracted = extract_display_text(stdout, true);
            if extracted.trim().is_empty() {
                stdout.trim().to_string()
            } else {
                extracted
            }
        });

    if !success {
        let stderr_text = {
            let extracted = extract_display_text(stderr, true);
            if extracted.trim().is_empty() {
                stderr.trim().to_string()
            } else {
                extracted
            }
        };
        if !stderr_text.trim().is_empty() {
            if !content.trim().is_empty() {
                content.push_str("\n\n");
            }
            content.push_str("stderr:\n");
            content.push_str(stderr_text.trim());
        }
    }

    if timed_out {
        let timeout_text = "任务执行超时，进程已被强制终止。";
        if content.trim().is_empty() {
            return timeout_text.to_string();
        }
        content = format!("{timeout_text}\n\n{content}");
    }

    if !success && content.trim().is_empty() {
        return "任务执行失败，但没有捕获到可展示的输出。".to_string();
    }
    if success && content.trim().is_empty() {
        return "任务已完成，但 CLI 没有返回可展示的文本输出。".to_string();
    }
    content
}

pub(super) fn chunk_text(text: &str, max_chars: usize) -> Vec<String> {
    let normalized = text.trim();
    if normalized.is_empty() {
        return vec!["(空输出)".to_string()];
    }
    if normalized.chars().count() <= max_chars {
        return vec![normalized.to_string()];
    }

    let mut chunks = Vec::new();
    let mut current = String::new();

    for paragraph in normalized.split("\n\n") {
        let paragraph = paragraph.trim_end();
        if paragraph.is_empty() {
            continue;
        }

        if try_append_chunk(&mut current, "\n\n", paragraph, max_chars) {
            continue;
        }

        flush_chunk(&mut chunks, &mut current);

        if paragraph.chars().count() <= max_chars {
            current.push_str(paragraph);
            continue;
        }

        current = chunk_long_paragraph(&mut chunks, paragraph, max_chars);
    }

    flush_chunk(&mut chunks, &mut current);

    chunks
}

fn try_append_chunk(
    current: &mut String,
    separator: &str,
    segment: &str,
    max_chars: usize,
) -> bool {
    let separator = if current.is_empty() { "" } else { separator };
    if current.chars().count() + separator.chars().count() + segment.chars().count() > max_chars {
        return false;
    }
    current.push_str(separator);
    current.push_str(segment);
    true
}

fn flush_chunk(chunks: &mut Vec<String>, current: &mut String) {
    if !current.is_empty() {
        chunks.push(std::mem::take(current));
    }
}

fn chunk_long_paragraph(chunks: &mut Vec<String>, paragraph: &str, max_chars: usize) -> String {
    let mut line_buf = String::new();

    for line in paragraph.lines() {
        if try_append_chunk(&mut line_buf, "\n", line, max_chars) {
            continue;
        }

        flush_chunk(chunks, &mut line_buf);

        if line.chars().count() <= max_chars {
            line_buf.push_str(line);
            continue;
        }

        let mut fragments = split_long_fragment(line, max_chars);
        line_buf = fragments.pop().unwrap_or_default();
        chunks.extend(fragments);
    }

    line_buf
}

fn split_long_fragment(text: &str, max_chars: usize) -> Vec<String> {
    let mut fragments = Vec::new();
    let mut fragment = String::new();
    let mut fragment_len = 0usize;

    for ch in text.chars() {
        if fragment_len >= max_chars {
            fragments.push(std::mem::take(&mut fragment));
            fragment_len = 0;
        }
        fragment.push(ch);
        fragment_len += 1;
    }

    if !fragment.is_empty() {
        fragments.push(fragment);
    }

    fragments
}

fn chunk_telegram_source_by_rendered_limit(content: &str, max_chars: usize) -> Vec<String> {
    let normalized = content.trim();
    if normalized.is_empty() {
        return vec!["(空输出)".to_string()];
    }

    let segments = split_telegram_source_segments(normalized);
    let mut chunks = Vec::<String>::new();
    let mut current = String::new();

    for segment in segments {
        if segment.is_fenced_code {
            append_fenced_segment(&mut chunks, &mut current, &segment.text, max_chars.max(1));
        } else {
            append_plain_segment(&mut chunks, &mut current, &segment.text, max_chars.max(1));
        }
    }

    if !current.trim().is_empty() {
        chunks.push(current.trim().to_string());
    }
    if chunks.is_empty() {
        chunks.push("(空输出)".to_string());
    }
    chunks
}

fn split_telegram_source_segments(content: &str) -> Vec<SourceSegment> {
    let mut out = Vec::<SourceSegment>::new();
    let mut remaining = content;
    while let Some((start, fence_char, fence_len)) = find_next_fence(remaining) {
        let plain = &remaining[..start];
        if !plain.is_empty() {
            out.push(SourceSegment {
                text: plain.to_string(),
                is_fenced_code: false,
            });
        }

        let marker = std::iter::repeat_n(fence_char, fence_len).collect::<String>();
        let after_start = &remaining[start + marker.len()..];
        let Some(end) = after_start.find(&marker) else {
            out.push(SourceSegment {
                text: remaining[start..].to_string(),
                is_fenced_code: false,
            });
            return out;
        };
        let block_end = start + marker.len() + end + marker.len();
        out.push(SourceSegment {
            text: remaining[start..block_end].to_string(),
            is_fenced_code: true,
        });
        remaining = &remaining[block_end..];
    }

    if !remaining.is_empty() {
        out.push(SourceSegment {
            text: remaining.to_string(),
            is_fenced_code: false,
        });
    }
    out
}

fn append_plain_segment(
    chunks: &mut Vec<String>,
    current: &mut String,
    text: &str,
    max_chars: usize,
) {
    let mut remaining = text.trim();
    while !remaining.is_empty() {
        let candidate = concat_source(current, remaining);
        if telegram_rendered_len(&candidate) <= max_chars {
            *current = candidate;
            break;
        }

        if !current.trim().is_empty() {
            chunks.push(current.trim().to_string());
            current.clear();
            continue;
        }

        let (prefix, rest) = take_plain_prefix_fitting_limit(remaining, max_chars);
        if prefix.trim().is_empty() {
            let forced = force_take_chars(remaining, 1);
            chunks.push(forced.to_string());
            remaining = remaining[forced.len()..].trim_start();
            continue;
        }
        chunks.push(prefix.trim().to_string());
        remaining = rest.trim_start();
    }
}

fn append_fenced_segment(
    chunks: &mut Vec<String>,
    current: &mut String,
    fence_block: &str,
    max_chars: usize,
) {
    let candidate = concat_source(current, fence_block);
    if telegram_rendered_len(&candidate) <= max_chars {
        *current = candidate;
        return;
    }

    if !current.trim().is_empty() {
        chunks.push(current.trim().to_string());
        current.clear();
    }

    if telegram_rendered_len(fence_block) <= max_chars {
        *current = fence_block.trim().to_string();
        return;
    }

    for piece in split_oversized_fenced_block(fence_block, max_chars) {
        if piece.trim().is_empty() {
            continue;
        }
        chunks.push(piece.trim().to_string());
    }
}

fn split_oversized_fenced_block(block: &str, max_chars: usize) -> Vec<String> {
    let Some((fence_char, fence_len, language, code)) = parse_fenced_block(block) else {
        return split_plain_hard(block, max_chars);
    };

    let marker = std::iter::repeat_n(fence_char, fence_len).collect::<String>();
    let lang_prefix = language
        .as_deref()
        .map(|lang| format!("{lang}\n"))
        .unwrap_or_default();

    let mut out = Vec::<String>::new();
    let mut current_code = String::new();
    for line in code.split('\n') {
        let line_with_newline = if current_code.is_empty() {
            line.to_string()
        } else {
            format!("\n{line}")
        };
        let candidate_code = format!("{current_code}{line_with_newline}");
        let candidate = format!("{marker}\n{lang_prefix}{candidate_code}\n{marker}");
        if telegram_rendered_len(&candidate) <= max_chars {
            current_code = candidate_code;
            continue;
        }

        if !current_code.is_empty() {
            out.push(format!("{marker}\n{lang_prefix}{current_code}\n{marker}"));
            current_code.clear();
            let retry = format!("{marker}\n{lang_prefix}{line}\n{marker}");
            if telegram_rendered_len(&retry) <= max_chars {
                current_code = line.to_string();
                continue;
            }
        }

        let wrapped_limit = max_chars.saturating_sub(telegram_rendered_len(&format!(
            "{marker}\n{lang_prefix}\n{marker}"
        )));
        let fragments = split_plain_hard(line, wrapped_limit.max(1));
        for frag in fragments {
            out.push(format!("{marker}\n{lang_prefix}{frag}\n{marker}"));
        }
    }

    if !current_code.is_empty() {
        out.push(format!("{marker}\n{lang_prefix}{current_code}\n{marker}"));
    }
    if out.is_empty() {
        out.push(block.to_string());
    }
    out
}

fn parse_fenced_block(block: &str) -> Option<(char, usize, Option<String>, String)> {
    let (start, fence_char, fence_len) = find_next_fence(block)?;
    if start != 0 {
        return None;
    }
    let marker = std::iter::repeat_n(fence_char, fence_len).collect::<String>();
    let after_start = &block[marker.len()..];
    let end = after_start.rfind(&marker)?;
    let inner = after_start[..end].trim_start_matches('\n');

    let (language, code) = match inner.split_once('\n') {
        Some((first, rest))
            if !first.contains(' ')
                && !first.is_empty()
                && first.len() <= 24
                && first
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-') =>
        {
            (Some(first.to_string()), rest.to_string())
        }
        _ => (None, inner.to_string()),
    };
    Some((fence_char, fence_len, language, code))
}

fn split_plain_hard(text: &str, max_chars: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut remaining = text;
    while !remaining.is_empty() {
        let piece = force_take_chars(remaining, max_chars.max(1));
        out.push(piece.to_string());
        remaining = &remaining[piece.len()..];
    }
    if out.is_empty() {
        out.push(String::new());
    }
    out
}

fn concat_source(current: &str, next: &str) -> String {
    if current.trim().is_empty() {
        return next.trim().to_string();
    }
    let left = current.trim_end();
    let right = next.trim_start();
    if left.ends_with('\n') || right.starts_with('\n') {
        format!("{left}{right}")
    } else {
        format!("{left}\n{right}")
    }
}

fn take_plain_prefix_fitting_limit(text: &str, max_chars: usize) -> (&str, &str) {
    let mut best_cut = None::<usize>;
    let mut last_was_newline = false;
    for (idx, ch) in text.char_indices() {
        if ch == '\n' || ch == ' ' || (last_was_newline && ch == '\n') {
            let cut = idx + ch.len_utf8();
            let prefix = text[..cut].trim_end();
            if !prefix.is_empty() && telegram_rendered_len(prefix) <= max_chars {
                best_cut = Some(cut);
            }
        }
        last_was_newline = ch == '\n';
    }

    if let Some(cut) = best_cut {
        let prefix = text[..cut].trim_end();
        return (prefix, &text[cut..]);
    }

    let mut fallback_cut = 0usize;
    for (idx, ch) in text.char_indices() {
        let cut = idx + ch.len_utf8();
        let prefix = text[..cut].trim_end();
        if prefix.is_empty() {
            continue;
        }
        if telegram_rendered_len(prefix) <= max_chars {
            fallback_cut = cut;
        } else {
            break;
        }
    }
    if fallback_cut == 0 {
        let forced = force_take_chars(text, 1);
        return (forced, &text[forced.len()..]);
    }
    let prefix = text[..fallback_cut].trim_end();
    (prefix, &text[fallback_cut..])
}

fn force_take_chars(text: &str, chars: usize) -> &str {
    if chars == 0 {
        return "";
    }
    let cut = text
        .char_indices()
        .nth(chars)
        .map(|(idx, _)| idx)
        .unwrap_or(text.len());
    &text[..cut]
}

fn telegram_rendered_len(source: &str) -> usize {
    format_telegram_message(source).chars().count()
}

pub(super) fn cap_for_streaming(text: &str, max_chars: usize) -> String {
    let total_chars = text.chars().count();
    if total_chars <= max_chars {
        return text.to_string();
    }

    let keep_chars = max_chars.saturating_sub(4);
    let skip_chars = total_chars.saturating_sub(keep_chars);
    let tail_start = text
        .char_indices()
        .nth(skip_chars)
        .map(|(index, _)| index)
        .unwrap_or(0);
    format!("...\n{}", &text[tail_start..])
}

fn format_telegram_message(content: &str) -> String {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let (title, body) = split_message_title(trimmed);
    let mut out = String::new();
    if let Some(title) = title {
        out.push_str("<b>");
        out.push_str(&escape_html(title));
        out.push_str("</b>");
        if let Some(body) = body {
            out.push('\n');
            out.push_str(&format_telegram_body(body));
        }
    } else {
        out.push_str(&format_telegram_body(trimmed));
    }
    out
}

fn split_message_title(content: &str) -> (Option<&str>, Option<&str>) {
    let Some((first_line, rest)) = content.split_once('\n') else {
        return (None, Some(content));
    };
    let trimmed = first_line.trim();
    if !(trimmed.starts_with('[') || trimmed.starts_with('⚠') || trimmed.starts_with('❌')) {
        return (None, Some(content));
    }
    let rest = rest.trim();
    if rest.is_empty() {
        (Some(trimmed), None)
    } else {
        (Some(trimmed), Some(rest))
    }
}

fn format_telegram_body(body: &str) -> String {
    if contains_fence_marker(body) {
        return render_fenced_code_blocks(body);
    }
    if looks_like_code_block(body) {
        return wrap_preformatted(body);
    }
    render_inline_code(body)
}

fn render_fenced_code_blocks(body: &str) -> String {
    let mut out = String::new();
    let mut remaining = body;

    while let Some((start, fence_char, fence_len)) = find_next_fence(remaining) {
        let plain = &remaining[..start];
        let fence_marker = std::iter::repeat_n(fence_char, fence_len).collect::<String>();
        let after_start = &remaining[start + fence_marker.len()..];
        let (code_block, rest) = match after_start.find(&fence_marker) {
            Some(end) => (
                &after_start[..end],
                &after_start[end + fence_marker.len()..],
            ),
            None => {
                if !plain.is_empty() {
                    if !out.is_empty() {
                        out.push('\n');
                    }
                    out.push_str(&render_inline_code(plain.trim()));
                }
                if !out.is_empty() {
                    out.push('\n');
                }
                out.push_str(&wrap_preformatted(after_start.trim()));
                return out;
            }
        };

        let plain = plain.trim();
        if !plain.is_empty() {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(&render_inline_code(plain));
        }

        if !out.is_empty() {
            out.push('\n');
        }

        let code = strip_code_fence_language(code_block);
        out.push_str(&wrap_preformatted(code.trim()));
        remaining = rest;
    }

    let tail = remaining.trim();
    if !tail.is_empty() {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&render_inline_code(tail));
    }

    out
}

fn contains_fence_marker(body: &str) -> bool {
    body.contains("```") || body.contains("~~~")
}

fn find_next_fence(text: &str) -> Option<(usize, char, usize)> {
    let next_backtick = text.find("```").map(|idx| (idx, '`'));
    let next_tilde = text.find("~~~").map(|idx| (idx, '~'));
    let (idx, ch) = match (next_backtick, next_tilde) {
        (Some(left), Some(right)) => {
            if left.0 <= right.0 {
                left
            } else {
                right
            }
        }
        (Some(item), None) | (None, Some(item)) => item,
        (None, None) => return None,
    };

    let len = text[idx..].chars().take_while(|item| *item == ch).count();
    Some((idx, ch, len.max(3)))
}

fn strip_code_fence_language(block: &str) -> &str {
    let trimmed = block.trim_start_matches('\n');
    let Some((first_line, rest)) = trimmed.split_once('\n') else {
        return trimmed;
    };
    let looks_like_lang = !first_line.contains(' ')
        && first_line.len() <= 24
        && first_line
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-');
    if looks_like_lang { rest } else { trimmed }
}

fn render_inline_code(body: &str) -> String {
    let mut out = String::new();
    let mut in_code = false;
    let mut buffer = String::new();

    for ch in body.chars() {
        if ch == '`' {
            if in_code {
                out.push_str("<code>");
                out.push_str(&escape_html(&buffer));
                out.push_str("</code>");
                buffer.clear();
            } else {
                out.push_str(&escape_html(&buffer));
                buffer.clear();
            }
            in_code = !in_code;
            continue;
        }
        buffer.push(ch);
    }

    if in_code {
        out.push('`');
    }
    out.push_str(&escape_html(&buffer));
    out
}

fn wrap_preformatted(body: &str) -> String {
    format!("<pre><code>{}</code></pre>", escape_html(body))
}

fn looks_like_code_block(body: &str) -> bool {
    let lines: Vec<_> = body
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    if lines.len() < 3 {
        return false;
    }

    let scores = lines
        .iter()
        .map(|line| code_line_score(line))
        .collect::<Vec<_>>();
    let strong_lines = scores.iter().filter(|score| **score >= 2).count();
    let total_score: usize = scores.iter().sum();

    strong_lines >= 2 && (strong_lines * 2 >= lines.len() || total_score >= lines.len() + 2)
}

fn code_line_score(line: &str) -> usize {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return 0;
    }

    let mut score = 0usize;
    if line.starts_with("    ") || line.starts_with('\t') {
        score += 2;
    }
    if trimmed.starts_with('$') || trimmed.starts_with("#!/") {
        score += 2;
    }
    if [
        "fn ", "let ", "pub ", "const ", "use ", "impl ", "struct ", "enum ", "async ", "match ",
        "if ", "for ", "while ", "return ", "class ", "def ", "import ", "from ", "SELECT ",
        "INSERT ", "UPDATE ", "DELETE ",
    ]
    .iter()
    .any(|prefix| trimmed.starts_with(prefix))
    {
        score += 2;
    }
    if trimmed.contains("::") || trimmed.contains("=>") || trimmed.contains("->") {
        score += 2;
    }
    if trimmed == "{" || trimmed == "}" || trimmed.ends_with('{') || trimmed.ends_with("};") {
        score += 1;
    }
    if trimmed.ends_with(';')
        && (trimmed.contains('=') || trimmed.contains('(') || trimmed.contains("return "))
    {
        score += 2;
    }

    score
}

fn escape_html(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat_bridge::adapter::ParseMode;
    use crate::storage::ChatPlatform;

    #[test]
    fn build_live_output_keeps_body_only() {
        assert_eq!(
            build_live_output(&["OK".to_string()]).as_deref(),
            Some("OK")
        );
    }

    #[test]
    fn format_streaming_message_keeps_session_label() {
        assert_eq!(
            format_streaming_message(
                "[alpha(#1)|claude|demo]",
                "OK",
                crate::chat_bridge::MESSAGE_CHAR_LIMIT,
            ),
            "[alpha(#1)|claude|demo]\nOK"
        );
    }

    #[test]
    fn chunk_text_prefers_paragraph_boundaries() {
        let chunks = chunk_text("aaa\n\nbbb\n\nccc", 5);
        assert_eq!(chunks, vec!["aaa", "bbb", "ccc"]);
    }

    #[test]
    fn render_chat_message_formats_telegram_code() {
        let rendered = render_chat_message(
            ChatPlatform::Telegram,
            "[#1|codex|demo]\n```rust\nfn main() {}\n```",
        );
        assert_eq!(rendered.parse_mode, ParseMode::Html);
        assert!(rendered.content.contains("<b>[#1|codex|demo]</b>"));
        assert!(rendered.content.contains("<pre><code>fn main() {}"));
    }

    #[test]
    fn looks_like_code_block_ignores_punctuation_heavy_prose() {
        let text = "One line talks about {choices};\nSecond line asks: what happens here;\nThird line closes with } and words.";
        assert!(!looks_like_code_block(text));
    }

    #[test]
    fn render_chat_message_supports_longer_fence_wrapping_nested_backticks() {
        let rendered = render_chat_message(
            ChatPlatform::Telegram,
            "[#1|codex|demo]\n````markdown\ncode with ``` inside\n````",
        );
        assert!(rendered.content.contains("<pre><code>code with ``` inside"));
    }

    #[test]
    fn render_chat_message_chunks_respects_telegram_rendered_limit_for_escaped_text() {
        let input = format!("[#1|codex|demo]\n{}", "<>&\"".repeat(1400));
        let chunks = render_chat_message_chunks(ChatPlatform::Telegram, &input, 320);
        assert!(chunks.len() > 1);
        for chunk in chunks {
            assert_eq!(chunk.parse_mode, ParseMode::Html);
            assert!(chunk.content.chars().count() <= 320);
        }
    }

    #[test]
    fn render_chat_message_chunks_keeps_fenced_block_intact_when_possible() {
        let input = "[#1|codex|demo]\n开始\n```rust\nfn main() {}\nprintln!(\"ok\");\n```\n结束";
        let chunks = render_chat_message_chunks(ChatPlatform::Telegram, input, 90);
        assert!(chunks.iter().any(|chunk| {
            chunk.content.contains("<pre><code>fn main() {}")
                && chunk.content.contains("println!(&quot;ok&quot;);")
        }));
        assert!(chunks.iter().all(|chunk| {
            let open = chunk.content.matches("<pre><code>").count();
            let close = chunk.content.matches("</code></pre>").count();
            open == close
        }));
    }

    #[test]
    fn render_chat_message_chunks_splits_oversized_fenced_block_into_balanced_blocks() {
        let code = (0..300)
            .map(|idx| format!("line_{idx:03} = value_{idx:03};"))
            .collect::<Vec<_>>()
            .join("\n");
        let input = format!("```rust\n{code}\n```");
        let chunks = render_chat_message_chunks(ChatPlatform::Telegram, &input, 280);
        assert!(chunks.len() > 1);
        for chunk in chunks {
            assert!(chunk.content.chars().count() <= 280);
            assert_eq!(chunk.content.matches("<pre><code>").count(), 1);
            assert_eq!(chunk.content.matches("</code></pre>").count(), 1);
        }
    }
}
