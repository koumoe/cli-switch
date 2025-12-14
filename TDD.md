# CliSwitch 技术设计文档（TDD）

更新时间：2025-12-15 00:20 CST  
版本：v0.1（草案，对应 PRD v0.1）  

---

## 1. 结论与约束

### 1.1 后端技术选型：Rust 1.92.0

选择理由（对应本项目的“本地流式代理 + 故障转移 + 单文件分发”核心诉求）：

- **流式代理更可控**：基于 `hyper`/`axum` 可做到端到端 body 流式透传，便于实现“未输出前可 failover、已输出后不可 failover”的规则。
- **单文件分发更简单**：Rust 静态链接与资源内嵌更成熟，便于做“单可执行文件 + 内嵌前端静态资源 + 内嵌 SQLite（bundled）”。
- **跨平台一致性**：同一套代码可覆盖 macOS/Windows/Linux，后续做开机自启/托盘常驻也有可选生态（但不在 MVP）。

### 1.2 前端：Node.js 25.2.1（构建期）

- 前端仅在开发/构建阶段需要 Node。
- 发布时前端产物（静态文件）会被内嵌进 Rust 二进制，运行时不依赖 Node。

### 1.3 关键约束/未知点

- **本地是否必须 HTTPS**：你暂不确定；MVP 默认以 `http://127.0.0.1:<port>` 设计，预留 HTTPS 扩展点（自签/用户证书/本机信任）。
- **成功响应一定包含 token 用量**：你已确认该假设；实现上仍会做字段兼容与兜底（避免字段名差异导致统计丢失）。

---

## 2. 总体架构

单进程单可执行文件 `cliswitch`，同时提供：

1) **代理入口（Proxy Inbound）**：给 CLI 指向的本地 API Base URL  
2) **管理 API（Admin API）**：给 Web UI 调用  
3) **Web UI 静态站点（UI）**：内嵌静态资源，浏览器访问  
4) **后台任务（Jobs）**：价格同步、清理/归档（可选）

### 2.1 端口与绑定

- 默认监听：`127.0.0.1:3210`（可配置）。
- UI 与 Admin API 与 Proxy Inbound 共用同一端口，用不同路径区分。

---

## 3. 协议与入口设计（Inbound）

> 目标：让不同 CLI “以它们原生协议”访问本机服务，尽量少做语义转换。

### 3.1 路径分区

- OpenAI 兼容入口：`/v1/*`（覆盖 chat/completions、responses 等）
- Anthropic 入口：`/v1/messages`
- Gemini 入口：`/v1beta/*`（以及必要时的 `/v1/*`）

说明：
- 路径设计以 CLI 原生调用路径为准，尽量透传，减少配置复杂度。
- 协议区分通过请求头（如 `x-api-key` vs `Authorization`）或请求体字段判断，而非路径前缀。

### 3.2 健康检查

- `GET /api/health`：用于 shim/wrapper 与 UI 检测服务是否可用。

---

## 4. 路由、重试与熔断（Router）

### 4.1 基本数据结构

- `Channel`（渠道）：一个上游出口（平台 + base_url + key + 协议）
- `Route`（路由）：一组匹配条件 + 渠道列表（带优先级） + 重试策略

MVP 建议路由粒度：
- 先按 **协议** 路由（openai/anthropic/gemini）；
- 模型级路由作为扩展（可在同协议内按 `model` 字段匹配）。

### 4.2 Failover 规则

请求处理流程（简化）：

1) 读入并缓存请求体（用于多次重试重放；需设置最大体积上限）。  
2) 根据入口协议 +（可选）模型匹配 route。  
3) 获取 route 下按优先级排序的 channel 列表。  
4) 依次尝试 channel：
   - 网络错误 / 超时 / 429 / 5xx / 504 等 → 进入下一 channel
   - 400 / 401 / 403 / 404 等 → 直接失败（不再扩散到其他 channel）
5) 成功后将响应（含流式）回传给 CLI。

流式特殊规则：
- **仅在尚未向客户端写出任何字节前允许 failover**。
- 一旦开始向客户端输出（流式首块已发送），即使上游中断也不再切换，直接报错结束该请求。

### 4.3 熔断/冷却（简版）

每个 channel 维护：

- `consecutive_failures`
- `cooldown_until`

当连续失败达到阈值（例如 3 次）：
- 进入冷却期（例如 60s），冷却期内路由选择时跳过该 channel。

冷却到期后：
- 允许再次尝试，若成功则清零失败计数。

### 4.4 重试幂等性与请求体重放

- 为了支持 failover，**必须缓存请求体**（JSON 请求一般可接受）。
- 需要限制最大体积（例如 16MB/32MB，具体实现可配置），超出则：
  - 可选择“禁用 failover（只尝试首个 channel）”或直接拒绝（返回 413）。

---

## 5. 上游转发与流式透传（Outbound Proxy）

### 5.1 头与路径处理

- 原样保留大部分 headers（User-Agent、Accept、Content-Type 等）。
- 需要剔除/重写的 headers：
  - `Host`：按上游 host 重写
  - `Content-Length`：若请求体被重新构造，需重算或交由库处理
  - `Connection` 等 hop-by-hop headers：按 RFC 处理

### 5.2 鉴权注入

channel 配置包含鉴权类型：

- OpenAI 兼容：`Authorization: Bearer <token>`（或上游要求的其他 header）
- Anthropic：可能是 `x-api-key` 或 `Authorization`（以 channel 配置为准）
- Gemini：可能是 header 或 query key（以 channel 配置为准）

实现策略：
- **忽略 CLI 自带的 key**（CLI 的 key 作为“本地占位”），转发时按 channel 注入真实上游 key。
- 若未来需要“按 CLI key 选择 channel”，再扩展（MVP 不做）。

### 5.3 流式透传与边读边解析

目标：
- CLI 侧实时收到流式输出；
- 服务端在不阻塞转发的前提下，尽力解析出：
  - `model`
  - `usage`（input/output tokens）

实现要点：
- 响应 body 以 stream 方式从上游读出并写给下游；
- 同时在 stream map 中做轻量解析（例如 SSE `data:` 行 JSON）。
- 解析只做“增量、容错”，不应因解析失败而影响转发。

---

## 6. 用量统计与计费

### 6.1 计费口径

- **仅成功请求计费**：失败请求不扣费（费用为 0 或 NULL）。
- token 用量来自上游成功响应中的 usage 字段（字段名可能因协议不同而不同）。

### 6.2 价格来源（OpenRouter）

- 从 `https://openrouter.ai/api/v1/models` 同步模型价格。
- 你已确认该接口可访问且包含 `pricing` 字段（例如 `prompt`/`completion` 等）。

同步策略（MVP）：
- 服务启动后立即同步一次（失败不影响服务启动）。
- 定时同步（例如每 12 小时；可配置）。
- 将价格写入 SQLite，并更新内存缓存（可选）。

### 6.3 费用计算

在写入 `usage_events` 前或后（两阶段均可）：

1) 从 usage 得到 `prompt_tokens` 与 `completion_tokens`（字段名因协议不同而映射）。  
2) 查找 `pricing_models` 中对应 `model_id` 的单价：`prompt_price`、`completion_price`。  
3) 计算：  
   - `estimated_cost = prompt_tokens * prompt_price + completion_tokens * completion_price`  
4) 写入/更新到 `usage_events.estimated_cost_usd`。

> 若 `model_id` 无法在价格表中找到：费用置为 NULL，并在 UI 标记“无价格数据”。

---

## 7. SQLite 数据设计（定稿建议）

> MVP 优先保证可用与可迁移；后续可加 materialized 聚合表。

### 7.1 表结构

`channels`

- `id` TEXT PRIMARY KEY（UUID）
- `name` TEXT NOT NULL
- `protocol` TEXT NOT NULL CHECK(protocol IN ('openai','anthropic','gemini'))
- `base_url` TEXT NOT NULL
- `auth_type` TEXT NOT NULL（例如 'bearer','x-api-key','query'）
- `auth_ref` TEXT NOT NULL（引用本机安全存储；MVP 可先明文存但强烈不建议）
- `enabled` INTEGER NOT NULL
- `created_at_ms` INTEGER NOT NULL
- `updated_at_ms` INTEGER NOT NULL

`routes`

- `id` TEXT PRIMARY KEY（UUID）
- `name` TEXT NOT NULL
- `protocol` TEXT NOT NULL
- `match_model` TEXT NULL（MVP 可空）
- `enabled` INTEGER NOT NULL
- `created_at_ms` INTEGER NOT NULL
- `updated_at_ms` INTEGER NOT NULL

`route_channels`

- `route_id` TEXT NOT NULL
- `channel_id` TEXT NOT NULL
- `priority` INTEGER NOT NULL（小值优先）
- `cooldown_until_ms` INTEGER NULL
- PRIMARY KEY (`route_id`,`channel_id`)

`pricing_models`

- `model_id` TEXT PRIMARY KEY
- `prompt_price` TEXT NULL（用字符串存 decimal，避免浮点误差）
- `completion_price` TEXT NULL
- `request_price` TEXT NULL
- `raw_json` TEXT NULL
- `updated_at_ms` INTEGER NOT NULL

`usage_events`

- `id` TEXT PRIMARY KEY（UUID）
- `ts_ms` INTEGER NOT NULL
- `protocol` TEXT NOT NULL
- `route_id` TEXT NULL
- `channel_id` TEXT NOT NULL
- `model` TEXT NULL
- `success` INTEGER NOT NULL
- `http_status` INTEGER NULL
- `error_kind` TEXT NULL
- `latency_ms` INTEGER NOT NULL
- `prompt_tokens` INTEGER NULL
- `completion_tokens` INTEGER NULL
- `total_tokens` INTEGER NULL
- `estimated_cost_usd` TEXT NULL

### 7.2 索引

- `CREATE INDEX idx_usage_ts ON usage_events(ts_ms);`
- `CREATE INDEX idx_usage_channel_ts ON usage_events(channel_id, ts_ms);`
- `CREATE INDEX idx_usage_success_ts ON usage_events(success, ts_ms);`

---

## 8. 管理 API（Admin API）

前缀：`/api`

### 8.1 Channels

- `GET /api/channels`
- `POST /api/channels`
- `PUT /api/channels/:id`
- `POST /api/channels/:id/enable`
- `POST /api/channels/:id/disable`
- `POST /api/channels/:id/test`（连通性/鉴权测试：返回耗时与状态）

### 8.2 Routes

- `GET /api/routes`
- `POST /api/routes`
- `PUT /api/routes/:id`
- `POST /api/routes/:id/channels/reorder`（提交 route 下渠道优先级）

### 8.3 Pricing

- `POST /api/pricing/sync`
- `GET /api/pricing/models?query=...`
- `GET /api/pricing/status`（上次同步时间、条目数）

### 8.4 Stats

- `GET /api/stats/summary?range=today|month`
- `GET /api/stats/channels?range=today|month`

统计口径：
- `today`：按本机本地时区的“今日 00:00:00 至现在”
- `month`：按本机本地时区的“本月 1 日 00:00:00 至现在”

实现建议：
- 计算“本地日/月起点”的 UNIX ms，再在 SQL 里做 `WHERE ts_ms >= :start_ms`。

### 8.5 接入向导（无感配置）

- `POST /api/connect/claude`：写入 Claude Code 配置（见 9.1）
- `POST /api/connect/codex`：写入 Codex CLI 配置（见 9.2）
- `POST /api/connect/gemini`：写入 Gemini CLI 配置（见 9.3）
- `POST /api/connect/rollback`：回滚最近一次修改（或按工具生成的备份文件恢复）

---

## 9. 无感接入实现（跨平台）

你给的 DuckCoding 文档提供了明确的配置入口；CliSwitch MVP 采用“写配置文件 + 备份可回滚”的方式实现尽量无感。

### 9.1 Claude Code（@anthropic-ai/claude-code）

配置文件（优先级建议：用户目录）：

- macOS/Linux：`~/.claude/settings.json`
- Windows：`%USERPROFILE%\\.claude\\settings.json`

写入/更新字段：

```json
{
  "env": {
    "ANTHROPIC_AUTH_TOKEN": "<本地占位或本地访问令牌>",
    "ANTHROPIC_BASE_URL": "http://127.0.0.1:3210"
  }
}
```

说明：
- `ANTHROPIC_AUTH_TOKEN` 在 MVP 可作为占位（CLI 可能要求非空）；是否作为“本地鉴权令牌”可后续增强。

### 9.2 Codex CLI（@openai/codex）

配置目录：

- macOS/Linux：`~/.codex/`
- Windows：`%USERPROFILE%\\.codex\\`

`config.toml` 关键项：

- `base_url = "http://127.0.0.1:3210/v1"`
- 其他字段（如 `wire_api`）保持用户原配置或按向导写入默认值（以你的环境验证为准）

`auth.json`：

```json
{ "OPENAI_API_KEY": "<本地占位或本地访问令牌>" }
```

### 9.3 Gemini CLI（@google/gemini-cli）

配置目录：

- macOS/Linux：`~/.gemini/`
- Windows：`%USERPROFILE%\\.gemini\\`

`.env`：

```
GOOGLE_GEMINI_BASE_URL=http://127.0.0.1:3210
GEMINI_API_KEY=<本地占位或本地访问令牌>
GEMINI_MODEL=<保留用户原值或向导默认值>
```

`settings.json`：保持 `selectedType = "gemini-api-key"`（按文档）。

### 9.4 备份与回滚策略

- 每次写入前先备份：
  - 例如 `settings.json.cliswitch.bak.<ts_ms>`
  - 或在 `~/.cliswitch/backups/` 下按路径镜像存放
- 在 DB 记录最近一次写入的变更清单（文件路径、备份路径、ts_ms），用于“一键回滚”。

---

## 10. Web UI（Node 构建，静态内嵌）

### 10.1 页面与数据流

- Dashboard：`/api/stats/summary`
- Channels：`/api/channels` + `/api/stats/channels`
- Routes：`/api/routes`
- Pricing：`/api/pricing/status` + `/api/pricing/models`
- Connect：调用 `/api/connect/*` 完成无感接入

### 10.2 静态资源内嵌

发布构建：
- `ui` 构建出 `ui/dist/`
- Rust 编译时将 `ui/dist/` 内嵌进二进制（例如 `rust-embed` 或 `include_dir`）
- 运行时由后端静态文件 handler 提供：
  - `/` 返回 `index.html`
  - 其余静态资源按路径返回

---

## 11. 单可执行文件打包与发布

MVP 输出物：
- macOS：`cliswitch`
- Windows：`cliswitch.exe`
- Linux：`cliswitch`

构建流程（建议）：

1) `ui`：`npm ci && npm run build`
2) `backend`：`cargo build --release`
3) 输出：`target/release/cliswitch*`（单文件）

跨平台分发：
- MVP 不强制安装器；后续做开机自启/托盘时再评估安装包（msi/pkg/deb/rpm）。

---

## 12. 测试与验证（建议）

- 单元测试：
  - 路由选择（priority、熔断、可重试/不可重试判定）
  - 价格解析（OpenRouter pricing 字段兼容）
  - 用量解析（不同字段名映射）
- 集成测试（本地）：
  - 启动服务后用 curl 模拟 `/v1/*` 请求
  - 模拟上游 429/5xx/超时，验证 failover
  - 流式转发：用简单 SSE 上游 mock 验证不阻塞

---

## 13. 后续扩展点（非 MVP）

- 本地 HTTPS：自签证书生成与信任引导
- 开机自启：
  - macOS：LaunchAgent
  - Windows：Task Scheduler
  - Linux：systemd user service
- 托盘常驻：托盘菜单（打开 UI / 启停服务 / 查看今日 token）
- 多维度统计：按项目/目录/仓库/机器等标签

