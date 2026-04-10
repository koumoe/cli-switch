import {
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import { useRouterState } from "@tanstack/react-router";

import { getHealth } from "@/api";
import { cn } from "@/lib/utils";
import type { Health } from "@/types/api";

import { Sidebar, type SidebarRoute } from "./sidebar";

const SIDEBAR_KEY = "cliswitch-sidebar-collapsed";
const AUTO_HEIGHT_ROUTES: SidebarRoute[] = [
  "overview",
  "channels",
  "accounts",
  "settings",
];

function routeFromPath(pathname: string): SidebarRoute {
  if (pathname === "/") return "overview";
  if (pathname.startsWith("/channels")) return "channels";
  if (pathname.startsWith("/accounts")) return "accounts";
  if (pathname.startsWith("/prompts")) return "prompts";
  if (pathname.startsWith("/monitor")) return "monitor";
  if (pathname.startsWith("/logs")) return "logs";
  if (pathname.startsWith("/settings")) return "settings";
  return "overview";
}

export function PageShell({
  children,
  header,
}: {
  children: ReactNode;
  header?: ReactNode;
}) {
  const pathname = useRouterState({
    select: (state) => state.location.pathname,
  });
  const activeRoute = useMemo(() => routeFromPath(pathname), [pathname]);
  const useAutoHeightLayout = AUTO_HEIGHT_ROUTES.includes(activeRoute);
  const [collapsed, setCollapsed] = useState(() => {
    if (typeof window === "undefined") return false;
    const value = window.localStorage.getItem(SIDEBAR_KEY);
    if (value === null) return true;
    return value === "true";
  });
  const [health, setHealth] = useState<Health>({ status: "..." });

  useEffect(() => {
    let cancelled = false;

    getHealth()
      .then((nextHealth) => {
        if (!cancelled) {
          setHealth(nextHealth);
        }
      })
      .catch(() => {
        if (!cancelled) {
          setHealth({ status: "offline" });
        }
      });

    return () => {
      cancelled = true;
    };
  }, []);

  const toggleCollapsed = () => {
    setCollapsed((prev) => {
      const next = !prev;
      window.localStorage.setItem(SIDEBAR_KEY, String(next));
      return next;
    });
  };

  return (
    <div className="app-canvas flex h-full overflow-hidden text-foreground">
      <Sidebar
        activeRoute={activeRoute}
        collapsed={collapsed}
        health={health}
        onToggle={toggleCollapsed}
      />

      <div className="flex min-w-0 flex-1 flex-col">
        {header ? (
          <div className="border-b border-border/70 bg-background/45 px-5 py-3 supports-[backdrop-filter]:backdrop-blur-xl">
            {header}
          </div>
        ) : null}
        <main
          className={cn(
            "flex-1 bg-transparent",
            useAutoHeightLayout ? "overflow-auto" : "overflow-hidden",
          )}
        >
          <div
            className={cn(
              "mx-auto w-full max-w-7xl p-5",
              useAutoHeightLayout
                ? "min-h-full"
                : "flex h-full min-h-0 flex-col overflow-hidden",
            )}
          >
            {children}
          </div>
        </main>
      </div>
    </div>
  );
}
