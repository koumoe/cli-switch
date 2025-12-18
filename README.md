# CliSwitch

[![License: AGPL v3](https://img.shields.io/badge/License-AGPL%20v3-blue.svg)](https://www.gnu.org/licenses/agpl-3.0)
[![CI](https://github.com/koumoe/cli-switch/actions/workflows/ci.yml/badge.svg)](https://github.com/koumoe/cli-switch/actions/workflows/ci.yml)
[![Rust](https://img.shields.io/badge/Rust-1.92.0-orange.svg)](https://www.rust-lang.org/)
[![Node.js](https://img.shields.io/badge/Node.js-25.2.1-green.svg)](https://nodejs.org/)

本地多渠道 AI CLI 代理服务，支持 Claude Code / Codex CLI / Gemini CLI 的无感接入、故障转移与用量统计。

## 功能特性

- **多 CLI 支持**：统一代理 Claude Code、Codex CLI、Gemini CLI
- **多渠道管理**：支持同平台多 Key、不同平台混合配置
- **智能路由**：按优先级自动重试与故障转移
- **用量统计**：记录 token 使用量，基于 OpenRouter 价格估算费用
- **可视化界面**：Web UI 管理渠道、路由与统计
- **单文件分发**：Rust 编译为单可执行文件，内嵌前端资源

## 快速开始

### 环境要求

| 依赖 | 版本 | 说明 |
|------|------|------|
| Rust | 1.92.0 | 运行时 |
| Node.js | 25.2.1 | 仅构建期 |

### 安装

```bash
# 克隆项目
git clone https://github.com/koumoe/cli-switch.git
cd cli-switch

# 构建前端
cd ui
npm ci
npm run build
cd ..

# 构建桌面端（单可执行文件，默认启用 `desktop`）
cargo build --release

# 更短的命令（见 `.cargo/config.toml`）
# cargo gui-build
```

### 下载预编译（Release）

在 GitHub Releases 下载对应平台的包：

- macOS：`CliSwitch-<version>-macos-<arch>.zip`，解压后双击 `CliSwitch.app`（不会启动/常驻终端）
- Windows：`CliSwitch-<version>-windows-<arch>.zip`，解压后双击 `CliSwitch.exe`（不会弹出终端窗口）

### 运行

```bash
# 桌面端（构建时启用了 `desktop` feature）：直接运行即可（默认端口 3210）
./target/release/cliswitch

# Web 管理界面（serve，默认端口 3210，会打开浏览器）
./target/release/cliswitch serve

# 如需改端口
./target/release/cliswitch serve --port 4000
./target/release/cliswitch desktop --port 4000
```

桌面端会在窗口里打开管理界面；Web 模式访问 http://127.0.0.1:3210。
`serve` 默认会自动打开浏览器；如不需要可加 `--no-open`。

### macOS：避免双击出现终端窗口

macOS 上如果你直接在 Finder 里双击 `cliswitch` 可执行文件，系统会用「终端」来启动它，因此会常驻一个终端窗口。
需要以 `.app` 形式打包后再启动：

```bash
chmod +x scripts/bundle-macos-app.sh
./scripts/bundle-macos-app.sh
open dist/macos/CliSwitch.app
```

## 开发

### 后端开发

```bash
# 桌面端（默认，端口 3210）
cargo run

# 或用更短的别名
cargo gui
```

### 前端开发

```bash
cd ui
npm install
npm run dev
```

前端开发服务器已配置 `/api` 代理到 `http://127.0.0.1:3210`。

## 贡献（Fork & PR）

欢迎贡献代码与想法！推荐流程如下：

1. 在 GitHub 上 Fork 本仓库到你的账号
2. 克隆你 Fork 后的仓库（示例）：

```bash
git clone https://github.com/<yourname>/cli-switch.git
cd cli-switch
```

3. 创建分支（建议使用 `feat/`、`fix/` 等前缀）：

```bash
git checkout -b feat/your-change
```

4. 本地自检（按改动范围选择）：

```bash
# Rust
cargo fmt
cargo clippy
cargo test

# UI（如涉及前端/内嵌资源）
cd ui
npm ci
npm run build
```

5. 推送并发起 Pull Request：在 PR 描述中说明动机、改动点、验证方式，关联 Issue（如有）

约定：

- 合并到默认分支请走 PR
- Commit message 建议使用 `feat:` / `fix:` / `docs:` 等前缀

## API

### 健康检查

```bash
curl http://127.0.0.1:3210/api/health
```

### 代理配置示例（MVP）

当前代理逻辑：按 `protocol` 选择**第一个启用的**渠道（按 `priority` 从大到小排序，其次按 `name`），并将请求原样透传到该渠道的 `base_url`。

支持的 `auth_type`：

- `bearer`：注入 `Authorization: Bearer <auth_ref>`
- `x-api-key`：注入 `x-api-key: <auth_ref>`（Anthropic 常用）
- `x-goog-api-key`：注入 `x-goog-api-key: <auth_ref>`（Gemini 可用）
- `query`：在 URL query 里追加 `key=<auth_ref>`（Gemini 常用）

创建 OpenAI 渠道（示例）：

```bash
curl -X POST http://127.0.0.1:3210/api/channels \
  -H 'content-type: application/json' \
  -d '{
    "name": "openai-main",
    "protocol": "openai",
    "base_url": "https://api.openai.com/v1",
    "auth_type": "bearer",
    "auth_ref": "<OPENAI_API_KEY>",
    "priority": 10,
    "enabled": true
  }'
```

创建 Anthropic 渠道（示例）：

```bash
curl -X POST http://127.0.0.1:3210/api/channels \
  -H 'content-type: application/json' \
  -d '{
    "name": "anthropic-main",
    "protocol": "anthropic",
    "base_url": "https://api.anthropic.com/v1",
    "auth_type": "x-api-key",
    "auth_ref": "<ANTHROPIC_API_KEY>",
    "priority": 10,
    "enabled": true
  }'
```

创建 Gemini 渠道（示例）：

```bash
curl -X POST http://127.0.0.1:3210/api/channels \
  -H 'content-type: application/json' \
  -d '{
    "name": "gemini-main",
    "protocol": "gemini",
    "base_url": "https://generativelanguage.googleapis.com/v1beta",
    "auth_type": "query",
    "auth_ref": "<GEMINI_API_KEY>",
    "priority": 10,
    "enabled": true
  }'
```

配置好渠道后，即可直接把 CLI 的 Base URL 指向 `http://127.0.0.1:3210`，并按原协议路径发起请求：

- OpenAI：`/v1/*`
- Anthropic：`/v1/messages`
- Gemini：`/v1beta/*`

### 管理接口

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/channels` | 获取渠道列表 |
| POST | `/api/channels` | 创建渠道 |
| PUT | `/api/channels/{id}` | 更新渠道 |
| POST | `/api/channels/{id}/enable` | 启用渠道 |
| POST | `/api/channels/{id}/disable` | 禁用渠道 |
| GET | `/api/routes` | 获取路由列表 |
| POST | `/api/routes` | 创建路由 |
| PUT | `/api/routes/{id}` | 更新路由 |

### 代理入口

| 路径 | 协议 |
|------|------|
| `/v1/*` | OpenAI 兼容 |
| `/v1/messages` | Anthropic |
| `/v1beta/*` | Gemini |

## 项目结构

```
cliswitch/
├── src/                # Rust 后端源码
│   ├── main.rs
│   ├── server.rs
│   └── storage.rs
├── ui/                 # React 前端
│   ├── src/
│   └── package.json
├── migrations/         # SQLite 迁移脚本
└── Cargo.toml
```

## 开发状态

当前为 MVP 开发阶段，已完成：

- [x] 项目骨架搭建
- [x] SQLite 数据模型
- [x] 渠道/路由 CRUD API
- [ ] 代理转发核心逻辑
- [ ] 价格同步
- [ ] 用量统计
- [ ] Web UI 页面

## 许可证

本项目采用 [GNU Affero General Public License v3.0 (AGPL-3.0)](https://www.gnu.org/licenses/agpl-3.0) 许可证。
详见 [LICENSE](./LICENSE) 文件。
