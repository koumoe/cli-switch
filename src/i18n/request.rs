use axum::extract::{Request, State};
use axum::http::HeaderValue;
use axum::middleware::Next;
use axum::response::Response;

use crate::server::AppState;

use super::AppLocale;

tokio::task_local! {
    static REQUEST_CONTEXT: RequestContext;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocaleSource {
    Header,
    Query,
    Settings,
    Default,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestContext {
    pub locale: AppLocale,
    pub source: LocaleSource,
}

pub fn current_request_context() -> Option<RequestContext> {
    REQUEST_CONTEXT.try_with(|ctx| *ctx).ok()
}

pub fn current_request_locale() -> Option<AppLocale> {
    current_request_context().map(|ctx| ctx.locale)
}

pub fn current_locale() -> Option<AppLocale> {
    current_request_locale()
}

pub async fn locale_context_middleware(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Response {
    let ctx = resolve_request_context(&state, &req);
    REQUEST_CONTEXT
        .scope(ctx, async move {
            let mut response = next.run(req).await;
            response.headers_mut().insert(
                http::header::CONTENT_LANGUAGE,
                HeaderValue::from_static(ctx.locale.as_str()),
            );
            response
        })
        .await
}

fn resolve_request_context(state: &AppState, req: &Request) -> RequestContext {
    let path = req.uri().path();

    if let Some(raw) = req
        .headers()
        .get("X-CliSwitch-Locale")
        .and_then(|value| value.to_str().ok())
        .and_then(AppLocale::parse)
    {
        return RequestContext {
            locale: raw,
            source: LocaleSource::Header,
        };
    }

    if path == "/api/update/changelog"
        && let Some(locale) = req
            .uri()
            .query()
            .and_then(query_locale_value)
            .and_then(AppLocale::parse)
    {
        return RequestContext {
            locale,
            source: LocaleSource::Query,
        };
    }

    let settings_locale = state.settings_snapshot().ui_locale;
    if settings_locale != AppLocale::default() {
        return RequestContext {
            locale: settings_locale,
            source: LocaleSource::Settings,
        };
    }

    RequestContext {
        locale: AppLocale::default(),
        source: LocaleSource::Default,
    }
}

fn query_locale_value(query: &str) -> Option<&str> {
    for pair in query.split('&') {
        let mut parts = pair.splitn(2, '=');
        let key = parts.next().unwrap_or_default();
        if key != "locale" {
            continue;
        }
        return parts.next();
    }
    None
}
