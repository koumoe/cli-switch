use axum::body::Bytes;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use hmac::{Hmac, Mac};
use serde::Deserialize;
use sha2::Sha256;
use std::collections::HashMap;

use crate::chat_bridge::adapter::IncomingAttachmentKind;
use crate::chat_bridge::adapter::whatsapp::{WhatsAppWebhookAttachment, WhatsAppWebhookMessage};
use crate::server::AppState;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Deserialize)]
pub(in crate::server) struct WebhookVerifyQuery {
    #[serde(rename = "hub.mode")]
    mode: Option<String>,
    #[serde(rename = "hub.challenge")]
    challenge: Option<String>,
    #[serde(rename = "hub.verify_token")]
    verify_token: Option<String>,
}

pub(in crate::server) async fn whatsapp_cloud_webhook_verify(
    State(state): State<AppState>,
    Query(query): Query<WebhookVerifyQuery>,
) -> impl IntoResponse {
    let settings = state.settings_snapshot();
    if !settings.chat_bridge_enabled || !settings.chat_bridge_whatsapp_enabled {
        return StatusCode::NOT_FOUND.into_response();
    }

    let expected = settings
        .chat_bridge_whatsapp_verify_token
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());
    let Some(expected) = expected else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };

    if query.mode.as_deref() != Some("subscribe") {
        return StatusCode::BAD_REQUEST.into_response();
    }

    if query.verify_token.as_deref() != Some(expected) {
        return StatusCode::FORBIDDEN.into_response();
    }

    match query.challenge {
        Some(value) => value.into_response(),
        None => StatusCode::BAD_REQUEST.into_response(),
    }
}

pub(in crate::server) async fn whatsapp_cloud_webhook_receive(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let settings = state.settings_snapshot();
    if !settings.chat_bridge_enabled || !settings.chat_bridge_whatsapp_enabled {
        return StatusCode::NOT_FOUND;
    }

    if let Some(app_secret) = settings
        .chat_bridge_whatsapp_app_secret
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        if !verify_signature(&headers, &body, app_secret) {
            tracing::warn!("whatsapp webhook signature verification failed");
            return StatusCode::UNAUTHORIZED;
        }
    }

    let tx = match state.whatsapp_webhook_tx.as_ref() {
        Some(tx) => tx.clone(),
        None => return StatusCode::SERVICE_UNAVAILABLE,
    };

    let configured_phone_number_id = settings
        .chat_bridge_whatsapp_phone_number_id
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());

    let payload: WebhookPayload = match serde_json::from_slice(&body) {
        Ok(value) => value,
        Err(err) => {
            tracing::warn!(err = %err, "decode whatsapp webhook payload failed");
            return StatusCode::BAD_REQUEST;
        }
    };

    let items = extract_webhook_messages(&payload, configured_phone_number_id);
    for item in items {
        if let Err(err) = tx.try_send(item) {
            tracing::warn!(err = %err, "enqueue whatsapp webhook message failed; dropping");
        }
    }

    StatusCode::OK
}

fn verify_signature(headers: &HeaderMap, body: &[u8], app_secret: &str) -> bool {
    let Some(raw) = headers.get("X-Hub-Signature-256") else {
        return false;
    };
    let Ok(raw) = raw.to_str() else {
        return false;
    };
    let Some(hex_sig) = raw.trim().strip_prefix("sha256=") else {
        return false;
    };
    let Ok(expected) = hex::decode(hex_sig) else {
        return false;
    };
    let Ok(mut mac) = HmacSha256::new_from_slice(app_secret.as_bytes()) else {
        return false;
    };
    mac.update(body);
    mac.verify_slice(&expected).is_ok()
}

#[derive(Debug, Deserialize)]
struct WebhookPayload {
    entry: Vec<WebhookEntry>,
}

#[derive(Debug, Deserialize)]
struct WebhookEntry {
    changes: Vec<WebhookChange>,
}

#[derive(Debug, Deserialize)]
struct WebhookChange {
    field: Option<String>,
    value: WebhookValue,
}

#[derive(Debug, Deserialize)]
struct WebhookValue {
    metadata: Option<WebhookMetadata>,
    contacts: Option<Vec<WebhookContact>>,
    messages: Option<Vec<WebhookMessage>>,
}

#[derive(Debug, Deserialize)]
struct WebhookMetadata {
    phone_number_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WebhookContact {
    wa_id: String,
    profile: Option<WebhookContactProfile>,
}

#[derive(Debug, Deserialize)]
struct WebhookContactProfile {
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WebhookMessage {
    from: String,
    id: String,
    timestamp: String,
    #[serde(rename = "type")]
    kind: String,
    text: Option<WebhookText>,
    image: Option<WebhookMedia>,
    document: Option<WebhookDocument>,
}

#[derive(Debug, Deserialize)]
struct WebhookText {
    body: String,
}

#[derive(Debug, Deserialize)]
struct WebhookMedia {
    id: String,
    mime_type: Option<String>,
    caption: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WebhookDocument {
    id: String,
    mime_type: Option<String>,
    filename: Option<String>,
    caption: Option<String>,
}

fn extract_webhook_messages(
    payload: &WebhookPayload,
    expected_phone_number_id: Option<&str>,
) -> Vec<WhatsAppWebhookMessage> {
    let mut out = Vec::new();

    for entry in &payload.entry {
        for change in &entry.changes {
            if change.field.as_deref() != Some("messages") {
                continue;
            }

            if let Some(expected) = expected_phone_number_id
                && let Some(meta) = change.value.metadata.as_ref()
                && meta
                    .phone_number_id
                    .as_deref()
                    .is_some_and(|v| v.trim() != expected)
            {
                continue;
            }

            let mut names = HashMap::<String, String>::new();
            if let Some(contacts) = change.value.contacts.as_ref() {
                for contact in contacts {
                    if let Some(name) = contact
                        .profile
                        .as_ref()
                        .and_then(|p| p.name.as_deref())
                        .map(str::trim)
                        .filter(|v| !v.is_empty())
                    {
                        names.insert(contact.wa_id.clone(), name.to_string());
                    }
                }
            }

            let Some(messages) = change.value.messages.as_ref() else {
                continue;
            };

            for msg in messages {
                let timestamp_ms = msg
                    .timestamp
                    .trim()
                    .parse::<i64>()
                    .ok()
                    .unwrap_or(0)
                    .saturating_mul(1000);
                let contact_name = names.get(&msg.from).cloned();

                match msg.kind.as_str() {
                    "text" => {
                        let Some(text) = msg.text.as_ref() else {
                            continue;
                        };
                        out.push(WhatsAppWebhookMessage {
                            from: msg.from.clone(),
                            contact_name,
                            message_id: msg.id.clone(),
                            timestamp_ms,
                            text: text.body.clone(),
                            attachments: Vec::new(),
                        });
                    }
                    "image" => {
                        let Some(image) = msg.image.as_ref() else {
                            continue;
                        };
                        out.push(WhatsAppWebhookMessage {
                            from: msg.from.clone(),
                            contact_name,
                            message_id: msg.id.clone(),
                            timestamp_ms,
                            text: image.caption.clone().unwrap_or_default(),
                            attachments: vec![WhatsAppWebhookAttachment {
                                kind: IncomingAttachmentKind::Image,
                                media_id: image.id.clone(),
                                mime_type: image.mime_type.clone(),
                                filename: None,
                                caption: image.caption.clone(),
                            }],
                        });
                    }
                    "document" => {
                        let Some(doc) = msg.document.as_ref() else {
                            continue;
                        };
                        out.push(WhatsAppWebhookMessage {
                            from: msg.from.clone(),
                            contact_name,
                            message_id: msg.id.clone(),
                            timestamp_ms,
                            text: doc.caption.clone().unwrap_or_default(),
                            attachments: vec![WhatsAppWebhookAttachment {
                                kind: IncomingAttachmentKind::File,
                                media_id: doc.id.clone(),
                                mime_type: doc.mime_type.clone(),
                                filename: doc.filename.clone(),
                                caption: doc.caption.clone(),
                            }],
                        });
                    }
                    other => {
                        tracing::debug!(kind = %other, "ignore unsupported whatsapp message type");
                    }
                }
            }
        }
    }

    out
}
