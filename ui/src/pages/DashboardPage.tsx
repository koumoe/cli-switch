import React, { useEffect, useMemo, useState } from "react";
import { pricingStatus, pricingSync, statsSummary, usageRecent, type StatsSummary, type UsageEvent } from "../api";
import { Button, Card, Badge } from "../components/ui";
import { formatDateTime, formatDuration, clampStr } from "../lib";

function SummaryTile({ s }: { s: StatsSummary }) {
  const successRate = s.requests > 0 ? Math.round((s.success / s.requests) * 100) : 0;
  return (
    <div className="grid-3">
      <div className="card" style={{ boxShadow: "none", background: "var(--panel2)" }}>
        <div className="muted" style={{ fontSize: 12 }}>
          Requests
        </div>
        <div style={{ marginTop: 6, fontSize: 18, fontWeight: 750 }}>{s.requests}</div>
      </div>
      <div className="card" style={{ boxShadow: "none", background: "var(--panel2)" }}>
        <div className="muted" style={{ fontSize: 12 }}>
          Success
        </div>
        <div style={{ marginTop: 6, fontSize: 18, fontWeight: 750 }}>{successRate}%</div>
      </div>
      <div className="card" style={{ boxShadow: "none", background: "var(--panel2)" }}>
        <div className="muted" style={{ fontSize: 12 }}>
          Tokens
        </div>
        <div style={{ marginTop: 6, fontSize: 18, fontWeight: 750 }}>{s.total_tokens}</div>
      </div>
    </div>
  );
}

export function DashboardPage() {
  const [today, setToday] = useState<StatsSummary | null>(null);
  const [month, setMonth] = useState<StatsSummary | null>(null);
  const [pricing, setPricing] = useState<{ count: number; last_sync_ms: number | null } | null>(null);
  const [recent, setRecent] = useState<UsageEvent[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [syncing, setSyncing] = useState(false);

  const recentFailures = useMemo(() => recent.filter((e) => !e.success).slice(0, 10), [recent]);

  async function refresh() {
    setError(null);
    try {
      const [t, m, p, r] = await Promise.all([
        statsSummary("today"),
        statsSummary("month"),
        pricingStatus(),
        usageRecent(50)
      ]);
      setToday(t);
      setMonth(m);
      setPricing(p);
      setRecent(r);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  useEffect(() => {
    void refresh();
  }, []);

  async function onSyncPricing() {
    setSyncing(true);
    setError(null);
    try {
      await pricingSync();
      await refresh();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setSyncing(false);
    }
  }

  return (
    <div className="stack">
      <div>
        <h1 className="page-title">Dashboard</h1>
        <p className="page-subtitle">今日概览、价格同步状态与最近请求。</p>
      </div>

      {error ? <div className="error">{error}</div> : null}

      <div className="grid-2">
        <Card
          title="Pricing"
          actions={
            <Button variant="primary" disabled={syncing} onClick={onSyncPricing} title="从 OpenRouter 同步模型价格">
              {syncing ? "同步中…" : "同步价格"}
            </Button>
          }
        >
          <div className="row" style={{ justifyContent: "space-between" }}>
            <div>
              <div className="muted" style={{ fontSize: 12 }}>
                Model Count
              </div>
              <div style={{ marginTop: 6, fontSize: 18, fontWeight: 750 }}>{pricing?.count ?? "-"}</div>
            </div>
            <div>
              <div className="muted" style={{ fontSize: 12 }}>
                Last Sync
              </div>
              <div style={{ marginTop: 6, fontSize: 12 }}>{formatDateTime(pricing?.last_sync_ms)}</div>
            </div>
          </div>
        </Card>

        <Card title="Today">{today ? <SummaryTile s={today} /> : <div className="muted">加载中…</div>}</Card>
      </div>

      <Card title="Month">{month ? <SummaryTile s={month} /> : <div className="muted">加载中…</div>}</Card>

      <Card title="Recent Failures" actions={<Button onClick={refresh}>刷新</Button>}>
        {recentFailures.length === 0 ? (
          <div className="muted">暂无失败记录。</div>
        ) : (
          <table className="table">
            <thead>
              <tr>
                <th style={{ width: 170 }}>Time</th>
                <th style={{ width: 100 }}>Protocol</th>
                <th style={{ width: 90 }}>Status</th>
                <th style={{ width: 110 }}>Latency</th>
                <th>Model</th>
                <th>Error</th>
              </tr>
            </thead>
            <tbody>
              {recentFailures.map((e) => (
                <tr key={e.id}>
                  <td className="muted">{formatDateTime(e.ts_ms)}</td>
                  <td>
                    <Badge>{e.protocol}</Badge>
                  </td>
                  <td>
                    <Badge kind="bad">{e.http_status ?? "ERR"}</Badge>
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
