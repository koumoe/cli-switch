# CliSwitch（暂定名）产品需求文档 PRD

更新时间：2025-12-15 00:19 CST  
版本：v0.1（草案）  
目标平台：本地桌面环境（macOS / Windows / Linux）  
前端构建：Node.js 25.2.1（仅构建期）  
后端运行：Rust 1.92.0（单可执行文件）  
数据存储：SQLite（本地）  

---

## 1. 背景与问题

用户同时使用多种 AI CLI（如 Claude Code / Codex CLI / Gemini CLI），并希望：

1) 统一把这些 CLI 的 API 请求指向本机端口；  
2) 由本机服务按“渠道优先级 + 重试/故障转移”规则转发到不同上游；  
3) 记录 token 使用量并按模型价格估算成本；  
4) 提供可视化界面管理渠道/路由/统计；  
5) 日常使用“无感”：用户继续使用原 CLI 命令，不需要每次手动改配置。

---

## 2. 目标（Goals）

### 2.1 MVP（v0.x）必须达成

- **本地代理服务**：对外监听一个本地端口，支持流式与非流式转发。
- **3 类 CLI 入口适配**：支持 Claude Code / Codex CLI / Gemini CLI 的“可配置 API Base URL/Endpoint”的场景（具体参数/路径以实际版本为准）。
- **多渠道管理**：支持不同平台不同 Key，以及同平台多 Key（均作为“不同渠道”管理）。
- **按优先级重试/故障转移**：可配置的优先级与可重试错误策略。
- **SQLite 持久化**：渠道、路由规则、价格缓存、用量统计等落库。
- **可视化管理界面**：Web UI（静态前端 + 本地后端 API）。
- **价格同步**：从 `https://openrouter.ai/api/v1/models` 拉取并缓存模型价格，用于估算费用。
- **用量展示**：界面显示“今日 token + 预估价格”；每个渠道显示“当日/当月统计”。
- **无感使用（尽可能）**：提供 shim/wrapper 机制，让用户仍使用原命令（如 `claude`/`codex`/`gemini`），自动注入本地 endpoint 并确保本地服务在运行。

### 2.2 后续（非 MVP）

- **开机自启**、**托盘常驻**（跨平台桌面化能力）。
- 统计维度扩展：按项目/目录/仓库/机器等更多维度切分。
- 更强的协议互转（例如将 Anthropic/Gemini 请求转换成 OpenAI 兼容以打到统一上游）——若确有需求再评估。
- 多用户/远程访问（默认不做，避免安全复杂度）。

---

## 3. 非目标（Non-goals）

- 不承诺所有版本/所有分支的 CLI 都能“零配置”接入：不同 CLI 的配置方式差异较大，需要以用户实际使用版本验证。
- 不在 MVP 做复杂的内容审计/脱敏/提示词存储（默认不落 prompts）。
- 不在 MVP 做云端同步或多端共享配置。

---

## 4. 用户画像与核心场景

### 4.1 用户画像

- 重度 CLI 用户：在终端/IDE 中调用 AI 辅助编码与问答。
- 有多 Key/多平台需求：有配额限制、网络质量差异、费用差异、稳定性差异。
- 需要成本可见：希望知道每天/每月大概花费、哪个渠道更划算/更稳定。

### 4.2 核心使用流程（MVP）

1) 用户运行 `cliswitch`（启动本地服务 + 打开管理 UI）。  
2) 在 UI 中添加渠道（平台、base_url、鉴权方式、优先级、启停）。  
3) 在 UI 中创建路由规则（例如：对某协议/某模型使用哪些渠道，按优先级）。  
4) 执行 `cliswitch shim install`（一次性安装 wrapper，让原命令无感接入）。  
5) 用户照常使用 `claude/codex/gemini`；请求会经本地转发、失败会按优先级切换；用量与费用在 UI 中可见。

---

## 5. 名词定义

- **协议（Protocol）**：请求/响应语义与路径风格（OpenAI 兼容 / Anthropic / Gemini）。  
- **渠道（Channel）**：一个可用的上游出口（某平台 + 某 Key + 某 base_url + 其他参数）。同平台多 Key 也算不同渠道。  
- **路由（Route）**：把某类请求（按协议/模型/其他条件）路由到一组渠道，并定义优先级、重试策略。  
- **用量事件（Usage Event）**：一次请求结束后可得的 token 统计与耗时等元数据。  
- **价格缓存（Pricing Cache）**：从 OpenRouter models 接口获取的模型价格（prompt/completion 等）。

---

## 6. 功能需求（Functional Requirements）

### 6.1 本地代理服务（Core Proxy）

- 监听地址默认 `127.0.0.1`，避免外部访问；端口可配置（默认 3210）。
- 支持以下能力：
  - HTTP 请求转发（保持 headers/状态码/体）。
  - **流式转发**：SSE / chunked 等按原样透传；同时在不阻塞输出的前提下提取必要统计信息。
  - 超时控制（连接超时、首包超时、整体超时）可配置。
  - 并发请求处理。
  - 请求 ID 贯穿（用于日志与排障）。

验收标准：
- 非流式：请求在本地->上游->本地->CLI 往返正确，响应字段不被破坏。
- 流式：CLI 能实时收到增量输出，不出现明显卡顿或乱序。

### 6.2 入口适配（CLI 协议兼容）

在同一端口提供多组入口路径（具体以实际 CLI 需要为准）：

- OpenAI 兼容入口：`/v1/*`（至少覆盖常见的 chat/completions / responses 等；最终以 CLI 实际调用为准）
- Anthropic 入口：`/v1/messages`
- Gemini 入口：`/v1beta/*`（以及必要时的 `/v1/*`）

说明：
- MVP 以“尽量透传”为主：同协议请求优先转发到同协议渠道，避免复杂互转。
- 若未来要实现跨协议 failover（例如 Claude 请求失败转 Gemini），需要明确转换规则与语义差异，先不纳入 MVP。

### 6.3 渠道管理（Channels）

每个渠道至少包含：

- `name`：显示名
- `protocol`：openai / anthropic / gemini（以及未来扩展）
- `base_url`：上游地址
- `auth`：鉴权配置（如 Bearer token / x-api-key / query key 等，按协议）
- `enabled`：启用开关
- `priority`：在路由中的优先级（或由 route-channel 关联表提供）
- 可选：附加 headers（如 OpenRouter 的 `HTTP-Referer`、`X-Title`）、请求限流、超时覆盖、代理设置（HTTP proxy）等

验收标准：
- UI 可新增/编辑/禁用/删除渠道（删除需二次确认）。
- 可对单个渠道做“连通性测试”（例如发起一个轻量探测请求或校验鉴权）。

### 6.4 路由与重试（Routing & Failover）

路由策略（MVP）：

- 按“协议”划分路由：OpenAI 入口只选择 openai 协议渠道；Anthropic 入口只选 anthropic 渠道；Gemini 同理。
- 在同协议内：
  - 按优先级依次尝试渠道。
  - 对可重试错误才切换下一渠道；不可重试错误直接返回。

可重试错误建议（可配置）：

- 网络类：连接失败、DNS 失败、TLS 握手失败、读写超时、上游无响应。
- HTTP 类：`429`、`500`、`502`、`503`、`504`。

不可重试建议：

- `400`（请求参数问题）、`401/403`（鉴权问题）、`404`（路径问题）等。

流式重试规则：

- **仅在尚未向客户端写出任何响应字节前允许 failover**。
- 一旦开始向客户端输出流内容，即使上游中途断开，也只返回中断错误，不再切换（避免重复/语义错乱）。

熔断/冷却（建议 MVP 即做简版）：

- 渠道连续失败达到阈值后进入冷却期，冷却期内跳过该渠道。

验收标准：
- 人为制造上游 429/5xx/超时后，能自动切换到下一个优先级渠道。
- 401/参数错误不应盲目切换（避免把同样错误扩散到所有渠道）。

### 6.5 价格同步与费用估算（Pricing & Cost）

- 后端定时从 `https://openrouter.ai/api/v1/models` 拉取模型列表与 `pricing` 字段并缓存到 SQLite。
- 价格缓存字段至少包含：
  - `model_id`
  - `prompt_price_per_token`（或按接口原始字段存储）
  - `completion_price_per_token`
  - `request_price`（若存在）
  - `updated_at`
- 费用估算：
  - 对每次请求记录的 `prompt_tokens`、`completion_tokens`，按对应价格计算 `estimated_cost_usd`。
  - **仅对成功请求计费**：请求失败时不计算费用（`estimated_cost_usd = 0` 或 `NULL`，以 UI 展示策略为准）。
  - 依赖上游在成功响应中返回 token 用量；若缺失则暂记为 `unknown`（保留后续做“估算 token”的扩展空间）。

验收标准：
- 能在 UI 中看到价格更新时间；能按 token 数得到一致的费用计算结果（允许因缺失 usage 而为 N/A）。

### 6.6 用量与统计（Usage & Stats）

必须统计维度：

- 全局：今日 token、今日预估费用。
- 按渠道：当日/当月（请求数、成功率、tokens、费用、延迟统计）。

记录粒度（建议）：

- 每次请求完成后写入一条 usage 事件：
  - 时间戳、协议、渠道、模型、成功/失败、错误码、耗时
  - `prompt_tokens`、`completion_tokens`、`total_tokens`（若可得）
  - `estimated_cost_usd`（若可得）

验收标准：
- UI 统计与 DB 聚合结果一致；跨天后“今日/当月”能正确滚动。

### 6.7 可视化管理界面（Web UI）

页面（MVP）：

- Dashboard：今日 tokens / 今日费用；近 7 天趋势（可选）。
- Channels：列表/新增/编辑/启停/测试；显示当日/当月概要。
- Routes：为不同协议/模型配置渠道优先级与策略（先做协议级，模型级可选）。
- Pricing：OpenRouter 同步状态、搜索模型、查看单价；允许对特定模型做“手动覆盖价”（可选）。
- Logs：最近失败请求、错误原因、耗时（默认不展示 prompts）。

交互要求：

- “一眼可见”当前默认路由与最优先渠道。
- 编辑保存后即时生效，不需要重启服务。

---

## 7. 无感接入方案（Shim/Wrapper）

目标：用户仍然输入原命令（`claude`/`codex`/`gemini`），无需每次手动配置 endpoint，做到“一次接入后长期可用”。

方案（MVP，推荐优先级从高到低）：

- **A. 一键写入各 CLI 的官方配置文件**（更“无感”，不依赖 PATH）  
  - 在 UI 中提供“接入向导”，自动写入/更新以下配置项，使 CLI 直接指向本机端口。  
  - 同时提供“恢复/回滚”能力：对被修改的文件做备份并可一键还原。
- **B. `cliswitch shim install` wrapper**（不改配置文件的备选）  
  - 在用户目录下创建 `~/.cliswitch/bin`（Windows 为对应目录），生成同名 wrapper（`claude/codex/gemini`）。  
  - wrapper 检测本地服务存活（`/api/health`）；若未启动则拉起 `cliswitch daemon`；并注入 endpoint。  
  - 提示用户把该目录放到 PATH 最前（或提供 shell/终端配置向导）。
- `cliswitch shim uninstall`：移除 wrapper；若使用方案 A，还应支持回滚配置文件改动。

验收标准：
- 安装后执行 `claude` 等命令时，会自动走本地代理；不需要手动改 CLI 配置文件（在可行前提下）。

风险/未知：
- 不同 CLI 对 endpoint 配置项的名称/优先级不同（环境变量 vs 配置文件 vs 参数），需要以实际版本验证。
- 本地是否必须 HTTPS 取决于 CLI 的约束与用户环境；MVP 默认以 HTTP localhost 为主，HTTPS 作为可选项评估实现。

### 7.1（附录）已知的 CLI 配置入口（参考你提供的 DuckCoding 文档）

> 说明：以下为“配置字段与文件位置”的已知模式，用于 CliSwitch 的接入向导；最终以你实际安装的 CLI 版本表现为准。

- Claude Code
  - 配置文件：`~/.claude/settings.json`（以及可能存在的 `.claude/settings.json`）
  - 关键字段（env）：`ANTHROPIC_AUTH_TOKEN`、`ANTHROPIC_BASE_URL`
- Codex CLI
  - 配置目录：`~/.codex/`
  - 配置文件：`config.toml`（包含 `base_url` 等）、`auth.json`（包含 `OPENAI_API_KEY`）
  - 常见行为：以 OpenAI `/v1` 兼容方式调用（例如 `wire_api = "responses"`）
- Gemini CLI
  - 配置目录：`~/.gemini/`
  - 配置文件：`.env`（包含 `GOOGLE_GEMINI_BASE_URL`、`GEMINI_API_KEY`、`GEMINI_MODEL`）、`settings.json`（选择 `gemini-api-key`）

---

## 8. 系统架构（技术方案概要）

### 8.1 模块划分（Rust）

- `server`：HTTP 入口（代理入口 + 管理 API + 静态站点）
- `protocol_openai` / `protocol_anthropic` / `protocol_gemini`：各协议的转发与（必要的）usage 解析
- `router`：路由选择、重试、熔断、超时
- `storage`：SQLite schema/migrations、读写、聚合查询
- `pricing`：OpenRouter models 同步与缓存
- `shim`：生成/管理 wrapper
- `ui_embed`：前端产物嵌入与静态资源服务

### 8.2 前端技术（建议）

- Vite + React（或 Svelte）构建静态站点。
- 与后端通过 `http://127.0.0.1:<port>/api/*` 通信。

---

## 9. 数据设计（SQLite 草案）

> 字段以实现时为准；此处给出 MVP 最小集合。

### 9.1 tables

- `channels`
  - `id` (pk)
  - `name`
  - `protocol`
  - `base_url`
  - `auth_ref`（引用本地安全存储；避免明文 key 入库）
  - `enabled`
  - `created_at`, `updated_at`

- `routes`
  - `id` (pk)
  - `name`
  - `protocol`
  - `match_model`（可空；MVP 可不做模型级）
  - `enabled`

- `route_channels`
  - `route_id`
  - `channel_id`
  - `priority`（小值优先）
  - `cooldown_until`（可选）

- `pricing_models`
  - `model_id` (pk)
  - `prompt_price`
  - `completion_price`
  - `request_price`
  - `raw_json`（可选，便于兼容字段变动）
  - `updated_at`

- `usage_events`
  - `id` (pk)
  - `ts`
  - `protocol`
  - `route_id`（可空）
  - `channel_id`
  - `model`
  - `success`
  - `http_status`（可空）
  - `error_kind`（可空：timeout/auth/upstream/…）
  - `latency_ms`
  - `prompt_tokens`（可空）
  - `completion_tokens`（可空）
  - `total_tokens`（可空）
  - `estimated_cost_usd`（可空）

### 9.2 索引

- `usage_events(ts)`
- `usage_events(channel_id, ts)`
- `usage_events(model, ts)`

---

## 10. 管理 API（示例草案）

前缀：`/api`

- `GET /api/health`
- `GET /api/channels`
- `POST /api/channels`
- `PUT /api/channels/:id`
- `POST /api/channels/:id/test`
- `GET /api/routes`
- `POST /api/routes`
- `PUT /api/routes/:id`
- `POST /api/routes/:id/reorder`（调整优先级）
- `GET /api/pricing/models?query=...`
- `POST /api/pricing/sync`
- `GET /api/stats/summary?range=today|month`
- `GET /api/stats/channels?range=today|month`

安全策略（MVP）：
- 仅监听 `127.0.0.1`；不做登录。
- 若未来支持远程访问，再引入 auth。

---

## 11. 非功能需求（NFR）

- **性能**：本机转发开销可控；流式不明显卡顿。
- **稳定性**：进程崩溃后重启不丢配置；DB 迁移可靠。
- **可观测性**：日志级别可配置；默认不记录敏感内容。
- **安全**：
  - 默认不把 API Key 明文写入 SQLite。
  - 默认只绑定 localhost。
  - 提供“清理统计数据/导出配置”的能力（导出不含密钥）。

---

## 12. 里程碑建议（可调整）

- M0：协议与 CLI 接入验证（确认 3 个 CLI 的 endpoint 注入方式与必要路径）
- M1（MVP）：本地代理 + 渠道/路由 + 重试熔断 + SQLite + UI + 价格同步 + 今日/当月统计
- M2：无感 shim 完善（多 shell 支持、诊断、自动拉起守护）
- M3：开机自启 + 托盘常驻（桌面化能力）
- M4：多维度统计（项目/目录等）与扩展功能

---

## 13. 风险与待确认事项

必须确认（影响实现细节）：

1) 是否需要本地 HTTPS（有些客户端可能强制 https；MVP 默认 http://127.0.0.1）。  
2) 你实际使用的 `claude/codex/gemini` CLI 版本是否与当前配置入口一致（字段名/文件位置可能随版本变化）。  
3) 流式场景下 usage/token 是否稳定可得（部分实现可能需要显式开启 include_usage；若缺失则无法准确计费）。  
4) 跨平台桌面化（开机自启/托盘）落地方式与分发形态（纯二进制 vs 安装包）需要后续明确。  

---

## 14. 验收清单（MVP）

- [ ] `cliswitch` 单文件启动后可访问 UI，并能新增/编辑渠道与路由
- [ ] OpenAI/Anthropic/Gemini 至少各完成一次非流式与流式请求转发
- [ ] 某渠道故障（超时/429/5xx）时按优先级自动切换
- [ ] SQLite 中可查到 usage_events；UI 展示今日 token 与预估费用
- [ ] UI 可按渠道展示当日/当月统计
- [ ] 价格可从 OpenRouter models 同步并用于费用计算
