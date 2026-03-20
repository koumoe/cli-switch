use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{AppLocale, render_optional};

#[derive(Debug, Clone, Default)]
pub struct UserFacingIssue {
    pub code: &'static str,
    pub args: BTreeMap<String, String>,
    pub detail: Option<String>,
}

impl UserFacingIssue {
    pub fn new(code: &'static str) -> Self {
        Self {
            code,
            args: BTreeMap::new(),
            detail: None,
        }
    }

    pub fn with_arg(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.args.insert(key.into(), value.into());
        self
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn to_payload(&self, locale: AppLocale) -> UserFacingIssuePayload {
        let message = render_optional(locale, self.code, &self.args)
            .or_else(|| render_optional(locale, &format!("errors.{}", self.code), &self.args))
            .unwrap_or_else(|| self.code.to_string());
        UserFacingIssuePayload {
            code: self.code.to_string(),
            message,
            args: self.args.clone(),
            detail: self.detail.clone(),
        }
    }

    pub fn legacy_error_text(&self, locale: AppLocale) -> String {
        self.detail
            .clone()
            .unwrap_or_else(|| self.to_payload(locale).message)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UserFacingIssuePayload {
    pub code: String,
    pub message: String,
    pub args: BTreeMap<String, String>,
    pub detail: Option<String>,
}
