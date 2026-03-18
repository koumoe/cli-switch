use crate::chat_bridge::cli::permission_mode_label;
use crate::cli_tools::CliToolId;
use crate::storage::{BridgePermissionMode, BridgeSession};

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
    Help,
}

#[derive(Debug, Clone)]
pub enum ResolveTargetResult {
    Exact(BridgeSession),
    Ambiguous(Vec<BridgeSession>),
    NotFound,
}

pub fn parse_input(text: &str) -> anyhow::Result<ParsedInput> {
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
            anyhow::ensure!(!rest.is_empty(), "用法: /bind <token>");
            Command::Bind {
                token: rest.to_string(),
            }
        }
        "projects" => Command::Projects,
        "codex" => parse_start_command(CliToolId::Codex, rest)?,
        "claude" => parse_start_command(CliToolId::Claude, rest)?,
        "gemini" => parse_start_command(CliToolId::Gemini, rest)?,
        "chat" => {
            let mut segments = rest.splitn(2, char::is_whitespace);
            let target = segments.next().unwrap_or("").trim().to_string();
            let message = segments.next().unwrap_or("").trim().to_string();
            anyhow::ensure!(
                !target.is_empty() && !message.is_empty(),
                "用法: /chat <target> <message>"
            );
            Command::Chat { target, message }
        }
        "switch" => {
            anyhow::ensure!(!rest.is_empty(), "用法: /switch <target>");
            Command::Switch {
                target: rest.to_string(),
            }
        }
        "sessions" => Command::Sessions,
        "stop" => {
            anyhow::ensure!(!rest.is_empty(), "用法: /stop <target> 或 /stop all");
            if rest.eq_ignore_ascii_case("all") {
                Command::StopAll
            } else {
                Command::Stop {
                    target: rest.to_string(),
                }
            }
        }
        "help" | "start" => Command::Help,
        other => anyhow::bail!("未知命令: /{other}，使用 /help 查看可用命令"),
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

pub fn format_sessions_list(items: &[BridgeSession], now_ms: i64) -> String {
    if items.is_empty() {
        return "当前没有活跃会话，使用 /codex、/claude 或 /gemini 启动一个。".to_string();
    }

    let mut lines = vec!["📋 活跃会话:".to_string()];
    for session in items {
        let alias = session
            .alias
            .as_deref()
            .map(|value| format!("[{value}] "))
            .unwrap_or_default();
        let default_mark = if session.is_default {
            "  ⭐ 默认"
        } else {
            ""
        };
        lines.push(format!(
            "#{}  {}{} @ {}  {}  {}{}",
            session.id,
            alias,
            session.cli_type.as_str(),
            session.project_name,
            session_status_label(session),
            permission_mode_label(session.permission_mode),
            default_mark,
        ));
        lines.push(format!("    路径: {}", session.working_dir,));
        lines.push(format!(
            "    最近活动: {}",
            relative_time_label(now_ms, session.last_active_ms.max(session.created_at_ms)),
        ));
    }

    lines.join("\n")
}

pub fn format_ambiguous_target(target: &str, items: &[BridgeSession]) -> String {
    let mut lines = vec![format!("找到多个匹配 `{target}` 的会话，请指定:")];
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
    lines.push("可以使用数字 ID 或 alias，例如 /chat 1 <消息> 或 /chat alpha <消息>".to_string());
    lines.join("\n")
}

pub fn help_text() -> String {
    [
        "可用命令",
        "",
        "认证",
        "/bind <token>  绑定当前聊天账号",
        "",
        "项目",
        "/projects  列出项目列表并生成临时编号（10 分钟内有效）",
        "",
        "启动代理",
        "/codex <project_ref> [alias] [--yolo]  启动 Codex 会话",
        "/claude <project_ref> [alias] [--yolo]  启动 Claude Code 会话",
        "/gemini <project_ref> [alias] [--yolo]  启动 Gemini CLI 会话",
        "",
        "会话交互",
        "/chat <target> <message>  向指定会话发送消息",
        "/switch <target>  切换默认会话",
        "/sessions  查看活跃会话",
        "/stop <target>  停止指定会话",
        "/stop all  停止全部会话",
        "/help  查看帮助",
        "",
        "说明",
        "- <project_ref> 支持 /projects 中的编号、绝对路径、唯一项目名",
        "- <target> 支持数字 ID、alias、唯一项目名、CLI 类型",
        "- 不带命令前缀的普通文本会自动路由到默认会话",
        "- 图片/文件会保存到应用数据目录的 chat-bridge/inbox/ 后再交给 CLI 处理",
        "- safe 为默认模式；追加 --yolo 可跳过权限确认",
    ]
    .join("\n")
}

fn parse_start_command(tool: CliToolId, rest: &str) -> anyhow::Result<Command> {
    let mut tokens = rest.split_whitespace();
    let project_ref = tokens.next().map(str::to_string).ok_or_else(|| {
        anyhow::anyhow!("用法: /{} <project_ref> [alias] [--yolo]", tool.as_str())
    })?;

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
        anyhow::bail!("用法: /{} <project_ref> [alias] [--yolo]", tool.as_str());
    }

    Ok(Command::Start {
        tool,
        project_ref,
        alias,
        permission_mode,
    })
}

fn session_status_label(session: &BridgeSession) -> &'static str {
    match session.status {
        crate::storage::BridgeSessionStatus::Running => "运行中",
        crate::storage::BridgeSessionStatus::Idle => "空闲",
        crate::storage::BridgeSessionStatus::Stopped => "已停止",
    }
}

fn relative_time_label(now_ms: i64, at_ms: i64) -> String {
    let diff_ms = now_ms.saturating_sub(at_ms).max(0);
    let minutes = diff_ms / 60_000;
    if minutes <= 0 {
        return "刚刚".to_string();
    }
    if minutes < 60 {
        return format!("{minutes} 分钟前");
    }
    let hours = minutes / 60;
    if hours < 24 {
        return format!("{hours} 小时前");
    }
    let days = hours / 24;
    format!("{days} 天前")
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
        let parsed = parse_input("/codex 1 alpha --yolo").expect("parse command");
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
        let parsed = parse_input("/chat alpha 补充 auth 模块测试").expect("parse chat");
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
}
