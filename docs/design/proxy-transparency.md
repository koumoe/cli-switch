# Proxy transparency and account switching / 代理透明性与账号切换

本文定义 CliSwitch 面向 Codex CLI、Codex App、Claude CLI 和 Gemini CLI 的代理边界。CliSwitch 的主要职责是聚合账号、选择渠道并替换认证信息。普通 API Key 渠道可以采用严格透明转发；ChatGPT OAuth 渠道连接的是 `chatgpt.com/backend-api/codex` 私有端点，只能采用有证据支持的最小必要适配。本文描述目标行为；实现调整应以本文为验收标准。

This document defines the proxy boundary for Codex CLI, Codex App, Claude CLI, and Gemini CLI. Strict transparency applies to ordinary API-key channels. ChatGPT OAuth channels use the private `chatgpt.com/backend-api/codex` endpoint and therefore allow only evidence-backed minimal adaptation. This target behavior is the acceptance baseline for implementation changes.

## 目标 / Goal

请求应尽可能保持客户端语义：

```text
客户端 → CliSwitch → 选定渠道
```

对于普通 API Key 渠道，除目标地址、账号认证和 HTTP 连接级字段外，CliSwitch 不应改变请求正文、客户端身份、会话上下文或上游响应。账号切换不应演变成客户端伪装或请求重写。

对于 ChatGPT OAuth 渠道，透明性取决于私有端点的实际容忍度。任何额外 header 或 body 改写都必须有可复现的上游行为证据，并且应尽量限制在 OAuth 渠道。

Ordinary API-key channels must preserve the client's semantics. ChatGPT OAuth channels may require adaptation because their upstream is private; every adaptation must be supported by reproducible upstream evidence and kept as narrow as possible.

本文同时记录当前实现和目标行为。当前实现审计如下；目标行为不能视为已经完成的功能。

| 当前实现 | 代码事实 |
| --- | --- |
| 请求 body | handler 会先完整读取请求体，限制为 64 MiB，以支持重试和读取 model/stream；内容通常保持字节不变，但请求不会以入站流的方式直接穿透。 |
| reasoning ID | `/v1/responses` 在渠道选择前执行兼容 shim；设置默认开启，只有命中非法 reasoning `id` 时才重新序列化 body。 |
| OAuth body | OpenAI managed channel 调用 `normalize_responses_body`；当前函数对合规 body 返回原始字节，仅在字段缺失/不合规时做最小修正；`model` 参数来自入站请求，没有独立的渠道 model 映射配置。 |
| OAuth 身份 header | 当前首发请求和 401 刷新重试都只替换 OAuth 凭证与 `chatgpt-account-id`；`originator`、`version`、`User-Agent`、session/thread header 直接透传。 |
| OAuth 其他 header | 当前不再由 OAuth 账号逻辑无条件覆盖 `Content-Type`、`Accept` 和 `openai-beta`；它们随客户端请求转发。 |
| 普通认证 | 当前按 `Channel.auth_type` 选择显式认证方式；`auto` 保留入站 header/URL 推断语义。 |
| 跨渠道身份残留 | 请求转发前会清理 `chatgpt-account-id` 和 `Cookie`，避免它们从 Codex/OAuth 请求进入其他渠道。 |
| OAuth Cookie 路由 | OpenAI OAuth 使用按账号缓存的独立 host-scoped cookie client；账号 A 的边缘路由 Cookie 不会进入账号 B 的 client。 |
| Anthropic 兼容 header | 如果入站没有 `anthropic-version`，当前会补 `2023-06-01`。 |
| OAuth 路径 | OpenAI managed channel 只接受 `/v1/responses`；其他 OpenAI 路径会跳过该渠道。 |
| OpenAI Responses 流 | 成功流会先做 bootstrap 缓冲；在语义输出或终止事件时释放；如果 SSE 缓冲达到 2 MiB 上限时仍未出现语义输出，则视为 bootstrap 失败并在尚未提交响应前重试，不向客户端提交一个未经确认的 200。明确的上游失败耗尽后保留原始响应，不再合成 `response.failed`；bootstrap 超限且无法重试时返回代理错误。 |
| 响应 header | 上游 header 会经过连接级字段过滤；`Content-Type` 不再因 SSE/JSON 检测而修正。 |
| locale | locale middleware 仍包住总 Router 以提供请求上下文，但现在会跳过 `/v1` 和 `/v1beta` 响应，不再写入 `Content-Language`。 |

This document records both the current implementation and the target behavior. The current implementation audit above is not a claim that the target has already been implemented.

## 转发边界 / Forwarding boundary

### 必须改变的内容

| 项目 | 规则 |
| --- | --- |
| 目标 URL | 根据渠道 `base_url` 和入站路径构造新的上游地址。 |
| `Authorization` | 替换为当前选中账号的凭证。 |
| `chatgpt-account-id` | OpenAI OAuth 渠道切换账号时，与凭证一起替换。 |
| Gemini 认证 | 根据渠道认证配置写入 `x-goog-api-key` 或 query `key`。 |
| Anthropic 认证 | 根据渠道认证配置写入 `x-api-key`。 |
| 连接级字段 | `Host`、`Content-Length`、`Connection` 等属于新 HTTP 连接，由代理或 HTTP 客户端按实际目标和 body 重新生成。 |

### 必须保留的内容

以下字段属于客户端或会话上下文，不属于账号身份。客户端已经发送时，必须原样保留：

| 字段 | 含义 |
| --- | --- |
| `originator` | 客户端来源，例如 `codex_cli_rs`、`codex-tui`、`Codex Desktop`。它用于识别客户端，不表示账号。 |
| `User-Agent` | 客户端和版本信息。 |
| `version` | 客户端版本字段（如果客户端发送）。代理不应凭空生成或覆盖。 |
| `session-id` / `session_id` | Codex 会话身份。不得随机生成替代值。 |
| `thread-id` / `thread_id` | Codex 线程身份。不得因账号切换而改变。 |
| `x-codex-*` | Codex 请求上下文和路由信息。 |
| `Accept`、`Content-Type`、`openai-beta` | 客户端声明的协议和能力。已有值时不覆盖。 |
| 请求正文 | 默认按原始字节发送，不解析后重新序列化。为支持账号重试，请求可以先整体缓冲；缓冲不等于改写。 |

### 必须清理或按渠道隔离的内容

这些字段不能跨账号或跨提供商无条件携带：

| 字段 | 规则 |
| --- | --- |
| `chatgpt-account-id` | OAuth 渠道替换为所选账号的值；发送到普通 API Key 渠道时删除，避免把 ChatGPT 账号身份带给其他提供商。 |
| `Cookie` | Cookie 可能包含提供商会话和账号路由状态。支持的 API Key 渠道默认不需要它，应避免把 OAuth/ChatGPT Cookie 带到其他提供商；OAuth 渠道的入站 Cookie 会被清理，随后使用当前账号专属的 cookie client。若未来支持 Cookie 认证提供商，必须改为按渠道显式允许。 |
| 与目标提供商专属的认证字段 | 清理客户端或其他渠道的认证字段，再写入当前渠道配置的认证字段。 |

`Cookie` 的清理是跨账号隔离规则，不是对所有未来 HTTP 用例的通用删除规则。当前支持的 Codex、Claude CLI 和 Gemini CLI 使用 API Key 或 OAuth 账号路径，因此可以按上述默认策略处理。

`Host`、`Content-Length` 和 hop-by-hop header 是每一条 HTTP 连接自己的传输字段。Codex → CliSwitch 与 CliSwitch → 上游是两条不同的连接：入站 `Host` 是 CliSwitch 的地址，不能照搬到上游；`Content-Length` 虽然在 body 未变时可能数值相同，也应由 HTTP 客户端根据实际 body 重新计算。删除这些字段不会改变 Codex 的应用层数据，也不表示代理做了协议转换。

`originator` 明确属于客户端元数据，而不是账号级字段。普通渠道必须保留它。OAuth 渠道需要额外考虑私有端点的客户端校验：`originator`、User-Agent 中的客户端名和版本字段必须作为一个整体处理，不能逐字段拼接出混合身份。

当前 OAuth 路径直接使用入站的客户端身份和请求上下文，不再检测本机 Codex 版本、不再缓存兼容身份，也不再为非 Codex 客户端合成身份。非 Codex 请求是否被 OAuth 上游拒绝，由上游真实结果决定；CliSwitch 不应通过伪造身份绕过该结果。

`originator` is client metadata, not account identity. For OAuth, `originator`, the client name/version in User-Agent, and any version header must be handled as one coherent identity tuple; they must not be mixed independently.

## Codex Responses 兼容修复 / Codex Responses compatibility fix

reasoning item ID 清理是兼容修复，不是账号切换逻辑。规则如下：

- 默认开启；
- 仅对路径 `/v1/responses` 生效；
- 不要求目标渠道是 OpenAI OAuth，普通 OpenAI API Key 渠道同样可以使用；
- 只删除不合法的 reasoning `id`，保留正文中的其他字段；
- 不改变其他协议的请求；
- 不因修复功能而改写 `model`、`stream`、`store`、`instructions` 或其他业务字段。

判断条件使用路径即可。当前代理将 `/v1/responses` 作为 Codex Responses 入口，Claude CLI 和 Gemini CLI 不使用该入口。该 shim 针对已经出现过的 reasoning ID 兼容故障，默认开启是当前既有行为；如果要调整默认值，必须先有用户可见的替代方案和 changelog 说明。需要注意：命中 shim 时会重新序列化 JSON，因而只能保证其他语义字段保留，不能保证原始空格、换行和字段顺序保持不变。

## OpenAI OAuth 渠道 / OpenAI OAuth channels

OAuth 渠道与普通 API Key 渠道的上游性质不同。当前端点是 `chatgpt.com/backend-api/codex` 私有接口，不能把普通 API Key 渠道的严格透明标准直接套用到它。当前已有的 body 规范化包含两类逻辑，必须拆开验证：

1. 可能是私有端点的硬性兼容要求，例如 `stream=true`、`store=false`、`include=["reasoning.encrypted_content"]`、`parallel_tool_calls`，以及删除端点不接受的字段；
2. `model` 写回。当前实现传入的 model 来自入站请求本身，仓库中没有渠道级 model 映射字段，因此这不是已经存在的渠道映射功能。若未来增加模型映射，应作为独立的显式渠道配置处理，不能和兼容清理混在一起。

在没有真实 OAuth 上游证据前，不应直接移除第一类逻辑，也不应继续扩大它。当前实现已将这类处理收敛为：合规 body 按原始字节发送，仅在客户端 body 不符合已确认的要求时做最小修正。每个字段都要有对应的复现请求和上游结果；没有证据的字段不能仅因为历史代码存在就继续扩大适配范围。

OAuth 的客户端身份 header 必须按整体身份处理。以下三个字段不能简单按“有则保留、无则补默认”逐个组合：

- `version`；
- `User-Agent`；
- `originator`；

客户端已生成的身份三元组应原样发送给官方 endpoint；CliSwitch 不再生成或覆盖它。`session-id` 和 `session_id` 作为可能的历史别名，客户端提供哪一种就透传哪一种，不因另一形式缺失而生成随机值。公开 Codex 源码对连字符形式的使用只是外部参考，不替代对具体客户端版本的验证。

历史修复 `65a5683`（`fix: stabilize OpenAI quota and upstream requests`）曾在一个子改动中加入身份覆盖；该提交还包含 Cookie 客户端、额度和更新检测等改动。当前已删除这段身份伪造逻辑，后续若再次引入必须有真实上游证据。

## 流式响应与首段缓冲 / Streaming response and bootstrap buffering

上游返回 HTTP 200 不代表 Responses 流已经成功。上游可能先返回 200，再在第一个 SSE 数据块中报告错误。因此 OpenAI Responses 流采用首段缓冲：

```text
上游建立连接并返回 200
→ CliSwitch 暂存首段 SSE
→ 等待第一个有效输出或明确终止事件
→ 成功：发送状态、响应头和已暂存内容
→ 后续内容保持真流式转发
```

该机制只影响首字节和响应头的释放时间，不等待整个响应完成，也不把完整响应读入内存。目标行为以“第一个确认成功的 chunk”作为提交点：在提交点之前可以切换账号；提交点之后保持真流式转发，不再重放 POST 或替换后续响应。达到 2 MiB 上限而仍没有语义输出时，当前实现返回 bootstrap 失败并在可能时重试；只有没有可用重试渠道时才返回代理错误。

失败情况的优先级：

1. 尚未向客户端提交响应：可以换下一个账号或渠道重试；
2. 已提交成功首段：保持当前上游连接，后续原样转发；
3. 已提交响应但上游随后失败：记录失败，不伪造另一账号的响应；
4. 所有尝试都在提交响应前失败：返回现有错误处理结果。

兼容性解析可以用于日志、usage 和状态记录，但不应因为业务事件解析而修改已经收到的响应正文。首段缓冲是为了延迟提交响应头和首字节，不是把整个响应读完后再返回。当前实现已删除语义失败耗尽后的合成 `response.failed`，会保留上游响应正文；如果首段尚未提交，仍可按既有策略重试。

当前实现的时序需要单独看待：OpenAI `/v1/responses` 成功流会先经过 bootstrap 缓冲；Anthropic 和 Gemini 流不会经过这层首段缓冲；启用 usage 记录时，小型 JSON 响应可能在 handler 返回前被完整读取。OpenAI bootstrap 在看到第一个语义输出或明确终止事件时释放响应；达到 2 MiB 上限而仍没有语义输出时返回 bootstrap 失败，不把未经确认的 200 提交给客户端。

Anthropic 的 `/v1/messages/count_tokens` 还存在一个独立例外：开启 `anthropic_count_tokens_mock_enabled` 后，请求不会发往上游，而是由本地估算 token 并返回模拟 JSON。这是显式的兼容功能，不能归入透明转发。

## 认证方式 / Authentication mode

渠道认证方式由 `Channel.auth_type` 决定。客户端传入的占位 token 不用于推断目标渠道的认证格式。

支持的明确模式为：

| 协议 | 认证模式 |
| --- | --- |
| OpenAI | `bearer` |
| Anthropic | `x-api-key` |
| Gemini | `x-goog-api-key` 或 `query-key` |

`auto` 保留现有语义：继续根据入站请求推断认证形式，并在无法推断时使用当前协议的默认形式。只有显式配置 `bearer`、`x-api-key`、`x-goog-api-key` 或 `query-key` 时，才强制使用该形式。当前 UI 已提供按协议限制的认证方式选择器，后端会规范化并校验新建/编辑值；OpenAI 远程 managed channel 继续保留 `managed_account`。数据库没有 `auth_type` 的 CHECK 约束，本次无需重写存量 `auto`/`managed_account`，因此不需要数据库迁移。

应用认证时，应先清理会泄露客户端或其他渠道凭证的同类字段，再写入当前渠道的认证字段；除此之外不修改 header。这是可预测性改进，不是安全修复。

Anthropic 当前还有一个兼容例外：入站没有 `anthropic-version` 时补 `2023-06-01`。它必须显式记录在允许的例外中，并通过 Claude CLI 与目标上游验证是否仍然需要；不能把它误认为账号切换字段。

### 允许的兼容例外 / Allowed compatibility exceptions

| 例外 | 当前行为 | 约束 |
| --- | --- | --- |
| Anthropic `anthropic-version` | 缺失时补 `2023-06-01`。 | 仅作为兼容默认值；需要独立的 Claude CLI/上游验证记录。 |
| Codex reasoning ID shim | 默认开启，仅在 `/v1/responses` 命中非法 reasoning `id` 时删除该字段并重新序列化。 | 只保留必要字段修复，不扩大到其他 body 字段；如果改默认值，需要 changelog 和替代方案。 |
| OpenAI OAuth body 适配 | 当前对 managed channel 调用最小化 Responses body 修正；合规 body 不重序列化。 | 私有端点字段必须逐项拿到可复现证据；`model` 映射若存在，作为独立渠道功能记录。 |
| OpenAI Responses bootstrap | 在首段成功或终止前暂存部分 SSE，以避免先提交 200 再立刻暴露错误。 | 不等待完整 EOF；首个成功 chunk 提交后不得切换账号，也不合成另一响应；达到 2 MiB 仍无语义输出时返回 bootstrap 失败。 |

## 验收标准 / Acceptance criteria

- 普通 API Key 渠道的请求正文、客户端 header 和成功响应按严格透明规则转发；
- 普通 API Key 渠道不会收到其他提供商的 `chatgpt-account-id` 或 Cookie 会话状态；
- OpenAI managed channel 使用按账号隔离的 host-scoped Cookie client；
- OpenAI OAuth 渠道只执行已有证据支持的最小必要适配；
- OAuth 身份字段直接透传，不产生 originator、User-Agent 和 version 的混合身份；首发请求和 401 刷新重试使用同一套规则；
- 如果客户端提供 `session-id` 或 `session_id`，代理不因另一别名缺失而生成随机值；
- reasoning ID 清理保持默认开启，并且只检查 `/v1/responses`；
- Codex Responses 在首个确认成功 chunk 交付前可以重试，交付后保持真流式且不换账号；
- 不因 SSE/JSON 解析而重写成功响应正文；
- 认证格式由 `auth_type` 配置决定，`auto` 保持入站推断语义；
- Anthropic `anthropic-version` 的补充行为作为显式兼容例外，具备独立的上游验证记录；
- OpenAI OAuth 仅支持 `/v1/responses` 的限制在文档和错误行为中保持一致；
- 管理 API 的响应处理（例如 `Content-Language`）不影响 `/v1` 代理响应；
- 如果未来再次新增或调整 OAuth 身份/body 适配，必须先使用真实 Codex CLI 完成一次流式请求，并使用至少一个非 Codex 客户端验证同一 OAuth 渠道的结果；本次移除身份伪造不以本地 mock 冒充真实上游验证。

## 证据与验证状态 / Evidence and verification status

- reasoning ID shim 的默认开启和 API Key/OAuth/故障转移共用路径，来源是仓库合并的 [PR #223](https://github.com/koumoe/cli-switch/pull/223)；
- 身份覆盖只是 [commit `65a5683`](https://github.com/koumoe/cli-switch/commit/65a5683eeb3404e9557f433f5255ab6df3d9b1f9) 的一部分，该提交同时包含 Cookie 客户端、额度和更新检测等改动；
- 截至 2026-09-12，公开 Codex 提交 [`53c542d`](https://github.com/openai/codex/tree/53c542d944c705f3a66780a19223223bee57cbb6) 的 [session header builder](https://github.com/openai/codex/blob/53c542d944c705f3a66780a19223223bee57cbb6/codex-rs/codex-api/src/requests/headers.rs) 使用 `session-id` 和 `thread-id` 连字符形式；这是外部来源，不是本仓库现状，修改 alias 逻辑前仍应保留回归测试；
- 同一提交的 Codex [default client](https://github.com/openai/codex/blob/53c542d944c705f3a66780a19223223bee57cbb6/codex-rs/login/src/auth/default_client.rs) 负责 `originator` 和 User-Agent；本仓库关于私有端点校验 `version` 的依据来自历史兼容代码和实测经验，不能当作公开 API 契约；
- 同一公开 Codex 提交的 [`build_responses_request`](https://github.com/openai/codex/blob/53c542d944c705f3a66780a19223223bee57cbb6/codex-rs/core/src/client.rs) 会生成 `stream=true`、`store=false`、`parallel_tool_calls`、`include=["reasoning.encrypted_content"]` 等字段；这支持保留这些字段的兼容依据，但不自动证明私有端点对所有删除字段的要求；
- 当前 CI 和仓库集成测试使用本地 mock，没有真实 `chatgpt.com/backend-api/codex` 验证。当前实现已删除身份伪造；任何重新引入 OAuth 身份或 body 适配的变更都必须补充真实 Codex CLI 流式请求和至少一个非 Codex 客户端的人工验证记录。
