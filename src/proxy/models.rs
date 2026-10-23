//! Local model list endpoints (`GET /v1/models`, `GET /v1beta/models`).
//!
//! Instead of passing the request through to a single channel, CliSwitch asks every
//! enabled channel of the requested protocol for its model list (with that channel's
//! own credentials) and merges the results, so clients only see models their channel
//! keys can actually use.

use axum::body::Body;
use axum::http::{HeaderMap, HeaderName, HeaderValue, Response, StatusCode};
use futures_util::future::join_all;
use reqwest::Url;
use serde_json::{Map, Value, json};
use std::collections::HashSet;
use std::time::Duration;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use super::{ProxyError, apply_auth, clear_channel_scoped_headers, filtered_headers};
use crate::storage::{Channel, ManagedRemoteProvider, Protocol};

/// Channels are fetched concurrently; this keeps the whole aggregation below
/// Claude Desktop's 10s model discovery timeout.
const CHANNEL_FETCH_TIMEOUT: Duration = Duration::from_secs(8);
const MAX_PAGES: usize = 5;
const PAGE_SIZE: &str = "1000";

/// Chooses which channels answer `GET /v1/models`, which Anthropic and OpenAI
/// clients share. Anthropic SDK clients (Claude Code, Claude Desktop) always send
/// `anthropic-version`.
pub fn protocol_for_headers(headers: &HeaderMap) -> Protocol {
    if headers.contains_key(HeaderName::from_static("anthropic-version"))
        || headers.contains_key(HeaderName::from_static("x-api-key"))
    {
        Protocol::Anthropic
    } else {
        Protocol::Openai
    }
}

pub async fn list_models(
    client: &reqwest::Client,
    protocol: Protocol,
    inbound_headers: &HeaderMap,
    channels: &[Channel],
) -> Result<Response<Body>, ProxyError> {
    let models = collect_models(client, protocol, inbound_headers, channels).await?;
    Ok(json_response(render_list(protocol, models)))
}

/// Returns `Ok(None)` when no enabled channel lists `model_id`.
pub async fn retrieve_model(
    client: &reqwest::Client,
    protocol: Protocol,
    inbound_headers: &HeaderMap,
    channels: &[Channel],
    model_id: &str,
) -> Result<Option<Response<Body>>, ProxyError> {
    let models = collect_models(client, protocol, inbound_headers, channels).await?;
    Ok(models
        .into_iter()
        .find(|model| model.id == model_id)
        .map(|model| json_response(render_model(protocol, model))))
}

pub(crate) fn build_models_url(mut url: Url, protocol: Protocol) -> Url {
    let base_path = url.path().trim_end_matches('/');
    let root = match protocol {
        Protocol::Openai | Protocol::Anthropic => "/v1",
        Protocol::Gemini => "/v1beta",
    };

    let path = if base_path.is_empty() {
        format!("{root}/models")
    } else if base_path.ends_with(root) {
        format!("{base_path}/models")
    } else {
        format!("{base_path}{root}/models")
    };

    url.set_path(&path);
    url
}

struct ModelEntry {
    id: String,
    raw: Map<String, Value>,
}

struct ModelsPage {
    models: Vec<ModelEntry>,
    next_cursor: Option<String>,
}

async fn collect_models(
    client: &reqwest::Client,
    protocol: Protocol,
    inbound_headers: &HeaderMap,
    channels: &[Channel],
) -> Result<Vec<ModelEntry>, ProxyError> {
    // Channels paused by auto-disable protection still count: clients usually
    // fetch the list once at startup, and the pause is temporary.
    let targets: Vec<&Channel> = channels
        .iter()
        .filter(|c| c.enabled && c.protocol == protocol)
        // OpenAI OAuth channels only serve `/v1/responses`.
        .filter(|c| c.managed_provider() != Some(ManagedRemoteProvider::Openai))
        .collect();
    if targets.is_empty() {
        return Err(ProxyError::NoEnabledChannel(protocol));
    }

    let results = join_all(
        targets
            .iter()
            .map(|channel| fetch_channel_models(client, protocol, inbound_headers, channel)),
    )
    .await;

    // Channels are already ordered by priority; the first channel listing an id wins.
    let mut seen = HashSet::new();
    let mut models = Vec::new();
    let mut any_ok = false;
    let mut last_err = None;
    for (channel, result) in targets.iter().zip(results) {
        match result {
            Ok(channel_models) => {
                any_ok = true;
                models.extend(
                    channel_models
                        .into_iter()
                        .filter(|model| seen.insert(model.id.clone())),
                );
            }
            Err(err) => {
                tracing::warn!(
                    protocol = protocol.as_str(),
                    channel_id = %channel.id,
                    err = %err,
                    "list models failed"
                );
                last_err = Some(err);
            }
        }
    }

    match (any_ok, last_err) {
        (false, Some(err)) => Err(err),
        _ => Ok(models),
    }
}

async fn fetch_channel_models(
    client: &reqwest::Client,
    protocol: Protocol,
    inbound_headers: &HeaderMap,
    channel: &Channel,
) -> Result<Vec<ModelEntry>, ProxyError> {
    let base_url = Url::parse(&channel.base_url)
        .map_err(|e| ProxyError::InvalidBaseUrl(format!("{} ({e})", channel.base_url)))?;
    // Same header handling as forwarding, so client identity and auth detection match.
    let mut headers = filtered_headers(inbound_headers);
    clear_channel_scoped_headers(&mut headers);
    // The body is parsed here, so let reqwest negotiate an encoding it can decode.
    headers.remove(axum::http::header::ACCEPT_ENCODING);

    let fetch = async {
        let mut models = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..MAX_PAGES {
            let mut url = build_models_url(base_url.clone(), protocol);
            set_page_query(&mut url, protocol, cursor.as_deref());
            let mut page_headers = headers.clone();
            apply_auth(channel, protocol, &mut url, &mut page_headers)?;

            let response = client
                .get(url)
                .headers(page_headers)
                .send()
                .await
                .map_err(|e| ProxyError::Upstream(e.to_string()))?;
            let status = response.status();
            if !status.is_success() {
                return Err(ProxyError::Upstream(format!(
                    "model list returned HTTP {status}"
                )));
            }
            let body: Value = response
                .json()
                .await
                .map_err(|e| ProxyError::Upstream(format!("invalid model list body: {e}")))?;
            let page = parse_models_page(protocol, &body).ok_or_else(|| {
                ProxyError::Upstream("model list body has no model array".to_string())
            })?;

            models.extend(page.models);
            match page.next_cursor {
                Some(next) if cursor.as_deref() != Some(next.as_str()) => cursor = Some(next),
                _ => break,
            }
        }
        Ok(models)
    };

    tokio::time::timeout(CHANNEL_FETCH_TIMEOUT, fetch)
        .await
        .map_err(|_| ProxyError::Upstream("model list request timed out".to_string()))?
}

fn set_page_query(url: &mut Url, protocol: Protocol, cursor: Option<&str>) {
    let (size_param, cursor_param) = match protocol {
        Protocol::Anthropic => ("limit", "after_id"),
        Protocol::Gemini => ("pageSize", "pageToken"),
        Protocol::Openai => return,
    };
    let mut query = url.query_pairs_mut();
    query.append_pair(size_param, PAGE_SIZE);
    if let Some(cursor) = cursor {
        query.append_pair(cursor_param, cursor);
    }
}

/// Accepts OpenAI/Anthropic (`data[].id`) and Gemini (`models[].name`) shapes,
/// since relay upstreams do not always answer in their channel's native format.
fn parse_models_page(protocol: Protocol, body: &Value) -> Option<ModelsPage> {
    let items = body
        .get("data")
        .or_else(|| body.get("models"))?
        .as_array()?;
    let models = items
        .iter()
        .filter_map(|item| {
            let raw = item.as_object()?;
            Some(ModelEntry {
                id: model_id(raw)?,
                raw: raw.clone(),
            })
        })
        .collect();

    let next_cursor = match protocol {
        Protocol::Anthropic => body
            .get("has_more")
            .and_then(Value::as_bool)
            .filter(|has_more| *has_more)
            .and_then(|_| body.get("last_id")?.as_str()),
        Protocol::Gemini => body.get("nextPageToken").and_then(Value::as_str),
        Protocol::Openai => None,
    }
    .filter(|cursor| !cursor.is_empty())
    .map(str::to_string);

    Some(ModelsPage {
        models,
        next_cursor,
    })
}

fn model_id(raw: &Map<String, Value>) -> Option<String> {
    let id = match raw.get("id").and_then(Value::as_str) {
        Some(id) => id,
        None => {
            let name = raw.get("name")?.as_str()?;
            name.strip_prefix("models/").unwrap_or(name)
        }
    };
    let id = id.trim();
    (!id.is_empty()).then(|| id.to_string())
}

fn render_list(protocol: Protocol, models: Vec<ModelEntry>) -> Value {
    let first_id = models.first().map(|model| model.id.clone());
    let last_id = models.last().map(|model| model.id.clone());
    let items: Vec<Value> = models
        .into_iter()
        .map(|model| render_model(protocol, model))
        .collect();
    match protocol {
        Protocol::Anthropic => json!({
            "data": items,
            "has_more": false,
            "first_id": first_id,
            "last_id": last_id,
        }),
        Protocol::Openai => json!({ "object": "list", "data": items }),
        Protocol::Gemini => json!({ "models": items }),
    }
}

/// Keeps every upstream field and only fills in what the client's protocol requires.
fn render_model(protocol: Protocol, model: ModelEntry) -> Value {
    let ModelEntry { id, mut raw } = model;
    match protocol {
        Protocol::Anthropic => {
            raw.entry("type").or_insert_with(|| json!("model"));
            raw.entry("display_name").or_insert_with(|| json!(id));
            if !raw.contains_key("created_at") {
                let created_at = raw
                    .get("created")
                    .and_then(Value::as_i64)
                    .and_then(|ts| OffsetDateTime::from_unix_timestamp(ts).ok())
                    .unwrap_or(OffsetDateTime::UNIX_EPOCH)
                    .format(&Rfc3339)
                    .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string());
                raw.insert("created_at".to_string(), json!(created_at));
            }
            raw.insert("id".to_string(), json!(id));
        }
        Protocol::Openai => {
            raw.entry("object").or_insert_with(|| json!("model"));
            raw.entry("owned_by").or_insert_with(|| json!("cliswitch"));
            if !raw.contains_key("created") {
                let created = raw
                    .get("created_at")
                    .and_then(Value::as_str)
                    .and_then(|ts| OffsetDateTime::parse(ts, &Rfc3339).ok())
                    .map_or(0, OffsetDateTime::unix_timestamp);
                raw.insert("created".to_string(), json!(created));
            }
            raw.insert("id".to_string(), json!(id));
        }
        Protocol::Gemini => {
            raw.entry("name")
                .or_insert_with(|| json!(format!("models/{id}")));
        }
    }
    Value::Object(raw)
}

fn json_response(payload: Value) -> Response<Body> {
    let mut resp = Response::new(Body::from(payload.to_string()));
    *resp.status_mut() = StatusCode::OK;
    resp.headers_mut().insert(
        axum::http::header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    resp
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(page: &ModelsPage) -> Vec<&str> {
        page.models.iter().map(|model| model.id.as_str()).collect()
    }

    #[test]
    fn protocol_for_headers_detects_anthropic_clients() {
        let mut headers = HeaderMap::new();
        assert_eq!(protocol_for_headers(&headers), Protocol::Openai);

        headers.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));
        assert_eq!(protocol_for_headers(&headers), Protocol::Anthropic);

        let mut headers = HeaderMap::new();
        headers.insert("x-api-key", HeaderValue::from_static("sk-test"));
        assert_eq!(protocol_for_headers(&headers), Protocol::Anthropic);
    }

    #[test]
    fn build_models_url_respects_protocol_root() {
        let cases = [
            ("https://api.example.com", Protocol::Openai, "/v1/models"),
            (
                "https://api.example.com/v1/",
                Protocol::Anthropic,
                "/v1/models",
            ),
            (
                "https://api.example.com/proxy",
                Protocol::Openai,
                "/proxy/v1/models",
            ),
            ("https://g.example.com", Protocol::Gemini, "/v1beta/models"),
            (
                "https://g.example.com/v1beta",
                Protocol::Gemini,
                "/v1beta/models",
            ),
        ];
        for (base, protocol, path) in cases {
            let url = build_models_url(Url::parse(base).unwrap(), protocol);
            assert_eq!(url.path(), path, "{base}");
        }
    }

    #[test]
    fn parse_models_page_reads_anthropic_cursor() {
        let body = json!({
            "data": [{"type": "model", "id": "claude-a"}, {"id": "claude-b"}, {"id": ""}],
            "has_more": true,
            "last_id": "claude-b",
        });
        let page = parse_models_page(Protocol::Anthropic, &body).unwrap();
        assert_eq!(ids(&page), ["claude-a", "claude-b"]);
        assert_eq!(page.next_cursor.as_deref(), Some("claude-b"));

        let body = json!({"data": [{"id": "claude-a"}], "has_more": false, "last_id": "claude-a"});
        let page = parse_models_page(Protocol::Anthropic, &body).unwrap();
        assert_eq!(page.next_cursor, None);
    }

    #[test]
    fn parse_models_page_reads_gemini_names() {
        let body = json!({
            "models": [{"name": "models/gemini-2.5-pro"}, {"name": "gemini-flash"}],
            "nextPageToken": "next",
        });
        let page = parse_models_page(Protocol::Gemini, &body).unwrap();
        assert_eq!(ids(&page), ["gemini-2.5-pro", "gemini-flash"]);
        assert_eq!(page.next_cursor.as_deref(), Some("next"));
    }

    #[test]
    fn parse_models_page_ignores_openai_cursor_and_rejects_missing_array() {
        let body = json!({"object": "list", "data": [{"id": "gpt-x"}], "has_more": true, "last_id": "gpt-x"});
        let page = parse_models_page(Protocol::Openai, &body).unwrap();
        assert_eq!(ids(&page), ["gpt-x"]);
        assert_eq!(page.next_cursor, None);

        assert!(parse_models_page(Protocol::Openai, &json!({"error": "nope"})).is_none());
    }

    #[test]
    fn render_model_fills_protocol_fields_from_other_shapes() {
        let openai_shaped = ModelEntry {
            id: "claude-a".to_string(),
            raw: json!({"id": "claude-a", "object": "model", "created": 1_700_000_000, "owned_by": "x"})
                .as_object()
                .unwrap()
                .clone(),
        };
        let rendered = render_model(Protocol::Anthropic, openai_shaped);
        assert_eq!(rendered["type"], "model");
        assert_eq!(rendered["display_name"], "claude-a");
        assert_eq!(rendered["created_at"], "2023-11-14T22:13:20Z");

        let anthropic_shaped = ModelEntry {
            id: "claude-b".to_string(),
            raw: json!({"type": "model", "id": "claude-b", "display_name": "Claude B", "created_at": "2023-11-14T22:13:20Z"})
                .as_object()
                .unwrap()
                .clone(),
        };
        let rendered = render_model(Protocol::Openai, anthropic_shaped);
        assert_eq!(rendered["object"], "model");
        assert_eq!(rendered["created"], 1_700_000_000);
        assert_eq!(rendered["owned_by"], "cliswitch");
        assert_eq!(rendered["display_name"], "Claude B");

        let bare = ModelEntry {
            id: "gemini-x".to_string(),
            raw: Map::new(),
        };
        assert_eq!(
            render_model(Protocol::Gemini, bare)["name"],
            "models/gemini-x"
        );
    }

    #[test]
    fn render_list_wraps_models_per_protocol() {
        let entry = |id: &str| ModelEntry {
            id: id.to_string(),
            raw: Map::new(),
        };

        let anthropic = render_list(Protocol::Anthropic, vec![entry("a"), entry("b")]);
        assert_eq!(anthropic["has_more"], false);
        assert_eq!(anthropic["first_id"], "a");
        assert_eq!(anthropic["last_id"], "b");
        assert_eq!(anthropic["data"][1]["id"], "b");

        let openai = render_list(Protocol::Openai, vec![entry("a")]);
        assert_eq!(openai["object"], "list");
        assert_eq!(openai["data"][0]["id"], "a");

        let gemini = render_list(Protocol::Gemini, vec![entry("a")]);
        assert_eq!(gemini["models"][0]["name"], "models/a");
    }
}
