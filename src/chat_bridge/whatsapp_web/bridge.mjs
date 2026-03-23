import fs from "node:fs";
import path from "node:path";
import readline from "node:readline";

import makeWASocket, {
  DisconnectReason,
  downloadMediaMessage,
  fetchLatestBaileysVersion,
} from "@whiskeysockets/baileys";
import { useMultiFileAuthState } from "@whiskeysockets/baileys";
import pino from "pino";

function envString(key, fallback) {
  const v = process.env[key];
  if (typeof v !== "string") return fallback;
  const t = v.trim();
  return t.length > 0 ? t : fallback;
}

function envInt(key, fallback) {
  const raw = process.env[key];
  const n = Number(raw);
  return Number.isFinite(n) && n > 0 ? Math.floor(n) : fallback;
}

const authDir = envString("CLISWITCH_WHATSAPP_AUTH_DIR", "./auth");
const maxAttachmentBytes = envInt(
  "CLISWITCH_WHATSAPP_MAX_ATTACHMENT_BYTES",
  20 * 1024 * 1024,
);
const bridgeVersion = envString("CLISWITCH_WHATSAPP_BRIDGE_VERSION", "dev");
const logLevel = envString("CLISWITCH_WHATSAPP_LOG_LEVEL", "silent");
const reconnectBaseDelayMs = envInt(
  "CLISWITCH_WHATSAPP_RECONNECT_BASE_DELAY_MS",
  2_000,
);
const reconnectMaxDelayMs = envInt(
  "CLISWITCH_WHATSAPP_RECONNECT_MAX_DELAY_MS",
  30_000,
);
const logger = pino({ level: logLevel });

let sock = null;
let connected = false;
let latestQr = null;
let me = null;
let reconnectAttempt = 0;
let reconnectTask = null;

const recentMessages = new Map(); // message_id -> WebMessageInfo
const RECENT_LIMIT = 256;
const sentByBridgeIds = new Set();
const sentByBridgeOrder = [];

function emit(obj) {
  process.stdout.write(`${JSON.stringify(obj)}\n`);
}

function setLatestQr(qr) {
  latestQr = qr;
  emit({ event: "qr", qr });
}

function setConnectionState(next, extra) {
  connected = next === "open";
  emit({ event: "connection", connection: next, ...(extra ?? {}) });
}

function cacheMessage(m) {
  const id = m?.key?.id;
  if (!id) return;
  recentMessages.set(id, m);
  if (recentMessages.size <= RECENT_LIMIT) return;
  const first = recentMessages.keys().next().value;
  if (first) recentMessages.delete(first);
}

function rememberSentMessageId(id) {
  if (!id) return;
  sentByBridgeIds.add(id);
  sentByBridgeOrder.push(id);
  while (sentByBridgeOrder.length > RECENT_LIMIT) {
    const oldest = sentByBridgeOrder.shift();
    if (oldest) sentByBridgeIds.delete(oldest);
  }
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function nextReconnectDelayMs() {
  const exponent = Math.min(reconnectAttempt, 5);
  const delay = Math.min(reconnectBaseDelayMs * (2 ** exponent), reconnectMaxDelayMs);
  reconnectAttempt += 1;
  return delay;
}

async function scheduleReconnect(reason) {
  if (reconnectTask) {
    return reconnectTask;
  }
  reconnectTask = (async () => {
    while (true) {
      const delayMs = nextReconnectDelayMs();
      emit({
        event: "reconnect",
        reason,
        delay_ms: delayMs,
        attempt: reconnectAttempt,
      });
      await sleep(delayMs);
      try {
        await startSocket();
        return;
      } catch (e) {
        emit({ event: "error", scope: "reconnect", error: String(e?.message ?? e) });
        reason = "retry_failed";
      }
    }
  })();
  try {
    await reconnectTask;
  } finally {
    reconnectTask = null;
  }
}

function unwrapMessageContent(message) {
  let msg = message;
  // Best-effort unwrap for common wrappers.
  // Baileys also exposes normalizeMessageContent, but we keep this minimal.
  for (let i = 0; i < 3; i++) {
    if (msg?.ephemeralMessage?.message) {
      msg = msg.ephemeralMessage.message;
      continue;
    }
    if (msg?.viewOnceMessage?.message) {
      msg = msg.viewOnceMessage.message;
      continue;
    }
    if (msg?.viewOnceMessageV2?.message) {
      msg = msg.viewOnceMessageV2.message;
      continue;
    }
    break;
  }
  return msg;
}

function extractTextFromMessage(message) {
  const msg = unwrapMessageContent(message) ?? {};
  if (typeof msg.conversation === "string") return msg.conversation;
  const extText = msg.extendedTextMessage?.text;
  if (typeof extText === "string") return extText;
  const imgCap = msg.imageMessage?.caption;
  if (typeof imgCap === "string") return imgCap;
  const docCap = msg.documentMessage?.caption;
  if (typeof docCap === "string") return docCap;
  return "";
}

function inferExtFromMime(mime) {
  if (!mime) return null;
  const m = mime.toLowerCase();
  if (m.includes("jpeg")) return "jpg";
  if (m.includes("png")) return "png";
  if (m.includes("webp")) return "webp";
  if (m.includes("gif")) return "gif";
  if (m.includes("pdf")) return "pdf";
  return null;
}

async function extractAttachments(m) {
  const msg = unwrapMessageContent(m.message) ?? {};
  const attachments = [];

  const maybeImage = msg.imageMessage;
  if (maybeImage) {
    const mime = maybeImage.mimetype ?? null;
    const ext = inferExtFromMime(mime) ?? "jpg";
    const filename = `image-${m.key.id ?? "unknown"}.${ext}`;
    const caption = typeof maybeImage.caption === "string" ? maybeImage.caption : null;
    const buf = await downloadMediaMessage(
      m,
      "buffer",
      {},
      { logger, reuploadRequest: sock?.updateMediaMessage },
    );
    if (!Buffer.isBuffer(buf)) {
      throw new Error("downloadMediaMessage did not return a buffer");
    }
    if (buf.length > maxAttachmentBytes) {
      throw new Error(`attachment too large: ${buf.length} bytes`);
    }
    attachments.push({
      kind: "image",
      filename,
      mime_type: mime,
      caption,
      data_b64: buf.toString("base64"),
    });
  }

  const maybeDoc = msg.documentMessage;
  if (maybeDoc) {
    const mime = maybeDoc.mimetype ?? null;
    const filename = maybeDoc.fileName ?? `file-${m.key.id ?? "unknown"}`;
    const caption = typeof maybeDoc.caption === "string" ? maybeDoc.caption : null;
    const buf = await downloadMediaMessage(
      m,
      "buffer",
      {},
      { logger, reuploadRequest: sock?.updateMediaMessage },
    );
    if (!Buffer.isBuffer(buf)) {
      throw new Error("downloadMediaMessage did not return a buffer");
    }
    if (buf.length > maxAttachmentBytes) {
      throw new Error(`attachment too large: ${buf.length} bytes`);
    }
    attachments.push({
      kind: "file",
      filename,
      mime_type: mime,
      caption,
      data_b64: buf.toString("base64"),
    });
  }

  return attachments;
}

function ensureAuthDirExists() {
  try {
    fs.mkdirSync(authDir, { recursive: true });
  } catch {
    // Ignore.
  }
}

async function shutdownSocket() {
  const old = sock;
  sock = null;
  if (!old) return;
  try {
    old.end?.();
  } catch {
    // Ignore.
  }
  try {
    old.ws?.close();
  } catch {
    // Ignore.
  }
}

async function startSocket() {
  ensureAuthDirExists();

  const { state, saveCreds } = await useMultiFileAuthState(authDir);
  const { version } = await fetchLatestBaileysVersion();

  await shutdownSocket();
  connected = false;
  latestQr = null;
  me = null;

  sock = makeWASocket({
    version,
    auth: state,
    printQRInTerminal: false,
    logger,
    browser: ["CliSwitch", "Desktop", bridgeVersion],
    syncFullHistory: false,
  });

  sock.ev.on("creds.update", saveCreds);

  sock.ev.on("connection.update", async (update) => {
    if (typeof update.qr === "string" && update.qr.trim().length > 0) {
      setLatestQr(update.qr);
    }

    if (update.connection) {
      setConnectionState(update.connection, { me });
    }

    if (update.connection === "open") {
      reconnectAttempt = 0;
      me = sock.user?.id ?? null;
      emit({ event: "ready", me });
    }

    if (update.connection === "close") {
      const reason =
        update.lastDisconnect?.error?.output?.statusCode ??
        update.lastDisconnect?.error?.output?.payload?.statusCode ??
        null;
      if (reason === DisconnectReason.loggedOut) {
        emit({ event: "logged_out" });
        reconnectAttempt = 0;
        me = null;
        latestQr = null;
        connected = false;
        return;
      }
      try {
        await scheduleReconnect(reason);
      } catch (e) {
        emit({ event: "error", scope: "reconnect", error: String(e?.message ?? e) });
      }
    }
  });

  sock.ev.on("messages.upsert", async ({ messages }) => {
    if (!Array.isArray(messages)) return;
    for (const m of messages) {
      try {
        if (!m?.message) continue;
        cacheMessage(m);
        const id = m.key?.id ?? null;
        if (id && sentByBridgeIds.has(id)) {
          sentByBridgeIds.delete(id);
          continue;
        }

        const remoteJid = m.key?.remoteJid ?? null;
        if (!remoteJid) continue;

        const participant = m.key?.participant ?? null;
        const senderId = participant ?? remoteJid;
        const chatId = remoteJid;

        const ts = Number(m.messageTimestamp ?? 0);
        const timestampMs = Number.isFinite(ts) ? Math.floor(ts * 1000) : 0;

        const senderDisplayName = typeof m.pushName === "string" ? m.pushName : null;
        const text = extractTextFromMessage(m.message);
        const attachments = await extractAttachments(m);

        emit({
          event: "message",
          message: {
            sender_id: senderId,
            sender_display_name: senderDisplayName,
            chat_id: chatId,
            message_id: id,
            timestamp_ms: timestampMs,
            text,
            attachments,
          },
        });
      } catch (e) {
        emit({ event: "error", scope: "messages.upsert", error: String(e?.message ?? e) });
      }
    }
  });
}

async function handleSend(req) {
  if (!sock || !connected) {
    throw new Error("whatsapp is not connected");
  }

  const chatId = String(req.chat_id ?? "").trim();
  if (!chatId) {
    throw new Error("chat_id is required");
  }

  const replyTo = typeof req.reply_to === "string" ? req.reply_to.trim() : "";
  const quoted = replyTo ? recentMessages.get(replyTo) : undefined;

  const content = typeof req.content === "string" ? req.content : "";
  const attachments = Array.isArray(req.attachments) ? req.attachments : [];

  let firstMessageId = null;
  if (content.trim().length > 0) {
    const info = await sock.sendMessage(chatId, { text: content }, quoted ? { quoted } : {});
    firstMessageId = info?.key?.id ?? firstMessageId;
    rememberSentMessageId(info?.key?.id ?? null);
  }

  for (let idx = 0; idx < attachments.length; idx++) {
    const att = attachments[idx] ?? {};
    const filename = typeof att.filename === "string" ? att.filename : `file-${idx + 1}`;
    const mimeType = typeof att.mime_type === "string" ? att.mime_type : "";
    const dataB64 = typeof att.data_b64 === "string" ? att.data_b64 : "";
    if (!dataB64) continue;
    const buf = Buffer.from(dataB64, "base64");
    if (buf.length > maxAttachmentBytes) {
      throw new Error(`attachment too large: ${buf.length} bytes`);
    }

    const shouldQuote = !firstMessageId && idx === 0;
    const sendOpts = shouldQuote && quoted ? { quoted } : {};

    if (mimeType.toLowerCase().startsWith("image/")) {
      const info = await sock.sendMessage(chatId, { image: buf }, sendOpts);
      firstMessageId = firstMessageId ?? info?.key?.id ?? null;
      rememberSentMessageId(info?.key?.id ?? null);
    } else {
      const payload = {
        document: buf,
        fileName: filename,
      };
      if (mimeType) payload.mimetype = mimeType;
      const info = await sock.sendMessage(chatId, payload, sendOpts);
      firstMessageId = firstMessageId ?? info?.key?.id ?? null;
      rememberSentMessageId(info?.key?.id ?? null);
    }
  }

  if (!firstMessageId) {
    throw new Error("outgoing message is empty");
  }
  return { message_id: firstMessageId };
}

async function handleLogout() {
  // Best-effort revoke session before deleting local auth state.
  try {
    await sock?.logout();
  } catch {
    // Ignore.
  }
  await shutdownSocket();

  try {
    fs.rmSync(authDir, { recursive: true, force: true });
  } catch {
    // Ignore.
  }

  await startSocket();
  return { ok: true };
}

async function handlePing() {
  return { connected, me, has_qr: Boolean(latestQr) };
}

async function main() {
  emit({ event: "boot" });
  await startSocket();

  const rl = readline.createInterface({ input: process.stdin, crlfDelay: Infinity });
  rl.on("line", async (line) => {
    const trimmed = String(line ?? "").trim();
    if (!trimmed) return;
    let req = null;
    try {
      req = JSON.parse(trimmed);
    } catch (e) {
      emit({ event: "error", scope: "stdin.parse", error: String(e?.message ?? e) });
      return;
    }

    const id = typeof req.id === "string" ? req.id : null;
    const type = typeof req.type === "string" ? req.type : null;
    if (!id || !type) {
      // Ignore malformed requests.
      return;
    }

    try {
      if (type === "send") {
        const result = await handleSend(req);
        emit({ id, ok: true, result });
        return;
      }
      if (type === "logout") {
        const result = await handleLogout();
        emit({ id, ok: true, result });
        return;
      }
      if (type === "ping") {
        const result = await handlePing();
        emit({ id, ok: true, result });
        return;
      }
      throw new Error(`unknown request type: ${type}`);
    } catch (e) {
      emit({ id, ok: false, error: String(e?.message ?? e) });
    }
  });
}

main().catch((e) => {
  emit({ event: "fatal", error: String(e?.message ?? e) });
  process.exitCode = 1;
});
