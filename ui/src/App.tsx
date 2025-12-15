import React, { useEffect, useMemo, useState } from "react";
import { DashboardPage } from "./pages/DashboardPage";
import { ChannelsPage } from "./pages/ChannelsPage";
import { LogsPage } from "./pages/LogsPage";
import { PricingPage } from "./pages/PricingPage";
import { RoutesPage } from "./pages/RoutesPage";
import { getHealth } from "./api";

type AppRoute = "dashboard" | "channels" | "routes" | "pricing" | "logs";

function routeFromPath(pathname: string): AppRoute {
  if (pathname === "/") return "dashboard";
  if (pathname.startsWith("/channels")) return "channels";
  if (pathname.startsWith("/routes")) return "routes";
  if (pathname.startsWith("/pricing")) return "pricing";
  if (pathname.startsWith("/logs")) return "logs";
  return "dashboard";
}

function hrefFor(route: AppRoute): string {
  switch (route) {
    case "dashboard":
      return "/";
    case "channels":
      return "/channels";
    case "routes":
      return "/routes";
    case "pricing":
      return "/pricing";
    case "logs":
      return "/logs";
  }
}

function navigate(to: string) {
  if (window.location.pathname === to) return;
  window.history.pushState({}, "", to);
  window.dispatchEvent(new PopStateEvent("popstate"));
}

function NavLink({
  route,
  current,
  label
}: {
  route: AppRoute;
  current: AppRoute;
  label: string;
}) {
  const href = hrefFor(route);
  const active = current === route;
  return (
    <a
      className={active ? "nav-item nav-item-active" : "nav-item"}
      href={href}
      onClick={(e) => {
        e.preventDefault();
        navigate(href);
      }}
    >
      {label}
    </a>
  );
}

export default function App() {
  const [pathname, setPathname] = useState(() => window.location.pathname);
  const route = useMemo(() => routeFromPath(pathname), [pathname]);
  const [health, setHealth] = useState<string>("检查中…");

  useEffect(() => {
    const onPop = () => setPathname(window.location.pathname);
    window.addEventListener("popstate", onPop);
    return () => window.removeEventListener("popstate", onPop);
  }, []);

  useEffect(() => {
    let cancelled = false;
    getHealth()
      .then((h) => {
        if (!cancelled) setHealth(h.status);
      })
      .catch(() => {
        if (!cancelled) setHealth("unreachable");
      });
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <div className="app">
      <aside className="sidebar">
        <div className="brand">
          <div className="brand-title">CliSwitch</div>
          <div className="brand-sub">本地多渠道 CLI 代理</div>
        </div>
        <nav className="nav">
          <NavLink route="dashboard" current={route} label="Dashboard" />
          <NavLink route="channels" current={route} label="Channels" />
          <NavLink route="routes" current={route} label="Routes" />
          <NavLink route="pricing" current={route} label="Pricing" />
          <NavLink route="logs" current={route} label="Logs" />
        </nav>
        <div className="sidebar-footer">
          <div className="muted">Health: {health}</div>
        </div>
      </aside>

      <main className="main">
        {route === "dashboard" ? (
          <DashboardPage />
        ) : route === "channels" ? (
          <ChannelsPage />
        ) : route === "routes" ? (
          <RoutesPage />
        ) : route === "pricing" ? (
          <PricingPage />
        ) : (
          <LogsPage />
        )}
      </main>
    </div>
  );
}
