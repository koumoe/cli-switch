import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import readline from "node:readline";

function envString(key, fallback) {
  const raw = process.env[key];
  if (typeof raw !== "string") return fallback;
  const trimmed = raw.trim();
  return trimmed.length > 0 ? trimmed : fallback;
}

function envInt(key, fallback) {
  const raw = Number(process.env[key]);
  return Number.isFinite(raw) && raw > 0 ? Math.floor(raw) : fallback;
}

const stateDir = envString("CLISWITCH_WEIXIN_STATE_DIR", "./state");
const defaultBaseUrl = envString(
  "CLISWITCH_WEIXIN_DEFAULT_BASE_URL",
  "https://ilinkai.weixin.qq.com",
);
const bridgeVersion = envString("CLISWITCH_WEIXIN_BRIDGE_VERSION", "dev");
const qrPollTimeoutMs = envInt("CLISWITCH_WEIXIN_QR_POLL_TIMEOUT_MS", 35_000);
const updatesTimeoutMs = envInt("CLISWITCH_WEIXIN_UPDATES_TIMEOUT_MS", 35_000);
const retryDelayMs = envInt("CLISWITCH_WEIXIN_RETRY_DELAY_MS", 2_000);
const sessionExpiredErrCode = -14;

const credentialsFile = path.join(stateDir, "credentials.json");
const cursorFile = path.join(stateDir, "get-updates-buf.txt");

let credentials = loadCredentials();
let currentStatus = {
  state: "starting",
  connected: false,
  me: credentials?.account_id ?? credentials?.user_id ?? null,
  qr: null,
  qr_image: null,
  last_error: null,
};
let loginTask = null;
let pollTask = null;
let latestContextTokens = new Map();
let typingTickets = new Map();

function emit(obj) {
  process.stdout.write(`${JSON.stringify(obj)}\n`);
}

function emitStatus(patch) {
  currentStatus = {
    ...currentStatus,
    ...patch,
  };
  emit({ event: "status", status: currentStatus });
}

function log(message) {
  emit({ event: "log", message });
}

function ensureStateDir() {
  fs.mkdirSync(stateDir, { recursive: true });
}

function loadJsonFile(filePath) {
  try {
    if (!fs.existsSync(filePath)) return null;
    return JSON.parse(fs.readFileSync(filePath, "utf-8"));
  } catch {
    return null;
  }
}

function writeJsonFile(filePath, value) {
  ensureStateDir();
  fs.writeFileSync(filePath, JSON.stringify(value, null, 2), "utf-8");
}

function loadCredentials() {
  return loadJsonFile(credentialsFile);
}

function saveCredentials(next) {
  credentials = {
    token: next.token,
    base_url: next.base_url || defaultBaseUrl,
    account_id: next.account_id ?? null,
    user_id: next.user_id ?? null,
    saved_at: new Date().toISOString(),
  };
  writeJsonFile(credentialsFile, credentials);
}

function clearCredentials() {
  credentials = null;
  latestContextTokens = new Map();
  typingTickets = new Map();
  try {
    fs.rmSync(credentialsFile, { force: true });
  } catch {
    // Ignore.
  }
  try {
    fs.rmSync(cursorFile, { force: true });
  } catch {
    // Ignore.
  }
}

function loadCursor() {
  try {
    if (!fs.existsSync(cursorFile)) return "";
    return fs.readFileSync(cursorFile, "utf-8");
  } catch {
    return "";
  }
}

function saveCursor(cursor) {
  ensureStateDir();
  fs.writeFileSync(cursorFile, cursor ?? "", "utf-8");
}

function ensureTrailingSlash(url) {
  return url.endsWith("/") ? url : `${url}/`;
}

function randomWechatUin() {
  const value = crypto.randomBytes(4).readUInt32BE(0);
  return Buffer.from(String(value), "utf-8").toString("base64");
}

function buildBaseInfo() {
  return { channel_version: bridgeVersion };
}

function buildApiHeaders(body, token) {
  const headers = {
    "Content-Type": "application/json",
    AuthorizationType: "ilink_bot_token",
    "Content-Length": String(Buffer.byteLength(body, "utf-8")),
    "X-WECHAT-UIN": randomWechatUin(),
  };
  if (typeof token === "string" && token.trim().length > 0) {
    headers.Authorization = `Bearer ${token.trim()}`;
  }
  return headers;
}

async function fetchText(url, init, timeoutMs) {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  try {
    const response = await fetch(url, { ...init, signal: controller.signal });
    const text = await response.text();
    return { response, text };
  } finally {
    clearTimeout(timer);
  }
}

function normalizeQrImage(raw) {
  if (typeof raw !== "string") return null;
  const trimmed = raw.trim();
  if (!trimmed) return null;
  if (trimmed.startsWith("data:image/")) return trimmed;
  if (trimmed.startsWith("http://") || trimmed.startsWith("https://")) return trimmed;
  if (/^[A-Za-z0-9+/=\r\n]+$/.test(trimmed)) {
    return `data:image/png;base64,${trimmed.replace(/\s+/g, "")}`;
  }
  return trimmed;
}

async function fetchQRCode(baseUrl) {
  const base = ensureTrailingSlash(baseUrl);
  const url = new URL("ilink/bot/get_bot_qrcode?bot_type=3", base);
  const { response, text } = await fetchText(url.toString(), { method: "GET" }, qrPollTimeoutMs);
  if (!response.ok) {
    throw new Error(`fetch qrcode failed: ${response.status} ${text}`);
  }
  return JSON.parse(text);
}

async function pollQRCodeStatus(baseUrl, qrcode) {
  const base = ensureTrailingSlash(baseUrl);
  const url = new URL(`ilink/bot/get_qrcode_status?qrcode=${encodeURIComponent(qrcode)}`, base);
  try {
    const { response, text } = await fetchText(
      url.toString(),
      {
        method: "GET",
        headers: {
          "iLink-App-ClientVersion": "1",
        },
      },
      qrPollTimeoutMs,
    );
    if (!response.ok) {
      throw new Error(`poll qrcode failed: ${response.status} ${text}`);
    }
    return JSON.parse(text);
  } catch (err) {
    if (err instanceof Error && err.name === "AbortError") {
      return { status: "wait" };
    }
    throw err;
  }
}

async function postWeixinApi(baseUrl, endpoint, token, payload, timeoutMs) {
  const base = ensureTrailingSlash(baseUrl);
  const url = new URL(endpoint, base);
  const body = JSON.stringify({
    ...payload,
    base_info: buildBaseInfo(),
  });
  const { response, text } = await fetchText(
    url.toString(),
    {
      method: "POST",
      headers: buildApiHeaders(body, token),
      body,
    },
    timeoutMs,
  );
  if (!response.ok) {
    throw new Error(`${endpoint} failed: ${response.status} ${text}`);
  }
  return text;
}

async function getUpdates(baseUrl, token, cursor, timeoutMs) {
  try {
    const raw = await postWeixinApi(
      baseUrl,
      "ilink/bot/getupdates",
      token,
      { get_updates_buf: cursor || "" },
      timeoutMs,
    );
    return JSON.parse(raw);
  } catch (err) {
    if (err instanceof Error && err.name === "AbortError") {
      return { ret: 0, msgs: [], get_updates_buf: cursor || "" };
    }
    throw err;
  }
}

async function sendMessage(baseUrl, token, chatId, content, contextToken) {
  const clientId = `cliswitch-weixin-${crypto.randomUUID()}`;
  await postWeixinApi(
    baseUrl,
    "ilink/bot/sendmessage",
    token,
    {
      msg: {
        from_user_id: "",
        to_user_id: chatId,
        client_id: clientId,
        message_type: 2,
        message_state: 2,
        item_list: [{ type: 1, text_item: { text: content } }],
        context_token: contextToken ?? undefined,
      },
    },
    15_000,
  );
  return { message_id: clientId };
}

async function getConfig(baseUrl, token, chatId, contextToken) {
  const raw = await postWeixinApi(
    baseUrl,
    "ilink/bot/getconfig",
    token,
    {
      ilink_user_id: chatId,
      context_token: contextToken ?? undefined,
    },
    10_000,
  );
  return JSON.parse(raw);
}

async function sendTyping(baseUrl, token, chatId, typingTicket) {
  await postWeixinApi(
    baseUrl,
    "ilink/bot/sendtyping",
    token,
    {
      ilink_user_id: chatId,
      typing_ticket: typingTicket,
      status: 1,
    },
    10_000,
  );
}

function messagePlaceholder(itemList) {
  if (!Array.isArray(itemList)) return "";
  if (itemList.some((item) => item?.type === 2)) return "[图片]";
  if (itemList.some((item) => item?.type === 5)) return "[视频]";
  if (itemList.some((item) => item?.type === 4)) return "[文件]";
  if (itemList.some((item) => item?.type === 3)) return "[语音]";
  return "";
}

function extractText(itemList) {
  if (!Array.isArray(itemList) || itemList.length === 0) return "";
  for (const item of itemList) {
    if (item?.type === 1 && typeof item?.text_item?.text === "string") {
      return item.text_item.text;
    }
    if (item?.type === 3 && typeof item?.voice_item?.text === "string") {
      return item.voice_item.text;
    }
  }
  return messagePlaceholder(itemList);
}

function rememberContextToken(chatId, contextToken) {
  if (typeof chatId !== "string" || !chatId.trim()) return;
  if (typeof contextToken !== "string" || !contextToken.trim()) return;
  latestContextTokens.set(chatId, contextToken);
}

async function ensureTypingTicket(chatId) {
  const cached = typingTickets.get(chatId);
  if (typeof cached === "string" && cached) {
    return cached;
  }
  if (!credentials?.token) return null;
  const contextToken = latestContextTokens.get(chatId);
  const config = await getConfig(
    credentials.base_url || defaultBaseUrl,
    credentials.token,
    chatId,
    contextToken,
  );
  const typingTicket = typeof config?.typing_ticket === "string" ? config.typing_ticket : null;
  if (typingTicket) {
    typingTickets.set(chatId, typingTicket);
  }
  return typingTicket;
}

async function startLoginFlow() {
  if (loginTask) return loginTask;
  loginTask = (async () => {
    while (!credentials?.token) {
      emitStatus({
        state: "starting",
        connected: false,
        me: null,
        qr: null,
        qr_image: null,
      });
      try {
        const qrResponse = await fetchQRCode(defaultBaseUrl);
        const qr = typeof qrResponse?.qrcode === "string" ? qrResponse.qrcode : null;
        const qrImage = normalizeQrImage(qrResponse?.qrcode_img_content);
        if (!qr) {
          throw new Error("weixin qrcode is missing");
        }

        emitStatus({
          state: "awaiting_qr",
          connected: false,
          me: null,
          qr,
          qr_image: qrImage,
          last_error: null,
        });

        while (!credentials?.token) {
          const status = await pollQRCodeStatus(defaultBaseUrl, qr);
          if (status?.status === "confirmed" && typeof status?.bot_token === "string") {
            saveCredentials({
              token: status.bot_token,
              base_url: status.baseurl || defaultBaseUrl,
              account_id: status.ilink_bot_id || status.ilink_user_id || null,
              user_id: status.ilink_user_id || null,
            });
            emitStatus({
              state: "starting",
              connected: false,
              me: credentials?.account_id ?? credentials?.user_id ?? null,
              qr: null,
              qr_image: null,
              last_error: null,
            });
            void ensurePolling();
            return;
          }
          if (status?.status === "expired") {
            break;
          }
        }
      } catch (err) {
        emitStatus({
          state: "error",
          connected: false,
          me: null,
          qr: null,
          qr_image: null,
          last_error: String(err?.message ?? err),
        });
        await sleep(retryDelayMs);
      }
    }
  })();

  try {
    await loginTask;
  } finally {
    loginTask = null;
  }
}

async function ensurePolling() {
  if (pollTask) return pollTask;
  pollTask = (async () => {
    let cursor = loadCursor();

    while (credentials?.token) {
      const me = credentials.account_id || credentials.user_id || null;
      emitStatus({
        state: "connected",
        connected: true,
        me,
        qr: null,
        qr_image: null,
        last_error: null,
      });

      try {
        const response = await getUpdates(
          credentials.base_url || defaultBaseUrl,
          credentials.token,
          cursor,
          updatesTimeoutMs,
        );

        const errcode = Number(response?.errcode ?? 0);
        const ret = Number(response?.ret ?? 0);
        if ((errcode && errcode !== 0) || (ret && ret !== 0)) {
          if (errcode === sessionExpiredErrCode || ret === sessionExpiredErrCode) {
            clearCredentials();
            emitStatus({
              state: "error",
              connected: false,
              me,
              qr: null,
              qr_image: null,
              last_error: "微信会话已过期，请重新扫码登录。",
            });
            void startLoginFlow();
            return;
          }
          throw new Error(
            `getupdates failed: ret=${String(response?.ret)} errcode=${String(response?.errcode)} errmsg=${String(response?.errmsg ?? "")}`,
          );
        }

        if (typeof response?.get_updates_buf === "string") {
          cursor = response.get_updates_buf;
          saveCursor(cursor);
        }

        const list = Array.isArray(response?.msgs) ? response.msgs : [];
        for (const msg of list) {
          if (!msg || typeof msg !== "object") continue;
          const senderId =
            typeof msg.from_user_id === "string" ? msg.from_user_id.trim() : "";
          if (!senderId) continue;
          if (Number(msg.message_type ?? 0) === 2) continue;

          rememberContextToken(senderId, msg.context_token);
          emit({
            event: "message",
            message: {
              sender_id: senderId,
              sender_display_name: null,
              chat_id: senderId,
              message_id:
                msg.message_id != null ? String(msg.message_id) : null,
              timestamp_ms: Number(msg.create_time_ms ?? Date.now()),
              text: extractText(msg.item_list),
            },
          });
        }
      } catch (err) {
        emitStatus({
          state: "error",
          connected: false,
          me,
          qr: null,
          qr_image: null,
          last_error: String(err?.message ?? err),
        });
        await sleep(retryDelayMs);
      }
    }
  })();

  try {
    await pollTask;
  } finally {
    pollTask = null;
  }
}

async function handleSend(req) {
  if (!credentials?.token) {
    throw new Error("weixin is not connected");
  }
  const chatId = String(req?.chat_id ?? "").trim();
  if (!chatId) {
    throw new Error("chat_id is required");
  }
  const content = typeof req?.content === "string" ? req.content : "";
  if (!content.trim()) {
    throw new Error("outgoing message is empty");
  }
  const attachments = Array.isArray(req?.attachments) ? req.attachments : [];
  if (attachments.length > 0) {
    throw new Error("weixin bridge does not support attachments yet");
  }

  return await sendMessage(
    credentials.base_url || defaultBaseUrl,
    credentials.token,
    chatId,
    content,
    latestContextTokens.get(chatId),
  );
}

async function handleTyping(req) {
  if (!credentials?.token) {
    return { sent: false };
  }
  const chatId = String(req?.chat_id ?? "").trim();
  if (!chatId) {
    return { sent: false };
  }

  const typingTicket = await ensureTypingTicket(chatId);
  if (!typingTicket) {
    return { sent: false };
  }
  await sendTyping(
    credentials.base_url || defaultBaseUrl,
    credentials.token,
    chatId,
    typingTicket,
  );
  return { sent: true };
}

async function handlePing() {
  return {
    ...currentStatus,
  };
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function bootstrap() {
  ensureStateDir();
  emit({ event: "boot" });
  if (credentials?.token) {
    void ensurePolling();
  } else {
    void startLoginFlow();
  }

  const rl = readline.createInterface({ input: process.stdin, crlfDelay: Infinity });
  rl.on("line", async (line) => {
    const trimmed = String(line ?? "").trim();
    if (!trimmed) return;

    let req = null;
    try {
      req = JSON.parse(trimmed);
    } catch (err) {
      emit({ event: "error", error: String(err?.message ?? err) });
      return;
    }

    const id = typeof req?.id === "string" ? req.id : null;
    const type = typeof req?.type === "string" ? req.type : null;
    if (!id || !type) {
      return;
    }

    try {
      if (type === "send") {
        const result = await handleSend(req);
        emit({ id, ok: true, result });
        return;
      }
      if (type === "typing") {
        const result = await handleTyping(req);
        emit({ id, ok: true, result });
        return;
      }
      if (type === "ping") {
        const result = await handlePing();
        emit({ id, ok: true, result });
        return;
      }
      throw new Error(`unknown request type: ${type}`);
    } catch (err) {
      emit({ id, ok: false, error: String(err?.message ?? err) });
    }
  });
}

bootstrap().catch((err) => {
  emit({ event: "fatal", error: String(err?.message ?? err) });
  process.exitCode = 1;
});
