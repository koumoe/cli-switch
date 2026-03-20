use crate::chat_bridge::cli::permission_mode_label;
use crate::cli_tools::CliToolId;
use crate::i18n::AppLocale;
use crate::storage::{BridgePermissionMode, BridgeSession};

use super::i18n::{args, t, t_args};

#[derive(Debug, Clone)]
pub enum ParsedInput {
    PlainText(String),
    Command(Command),
}

#[derive(Debug, Clone)]
pub enum Command {
    Bind {
        token: String,
    },
    Projects,
    Start {
        tool: CliToolId,
        project_ref: String,
        alias: Option<String>,
        permission_mode: BridgePermissionMode,
    },
    Chat {
        target: String,
        message: String,
    },
    Switch {
        target: String,
    },
    Sessions,
    Stop {
        target: String,
    },
    StopAll,
    Lang {
        locale: Option<String>,
    },
    Help,
}

#[derive(Debug, Clone)]
pub enum ResolveTargetResult {
    Exact(BridgeSession),
    Ambiguous(Vec<BridgeSession>),
    NotFound,
}

pub fn parse_input(text: &str, locale: AppLocale) -> anyhow::Result<ParsedInput> {
    let trimmed = text.trim();
    if !trimmed.starts_with('/') {
        return Ok(ParsedInput::PlainText(trimmed.to_string()));
    }

    let mut parts = trimmed.splitn(2, char::is_whitespace);
    let command_name = parts.next().unwrap_or_default();
    let rest = parts.next().unwrap_or("").trim();
    let name = command_name
        .trim_start_matches('/')
        .split('@')
        .next()
        .unwrap_or_default()
        .trim()
        .to_lowercase();

    let command = match name.as_str() {
        "bind" => {
            anyhow::ensure!(!rest.is_empty(), "{}", t(locale, "cmd.bind_usage"));
            Command::Bind {
                token: rest.to_string(),
            }
        }
        "projects" => Command::Projects,
        "codex" => parse_start_command(CliToolId::Codex, rest, locale)?,
        "claude" => parse_start_command(CliToolId::Claude, rest, locale)?,
        "gemini" => parse_start_command(CliToolId::Gemini, rest, locale)?,
        "chat" => {
            let mut segments = rest.splitn(2, char::is_whitespace);
            let target = segments.next().unwrap_or("").trim().to_string();
            let message = segments.next().unwrap_or("").trim().to_string();
            anyhow::ensure!(
                !target.is_empty() && !message.is_empty(),
                "{}",
                t(locale, "cmd.chat_usage")
            );
            Command::Chat { target, message }
        }
        "switch" => {
            anyhow::ensure!(!rest.is_empty(), "{}", t(locale, "cmd.switch_usage"));
            Command::Switch {
                target: rest.to_string(),
            }
        }
        "lang" => {
            if rest.is_empty() {
                Command::Lang { locale: None }
            } else {
                let raw = rest.to_ascii_lowercase();
                anyhow::ensure!(
                    raw == "zh" || raw == "en" || raw == "zh-cn" || raw == "en-us",
                    "{}",
                    t(locale, "cmd.lang_usage")
                );
                Command::Lang { locale: Some(raw) }
            }
        }
        "sessions" => Command::Sessions,
        "stop" => {
            anyhow::ensure!(!rest.is_empty(), "{}", t(locale, "cmd.stop_usage"));
            if rest.eq_ignore_ascii_case("all") {
                Command::StopAll
            } else {
                Command::Stop {
                    target: rest.to_string(),
                }
            }
        }
        "help" | "start" => Command::Help,
        other => anyhow::bail!(
            "{}",
            t_args(
                locale,
                "cmd.unknown",
                &args([("command", other.to_string())])
            )
        ),
    };

    Ok(ParsedInput::Command(command))
}

pub fn resolve_target(user_sessions: &[BridgeSession], target: &str) -> ResolveTargetResult {
    let trimmed = target.trim();
    if trimmed.is_empty() {
        return ResolveTargetResult::NotFound;
    }

    if let Ok(id) = trimmed.parse::<i64>()
        && let Some(session) = user_sessions.iter().find(|item| item.id == id)
    {
        return ResolveTargetResult::Exact(session.clone());
    }

    let lowered = trimmed.to_lowercase();
    let by_alias = user_sessions
        .iter()
        .filter(|item| {
            item.alias
                .as_deref()
                .map(|alias| alias.to_lowercase() == lowered)
                .unwrap_or(false)
        })
        .cloned()
        .collect::<Vec<_>>();
    if by_alias.len() == 1 {
        return ResolveTargetResult::Exact(
            by_alias
                .into_iter()
                .next()
                .unwrap_or_else(|| unreachable!()),
        );
    }

    let by_project = user_sessions
        .iter()
        .filter(|item| item.project_name.to_lowercase() == lowered)
        .cloned()
        .collect::<Vec<_>>();
    if by_project.len() == 1 {
        return ResolveTargetResult::Exact(
            by_project
                .into_iter()
                .next()
                .unwrap_or_else(|| unreachable!()),
        );
    }
    if by_project.len() > 1 {
        return ResolveTargetResult::Ambiguous(by_project);
    }

    let by_tool = user_sessions
        .iter()
        .filter(|item| item.cli_type.as_str() == lowered)
        .cloned()
        .collect::<Vec<_>>();
    match by_tool.len() {
        0 => ResolveTargetResult::NotFound,
        1 => {
            ResolveTargetResult::Exact(by_tool.into_iter().next().unwrap_or_else(|| unreachable!()))
        }
        _ => ResolveTargetResult::Ambiguous(by_tool),
    }
}

pub fn format_session_label(session: &BridgeSession) -> String {
    let id_part = match session.alias.as_deref() {
        Some(alias) if !alias.trim().is_empty() => format!("{alias}(#{})", session.id),
        _ => format!("#{}", session.id),
    };
    format!(
        "[{}|{}|{}]",
        id_part,
        session.cli_type.as_str(),
        session.project_name
    )
}

pub fn format_sessions_list(items: &[BridgeSession], now_ms: i64, locale: AppLocale) -> String {
    if items.is_empty() {
        return t(locale, "session.none");
    }

    let mut lines = vec![t(locale, "session.list_title")];
    for session in items {
        let alias = session
            .alias
            .as_deref()
            .map(|value| format!("[{value}] "))
            .unwrap_or_default();
        let default_mark = if session.is_default {
            format!("  {}", t(locale, "session.default_mark"))
        } else {
            String::new()
        };
        lines.push(format!(
            "#{}  {}{} @ {}  {}  {}{}",
            session.id,
            alias,
            session.cli_type.as_str(),
            session.project_name,
            session_status_label(session, locale),
            permission_mode_label(session.permission_mode),
            default_mark,
        ));
        lines.push(format!(
            "    {}: {}",
            t(locale, "session.path_label"),
            session.working_dir,
        ));
        lines.push(format!(
            "    {}: {}",
            t(locale, "session.last_active_label"),
            relative_time_label(
                now_ms,
                session.last_active_ms.max(session.created_at_ms),
                locale
            ),
        ));
    }

    lines.join("\n")
}

pub fn format_ambiguous_target(target: &str, items: &[BridgeSession], locale: AppLocale) -> String {
    let mut lines = vec![t_args(
        locale,
        "session.ambiguous_title",
        &args([("target", target.to_string())]),
    )];
    for session in items {
        let alias = session
            .alias
            .as_deref()
            .map(|value| format!("[{value}] "))
            .unwrap_or_default();
        lines.push(format!(
            "{}. {}{} @ {}",
            session.id,
            alias,
            session.cli_type.as_str(),
            session.project_name
        ));
    }
    lines.push(t(locale, "session.ambiguous_hint"));
    lines.join("\n")
}

pub fn help_text(locale: AppLocale) -> String {
    t(locale, "help.full")
}

fn parse_start_command(tool: CliToolId, rest: &str, locale: AppLocale) -> anyhow::Result<Command> {
    let usage = t_args(
        locale,
        "cmd.start_usage",
        &args([("tool", tool.as_str().to_string())]),
    );
    let mut tokens = rest.split_whitespace();
    let project_ref = tokens
        .next()
        .map(str::to_string)
        .ok_or_else(|| anyhow::anyhow!("{}", usage.clone()))?;

    let mut alias = None;
    let mut permission_mode = BridgePermissionMode::Safe;
    for token in tokens {
        if token == "--yolo" {
            permission_mode = BridgePermissionMode::Yolo;
            continue;
        }
        if alias.is_none() {
            alias = Some(token.to_string());
            continue;
        }
        anyhow::bail!("{}", usage);
    }

    Ok(Command::Start {
        tool,
        project_ref,
        alias,
        permission_mode,
    })
}

fn session_status_label(session: &BridgeSession, locale: AppLocale) -> String {
    match session.status {
        crate::storage::BridgeSessionStatus::Running => t(locale, "session.status_running"),
        crate::storage::BridgeSessionStatus::Idle => t(locale, "session.status_idle"),
        crate::storage::BridgeSessionStatus::Stopped => t(locale, "session.status_stopped"),
    }
}

fn relative_time_label(now_ms: i64, at_ms: i64, locale: AppLocale) -> String {
    let diff_ms = now_ms.saturating_sub(at_ms).max(0);
    let minutes = diff_ms / 60_000;
    if minutes <= 0 {
        return t(locale, "time.just_now");
    }
    if minutes < 60 {
        return t_args(
            locale,
            "time.minutes_ago",
            &args([("minutes", minutes.to_string())]),
        );
    }
    let hours = minutes / 60;
    if hours < 24 {
        return t_args(
            locale,
            "time.hours_ago",
            &args([("hours", hours.to_string())]),
        );
    }
    let days = hours / 24;
    t_args(locale, "time.days_ago", &args([("days", days.to_string())]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{BridgeSessionStatus, ChatPlatform, now_ms};

    fn sample_session(
        id: i64,
        alias: Option<&str>,
        project_name: &str,
        cli_type: CliToolId,
    ) -> BridgeSession {
        BridgeSession {
            id,
            alias: alias.map(ToOwned::to_owned),
            platform: ChatPlatform::Telegram,
            cli_type,
            cli_session_ref: None,
            project_id: Some(format!("/tmp/{project_name}")),
            project_name: project_name.to_string(),
            working_dir: format!("/tmp/{project_name}"),
            permission_mode: BridgePermissionMode::Safe,
            status: BridgeSessionStatus::Idle,
            is_default: id == 1,
            created_at_ms: now_ms(),
            last_active_ms: now_ms(),
        }
    }

    #[test]
    fn parse_start_command_supports_alias_and_yolo() {
        let parsed = parse_input("/codex 1 alpha --yolo", AppLocale::ZhCN).expect("parse command");
        match parsed {
            ParsedInput::Command(Command::Start {
                tool,
                project_ref,
                alias,
                permission_mode,
            }) => {
                assert_eq!(tool, CliToolId::Codex);
                assert_eq!(project_ref, "1");
                assert_eq!(alias.as_deref(), Some("alpha"));
                assert_eq!(permission_mode, BridgePermissionMode::Yolo);
            }
            other => panic!("unexpected parse result: {other:?}"),
        }
    }

    #[test]
    fn parse_chat_command_keeps_message_body() {
        let parsed =
            parse_input("/chat alpha 补充 auth 模块测试", AppLocale::ZhCN).expect("parse chat");
        match parsed {
            ParsedInput::Command(Command::Chat { target, message }) => {
                assert_eq!(target, "alpha");
                assert_eq!(message, "补充 auth 模块测试");
            }
            other => panic!("unexpected parse result: {other:?}"),
        }
    }

    #[test]
    fn resolve_target_prefers_exact_alias_match() {
        let sessions = vec![
            sample_session(1, Some("alpha"), "api-server", CliToolId::Claude),
            sample_session(2, Some("beta"), "api-server", CliToolId::Codex),
        ];
        match resolve_target(&sessions, "beta") {
            ResolveTargetResult::Exact(session) => assert_eq!(session.id, 2),
            other => panic!("unexpected resolve result: {other:?}"),
        }
    }

    #[test]
    fn resolve_target_reports_ambiguous_cli_type() {
        let sessions = vec![
            sample_session(1, Some("alpha"), "a", CliToolId::Codex),
            sample_session(2, Some("beta"), "b", CliToolId::Codex),
        ];
        match resolve_target(&sessions, "codex") {
            ResolveTargetResult::Ambiguous(items) => assert_eq!(items.len(), 2),
            other => panic!("unexpected resolve result: {other:?}"),
        }
    }

    #[test]
    fn help_text_matches_snapshot_zh_cn() {
        assert_eq!(
            help_text(AppLocale::ZhCN),
            "可用命令：\n/bind <配对码>\n/projects\n/codex <项目> [别名] [--yolo]\n/claude <项目> [别名] [--yolo]\n/gemini <项目> [别名] [--yolo]\n/chat <会话> <消息>\n/switch <会话>\n/sessions\n/stop <会话|all>\n/lang [zh|en]\n/help"
        );
    }

    #[test]
    fn help_text_matches_snapshot_en_us() {
        assert_eq!(
            help_text(AppLocale::EnUS),
            "Available commands:\n/bind <pairing-token>\n/projects\n/codex <project> [alias] [--yolo]\n/claude <project> [alias] [--yolo]\n/gemini <project> [alias] [--yolo]\n/chat <session> <message>\n/switch <session>\n/sessions\n/stop <session|all>\n/lang [zh|en]\n/help"
        );
    }

    #[test]
    fn bind_success_copy_matches_snapshot() {
        assert_eq!(
            t_args(
                AppLocale::ZhCN,
                "bind.success",
                &args([
                    ("platform", "Telegram".to_string()),
                    ("account", "@koumoe".to_string()),
                ]),
            ),
            "绑定成功：Telegram / @koumoe"
        );
        assert_eq!(
            t_args(
                AppLocale::EnUS,
                "bind.success",
                &args([
                    ("platform", "Telegram".to_string()),
                    ("account", "@koumoe".to_string()),
                ]),
            ),
            "Bound successfully: Telegram / @koumoe"
        );
    }

    #[test]
    fn switch_copy_matches_snapshot() {
        assert_eq!(
            t_args(
                AppLocale::ZhCN,
                "session.default_switched",
                &args([("label", "[alpha#1|codex|api-server]".to_string())]),
            ),
            "已切换默认会话为 [alpha#1|codex|api-server]。"
        );
        assert_eq!(
            t_args(
                AppLocale::EnUS,
                "session.default_switched",
                &args([("label", "[alpha#1|codex|api-server]".to_string())]),
            ),
            "Default session switched to [alpha#1|codex|api-server]."
        );
    }

    #[test]
    fn format_sessions_list_matches_snapshot_zh_cn() {
        let sessions = vec![
            BridgeSession {
                id: 1,
                alias: Some("alpha".to_string()),
                platform: ChatPlatform::Telegram,
                cli_type: CliToolId::Codex,
                cli_session_ref: None,
                project_id: Some("/tmp/api-server".to_string()),
                project_name: "api-server".to_string(),
                working_dir: "/tmp/api-server".to_string(),
                permission_mode: BridgePermissionMode::Safe,
                status: BridgeSessionStatus::Running,
                is_default: true,
                created_at_ms: 0,
                last_active_ms: 5 * 60_000,
            },
            BridgeSession {
                id: 2,
                alias: None,
                platform: ChatPlatform::Telegram,
                cli_type: CliToolId::Claude,
                cli_session_ref: None,
                project_id: Some("/tmp/web-ui".to_string()),
                project_name: "web-ui".to_string(),
                working_dir: "/tmp/web-ui".to_string(),
                permission_mode: BridgePermissionMode::Yolo,
                status: BridgeSessionStatus::Stopped,
                is_default: false,
                created_at_ms: 0,
                last_active_ms: 60_000,
            },
        ];

        assert_eq!(
            format_sessions_list(&sessions, 10 * 60_000, AppLocale::ZhCN),
            "当前会话：\n#1  [alpha] codex @ api-server  运行中  Safe  默认\n    路径: /tmp/api-server\n    最近活跃: 5 分钟前\n#2  claude @ web-ui  已停止  YOLO\n    路径: /tmp/web-ui\n    最近活跃: 9 分钟前"
        );
    }

    #[test]
    fn format_sessions_list_matches_snapshot_en_us() {
        let sessions = vec![
            BridgeSession {
                id: 1,
                alias: Some("alpha".to_string()),
                platform: ChatPlatform::Telegram,
                cli_type: CliToolId::Codex,
                cli_session_ref: None,
                project_id: Some("/tmp/api-server".to_string()),
                project_name: "api-server".to_string(),
                working_dir: "/tmp/api-server".to_string(),
                permission_mode: BridgePermissionMode::Safe,
                status: BridgeSessionStatus::Running,
                is_default: true,
                created_at_ms: 0,
                last_active_ms: 5 * 60_000,
            },
            BridgeSession {
                id: 2,
                alias: None,
                platform: ChatPlatform::Telegram,
                cli_type: CliToolId::Claude,
                cli_session_ref: None,
                project_id: Some("/tmp/web-ui".to_string()),
                project_name: "web-ui".to_string(),
                working_dir: "/tmp/web-ui".to_string(),
                permission_mode: BridgePermissionMode::Yolo,
                status: BridgeSessionStatus::Stopped,
                is_default: false,
                created_at_ms: 0,
                last_active_ms: 60_000,
            },
        ];

        assert_eq!(
            format_sessions_list(&sessions, 10 * 60_000, AppLocale::EnUS),
            "Current sessions:\n#1  [alpha] codex @ api-server  running  Safe  default\n    Path: /tmp/api-server\n    Last active: 5 minute(s) ago\n#2  claude @ web-ui  stopped  YOLO\n    Path: /tmp/web-ui\n    Last active: 9 minute(s) ago"
        );
    }
}
