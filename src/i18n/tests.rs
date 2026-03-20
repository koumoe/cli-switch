use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use regex::Regex;
use std::collections::BTreeMap;

use serde_json::Value;

use super::catalog::{
    shared_en_catalog_str, shared_zh_catalog_str, ui_en_catalog_str, ui_zh_catalog_str,
};
use super::translator::flatten_leaf_keys;
use super::{AppLocale, render};

fn parse_catalog(raw: &str) -> Value {
    serde_json::from_str(raw).expect("parse catalog")
}

fn flattened_keys(raw: &str) -> Vec<String> {
    let mut out = Vec::new();
    flatten_leaf_keys(&parse_catalog(raw), "", &mut out);
    out.sort();
    out
}

fn flattened_key_set(raw: &str) -> BTreeSet<String> {
    flattened_keys(raw).into_iter().collect()
}

fn collect_rs_files(root: &Path, out: &mut Vec<PathBuf>) {
    let entries =
        fs::read_dir(root).unwrap_or_else(|err| panic!("read_dir {root:?} failed: {err}"));
    for entry in entries {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        if path.is_dir() {
            collect_rs_files(&path, out);
        } else if path.extension().and_then(|value| value.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

#[test]
fn app_locale_normalizes_values() {
    assert_eq!(AppLocale::parse("zh"), Some(AppLocale::ZhCN));
    assert_eq!(AppLocale::parse("zh_CN"), Some(AppLocale::ZhCN));
    assert_eq!(AppLocale::parse("zh-Hans"), Some(AppLocale::ZhCN));
    assert_eq!(AppLocale::parse("en"), Some(AppLocale::EnUS));
    assert_eq!(AppLocale::parse("en_US"), Some(AppLocale::EnUS));
    assert_eq!(AppLocale::parse("fr"), None);
}

#[test]
fn shared_locale_keys_match() {
    assert_eq!(
        flattened_keys(shared_zh_catalog_str()),
        flattened_keys(shared_en_catalog_str())
    );
}

#[test]
fn ui_locale_keys_match() {
    assert_eq!(
        flattened_keys(ui_zh_catalog_str()),
        flattened_keys(ui_en_catalog_str())
    );
}

#[test]
fn shared_and_ui_do_not_overlap_on_leaf_keys() {
    let shared_keys = flattened_key_set(shared_zh_catalog_str());
    let ui_keys = flattened_key_set(ui_zh_catalog_str());
    let overlap = shared_keys
        .intersection(&ui_keys)
        .cloned()
        .collect::<Vec<_>>();

    assert!(
        overlap.is_empty(),
        "shared/ui locale leaf keys overlap unexpectedly: {overlap:?}"
    );
}

#[test]
fn translator_renders_shared_keys() {
    let rendered = render(AppLocale::EnUS, "errors.internal_error", &BTreeMap::new());
    assert_eq!(rendered, "Internal error");
}

#[test]
fn backend_error_codes_have_translations() {
    let literal_code_re = Regex::new(
        r#"(?x)
        ApiError::(?:bad_request|not_found|conflict|bad_gateway|unavailable)\(\s*"([^"]+)"
        |
        UserFacingIssue::new\(\s*"([^"]+)"
    "#,
    )
    .expect("literal code regex");
    let helper_body_re =
        Regex::new(r#"fn\s+code_for\s*\([^)]*\)\s*->\s*&'static\s+str\s*\{(?s:(.*?))\}"#)
            .expect("helper body regex");
    let helper_code_re =
        Regex::new(r#""([a-z]+(?:[._][a-z0-9]+){2,})""#).expect("helper code regex");

    let mut files = Vec::new();
    collect_rs_files(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut files,
    );

    let mut codes = BTreeSet::from(["internal_error".to_string()]);
    for path in files {
        let raw = fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("read source {path:?} failed: {err}"));
        for caps in literal_code_re.captures_iter(&raw) {
            if let Some(code) = caps.get(1).or_else(|| caps.get(2)) {
                codes.insert(code.as_str().to_string());
            }
        }
        for body in helper_body_re.captures_iter(&raw) {
            for caps in helper_code_re.captures_iter(&body[1]) {
                codes.insert(caps[1].to_string());
            }
        }
    }

    let shared_keys = flattened_key_set(shared_zh_catalog_str());
    let missing = codes
        .into_iter()
        .filter(|code| {
            !shared_keys.contains(code) && !shared_keys.contains(&format!("errors.{code}"))
        })
        .collect::<Vec<_>>();

    assert!(
        missing.is_empty(),
        "missing shared locale entries for backend error codes: {missing:?}"
    );
}
