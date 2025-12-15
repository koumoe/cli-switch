import React, { useEffect, useMemo, useState } from "react";
import {
  createChannel,
  deleteChannel,
  disableChannel,
  enableChannel,
  listChannels,
  statsChannels,
  testChannel,
  updateChannel,
  type Channel,
  type ChannelStats,
  type CreateChannelInput,
  type Protocol
} from "../api";
import { Badge, Button, Card, Modal } from "../components/ui";
import { formatDateTime, formatDuration, clampStr } from "../lib";

type ChannelDraft = CreateChannelInput;

function emptyDraft(): ChannelDraft {
  return {
    name: "",
    protocol: "openai",
    base_url: "https://api.openai.com/v1",
    auth_type: "bearer",
    auth_ref: "",
    enabled: true
  };
}

function protocolLabel(p: Protocol): string {
  if (p === "openai") return "openai";
  if (p === "anthropic") return "anthropic";
  return "gemini";
}

export function ChannelsPage() {
  const [channels, setChannels] = useState<Channel[]>([]);
  const [stats, setStats] = useState<Map<string, ChannelStats>>(new Map());
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [range, setRange] = useState<"today" | "month">("today");

  const [modalOpen, setModalOpen] = useState(false);
  const [modalMode, setModalMode] = useState<"create" | "edit">("create");
  const [editId, setEditId] = useState<string | null>(null);
  const [draft, setDraft] = useState<ChannelDraft>(emptyDraft());
  const [testing, setTesting] = useState<Record<string, boolean>>({});

  const statsById = useMemo(() => stats, [stats]);

  async function refresh() {
    setLoading(true);
    setError(null);
    try {
      const [chs, st] = await Promise.all([listChannels(), statsChannels(range)]);
      setChannels(chs);
      const map = new Map<string, ChannelStats>();
      for (const item of st.items) map.set(item.channel_id, item);
      setStats(map);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void refresh();
  }, [range]);

  function openCreate() {
    setModalMode("create");
    setEditId(null);
    setDraft(emptyDraft());
    setModalOpen(true);
  }

  function openEdit(c: Channel) {
    setModalMode("edit");
    setEditId(c.id);
    setDraft({
      name: c.name,
      protocol: c.protocol,
      base_url: c.base_url,
      auth_type: c.auth_type,
      auth_ref: c.auth_ref,
      enabled: c.enabled
    });
    setModalOpen(true);
  }

  async function submit() {
    setError(null);
    try {
      if (draft.name.trim().length === 0) throw new Error("name 不能为空");
      if (draft.base_url.trim().length === 0) throw new Error("base_url 不能为空");
      if (modalMode === "create") {
        await createChannel({ ...draft, name: draft.name.trim(), base_url: draft.base_url.trim() });
      } else {
        if (!editId) throw new Error("missing edit id");
        await updateChannel(editId, {
          name: draft.name.trim(),
          base_url: draft.base_url.trim(),
          auth_type: draft.auth_type.trim(),
          auth_ref: draft.auth_ref,
          enabled: draft.enabled
        });
      }
      setModalOpen(false);
      await refresh();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  async function toggleEnabled(c: Channel) {
    setError(null);
    try {
      if (c.enabled) await disableChannel(c.id);
      else await enableChannel(c.id);
      await refresh();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  async function onDelete(c: Channel) {
    if (!confirm(`删除渠道 "${c.name}"？此操作不可恢复。`)) return;
    setError(null);
    try {
      await deleteChannel(c.id);
      await refresh();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  async function onTest(c: Channel) {
    setTesting((m) => ({ ...m, [c.id]: true }));
    setError(null);
    try {
      const r = await testChannel(c.id);
      const msg = r.reachable
        ? `status=${r.status ?? "-"} ok=${r.ok} latency=${r.latency_ms}ms`
        : `unreachable latency=${r.latency_ms}ms error=${r.error ?? "-"}`;
      alert(msg);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setTesting((m) => ({ ...m, [c.id]: false }));
    }
  }

  return (
    <div className="stack">
      <div className="row" style={{ justifyContent: "space-between" }}>
        <div>
          <h1 className="page-title">Channels</h1>
          <p className="page-subtitle">管理上游渠道（base_url、鉴权、启停与连通性测试）。</p>
        </div>
        <div className="row">
          <select className="select" value={range} onChange={(e) => setRange(e.target.value as "today" | "month")}>
            <option value="today">today</option>
            <option value="month">month</option>
          </select>
          <Button onClick={refresh} disabled={loading}>
            {loading ? "刷新中…" : "刷新"}
          </Button>
          <Button variant="primary" onClick={openCreate}>
            新增渠道
          </Button>
        </div>
      </div>

      {error ? <div className="error">{clampStr(error, 2000)}</div> : null}

      <Card title="Channels List">
        {channels.length === 0 ? (
          <div className="muted">暂无渠道。先点击“新增渠道”。</div>
        ) : (
          <table className="table">
            <thead>
              <tr>
                <th style={{ width: 170 }}>Name</th>
                <th style={{ width: 110 }}>Protocol</th>
                <th>Base URL</th>
                <th style={{ width: 110 }}>Enabled</th>
                <th style={{ width: 90 }}>Req</th>
                <th style={{ width: 120 }}>Avg</th>
                <th style={{ width: 210 }}>Actions</th>
              </tr>
            </thead>
            <tbody>
              {channels.map((c) => {
                const st = statsById.get(c.id);
                return (
                  <tr key={c.id}>
                    <td>
                      <div style={{ fontWeight: 700 }}>{c.name}</div>
                      <div className="muted">{formatDateTime(c.updated_at_ms)}</div>
                    </td>
                    <td>
                      <Badge>{protocolLabel(c.protocol)}</Badge>
                    </td>
                    <td className="muted">
                      <code>{clampStr(c.base_url, 60)}</code>
                    </td>
                    <td>{c.enabled ? <Badge kind="ok">ON</Badge> : <Badge kind="bad">OFF</Badge>}</td>
                    <td className="muted">{st?.requests ?? 0}</td>
                    <td className="muted">{st?.avg_latency_ms ? formatDuration(Math.round(st.avg_latency_ms)) : "-"}</td>
                    <td>
                      <div className="row">
                        <Button onClick={() => openEdit(c)}>编辑</Button>
                        <Button onClick={() => toggleEnabled(c)}>{c.enabled ? "禁用" : "启用"}</Button>
                        <Button onClick={() => onTest(c)} disabled={!!testing[c.id]}>
                          {testing[c.id] ? "测试中…" : "测试"}
                        </Button>
                        <Button variant="danger" onClick={() => onDelete(c)}>
                          删除
                        </Button>
                      </div>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        )}
      </Card>

      <Modal
        open={modalOpen}
        title={modalMode === "create" ? "新增渠道" : "编辑渠道"}
        onClose={() => setModalOpen(false)}
      >
        <div className="stack">
          <div className="grid-2">
            <div className="field">
              <label>name</label>
              <input
                className="input"
                value={draft.name}
                onChange={(e) => setDraft((d) => ({ ...d, name: e.target.value }))}
                placeholder="openai-main"
              />
            </div>
            <div className="field">
              <label>protocol</label>
              <select
                className="select"
                value={draft.protocol}
                onChange={(e) => setDraft((d) => ({ ...d, protocol: e.target.value as Protocol }))}
                disabled={modalMode === "edit"}
                title={modalMode === "edit" ? "当前版本暂不支持修改 protocol" : undefined}
              >
                <option value="openai">openai</option>
                <option value="anthropic">anthropic</option>
                <option value="gemini">gemini</option>
              </select>
            </div>
            <div className="field" style={{ gridColumn: "1 / -1" }}>
              <label>base_url</label>
              <input
                className="input"
                value={draft.base_url}
                onChange={(e) => setDraft((d) => ({ ...d, base_url: e.target.value }))}
                placeholder="https://api.openai.com/v1"
              />
            </div>
            <div className="field">
              <label>auth_type</label>
              <select
                className="select"
                value={draft.auth_type}
                onChange={(e) => setDraft((d) => ({ ...d, auth_type: e.target.value }))}
              >
                <option value="bearer">bearer</option>
                <option value="x-api-key">x-api-key</option>
                <option value="x-goog-api-key">x-goog-api-key</option>
                <option value="query">query</option>
              </select>
            </div>
            <div className="field">
              <label>enabled</label>
              <select
                className="select"
                value={draft.enabled ? "true" : "false"}
                onChange={(e) => setDraft((d) => ({ ...d, enabled: e.target.value === "true" }))}
              >
                <option value="true">true</option>
                <option value="false">false</option>
              </select>
            </div>
            <div className="field" style={{ gridColumn: "1 / -1" }}>
              <label>auth_ref</label>
              <input
                className="input"
                value={draft.auth_ref}
                onChange={(e) => setDraft((d) => ({ ...d, auth_ref: e.target.value }))}
                placeholder="API Key / Token"
              />
            </div>
          </div>

          <div className="row" style={{ justifyContent: "flex-end" }}>
            <Button variant="ghost" onClick={() => setModalOpen(false)}>
              取消
            </Button>
            <Button variant="primary" onClick={submit}>
              保存
            </Button>
          </div>
        </div>
      </Modal>
    </div>
  );
}
