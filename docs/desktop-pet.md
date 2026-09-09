# Desktop pet / 桌面宠物

## English

Enable **Desktop pet** in **Settings → Application**, or choose **Show desktop pet** from the tray menu. The feature is disabled by default.

- The resident window is 64 × 64 logical pixels. It has no permanent title card or activity list.
- Click the pet to open a separate 190 × 136 activity panel containing at most two rows. Click another window or press Escape to dismiss it. **View all activities** opens the main CliSwitch activity page.
- Drag toward an outer left/right monitor edge and release to dock. The window shrinks to 28 × 40 and shows a small face and paw. Hovering does not expand it. Drag inward to undock.
- A confirmed completed turn briefly shows a receipt and a separate two-second notification; the docked pet stays small and changes its expression. Notifications do not take focus. The activity panel takes focus only after the user clicks the pet.
- Right-click the pet to hide it. Reopen it from the tray or settings. Position and docking are remembered in the local data directory.
- The badge counts running **activities**, not guaranteed unique conversations across every client. Activity state is local and in memory; restarting the backend does not replay previous completion notices.

### Activity semantics

All generation requests through the proxy are tracked, including non-streaming replies, streams, retries, errors, and disconnects. Token counting and management requests are excluded. Request/response bodies and credentials are not stored in the activity registry.

Supported Codex Responses clients that send a verified UUID `thread-id` (or legacy `thread_id`) are grouped by that thread. Concurrent requests belonging to the same thread keep the group running until all observed requests finish. Missing or conflicting identity remains unassigned request activity. Titles are not inferred from prompts; Codex groups use a short technical thread ID.

**A proxy response ending does not mean an agent turn has completed.** External Codex may be running local tools between requests. Without a confirmed lifecycle event this version shows `Response finished`. Controlled chat-bridge turns use structured Codex, Claude Code, and Gemini CLI terminal results to distinguish completed, failed, cancelled, and unknown states.

### Connect Codex completion notifications

With the pet enabled, expand **Codex completion notification setup** in application settings and click **Copy configuration**. Add the generated top-level `notify` entry to your user-level Codex configuration. Keep any existing `notify` command; if present, use a wrapper to invoke both handlers rather than replacing it. Start a new Codex session (or restart the client) after changing its configuration.

The command invokes `cliswitch --data-dir <directory> activity-notify <payload>`. Codex supplies the payload automatically. The helper reads only the event type, thread ID and turn ID, and forwards those identifiers to an authenticated loopback endpoint with a two-second total deadline. It bypasses network proxies and redirects. CliSwitch must have observed that exact thread and turn in verified Codex request metadata before accepting completion; late notifications cannot finish a newer turn, and duplicate notifications do not replay the animation. Clients that do not expose both the request metadata and the official notify event retain request-level status.

CliSwitch does not automatically install hooks, overwrite existing Codex configuration, read conversation files, or infer user-read status. The temporary local registration uses a private file and is removed when the server stops. If CliSwitch is not running, the helper quietly exits.

“Completed” does not mean “read.” There is no unread counter or unverified deep-link navigation. Bridge activity and request activity are separate when there is no trustworthy correlation.

### Platform behavior

The implementation uses the existing tao/wry desktop stack. On macOS, the pet and notification use non-activating presentation. Monitor placement reads the native work area on macOS, Windows and Linux to avoid system bars; conservative margins are a fallback when that information is unavailable. Transparent sprite pixels pass pointer events through on macOS, Windows, and X11 using an alpha hit map. Wayland does not expose global pointer coordinates; there the fallback retains the small rectangular pet hit area. Actual window-manager behavior still requires platform validation.

### Development checks

```bash
node src/desktop_pet/test/pet.test.mjs
node src/desktop_pet/test/dom.test.mjs
cargo test --bin cliswitch desktop::pet::tests
cargo test --no-default-features --features embed-ui
```

The DOM test uses the project's installed UI jsdom dependency. A hidden `--data-dir <path>` CLI option permits isolated desktop validation without touching an existing CliSwitch database.

## 中文

在 **设置 → 应用** 中开启“桌面宠物”，或使用托盘菜单“显示桌面宠物”。默认关闭。

- 常驻窗口为 64 × 64 逻辑像素，没有常驻标题卡或列表。
- 点击宠物，临时打开 190 × 136 的独立活动窗口，最多显示两条。点击其他窗口或按 Esc 收起；“在主窗口查看全部活动”进入 CliSwitch 活动页。
- 拖近屏幕外缘的左侧或右侧，松手后吸附，窗口缩为 28 × 40，只露小脸和爪子。鼠标经过不放大，往屏幕内拖动恢复全身。
- 已确认完成的轮次会举一次回执，并短暂显示两秒提示；贴边时保持小尺寸，只改变表情。完成通知不抢焦点；只有用户主动点击宠物时，活动面板才接收焦点。
- 右键宠物即可隐藏，可从托盘或设置恢复。位置与贴边状态保存在本地数据目录。
- 数字角标表示执行中的“活动”，不保证所有客户端都能识别为唯一对话。活动记录在本地内存中维护，重启不会重播历史完成提示。

### 状态含义

所有经过代理的生成请求都会纳入观察，包括普通响应、流式响应、重试、错误和断开。计数 token 和管理请求不作为对话活动。活动记录不保存请求正文、响应正文或凭证。

已核验的 Codex Responses 客户端若携带合法 UUID `thread-id`（或旧版 `thread_id`），会按对应会话归组。同一会话的并发请求只要仍有一个在运行，该组就保持运行。身份缺失、冲突时保留为未归属请求活动。不从提示词推测标题；Codex 会话显示短技术 ID。

**代理响应结束不代表一轮任务完成。** 外部 Codex 可能正在请求间隙执行本地工具。没有明确生命周期事件时，本版显示“响应已结束”。消息互联受控轮次会根据 Codex、Claude Code、Gemini CLI 的结构化终止结果，区分完成、失败、取消与未知。

### 接入 Codex 完成通知

开启宠物后，在应用设置中展开“Codex 完成通知设置”，点击“复制配置”。将生成的顶层 `notify` 配置加入用户级 Codex 配置。保留已有的 `notify` 命令；已有配置时应通过包装脚本同时调用两个处理程序，不应直接覆盖。修改后重新开启 Codex 会话，必要时重启客户端。

生成的命令为 `cliswitch --data-dir <目录> activity-notify <payload>`，payload 由 Codex 自动提供。转发程序仅提取事件类型、会话 ID 和轮次 ID，直接发往需要随机凭证的本机回环接口，不经过网络代理或重定向，总截止时间为两秒。只有此 CliSwitch 实例已从核验过的请求元数据中观察到同一会话和轮次，才会接受完成；迟到的通知不能完成更新一轮，重复通知不会重播动画。无法同时提供请求元数据和官方 notify 事件的客户端仍显示请求级状态。

CliSwitch 不会自动安装 hook、覆盖现有 Codex 配置、读取会话文件或判断用户已读。临时连接文件使用私有权限，服务停止后清理。CliSwitch 未运行时，转发程序安静退出。

“完成”不代表“已读”。没有未读计数，也没有未经验证的原对话跳转。无法可靠关联时，消息互联轮次与代理请求分别展示。

### 平台行为

使用项目现有 tao/wry 桌面框架。macOS 使用不激活应用的宠物和通知窗口。macOS、Windows、Linux 读取系统提供的屏幕工作区来避让系统栏，读取失败时使用保守预留边距。macOS、Windows 与 X11 通过像素命中图让透明区域鼠标穿透；Wayland 无法提供全局指针位置，降级为小尺寸矩形命中范围。窗口管理器的实际行为仍需实机验证。

隐藏的 `--data-dir <路径>` 启动选项用于隔离桌面验证，避免影响已有数据库。
