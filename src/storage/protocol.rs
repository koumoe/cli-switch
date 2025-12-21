use rusqlite::types::{FromSql, FromSqlError, ValueRef};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    Openai,
    Anthropic,
    Gemini,
}

impl Protocol {
    pub fn as_str(self) -> &'static str {
        match self {
            Protocol::Openai => "openai",
            Protocol::Anthropic => "anthropic",
            Protocol::Gemini => "gemini",
        }
    }
}

impl std::fmt::Display for Protocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for Protocol {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "openai" => Ok(Protocol::Openai),
            "anthropic" => Ok(Protocol::Anthropic),
            "gemini" => Ok(Protocol::Gemini),
            other => Err(anyhow::anyhow!("未知 protocol：{other}")),
        }
    }
}

impl FromSql for Protocol {
    fn column_result(value: ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        let s = value.as_str()?;
        s.parse::<Protocol>()
            .map_err(|e| FromSqlError::Other(e.into_boxed_dyn_error()))
    }
}

fn protocol_root(protocol: Protocol) -> &'static str {
    match protocol {
        Protocol::Openai | Protocol::Anthropic => "/v1",
        Protocol::Gemini => "/v1beta",
    }
}

pub(crate) fn normalize_base_url(protocol: Protocol, base_url: &str) -> String {
    let base_url = base_url.trim();
    let (without_fragment, fragment) = match base_url.split_once('#') {
        Some((a, b)) => (a, Some(b)),
        None => (base_url, None),
    };
    let (without_query, query) = match without_fragment.split_once('?') {
        Some((a, b)) => (a, Some(b)),
        None => (without_fragment, None),
    };

    let root = protocol_root(protocol);
    let without_query = without_query.trim_end_matches('/');
    let normalized = if without_query.ends_with(root) {
        without_query[..without_query.len().saturating_sub(root.len())]
            .trim_end_matches('/')
            .to_string()
    } else {
        without_query.to_string()
    };

    let mut out = normalized;
    if let Some(q) = query {
        out.push('?');
        out.push_str(q);
    }
    if let Some(f) = fragment {
        out.push('#');
        out.push_str(f);
    }
    out
}
