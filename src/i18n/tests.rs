use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use regex::Regex;
use std::collections::BTreeMap;

use serde_json::Value;

use super::catalog::{
    parse_catalog_strict, shared_en_catalog_str, shared_zh_catalog_str, ui_en_catalog_str,
    ui_zh_catalog_str,
};
use super::translator::flatten_leaf_keys;
use super::{AppLocale, render};

fn parse_catalog(raw: &str, label: &str) -> Value {
    parse_catalog_strict(raw, label).unwrap_or_else(|err| panic!("{err}"))
}

fn flattened_keys(raw: &str, label: &str) -> Vec<String> {
    let mut out = Vec::new();
    flatten_leaf_keys(&parse_catalog(raw, label), "", &mut out);
    out.sort();
    out
}

fn flattened_key_set(raw: &str, label: &str) -> BTreeSet<String> {
    flattened_keys(raw, label).into_iter().collect()
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

fn collect_literal_matches(raw: &str, pattern: &Regex, out: &mut BTreeSet<String>) {
    for caps in pattern.captures_iter(raw) {
        if let Some(key) = caps.get(1) {
            out.insert(key.as_str().to_string());
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
        flattened_keys(shared_zh_catalog_str(), "shared zh-CN"),
        flattened_keys(shared_en_catalog_str(), "shared en-US")
    );
}

#[test]
fn ui_locale_keys_match() {
    assert_eq!(
        flattened_keys(ui_zh_catalog_str(), "ui zh-CN"),
        flattened_keys(ui_en_catalog_str(), "ui en-US")
    );
}

#[test]
fn shared_and_ui_do_not_overlap_on_leaf_keys() {
    let shared_keys = flattened_key_set(shared_zh_catalog_str(), "shared zh-CN");
    let ui_keys = flattened_key_set(ui_zh_catalog_str(), "ui zh-CN");
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
fn strict_catalog_parser_rejects_duplicate_keys() {
    let err = parse_catalog_strict(
        r#"{"errors":{"internal_error":"a","internal_error":"b"}}"#,
        "duplicate fixture",
    )
    .expect_err("duplicate keys should fail");

    assert!(err.contains("duplicate key `internal_error` at $.errors"));
}

#[test]
fn shared_catalogs_reject_duplicate_keys() {
    parse_catalog_strict(shared_zh_catalog_str(), "shared zh-CN").expect("shared zh-CN catalog");
    parse_catalog_strict(shared_en_catalog_str(), "shared en-US").expect("shared en-US catalog");
}

#[test]
fn ui_catalogs_reject_duplicate_keys() {
    parse_catalog_strict(ui_zh_catalog_str(), "ui zh-CN").expect("ui zh-CN catalog");
    parse_catalog_strict(ui_en_catalog_str(), "ui en-US").expect("ui en-US catalog");
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

    let shared_keys = flattened_key_set(shared_zh_catalog_str(), "shared zh-CN");
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

#[test]
fn chat_bridge_message_keys_have_translations() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/chat_bridge");
    let direct_key_re = Regex::new(r#"(?:\bt\b|\bt_args\b)\(\s*locale\s*,\s*"([^"]+)""#)
        .expect("chat bridge direct key regex");

    let mut files = Vec::new();
    collect_rs_files(&root, &mut files);

    let mut keys = BTreeSet::new();
    for path in files {
        let raw = fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("read source {path:?} failed: {err}"));
        collect_literal_matches(&raw, &direct_key_re, &mut keys);
    }

    let shared_keys = flattened_key_set(shared_zh_catalog_str(), "shared zh-CN");
    let missing = keys
        .into_iter()
        .filter(|key| !shared_keys.contains(&format!("chatBridge.{key}")))
        .collect::<Vec<_>>();

    assert!(
        missing.is_empty(),
        "missing shared locale entries for chat bridge keys: {missing:?}"
    );
}

#[test]
fn structured_issue_surfaces_do_not_reintroduce_legacy_error_fields() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let rust_type_guards = [
        (
            "src/update.rs",
            r#"pub\s+struct\s+UpdateStatus\s*\{(?s:.*?)\berror\s*:"#,
        ),
        (
            "src/server/handlers/channel.rs",
            r#"struct\s+ChannelTestResponse\s*\{(?s:.*?)\berror\s*:"#,
        ),
        (
            "src/server/handlers/tools.rs",
            r#"struct\s+InstallCliToolResponse\s*\{(?s:.*?)terminal_shim_error\s*:"#,
        ),
        (
            "src/cli_tool_proxy_config.rs",
            r#"pub\s+struct\s+CliToolProxyConfigApplyItem\s*\{(?s:.*?)\berror\s*:"#,
        ),
        (
            "src/server/handlers/chat_bridge_whatsapp.rs",
            r#"struct\s+WhatsAppStatusResponse\s*\{(?s:.*?)last_error\s*:"#,
        ),
        (
            "src/server/handlers/chat_bridge_weixin.rs",
            r#"struct\s+WeixinStatusResponse\s*\{(?s:.*?)last_error\s*:"#,
        ),
    ];

    for (path, pattern) in rust_type_guards {
        let raw = fs::read_to_string(root.join(path))
            .unwrap_or_else(|err| panic!("read source {path:?} failed: {err}"));
        let re =
            Regex::new(pattern).unwrap_or_else(|err| panic!("invalid regex {pattern:?}: {err}"));
        assert!(
            !re.is_match(&raw),
            "legacy compatibility field matched forbidden pattern {pattern:?} in {path}"
        );
    }

    let ts_type_guards = [
        (
            "ui/src/api.ts",
            r#"export\s+type\s+UpdateStatus\s*=\s*\{(?s:.*?)\berror\?\s*:"#,
        ),
        (
            "ui/src/api.ts",
            r#"export\s+type\s+ChannelTestResponse\s*=\s*\{(?s:.*?)\berror\?\s*:"#,
        ),
        (
            "ui/src/api.ts",
            r#"export\s+type\s+CliToolProxyConfigApplyItem\s*=\s*\{(?s:.*?)\berror\?\s*:"#,
        ),
        (
            "ui/src/api.ts",
            r#"export\s+type\s+InstallCliToolResponse\s*=\s*\{(?s:.*?)terminal_shim_error\?\s*:"#,
        ),
        (
            "ui/src/api.ts",
            r#"export\s+type\s+ChatBridgeWhatsAppStatus\s*=\s*\{(?s:.*?)last_error\?\s*:"#,
        ),
        (
            "ui/src/api.ts",
            r#"export\s+type\s+ChatBridgeWeixinStatus\s*=\s*\{(?s:.*?)last_error\?\s*:"#,
        ),
    ];

    for (path, pattern) in ts_type_guards {
        let raw = fs::read_to_string(root.join(path))
            .unwrap_or_else(|err| panic!("read source {path:?} failed: {err}"));
        let re =
            Regex::new(pattern).unwrap_or_else(|err| panic!("invalid regex {pattern:?}: {err}"));
        assert!(
            !re.is_match(&raw),
            "legacy UI compatibility field matched forbidden pattern {pattern:?} in {path}"
        );
    }

    let legacy_tokens = [
        ("src/update.rs", "legacy_error_text("),
        ("ui/src/pages/SettingsPage.tsx", "updateStatus.error"),
        ("ui/src/pages/SettingsPage.tsx", "applied?.error"),
        ("ui/src/pages/ChannelsPage.tsx", "r.error"),
        ("ui/src/lib/cliToolInstaller.ts", "terminal_shim_error"),
        (
            "ui/src/pages/SettingsPage.tsx",
            "chatBridgeWeixinStatus?.last_error",
        ),
        (
            "ui/src/pages/SettingsPage.tsx",
            "chatBridgeWhatsAppStatus?.last_error",
        ),
    ];

    for (path, token) in legacy_tokens {
        let raw = fs::read_to_string(root.join(path))
            .unwrap_or_else(|err| panic!("read source {path:?} failed: {err}"));
        assert!(
            !raw.contains(token),
            "legacy compatibility token {token:?} unexpectedly present in {path}"
        );
    }
}
