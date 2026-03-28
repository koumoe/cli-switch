use time::{Month, OffsetDateTime, UtcOffset};

use super::AppLocale;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurrencyCode {
    Usd,
    Cny,
}

pub fn format_integer(locale: AppLocale, value: impl ToString) -> String {
    let raw = value.to_string();
    let (sign, digits) = if let Some(rest) = raw.strip_prefix('-') {
        ("-", rest)
    } else {
        ("", raw.as_str())
    };
    let grouped = group_digits(digits);
    match locale {
        AppLocale::ZhCN | AppLocale::EnUS => format!("{sign}{grouped}"),
    }
}

pub fn format_decimal(locale: AppLocale, value: f64, max_decimals: usize) -> String {
    if !value.is_finite() {
        return "-".to_string();
    }

    let precision = max_decimals.min(12);
    let mut raw = format!("{value:.precision$}");
    while raw.contains('.') && raw.ends_with('0') {
        raw.pop();
    }
    if raw.ends_with('.') {
        raw.pop();
    }

    let (sign, digits) = if let Some(rest) = raw.strip_prefix('-') {
        ("-", rest)
    } else {
        ("", raw.as_str())
    };
    let (int_part, frac_part) = digits.split_once('.').unwrap_or((digits, ""));
    let grouped = format_integer(locale, int_part);
    if frac_part.is_empty() {
        format!("{sign}{grouped}")
    } else {
        format!("{sign}{grouped}.{frac_part}")
    }
}

pub fn format_currency(
    locale: AppLocale,
    amount: f64,
    currency: CurrencyCode,
    max_decimals: usize,
) -> String {
    let value = format_decimal(locale, amount, max_decimals);
    if value == "-" {
        return value;
    }

    let prefix = match (locale, currency) {
        (AppLocale::EnUS, CurrencyCode::Usd) => "$",
        (AppLocale::ZhCN, CurrencyCode::Usd) => "US$",
        (AppLocale::EnUS, CurrencyCode::Cny) => "CN¥",
        (AppLocale::ZhCN, CurrencyCode::Cny) => "¥",
    };
    format!("{prefix}{value}")
}

pub fn format_local_timestamp_ms(locale: AppLocale, ms: i64) -> String {
    let Some(local) = local_offset_datetime(ms) else {
        return ms.to_string();
    };

    match locale {
        AppLocale::ZhCN => format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            local.year(),
            u8::from(local.month()),
            local.day(),
            local.hour(),
            local.minute(),
            local.second()
        ),
        AppLocale::EnUS => {
            let (hour, meridiem) = hour_and_meridiem(local.hour());
            format!(
                "{} {:02}, {:04}, {}:{:02}:{:02} {}",
                month_short_name(local.month()),
                local.day(),
                local.year(),
                hour,
                local.minute(),
                local.second(),
                meridiem
            )
        }
    }
}

fn local_offset_datetime(ms: i64) -> Option<OffsetDateTime> {
    let nanos = i128::from(ms).saturating_mul(1_000_000);
    let dt = OffsetDateTime::from_unix_timestamp_nanos(nanos).ok()?;
    let offset = UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC);
    Some(dt.to_offset(offset))
}

fn group_digits(raw: &str) -> String {
    if raw.len() <= 3 {
        return raw.to_string();
    }

    let mut out = String::with_capacity(raw.len() + raw.len() / 3);
    let first_group_len = if raw.len().is_multiple_of(3) {
        3
    } else {
        raw.len() % 3
    };
    out.push_str(&raw[..first_group_len]);
    let mut index = first_group_len;
    while index < raw.len() {
        out.push(',');
        out.push_str(&raw[index..index + 3]);
        index += 3;
    }
    out
}

fn hour_and_meridiem(hour: u8) -> (u8, &'static str) {
    match hour {
        0 => (12, "AM"),
        1..=11 => (hour, "AM"),
        12 => (12, "PM"),
        _ => (hour - 12, "PM"),
    }
}

fn month_short_name(month: Month) -> &'static str {
    match month {
        Month::January => "Jan",
        Month::February => "Feb",
        Month::March => "Mar",
        Month::April => "Apr",
        Month::May => "May",
        Month::June => "Jun",
        Month::July => "Jul",
        Month::August => "Aug",
        Month::September => "Sep",
        Month::October => "Oct",
        Month::November => "Nov",
        Month::December => "Dec",
    }
}
