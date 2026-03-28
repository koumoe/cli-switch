mod catalog;
mod format;
mod issue;
mod locale;
mod request;
mod translator;

pub use catalog::{
    shared_en_catalog_str, shared_zh_catalog_str, ui_en_catalog_str, ui_zh_catalog_str,
};
pub use format::{
    CurrencyCode, format_currency, format_decimal, format_integer, format_local_timestamp_ms,
};
pub use issue::{UserFacingIssue, UserFacingIssuePayload};
pub use locale::AppLocale;
pub use request::{
    LocaleSource, RequestContext, current_locale, current_request_context, current_request_locale,
    locale_context_middleware,
};
pub use translator::{has_translation, render, render_error, render_optional};

#[cfg(test)]
mod tests;
