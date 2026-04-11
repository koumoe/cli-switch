import {
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import { useRouterState } from "@tanstack/react-router";

import { getHealth } from "@/api";
import type { Health } from "@/types/api";

import { Sidebar, type SidebarRoute } from "./sidebar";

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
}: {
  children: ReactNode;
}) {
  const pathname = useRouterState({
    select: (state) => state.location.pathname,
  });
  const activeRoute = useMemo(() => routeFromPath(pathname), [pathname]);
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

  return (
    <div className="flex h-full overflow-hidden bg-background text-foreground">
      <Sidebar activeRoute={activeRoute} health={health} />

      <div className="flex min-w-0 flex-1 flex-col overflow-hidden">
        <main className="flex-1 overflow-hidden">{children}</main>
      </div>
    </div>
  );
}
