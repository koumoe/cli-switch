import React, { useEffect, useMemo, useState } from "react";
import {
  LayoutGrid,
  Radio,
  GitBranch,
  Activity,
  Settings,
  Sun,
  Moon,
  Monitor,
  Zap,
} from "lucide-react";
import { useTheme, type Theme } from "@/lib/theme";
import { useI18n } from "@/lib/i18n";
import { Button } from "@/components/ui";
import { getHealth } from "./api";

import { OverviewPage } from "./pages/OverviewPage";
import { ChannelsPage } from "./pages/ChannelsPage";
import { RoutesPage } from "./pages/RoutesPage";
import { MonitorPage } from "./pages/MonitorPage";
import { SettingsPage } from "./pages/SettingsPage";

type AppRoute = "overview" | "channels" | "routes" | "monitor" | "settings";

const NAV_ITEMS: { route: AppRoute; labelKey: string; icon: React.ElementType }[] = [
  { route: "overview", labelKey: "nav.overview", icon: LayoutGrid },
  { route: "channels", labelKey: "nav.channels", icon: Radio },
  { route: "routes", labelKey: "nav.routes", icon: GitBranch },
  { route: "monitor", labelKey: "nav.monitor", icon: Activity },
  { route: "settings", labelKey: "nav.settings", icon: Settings },
];

function routeFromPath(pathname: string): AppRoute {
  if (pathname === "/") return "overview";
  if (pathname.startsWith("/channels")) return "channels";
  if (pathname.startsWith("/routes")) return "routes";
  if (pathname.startsWith("/monitor")) return "monitor";
  if (pathname.startsWith("/settings")) return "settings";
  return "overview";
}

function hrefFor(route: AppRoute): string {
  if (route === "overview") return "/";
  return `/${route}`;
}

function navigate(to: string) {
  if (window.location.pathname === to) return;
  window.history.pushState({}, "", to);
  window.dispatchEvent(new PopStateEvent("popstate"));
}

function NavLink({
  route,
  current,
  label,
  icon: Icon,
}: {
  route: AppRoute;
  current: AppRoute;
  label: string;
  icon: React.ElementType;
}) {
  const href = hrefFor(route);
  const active = current === route;
  return (
    <a
      className={`flex items-center gap-3 px-3 py-2 rounded-lg text-sm transition-colors ${
        active
          ? "bg-accent text-accent-foreground font-medium"
          : "text-muted-foreground hover:bg-accent hover:text-accent-foreground"
      }`}
      href={href}
      onClick={(e) => {
        e.preventDefault();
        navigate(href);
      }}
    >
      <Icon className="h-4 w-4" />
      {label}
    </a>
  );
}

function ThemeToggle() {
  const { theme, setTheme } = useTheme();
  const { t } = useI18n();

  const cycleTheme = () => {
    const next: Record<Theme, Theme> = {
      light: "dark",
      dark: "system",
      system: "light",
    };
    setTheme(next[theme]);
  };

  const Icon = theme === "light" ? Sun : theme === "dark" ? Moon : Monitor;
  const title =
    theme === "light" ? t("theme.light") : theme === "dark" ? t("theme.dark") : t("theme.system");

  return (
    <Button variant="ghost" size="icon" onClick={cycleTheme} title={title}>
      <Icon className="h-4 w-4" />
    </Button>
  );
}

function StatusIndicator({ status }: { status: string }) {
  const { t } = useI18n();
  const isOk = status === "ok";
  const label =
    status === "..."
      ? t("status.checking")
      : status === "ok"
        ? t("status.running")
        : status === "离线"
          ? t("status.offline")
          : status;
  return (
    <div className="flex items-center gap-2 text-xs text-muted-foreground">
      <span
        className={`h-2 w-2 rounded-full ${
          isOk ? "bg-success" : "bg-destructive"
        }`}
      />
      {label}
    </div>
  );
}

export default function App() {
  const [pathname, setPathname] = useState(() => window.location.pathname);
  const route = useMemo(() => routeFromPath(pathname), [pathname]);
  const [health, setHealth] = useState<string>("...");
  const { t } = useI18n();

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
        if (!cancelled) setHealth("离线");
      });
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <div className="flex h-full">
      {/* 侧边栏 */}
      <aside className="w-56 flex-shrink-0 border-r bg-sidebar flex flex-col">
        {/* Logo */}
        <div className="h-14 flex items-center gap-2 px-4 border-b">
          <Zap className="h-5 w-5 text-foreground" />
          <span className="font-semibold">CliSwitch</span>
        </div>

        {/* 导航 */}
        <nav className="flex-1 p-3 space-y-1">
          {NAV_ITEMS.map((item) => (
            <NavLink
              key={item.route}
              route={item.route}
              current={route}
              label={t(item.labelKey)}
              icon={item.icon}
            />
          ))}
        </nav>

        {/* 底部 */}
        <div className="p-3 border-t space-y-3">
          <StatusIndicator status={health} />
          <div className="flex items-center justify-between">
            <span className="text-xs text-muted-foreground">v0.1.0</span>
            <ThemeToggle />
          </div>
        </div>
      </aside>

      {/* 主内容 */}
      <main className="flex-1 overflow-auto">
        <div className="p-6 max-w-6xl mx-auto">
          {route === "overview" ? (
            <OverviewPage />
          ) : route === "channels" ? (
            <ChannelsPage />
          ) : route === "routes" ? (
            <RoutesPage />
          ) : route === "monitor" ? (
            <MonitorPage />
          ) : (
            <SettingsPage />
          )}
        </div>
      </main>
    </div>
  );
}
