import React, { useEffect, useState } from "react";

type Health = { status: string };

async function fetchHealth(): Promise<Health> {
  const res = await fetch("/api/health");
  if (!res.ok) throw new Error(`/api/health failed: ${res.status}`);
  return (await res.json()) as Health;
}

export default function App() {
  const [health, setHealth] = useState<Health | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    fetchHealth()
      .then(setHealth)
      .catch((e) => setError(e instanceof Error ? e.message : String(e)));
  }, []);

  return (
    <div style={{ fontFamily: "system-ui, sans-serif", padding: 24 }}>
      <h1 style={{ margin: 0 }}>CliSwitch</h1>
      <p style={{ marginTop: 8, color: "#555" }}>
        本地无感切换多渠道的 CLI 代理（MVP UI 骨架）。
      </p>

      <div
        style={{
          marginTop: 16,
          padding: 12,
          border: "1px solid #ddd",
          borderRadius: 8
        }}
      >
        <div style={{ fontWeight: 600 }}>服务状态</div>
        {error ? (
          <div style={{ marginTop: 8, color: "#b00020" }}>{error}</div>
        ) : health ? (
          <div style={{ marginTop: 8, color: "#0a7a28" }}>{health.status}</div>
        ) : (
          <div style={{ marginTop: 8 }}>加载中...</div>
        )}
      </div>

      <div style={{ marginTop: 16, color: "#777" }}>
        下一步：实现 Channels/Routes/Pricing/Stats 的管理 API 与页面。
      </div>
    </div>
  );
}

