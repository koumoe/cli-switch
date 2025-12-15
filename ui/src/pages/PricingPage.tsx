import React, { useEffect, useState } from "react";
import { pricingModels, pricingStatus, pricingSync, type PricingModel, type PricingStatus } from "../api";
import { Badge, Button, Card } from "../components/ui";
import { clampStr, formatDateTime } from "../lib";

export function PricingPage() {
  const [status, setStatus] = useState<PricingStatus | null>(null);
  const [models, setModels] = useState<PricingModel[]>([]);
  const [query, setQuery] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function refresh() {
    setLoading(true);
    setError(null);
    try {
      const [s, m] = await Promise.all([pricingStatus(), pricingModels(query, 200)]);
      setStatus(s);
      setModels(m);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void refresh();
  }, []);

  async function onSync() {
    setLoading(true);
    setError(null);
    try {
      await pricingSync();
      await refresh();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setLoading(false);
    }
  }

  async function onSearch() {
    await refresh();
  }

  return (
    <div className="stack">
      <div className="row" style={{ justifyContent: "space-between" }}>
        <div>
          <h1 className="page-title">Pricing</h1>
          <p className="page-subtitle">从 OpenRouter 同步模型价格并查询。</p>
        </div>
        <div className="row">
          <Button variant="primary" onClick={onSync} disabled={loading}>
            {loading ? "同步中…" : "同步价格"}
          </Button>
        </div>
      </div>

      {error ? <div className="error">{clampStr(error, 2000)}</div> : null}

      <Card title="Status">
        <div className="row" style={{ justifyContent: "space-between" }}>
          <div>
            <div className="muted" style={{ fontSize: 12 }}>
              Model Count
            </div>
            <div style={{ marginTop: 6, fontSize: 18, fontWeight: 750 }}>{status?.count ?? "-"}</div>
          </div>
          <div>
            <div className="muted" style={{ fontSize: 12 }}>
              Last Sync
            </div>
            <div style={{ marginTop: 6, fontSize: 12 }}>{formatDateTime(status?.last_sync_ms)}</div>
          </div>
        </div>
      </Card>

      <Card
        title="Search"
        actions={
          <div className="row">
            <input className="input" value={query} onChange={(e) => setQuery(e.target.value)} placeholder="openai/gpt-4o" />
            <Button onClick={onSearch} disabled={loading}>
              查询
            </Button>
          </div>
        }
      >
        {models.length === 0 ? (
          <div className="muted">暂无数据（先同步价格）。</div>
        ) : (
          <table className="table">
            <thead>
              <tr>
                <th>Model</th>
                <th style={{ width: 140 }}>Prompt</th>
                <th style={{ width: 140 }}>Completion</th>
                <th style={{ width: 140 }}>Request</th>
                <th style={{ width: 180 }}>Updated</th>
              </tr>
            </thead>
            <tbody>
              {models.map((m) => (
                <tr key={m.model_id}>
                  <td>
                    <div style={{ fontWeight: 700 }}>{m.model_id}</div>
                  </td>
                  <td className="muted">{m.prompt_price ?? "-"}</td>
                  <td className="muted">{m.completion_price ?? "-"}</td>
                  <td className="muted">{m.request_price ?? "-"}</td>
                  <td className="muted">
                    <Badge>{formatDateTime(m.updated_at_ms)}</Badge>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </Card>
    </div>
  );
}
