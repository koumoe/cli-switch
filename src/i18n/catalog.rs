use serde_json::Value;
use std::sync::OnceLock;

use super::AppLocale;

static SHARED_ZH: OnceLock<Value> = OnceLock::new();
static SHARED_EN: OnceLock<Value> = OnceLock::new();

const SHARED_ZH_STR: &str = include_str!("../../i18n/locales/shared/zh-CN.json");
const SHARED_EN_STR: &str = include_str!("../../i18n/locales/shared/en-US.json");
const UI_ZH_STR: &str = include_str!("../../ui/src/locales/ui/zh-CN.json");
const UI_EN_STR: &str = include_str!("../../ui/src/locales/ui/en-US.json");

pub fn shared_en_catalog_str() -> &'static str {
    SHARED_EN_STR
}

pub fn shared_zh_catalog_str() -> &'static str {
    SHARED_ZH_STR
}

pub fn ui_en_catalog_str() -> &'static str {
    UI_EN_STR
}

pub fn ui_zh_catalog_str() -> &'static str {
    UI_ZH_STR
}

pub(crate) fn shared_catalog(locale: AppLocale) -> &'static Value {
    match locale {
        AppLocale::ZhCN => SHARED_ZH.get_or_init(|| parse_catalog(SHARED_ZH_STR, "shared zh-CN")),
        AppLocale::EnUS => SHARED_EN.get_or_init(|| parse_catalog(SHARED_EN_STR, "shared en-US")),
    }
}

fn parse_catalog(raw: &str, label: &str) -> Value {
    serde_json::from_str(raw).unwrap_or_else(|err| panic!("parse {label} catalog failed: {err}"))
}
