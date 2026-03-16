use serde_json::Value;

use crate::chat_bridge::MAX_DISPLAY_JSON_DEPTH;

pub(in crate::chat_bridge) fn extract_display_text(raw: &str, allow_plain_lines: bool) -> String {
    let mut chunks = Vec::<String>::new();
    append_display_chunks_from_raw(raw, allow_plain_lines, &mut chunks);
    chunks.join("\n")
}

pub(super) fn append_display_chunks_from_raw(
    raw: &str,
    allow_plain_lines: bool,
    chunks: &mut Vec<String>,
) {
    let trimmed = raw.trim();
    if !trimmed.is_empty()
        && let Ok(value) = serde_json::from_str::<Value>(trimmed)
    {
        if apply_known_display_update(&value, chunks) {
            return;
        }
        collect_display_strings(&value, chunks, false, 0);
        if !chunks.is_empty() {
            return;
        }
    }

    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
            if apply_known_display_update(&value, chunks) {
                continue;
            }
            collect_display_strings(&value, chunks, false, 0);
            continue;
        }
        if allow_plain_lines {
            push_display_chunk(chunks, trimmed);
        }
    }
}

enum DisplayUpdate {
    Ignore,
    AppendDelta(String),
    Replace(String),
}

fn apply_known_display_update(value: &Value, chunks: &mut Vec<String>) -> bool {
    let Some(update) = known_display_update(value) else {
        return false;
    };

    match update {
        DisplayUpdate::Ignore => {}
        DisplayUpdate::AppendDelta(text) => append_display_delta(chunks, &text),
        DisplayUpdate::Replace(text) => replace_display_chunks(chunks, &text),
    }
    true
}

fn known_display_update(value: &Value) -> Option<DisplayUpdate> {
    let Value::Object(map) = value else {
        return None;
    };

    let object_type = map
        .get("type")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());

    match object_type {
        Some("init" | "system" | "thread.started" | "turn.started" | "turn.completed") => {
            Some(DisplayUpdate::Ignore)
        }
        Some("message") => known_message_update(map),
        Some("result") => known_result_update(map),
        Some("assistant") => known_claude_assistant_update(map),
        Some("stream_event") => known_claude_stream_event_update(map),
        Some("item.completed" | "item.updated") => known_codex_item_update(map),
        _ => None,
    }
}

fn known_message_update(map: &serde_json::Map<String, Value>) -> Option<DisplayUpdate> {
    let role = map
        .get("role")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let content = map
        .get("content")
        .and_then(Value::as_str)
        .map(str::to_string);

    match role {
        Some("assistant") => content
            .filter(|value| !value.is_empty())
            .map(|value| {
                if map.get("delta").is_some() {
                    DisplayUpdate::AppendDelta(value)
                } else {
                    DisplayUpdate::Replace(value)
                }
            })
            .or(Some(DisplayUpdate::Ignore)),
        Some("user" | "system" | "tool") => Some(DisplayUpdate::Ignore),
        _ => None,
    }
}

fn known_result_update(map: &serde_json::Map<String, Value>) -> Option<DisplayUpdate> {
    if let Some(text) = map
        .get("result")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Some(DisplayUpdate::Replace(text.to_string()));
    }
    Some(DisplayUpdate::Ignore)
}

fn known_claude_assistant_update(map: &serde_json::Map<String, Value>) -> Option<DisplayUpdate> {
    let message = map.get("message")?;
    extract_claude_message_text(message).map(DisplayUpdate::Replace)
}

fn known_claude_stream_event_update(map: &serde_json::Map<String, Value>) -> Option<DisplayUpdate> {
    let event = map.get("event")?.as_object()?;
    let event_type = event
        .get("type")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?;

    match event_type {
        "content_block_delta" => {
            let delta = event.get("delta")?.as_object()?;
            if delta.get("type").and_then(Value::as_str) != Some("text_delta") {
                return Some(DisplayUpdate::Ignore);
            }
            let text = delta
                .get("text")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())?;
            Some(DisplayUpdate::AppendDelta(text.to_string()))
        }
        "message_start"
        | "content_block_start"
        | "content_block_stop"
        | "message_delta"
        | "message_stop" => Some(DisplayUpdate::Ignore),
        _ => Some(DisplayUpdate::Ignore),
    }
}

fn known_codex_item_update(map: &serde_json::Map<String, Value>) -> Option<DisplayUpdate> {
    let item = map.get("item")?.as_object()?;
    if item.get("type").and_then(Value::as_str) != Some("agent_message") {
        return Some(DisplayUpdate::Ignore);
    }
    let text = item
        .get("text")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    Some(DisplayUpdate::Replace(text.to_string()))
}

fn extract_claude_message_text(value: &Value) -> Option<String> {
    let message = value.as_object()?;
    let content = message.get("content")?.as_array()?;
    let mut combined = String::new();

    for block in content {
        let block = block.as_object()?;
        if block.get("type").and_then(Value::as_str) != Some("text") {
            continue;
        }
        let text = block
            .get("text")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())?;
        combined.push_str(text);
    }

    (!combined.is_empty()).then_some(combined)
}

fn append_display_delta(chunks: &mut Vec<String>, text: &str) {
    if text.is_empty() {
        return;
    }

    if let Some(last) = chunks.last_mut() {
        last.push_str(text);
    } else {
        chunks.push(text.to_string());
    }
}

fn replace_display_chunks(chunks: &mut Vec<String>, text: &str) {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return;
    }
    if chunks.len() == 1 && chunks.first().is_some_and(|value| value == trimmed) {
        return;
    }
    chunks.clear();
    chunks.push(trimmed.to_string());
}

fn collect_display_strings(
    value: &Value,
    out: &mut Vec<String>,
    allow_plain_string: bool,
    depth: usize,
) -> bool {
    if depth >= MAX_DISPLAY_JSON_DEPTH {
        return false;
    }

    match value {
        Value::String(text) if allow_plain_string => {
            push_display_chunk(out, text);
            true
        }
        Value::Array(items) => {
            let mut found = false;
            for item in items {
                found |= collect_display_strings(item, out, allow_plain_string, depth + 1);
            }
            found
        }
        Value::Object(map) => {
            if should_ignore_display_object(map) {
                return false;
            }

            for key in [
                "delta",
                "text",
                "content",
                "completion",
                "result",
                "response",
                "output",
                "body",
                "parts",
                "candidates",
                "error",
                "detail",
                "message",
            ] {
                let Some(next) = map.get(key) else {
                    continue;
                };
                let found = match key {
                    "delta" | "text" | "content" | "completion" | "result" | "response"
                    | "output" | "detail" | "message" => {
                        collect_display_strings(next, out, true, depth + 1)
                    }
                    _ => collect_display_strings(next, out, false, depth + 1),
                };
                if found {
                    return true;
                }
            }
            for (key, next) in map {
                if matches!(key.as_str(), "messages") {
                    continue;
                }
                if (next.is_array() || next.is_object())
                    && collect_display_strings(next, out, false, depth + 1)
                {
                    return true;
                }
            }
            false
        }
        _ => false,
    }
}

fn should_ignore_display_object(map: &serde_json::Map<String, Value>) -> bool {
    let role = map
        .get("role")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if matches!(role, Some("user" | "system" | "tool")) {
        return true;
    }

    let object_type = map
        .get("type")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if matches!(object_type, Some("init" | "system" | "user")) {
        return true;
    }

    false
}

fn push_display_chunk(chunks: &mut Vec<String>, raw: &str) {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return;
    }
    if matches!(
        trimmed,
        "assistant" | "user" | "system" | "tool" | "result" | "message"
    ) {
        return;
    }
    if chunks.last().is_some_and(|item| item == trimmed) {
        return;
    }
    chunks.push(trimmed.to_string());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_display_text_reads_simple_response_string() {
        let raw = r#"
{
  "session_id": "demo",
  "response": "OK"
}
"#;

        assert_eq!(extract_display_text(raw, true), "OK");
    }

    #[test]
    fn extract_display_text_ignores_gemini_user_stream_message() {
        let raw = r#"{"type":"message","role":"user","content":"Reply with exactly OK."}
{"type":"message","role":"assistant","content":"OK","delta":true}"#;

        assert_eq!(extract_display_text(raw, true), "OK");
    }

    #[test]
    fn extract_display_text_concatenates_gemini_deltas() {
        let raw = r#"{"type":"message","role":"assistant","content":"SECOND","delta":true}
{"type":"message","role":"assistant","content":".","delta":true}"#;

        assert_eq!(extract_display_text(raw, true), "SECOND.");
    }

    #[test]
    fn extract_display_text_prefers_claude_final_message_over_partial_deltas() {
        let raw = r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"one"}}}
{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"\ntwo\nthree\nfour\nfive"}}}
{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"one\ntwo\nthree\nfour\nfive"}]}}
{"type":"result","result":"one\ntwo\nthree\nfour\nfive"}"#;

        assert_eq!(
            extract_display_text(raw, true),
            "one\ntwo\nthree\nfour\nfive"
        );
    }

    #[test]
    fn append_display_chunks_keeps_gemini_stream_delta_text_on_same_line() {
        let mut chunks = Vec::new();
        append_display_chunks_from_raw(
            r#"{"type":"message","role":"assistant","content":"SECOND","delta":true}"#,
            false,
            &mut chunks,
        );
        append_display_chunks_from_raw(
            r#"{"type":"message","role":"assistant","content":".","delta":true}"#,
            false,
            &mut chunks,
        );

        assert_eq!(chunks, vec!["SECOND.".to_string()]);
    }
}
