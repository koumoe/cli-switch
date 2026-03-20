use std::collections::BTreeMap;

#[cfg(test)]
use serde_json::Value;

use super::AppLocale;
use super::catalog::shared_catalog;

pub fn render(locale: AppLocale, key: &str, args: &BTreeMap<String, String>) -> String {
    render_optional(locale, key, args).unwrap_or_else(|| key.to_string())
}

pub fn render_optional(
    locale: AppLocale,
    key: &str,
    args: &BTreeMap<String, String>,
) -> Option<String> {
    lookup(locale, key)
        .or_else(|| lookup(AppLocale::ZhCN, key))
        .map(|template| interpolate(template, args))
}

pub fn render_error(
    locale: AppLocale,
    code: &str,
    args: &BTreeMap<String, String>,
    fallback: &str,
) -> String {
    render_optional(locale, code, args)
        .or_else(|| render_optional(locale, &format!("errors.{code}"), args))
        .unwrap_or_else(|| fallback.to_string())
}

pub fn has_translation(locale: AppLocale, key: &str) -> bool {
    lookup(locale, key).is_some() || lookup(AppLocale::ZhCN, key).is_some()
}

fn lookup(locale: AppLocale, key: &str) -> Option<&'static str> {
    let mut current = shared_catalog(locale);
    for part in key.split('.').filter(|part| !part.is_empty()) {
        current = current.get(part)?;
    }
    current.as_str()
}

fn interpolate(template: &str, args: &BTreeMap<String, String>) -> String {
    let mut out = template.to_string();
    for (key, value) in args {
        let direct = format!("{{{{{key}}}}}");
        let spaced = format!("{{{{ {key} }}}}");
        out = out.replace(&direct, value);
        out = out.replace(&spaced, value);
    }
    out
}

#[cfg(test)]
pub(crate) fn flatten_leaf_keys(value: &Value, prefix: &str, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, next) in map {
                let next_prefix = if prefix.is_empty() {
                    key.to_string()
                } else {
                    format!("{prefix}.{key}")
                };
                flatten_leaf_keys(next, &next_prefix, out);
            }
        }
        Value::String(_) => out.push(prefix.to_string()),
        _ => {}
    }
}
