import React, { useEffect, useMemo, useState } from "react";
import {
  createRoute,
  deleteRoute,
  listChannels,
  listRouteChannels,
  listRoutes,
  reorderRouteChannels,
  updateRoute,
  type Channel,
  type CreateRouteInput,
  type Protocol,
  type Route
} from "../api";
import { Badge, Button, Card, Modal } from "../components/ui";
import { clampStr, formatDateTime } from "../lib";

type RouteDraft = CreateRouteInput;

function emptyDraft(): RouteDraft {
  return { name: "", protocol: "openai", match_model: null, enabled: true };
}

function channelNameById(channels: Channel[]): Map<string, string> {
  const m = new Map<string, string>();
  for (const c of channels) m.set(c.id, c.name);
  return m;
}

export function RoutesPage() {
  const [routes, setRoutes] = useState<Route[]>([]);
  const [channels, setChannels] = useState<Channel[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const [routeModalOpen, setRouteModalOpen] = useState(false);
  const [routeMode, setRouteMode] = useState<"create" | "edit">("create");
  const [editRouteId, setEditRouteId] = useState<string | null>(null);
  const [draft, setDraft] = useState<RouteDraft>(emptyDraft());

  const [manageOpen, setManageOpen] = useState(false);
  const [manageRoute, setManageRoute] = useState<Route | null>(null);
  const [assigned, setAssigned] = useState<string[]>([]);
  const [manageLoading, setManageLoading] = useState(false);

  const channelNames = useMemo(() => channelNameById(channels), [channels]);

  async function refresh() {
    setLoading(true);
    setError(null);
    try {
      const [rs, cs] = await Promise.all([listRoutes(), listChannels()]);
      setRoutes(rs);
      setChannels(cs);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void refresh();
  }, []);

  function openCreate() {
    setRouteMode("create");
    setEditRouteId(null);
    setDraft(emptyDraft());
    setRouteModalOpen(true);
  }

  function openEdit(r: Route) {
    setRouteMode("edit");
    setEditRouteId(r.id);
    setDraft({ name: r.name, protocol: r.protocol, match_model: r.match_model, enabled: r.enabled });
    setRouteModalOpen(true);
  }

  async function submitRoute() {
    setError(null);
    try {
      if (draft.name.trim().length === 0) throw new Error("name 不能为空");
      if (routeMode === "create") {
        await createRoute({ ...draft, name: draft.name.trim() });
      } else {
        if (!editRouteId) throw new Error("missing route id");
        await updateRoute(editRouteId, { name: draft.name.trim(), match_model: draft.match_model, enabled: draft.enabled });
      }
      setRouteModalOpen(false);
      await refresh();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  async function onDelete(r: Route) {
    if (!confirm(`删除路由 "${r.name}"？`)) return;
    setError(null);
    try {
      await deleteRoute(r.id);
      await refresh();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  async function openManage(r: Route) {
    setManageOpen(true);
    setManageRoute(r);
    setManageLoading(true);
    setError(null);
    try {
      const items = await listRouteChannels(r.id);
      const ordered = [...items].sort((a, b) => a.priority - b.priority).map((i) => i.channel_id);
      setAssigned(ordered);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setManageLoading(false);
    }
  }

  function move(id: string, dir: -1 | 1) {
    setAssigned((list) => {
      const idx = list.indexOf(id);
      if (idx < 0) return list;
      const j = idx + dir;
      if (j < 0 || j >= list.length) return list;
      const next = [...list];
      [next[idx], next[j]] = [next[j], next[idx]];
      return next;
    });
  }

  function remove(id: string) {
    setAssigned((list) => list.filter((x) => x !== id));
  }

  function add(id: string) {
    setAssigned((list) => (list.includes(id) ? list : [...list, id]));
  }

  async function saveManage() {
    if (!manageRoute) return;
    setManageLoading(true);
    setError(null);
    try {
      await reorderRouteChannels(manageRoute.id, assigned);
      setManageOpen(false);
      setManageRoute(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setManageLoading(false);
    }
  }

  const available = useMemo(() => {
    if (!manageRoute) return [];
    return channels
      .filter((c) => c.protocol === manageRoute.protocol)
      .filter((c) => !assigned.includes(c.id))
      .sort((a, b) => a.name.localeCompare(b.name));
  }, [channels, assigned, manageRoute]);

  return (
    <div className="stack">
      <div className="row" style={{ justifyContent: "space-between" }}>
        <div>
          <h1 className="page-title">Routes</h1>
          <p className="page-subtitle">配置路由规则与渠道优先级（按协议）。</p>
        </div>
        <div className="row">
          <Button onClick={refresh} disabled={loading}>
            {loading ? "刷新中…" : "刷新"}
          </Button>
          <Button variant="primary" onClick={openCreate}>
            新增路由
          </Button>
        </div>
      </div>

      {error ? <div className="error">{clampStr(error, 2000)}</div> : null}

      <Card title="Routes List">
        {routes.length === 0 ? (
          <div className="muted">暂无路由。先点击“新增路由”。</div>
        ) : (
          <table className="table">
            <thead>
              <tr>
                <th style={{ width: 180 }}>Name</th>
                <th style={{ width: 110 }}>Protocol</th>
                <th>Match Model</th>
                <th style={{ width: 100 }}>Enabled</th>
                <th style={{ width: 220 }}>Actions</th>
              </tr>
            </thead>
            <tbody>
              {routes.map((r) => (
                <tr key={r.id}>
                  <td>
                    <div style={{ fontWeight: 700 }}>{r.name}</div>
                    <div className="muted">{formatDateTime(r.updated_at_ms)}</div>
                  </td>
                  <td>
                    <Badge>{r.protocol}</Badge>
                  </td>
                  <td className="muted">{r.match_model ?? "-"}</td>
                  <td>{r.enabled ? <Badge kind="ok">ON</Badge> : <Badge kind="bad">OFF</Badge>}</td>
                  <td>
                    <div className="row">
                      <Button onClick={() => openEdit(r)}>编辑</Button>
                      <Button onClick={() => openManage(r)}>渠道优先级</Button>
                      <Button variant="danger" onClick={() => onDelete(r)}>
                        删除
                      </Button>
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </Card>

      <Modal
        open={routeModalOpen}
        title={routeMode === "create" ? "新增路由" : "编辑路由"}
        onClose={() => setRouteModalOpen(false)}
      >
        <div className="stack">
          <div className="grid-2">
            <div className="field">
              <label>name</label>
              <input className="input" value={draft.name} onChange={(e) => setDraft((d) => ({ ...d, name: e.target.value }))} />
            </div>
            <div className="field">
              <label>protocol</label>
              <select
                className="select"
                value={draft.protocol}
                onChange={(e) => setDraft((d) => ({ ...d, protocol: e.target.value as Protocol }))}
                disabled={routeMode === "edit"}
                title={routeMode === "edit" ? "当前版本暂不支持修改 protocol" : undefined}
              >
                <option value="openai">openai</option>
                <option value="anthropic">anthropic</option>
                <option value="gemini">gemini</option>
              </select>
            </div>
            <div className="field" style={{ gridColumn: "1 / -1" }}>
              <label>match_model（可选）</label>
              <input
                className="input"
                value={draft.match_model ?? ""}
                onChange={(e) => setDraft((d) => ({ ...d, match_model: e.target.value.trim() ? e.target.value : null }))}
                placeholder="例如：gpt-4o / claude-3-5-sonnet"
              />
            </div>
            <div className="field">
              <label>enabled</label>
              <select className="select" value={draft.enabled ? "true" : "false"} onChange={(e) => setDraft((d) => ({ ...d, enabled: e.target.value === "true" }))}>
                <option value="true">true</option>
                <option value="false">false</option>
              </select>
            </div>
          </div>

          <div className="row" style={{ justifyContent: "flex-end" }}>
            <Button variant="ghost" onClick={() => setRouteModalOpen(false)}>
              取消
            </Button>
            <Button variant="primary" onClick={submitRoute}>
              保存
            </Button>
          </div>
        </div>
      </Modal>

      <Modal open={manageOpen} title={`渠道优先级：${manageRoute?.name ?? ""}`} onClose={() => setManageOpen(false)}>
        {manageLoading ? (
          <div className="muted">加载中…</div>
        ) : !manageRoute ? (
          <div className="muted">未选择 route。</div>
        ) : (
          <div className="stack">
            <div className="grid-2">
              <div>
                <div className="muted" style={{ fontSize: 12 }}>
                  已绑定（从上到下优先级递减）
                </div>
                <div style={{ marginTop: 8 }} className="stack">
                  {assigned.length === 0 ? (
                    <div className="muted">暂无绑定渠道。</div>
                  ) : (
                    assigned.map((id, idx) => (
                      <div key={id} className="row" style={{ justifyContent: "space-between" }}>
                        <div>
                          <div style={{ fontWeight: 700 }}>
                            {idx + 1}. {channelNames.get(id) ?? id}
                          </div>
                          <div className="muted">
                            <code>{id}</code>
                          </div>
                        </div>
                        <div className="row">
                          <Button onClick={() => move(id, -1)} disabled={idx === 0}>
                            ↑
                          </Button>
                          <Button onClick={() => move(id, 1)} disabled={idx === assigned.length - 1}>
                            ↓
                          </Button>
                          <Button variant="danger" onClick={() => remove(id)}>
                            移除
                          </Button>
                        </div>
                      </div>
                    ))
                  )}
                </div>
              </div>
              <div>
                <div className="muted" style={{ fontSize: 12 }}>
                  可添加（仅显示与 route 协议一致的渠道）
                </div>
                <div style={{ marginTop: 8 }} className="stack">
                  {available.length === 0 ? (
                    <div className="muted">暂无可添加渠道。</div>
                  ) : (
                    available.map((c) => (
                      <div key={c.id} className="row" style={{ justifyContent: "space-between" }}>
                        <div>
                          <div style={{ fontWeight: 700 }}>{c.name}</div>
                          <div className="muted">
                            <code>{clampStr(c.base_url, 46)}</code>
                          </div>
                        </div>
                        <Button onClick={() => add(c.id)}>添加</Button>
                      </div>
                    ))
                  )}
                </div>
              </div>
            </div>

            <div className="row" style={{ justifyContent: "flex-end" }}>
              <Button variant="ghost" onClick={() => setManageOpen(false)}>
                取消
              </Button>
              <Button variant="primary" onClick={saveManage} disabled={manageLoading}>
                保存
              </Button>
            </div>
          </div>
        )}
      </Modal>
    </div>
  );
}
