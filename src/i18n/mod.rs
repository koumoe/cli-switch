mod catalog;
mod issue;
mod locale;
mod request;
mod translator;

pub use catalog::{
    shared_en_catalog_str, shared_zh_catalog_str, ui_en_catalog_str, ui_zh_catalog_str,
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
