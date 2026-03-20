use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde_json::Value;
use std::fmt;
use std::sync::OnceLock;

use super::AppLocale;

static SHARED_ZH: OnceLock<Value> = OnceLock::new();
static SHARED_EN: OnceLock<Value> = OnceLock::new();

const SHARED_ZH_STR: &str = include_str!("../../i18n/locales/shared/zh-CN.json");
const SHARED_EN_STR: &str = include_str!("../../i18n/locales/shared/en-US.json");
#[cfg(test)]
const UI_ZH_STR: &str = include_str!("../../ui/src/locales/ui/zh-CN.json");
#[cfg(test)]
const UI_EN_STR: &str = include_str!("../../ui/src/locales/ui/en-US.json");

pub fn shared_en_catalog_str() -> &'static str {
    SHARED_EN_STR
}

pub fn shared_zh_catalog_str() -> &'static str {
    SHARED_ZH_STR
}

pub fn ui_en_catalog_str() -> &'static str {
    #[cfg(not(test))]
    panic!("ui catalog strings are only available in tests");

    #[cfg(test)]
    UI_EN_STR
}

pub fn ui_zh_catalog_str() -> &'static str {
    #[cfg(not(test))]
    panic!("ui catalog strings are only available in tests");

    #[cfg(test)]
    UI_ZH_STR
}

pub(crate) fn shared_catalog(locale: AppLocale) -> &'static Value {
    match locale {
        AppLocale::ZhCN => SHARED_ZH.get_or_init(|| parse_catalog(SHARED_ZH_STR, "shared zh-CN")),
        AppLocale::EnUS => SHARED_EN.get_or_init(|| parse_catalog(SHARED_EN_STR, "shared en-US")),
    }
}

pub(super) fn parse_catalog_strict(raw: &str, label: &str) -> Result<Value, String> {
    let mut deserializer = serde_json::Deserializer::from_str(raw);
    let value = StrictValueSeed::new("$".to_string())
        .deserialize(&mut deserializer)
        .map_err(|err| format!("parse {label} catalog failed: {err}"))?;
    deserializer
        .end()
        .map_err(|err| format!("parse {label} catalog failed: {err}"))?;
    Ok(value)
}

fn parse_catalog(raw: &str, label: &str) -> Value {
    parse_catalog_strict(raw, label).unwrap_or_else(|err| panic!("{err}"))
}

struct StrictValueSeed {
    path: String,
}

impl StrictValueSeed {
    fn new(path: String) -> Self {
        Self { path }
    }
}

impl<'de> DeserializeSeed<'de> for StrictValueSeed {
    type Value = Value;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(StrictValueVisitor { path: self.path })
    }
}

struct StrictValueVisitor {
    path: String,
}

impl<'de> Visitor<'de> for StrictValueVisitor {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("valid JSON value")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
        Ok(Value::Bool(value))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
        Ok(Value::Number(value.into()))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
        Ok(Value::Number(value.into()))
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        serde_json::Number::from_f64(value)
            .map(Value::Number)
            .ok_or_else(|| E::custom(format!("invalid floating-point value at {}", self.path)))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
        Ok(Value::String(value.to_string()))
    }

    fn visit_borrowed_str<E>(self, value: &'de str) -> Result<Self::Value, E> {
        Ok(Value::String(value.to_string()))
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
        Ok(Value::String(value))
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(Value::Null)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(Value::Null)
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut items = Vec::new();
        let mut index = 0usize;
        while let Some(value) =
            seq.next_element_seed(StrictValueSeed::new(format!("{}[{index}]", self.path)))?
        {
            items.push(value);
            index += 1;
        }
        Ok(Value::Array(items))
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut out = serde_json::Map::new();
        while let Some(key) = map.next_key::<String>()? {
            if out.contains_key(&key) {
                return Err(de::Error::custom(format!(
                    "duplicate key `{key}` at {}",
                    self.path
                )));
            }

            let next_path = if self.path == "$" {
                format!("$.{key}")
            } else {
                format!("{}.{}", self.path, key)
            };
            let value = map.next_value_seed(StrictValueSeed::new(next_path))?;
            out.insert(key, value);
        }
        Ok(Value::Object(out))
    }
}
