use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AppLocale {
    #[default]
    ZhCN,
    EnUS,
}

impl AppLocale {
    pub const fn as_str(self) -> &'static str {
        match self {
            AppLocale::ZhCN => "zh-CN",
            AppLocale::EnUS => "en-US",
        }
    }

    pub fn parse(input: &str) -> Option<Self> {
        let lowered = input.trim().replace('_', "-").to_ascii_lowercase();
        if lowered.is_empty() {
            return None;
        }
        if lowered == "zh" || lowered.starts_with("zh-") {
            return Some(AppLocale::ZhCN);
        }
        if lowered == "en" || lowered.starts_with("en-") {
            return Some(AppLocale::EnUS);
        }
        None
    }

    pub fn parse_or_default(input: &str) -> Self {
        Self::parse(input).unwrap_or_default()
    }
}

impl std::fmt::Display for AppLocale {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Serialize for AppLocale {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for AppLocale {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        AppLocale::parse(&raw).ok_or_else(|| serde::de::Error::custom("invalid locale"))
    }
}
