use std::collections::BTreeMap;

use crate::i18n::{AppLocale, render_optional};

pub(super) fn args<const N: usize>(pairs: [(&str, String); N]) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for (key, value) in pairs {
        out.insert(key.to_string(), value);
    }
    out
}

pub(super) fn t(locale: AppLocale, key: &str) -> String {
    render_optional(locale, &format!("chatBridge.{key}"), &BTreeMap::new())
        .unwrap_or_else(|| key.to_string())
}

pub(super) fn t_args(locale: AppLocale, key: &str, args: &BTreeMap<String, String>) -> String {
    render_optional(locale, &format!("chatBridge.{key}"), args).unwrap_or_else(|| key.to_string())
}
