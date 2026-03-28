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

    pub fn parse_accept_language(input: &str) -> Option<Self> {
        let mut best = None::<(u16, usize, AppLocale)>;
        for (index, entry) in input.split(',').enumerate() {
            let mut parts = entry.trim().split(';');
            let locale = parts.next().and_then(Self::parse);
            let quality = parts
                .find_map(|part| part.trim().strip_prefix("q="))
                .and_then(parse_quality_value)
                .unwrap_or(1000);
            if quality == 0 {
                continue;
            }
            if let Some(locale) = locale {
                let candidate = (quality, usize::MAX - index, locale);
                if best.map(|current| candidate > current).unwrap_or(true) {
                    best = Some(candidate);
                }
            }
        }
        best.map(|(_, _, locale)| locale)
    }

    pub fn parse_or_default(input: &str) -> Self {
        Self::parse(input).unwrap_or_default()
    }
}

fn parse_quality_value(input: &str) -> Option<u16> {
    let raw = input.trim();
    if raw.is_empty() {
        return None;
    }

    let mut parts = raw.split('.');
    let major = parts.next()?.trim().parse::<u16>().ok()?;
    if major > 1 {
        return None;
    }
    let fraction = parts.next().unwrap_or("0");
    if parts.next().is_some() {
        return None;
    }
    let digits = fraction
        .chars()
        .filter(|ch| ch.is_ascii_digit())
        .take(3)
        .collect::<String>();
    let padded = format!("{digits:0<3}");
    let minor = padded.parse::<u16>().ok()?;
    if major == 1 && minor > 0 {
        return None;
    }
    Some(major * 1000 + minor)
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
