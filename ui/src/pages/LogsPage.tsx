import React, { useEffect, useMemo, useState } from "react";
import { listChannels, usageRecent, type Channel, type UsageEvent } from "../api";
import { Badge, Button, Card } from "../components/ui";
import { clampStr, formatDateTime, formatDuration } from "../lib";

function channelNameMap(channels: Channel[]): Map<string, string> {
  const m = new Map<string, string>();
  for (const c of channels) m.set(c.id, c.name);
  return m;
}

export function LogsPage() {
  const [events, setEvents] = useState<UsageEvent[]>([]);
  const [channels, setChannels] = useState<Channel[]>([]);
  const [onlyFailures, setOnlyFailures] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const channelNames = useMemo(() => channelNameMap(channels), [channels]);

  const filtered = useMemo(() => {
    if (!onlyFailures) return events;
    return events.filter((e) => !e.success);
  }, [events, onlyFailures]);

  async function refresh() {
    setLoading(true);
    setError(null);
    try {
      const [cs, es] = await Promise.all([listChannels(), usageRecent(200)]);
      setChannels(cs);
      setEvents(es);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void refresh();
  }, []);

  return (
    <div className="stack">
      <div className="row" style={{ justifyContent: "space-between" }}>
        <div>
          <h1 className="page-title">Logs</h1>
          <p className="page-subtitle">最近请求事件（简版）。</p>
        </div>
        <div className="row">
          <label className="muted" style={{ fontSize: 12 }}>
            <input type="checkbox" checked={onlyFailures} onChange={(e) => setOnlyFailures(e.target.checked)} /> 仅失败
          </label>
          <Button onClick={refresh} disabled={loading}>
            {loading ? "刷新中…" : "刷新"}
          </Button>
        </div>
      </div>

      {error ? <div className="error">{clampStr(error, 2000)}</div> : null}

      <Card title="Recent Events">
        {filtered.length === 0 ? (
          <div className="muted">暂无事件。</div>
        ) : (
          <table className="table">
            <thead>
              <tr>
                <th style={{ width: 170 }}>Time</th>
                <th style={{ width: 110 }}>Protocol</th>
                <th style={{ width: 170 }}>Channel</th>
                <th style={{ width: 90 }}>Status</th>
                <th style={{ width: 110 }}>Latency</th>
                <th style={{ width: 160 }}>Model</th>
                <th>Error</th>
              </tr>
            </thead>
            <tbody>
              {filtered.map((e) => (
                <tr key={e.id}>
                  <td className="muted">{formatDateTime(e.ts_ms)}</td>
                  <td>
                    <Badge>{e.protocol}</Badge>
                  </td>
                  <td>
                    <div style={{ fontWeight: 700 }}>{channelNames.get(e.channel_id) ?? "-"}</div>
                    <div className="muted">
                      <code>{clampStr(e.channel_id, 18)}</code>
                    </div>
                  </td>
                  <td>
                    {e.success ? <Badge kind="ok">{e.http_status ?? 200}</Badge> : <Badge kind="bad">{e.http_status ?? "ERR"}</Badge>}
                  </td>
                  <td className="muted">{formatDuration(e.latency_ms)}</td>
                  <td className="muted">{e.model ?? "-"}</td>
                  <td className="muted">{e.error_kind ? clampStr(e.error_kind, 120) : "-"}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </Card>
    </div>
  );
}
