import React, { useEffect, useMemo, useState } from "react";
import {
  LayoutGrid,
  Radio,
  Activity,
  ScrollText,
  Settings,
  Zap,
  PanelLeftClose,
  PanelLeftOpen,
} from "lucide-react";
import { useI18n } from "@/lib/i18n";
import { useTheme } from "@/lib/theme";
import { cn } from "@/lib/utils";
import {
  Button,
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  Switch,
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui";
import { toast } from "sonner";
import { getHealth, pricingStatus, pricingSync } from "./api";

import { OverviewPage } from "./pages/OverviewPage";
import { ChannelsPage } from "./pages/ChannelsPage";
import { MonitorPage } from "./pages/MonitorPage";
import { LogsPage } from "./pages/LogsPage";
import { SettingsPage } from "./pages/SettingsPage";

type AppRoute = "overview" | "channels" | "monitor" | "logs" | "settings";

const NAV_ITEMS: { route: AppRoute; labelKey: string; icon: React.ElementType }[] = [
  { route: "overview", labelKey: "nav.overview", icon: LayoutGrid },
  { route: "channels", labelKey: "nav.channels", icon: Radio },
  { route: "monitor", labelKey: "nav.monitor", icon: Activity },
  { route: "logs", labelKey: "nav.logs", icon: ScrollText },
  { route: "settings", labelKey: "nav.settings", icon: Settings },
];

const SIDEBAR_KEY = "cliswitch-sidebar-collapsed";
const PRICING_ONBOARDING_SHOWN_KEY = "cliswitch-pricing-onboarding-shown";

function routeFromPath(pathname: string): AppRoute {
  if (pathname === "/") return "overview";
  if (pathname.startsWith("/channels")) return "channels";
  if (pathname.startsWith("/monitor")) return "monitor";
  if (pathname.startsWith("/logs")) return "logs";
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

function postIpc(payload: unknown) {
  const anyWindow = window as any;
  const fn = anyWindow?.ipc?.postMessage as ((msg: string) => void) | undefined;
  if (fn) fn(JSON.stringify(payload));
}

function NavTab({
  route,
  current,
  label,
  icon: Icon,
  collapsed,
}: {
  route: AppRoute;
  current: AppRoute;
  label: string;
  icon: React.ElementType;
  collapsed: boolean;
}) {
  const href = hrefFor(route);
  const active = current === route;

  const link = (
    <a
      aria-current={active ? "page" : undefined}
      className={cn(
        "group flex items-center rounded-md transition-colors select-none",
        collapsed ? "justify-center p-2" : "gap-2 px-2.5 py-1.5",
        active
          ? "bg-sidebar-accent text-sidebar-accent-foreground font-medium"
          : "text-sidebar-foreground/70 hover:text-sidebar-foreground hover:bg-sidebar-accent/60"
      )}
      href={href}
      onClick={(e) => {
        e.preventDefault();
        navigate(href);
      }}
    >
      <Icon
        className={cn(
          "h-4 w-4 flex-shrink-0",
          active ? "text-sidebar-accent-foreground" : "text-muted-foreground group-hover:text-sidebar-foreground"
        )}
      />
      {!collapsed && <span className="text-[13px]">{label}</span>}
    </a>
  );

  if (collapsed) {
    return (
      <Tooltip>
        <TooltipTrigger asChild>{link}</TooltipTrigger>
        <TooltipContent side="right" sideOffset={8}>
          {label}
        </TooltipContent>
      </Tooltip>
    );
  }

  return link;
}

function StatusIndicator({ status, collapsed }: { status: string; collapsed: boolean }) {
  const { t } = useI18n();
  const isOk = status === "ok";
  const isChecking = status === "...";
  const label =
    status === "..."
      ? t("status.checking")
      : status === "ok"
        ? t("status.running")
        : status === "离线"
          ? t("status.offline")
          : status;

  const dot = (
    <span
      className={cn(
        "h-1.5 w-1.5 rounded-full flex-shrink-0",
        isOk ? "bg-success" : isChecking ? "bg-muted-foreground" : "bg-destructive"
      )}
    />
  );

  if (collapsed) {
    return (
      <Tooltip>
        <TooltipTrigger asChild>
          <div className="p-1.5 flex items-center justify-center">{dot}</div>
        </TooltipTrigger>
        <TooltipContent side="right" sideOffset={8}>
          {label}
        </TooltipContent>
      </Tooltip>
    );
  }

  return (
    <div className="flex items-center gap-1.5 text-xs text-muted-foreground">
      {dot}
      {label}
    </div>
  );
}

export default function App() {
  const [pathname, setPathname] = useState(() => window.location.pathname);
  const route = useMemo(() => routeFromPath(pathname), [pathname]);
  const [health, setHealth] = useState<string>("...");
  const [collapsed, setCollapsed] = useState(() => {
    if (typeof window === "undefined") return false;
    const v = localStorage.getItem(SIDEBAR_KEY);
    if (v === null) return true;
    return v === "true";
  });
  const { t } = useI18n();
  const [pricingOnboardingOpen, setPricingOnboardingOpen] = useState(false);
  const [pricingSyncing, setPricingSyncing] = useState(false);
  const [closePromptOpen, setClosePromptOpen] = useState(false);
  const [closeRemember, setCloseRemember] = useState(false);
  const [closeDecisionSent, setCloseDecisionSent] = useState(false);

  // 确保主题在应用启动时被应用
  useTheme();

  const toggleCollapsed = () => {
    setCollapsed((prev) => {
      const next = !prev;
      localStorage.setItem(SIDEBAR_KEY, String(next));
      return next;
    });
  };

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

  useEffect(() => {
    const shown = localStorage.getItem(PRICING_ONBOARDING_SHOWN_KEY) === "true";
    if (shown) return;

    pricingStatus()
      .then((st) => {
        if (st.count <= 0) {
          localStorage.setItem(PRICING_ONBOARDING_SHOWN_KEY, "true");
          setPricingOnboardingOpen(true);
        }
      })
      .catch(() => {
        // ignore
      });
  }, []);

  useEffect(() => {
    const onCloseRequested = () => {
      setCloseDecisionSent(false);
      setCloseRemember(false);
      setClosePromptOpen(true);
    };
    window.addEventListener("cliswitch-close-requested", onCloseRequested as EventListener);
    return () => {
      window.removeEventListener("cliswitch-close-requested", onCloseRequested as EventListener);
    };
  }, []);

  const sendCloseDecision = (action: "minimize_to_tray" | "quit" | "cancel", remember: boolean) => {
    setCloseDecisionSent(true);
    postIpc({ type: "close-decision", action, remember });
    setClosePromptOpen(false);
  };

  return (
    <div className="flex h-full bg-background text-foreground">
      <Dialog
        open={closePromptOpen}
        onOpenChange={(open) => {
          if (!open && closePromptOpen && !closeDecisionSent) {
            sendCloseDecision("cancel", false);
            return;
          }
          setClosePromptOpen(open);
        }}
      >
        <DialogContent className="sm:max-w-[520px]">
          <DialogHeader>
            <DialogTitle>{t("closePrompt.title")}</DialogTitle>
            <DialogDescription>{t("closePrompt.description")}</DialogDescription>
          </DialogHeader>

          <div className="flex items-center justify-between gap-3 py-1">
            <div>
              <div className="font-medium text-sm">{t("closePrompt.remember")}</div>
              <div className="text-xs text-muted-foreground">{t("closePrompt.rememberHint")}</div>
            </div>
            <Switch checked={closeRemember} onCheckedChange={setCloseRemember} />
          </div>

          <DialogFooter>
            <Button variant="outline" onClick={() => sendCloseDecision("cancel", false)}>
              {t("common.cancel")}
            </Button>
            <Button
              variant="outline"
              onClick={() => sendCloseDecision("minimize_to_tray", closeRemember)}
            >
              {t("closePrompt.minimize")}
            </Button>
            <Button onClick={() => sendCloseDecision("quit", closeRemember)}>
              {t("closePrompt.quit")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog open={pricingOnboardingOpen} onOpenChange={setPricingOnboardingOpen}>
        <DialogContent className="sm:max-w-[520px]">
          <DialogHeader>
            <DialogTitle>{t("pricing.onboarding.title")}</DialogTitle>
            <DialogDescription>{t("pricing.onboarding.description")}</DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button
              variant="outline"
              onClick={() => setPricingOnboardingOpen(false)}
              disabled={pricingSyncing}
            >
              {t("pricing.onboarding.skip")}
            </Button>
            <Button
              onClick={async () => {
                setPricingSyncing(true);
                try {
                  await pricingSync();
                  toast.success(t("pricing.onboarding.syncOk"));
                  setPricingOnboardingOpen(false);
                } catch (e) {
                  toast.error(t("pricing.onboarding.syncFail"), { description: String(e) });
                } finally {
                  setPricingSyncing(false);
                }
              }}
              disabled={pricingSyncing}
            >
              {pricingSyncing ? t("pricing.onboarding.syncing") : t("pricing.onboarding.sync")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* 侧边栏导航 */}
      <aside
        className={cn(
          "flex-shrink-0 border-r border-sidebar-border bg-sidebar flex flex-col transition-all duration-200",
          collapsed ? "w-12" : "w-44"
        )}
      >
        <div
          className={cn(
            "h-11 flex items-center border-b border-sidebar-border",
            collapsed ? "justify-center" : "justify-center gap-1.5"
          )}
        >
          <Zap className="h-4 w-4 text-foreground flex-shrink-0" />
          {!collapsed && <span className="font-semibold text-sm">CliSwitch</span>}
        </div>

        <nav className={cn("flex-1 py-2 space-y-0.5", collapsed ? "px-1" : "px-1.5")}>
          {NAV_ITEMS.map((item) => (
            <NavTab
              key={item.route}
              route={item.route}
              current={route}
              label={t(item.labelKey)}
              icon={item.icon}
              collapsed={collapsed}
            />
          ))}
        </nav>

        <div
          className={cn(
            "py-2 border-t border-sidebar-border flex items-center",
            collapsed ? "flex-col gap-1 px-1" : "justify-between px-2"
          )}
        >
          <StatusIndicator status={health} collapsed={collapsed} />
          <Tooltip>
            <TooltipTrigger asChild>
              <button
                className="p-1.5 rounded text-muted-foreground hover:text-foreground hover:bg-accent/50 transition-colors"
                onClick={toggleCollapsed}
              >
                {collapsed ? (
                  <PanelLeftOpen className="h-4 w-4" />
                ) : (
                  <PanelLeftClose className="h-4 w-4" />
                )}
              </button>
            </TooltipTrigger>
            <TooltipContent side="right" sideOffset={8}>
              {collapsed ? t("sidebar.expand") : t("sidebar.collapse")}
            </TooltipContent>
          </Tooltip>
        </div>
      </aside>

      {/* 内容区 */}
      <div className="flex-1 min-w-0 flex flex-col">
        <main className="flex-1 overflow-auto bg-muted/30">
          <div className="mx-auto w-full max-w-7xl p-5 h-full min-h-0 flex flex-col">
            {route === "overview" ? (
              <OverviewPage />
            ) : route === "channels" ? (
              <ChannelsPage />
            ) : route === "monitor" ? (
              <MonitorPage />
            ) : route === "logs" ? (
              <LogsPage />
            ) : (
              <SettingsPage />
            )}
          </div>
        </main>
      </div>
    </div>
  );
}
