# Desktop pet activity state / 桌面宠物活动状态

The pet observes requests passing through CliSwitch and turns executed by its chat bridge. It does not inspect other applications' conversation files or infer conversation titles from prompts.

宠物观察经过 CliSwitch 的代理请求和由聊天桥执行的轮次，不读取其他应用的会话文件，也不从提示词推断标题。

## State contract / 状态约定

`GET /api/activities` returns an in-memory `ActivitySnapshot` (`revision`, `entries`, `omitted_running`). Native clients can also call `activity::snapshot()` and react to `AppEvent::ActivityChanged`.

| State | Meaning / 含义 |
| --- | --- |
| `running` | A request or bridge turn is executing / 请求或桥接轮次正在执行 |
| `response_finished` | A proxy response ended; the client may run more tools or requests / 代理响应结束，客户端可能继续调用工具或发送请求 |
| `completed` | A controlled bridge CLI emitted a recognized successful turn result and exited successfully / 受控桥接 CLI 发出已识别的成功轮次结果且进程正常退出 |
| `failed` | A known request or execution failure / 已知请求或执行失败 |
| `cancelled` | An explicit cancellation signal / 明确的取消信号 |
| `unknown` | A handler/stream disappeared without an authoritative result, or a CLI result was not recognized / 处理过程或连接中断且结果不明，或 CLI 结果未被识别 |

Only `completed` is a confirmed turn completion. A proxy `response.completed` event is not a Codex `turn.completed` event. The UI must not label a count of unassigned requests as a count of conversations, or represent unknown results as successes.

只有 `completed` 表示确认的轮次完成。代理的 `response.completed` 事件不等于 Codex 的 `turn.completed`。未归属请求数量不能称为对话数量，结果未知不能展示为成功。

Each incoming request owns one lifecycle guard across all internal channel retries and the entire response body. Concurrent requests finish independently. Dropped bodies are finalized; an explicit successful response terminal remains `response_finished` even if the downstream client stops reading immediately afterward.

每个入站请求使用一个贯穿渠道重试和响应体的生命周期记录。并发请求各自收尾；客户端在成功响应终止事件后立即停止读取时，状态仍保留为 `response_finished`。

The registry holds at most 256 request/turn records and bounds labels to 120 characters. It evicts the oldest terminal record first. If all slots are active, additional requests contribute to `omitted_running` until they end. It does not persist bodies, API keys, or activity history across application restarts.

注册表最多保存 256 条记录，标签限制为 120 个字符，优先移除最早的终态记录。全部槽位都在执行时，额外请求通过 `omitted_running` 计数直至结束。不持久化正文、密钥或跨重启活动历史。

## Codex identity / Codex 身份识别

For OpenAI Responses requests from a recognized Codex client, the adapter accepts an explicit UUID `thread-id` header and the older `thread_id` spelling. Conflicting aliases, repeated header values, malformed IDs, and unrelated client identities are rejected. A `session-id` / `session_id` value is **not** used as a thread ID: modern Codex distinguishes session and thread identities.

对于明确来自 Codex 客户端的 OpenAI Responses 请求，适配器接受 UUID 格式的 `thread-id`，兼容旧拼写 `thread_id`。两个字段冲突、字段重复、ID 格式错误或客户端来源不符时，保持未归属。新版 Codex 区分 session 和 thread，因此不会把 `session-id` / `session_id` 当成对话 ID。

The snapshot groups those requests by the verified thread ID. One running request keeps the group running even when another request finishes. Titles remain unknown; a UI may display `Codex · <short thread ID>` as an identifier rather than a conversation title. Other clients remain separate unassigned request activities when no verified identity is available. Bridge turns and native proxy requests are not guessed to be the same activity.

快照按已验证的 thread ID 合并展示；同组中仍有任何请求执行时，另一请求结束不会使整组停止。标题保持未知，界面可以显示 `Codex · 短 ID` 作为标识。没有可靠身份的其他客户端仍显示独立的未归属请求。桥接轮次与原生代理请求不会凭相似内容强行合并。

This identifies threads, not the whole execution period between model requests. A client may still be executing a local tool while no proxy request is active. Request headers alone cannot provide reliable turn completion or continuous tool execution status.

这能够识别请求所属对话，不能观察模型请求之间的完整执行过程：没有代理请求时，客户端仍可能运行本地工具。仅靠请求头无法确认整轮完成或连续的工具执行状态。

## External turn completion / 外部对话完成

For external Codex clients, precise turn completion needs an explicit event integration. Codex's documented `notify` hook receives `agent-turn-complete`, `thread-id`, and `turn-id`. This implementation does not install or overwrite hooks, change the user's Codex configuration, or connect to another app-server instance automatically.

外部 Codex 的精确完成提醒需要显式接入事件。官方 `notify` 可收到 `agent-turn-complete`、`thread-id` 和 `turn-id`。当前实现不会自动安装或覆盖 hook，不修改用户 Codex 配置，也不自动连接另一个 app-server。

A future opt-in adapter must preserve existing hooks and retain only an allowlist of identity/status fields. `notify` also contains user input and the last assistant message; these bodies must be discarded. Compatibility with the user's actual desktop build must be verified before claiming support. Hook installation, native thread navigation, and conversation-title lookup are outside this implementation.

后续显式接入需要保留用户已有 hook，只接收白名单中的身份与状态字段。`notify` 还包含用户输入和助手最后回复，这些正文应直接丢弃；必须核验实际桌面版本后才能承诺支持。hook 安装、原生对话跳转和对话标题查询不在当前实现范围内。

## Verified references / 核验依据

- [Codex non-interactive JSON events](https://learn.chatgpt.com/docs/non-interactive-mode#make-output-machine-readable): distinguishes `turn.completed`, `turn.failed`, and item events.
- [Codex external notifications](https://learn.chatgpt.com/docs/config-file/config-advanced#notifications): documented `notify` payload and supported completion event.
- [Codex hyphenated identity headers](https://github.com/openai/codex/blob/7c7b4861d88960f7e3bd5b7f30f8351be666dd84/codex-rs/codex-api/src/requests/headers.rs): explicit `session-id` and `thread-id` fields.
- [Codex historical underscore headers](https://github.com/openai/codex/blob/a98623511ba433154ec811fc63091617f5945438/codex-rs/codex-api/src/requests/headers.rs): explicit `session_id` and `thread_id` fields.
- [Codex client identity predicate](https://github.com/openai/codex/blob/main/codex-rs/login/src/auth/default_client.rs): recognized first-party originators.
- [Claude SDK ResultMessage](https://github.com/anthropics/claude-agent-sdk-python/blob/main/src/claude_agent_sdk/types.py): `is_error` and cancellation `terminal_reason`.
- [Gemini headless output](https://geminicli.com/docs/cli/headless/): JSON/stream result and exit-code semantics.

## Optional Codex notify bridge / 可选 Codex notify 接入

CliSwitch creates a per-process, random-token rendezvous file under its data directory with mode `0600`. The file contains only the loopback port, process ID, and token. The server accepts `POST /api/activities/codex-notify` only from a loopback peer with the matching bearer token. It validates `agent-turn-complete`, an observed `thread-id`, and an observed `turn-id`; requests with an active newer turn or a late previous turn are rejected without changing state. The response body is never stored. The sender uses direct loopback HTTP with redirects disabled and a two-second total deadline.

CliSwitch 会在数据目录创建每进程独立的随机 token rendezvous 文件，权限为 `0600`，只包含 loopback 端口、进程 ID 和 token。服务端只接受 loopback 来源、匹配 bearer token 且 `thread-id` 与 `turn-id` 均已在本实例观察到的 `POST /api/activities/codex-notify`。如果有更新轮次正在执行，或通知属于上一轮，拒绝且不改变状态。正文不保存；发送端直连 loopback、禁用重定向，总超时为两秒。

The server does not alter `~/.codex/config.toml`. `GET /api/activities/codex-notify-command` returns the executable command for the user to merge into an existing `notify` array. Existing notify commands must be preserved manually. The completion event includes `completed_turn_id` and is published as `AppEvent::ActivityCompleted` in addition to the snapshot revision event, so a fast subsequent request cannot erase the one-shot completion animation.

服务端不会修改 `~/.codex/config.toml`。`GET /api/activities/codex-notify-command` 返回可由用户手动合并到已有 `notify` 数组的可执行命令；原有 notify 命令必须由用户自行保留。完成事件包含 `completed_turn_id`，并额外发布 `AppEvent::ActivityCompleted`，因此后续快速请求不会抹掉一次性完成动画。
