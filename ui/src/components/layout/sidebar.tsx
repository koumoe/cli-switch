import { Link } from "@tanstack/react-router";
import {
  Activity,
  FileText,
  LayoutGrid,
  PanelLeftClose,
  PanelLeftOpen,
  Radio,
  ScrollText,
  Settings,
  User,
  Zap,
  type LucideIcon,
} from "lucide-react";

import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui";
import { useI18n } from "@/hooks/use-i18n";
import { cn } from "@/lib/utils";
import type { Health } from "@/types/api";

export type SidebarRoute =
  | "overview"
  | "channels"
  | "accounts"
  | "prompts"
  | "monitor"
  | "logs"
  | "settings";

type SidebarPath =
  | "/"
  | "/channels"
  | "/accounts"
  | "/prompts"
  | "/monitor"
  | "/logs"
  | "/settings";

const NAV_ITEMS: Array<{ route: SidebarRoute; labelKey: string; icon: LucideIcon }> = [
  { route: "overview", labelKey: "nav.overview", icon: LayoutGrid },
  { route: "channels", labelKey: "nav.channels", icon: Radio },
  { route: "accounts", labelKey: "nav.accounts", icon: User },
  { route: "prompts", labelKey: "nav.prompts", icon: FileText },
  { route: "monitor", labelKey: "nav.monitor", icon: Activity },
  { route: "logs", labelKey: "nav.logs", icon: ScrollText },
  { route: "settings", labelKey: "nav.settings", icon: Settings },
];

function hrefFor(route: SidebarRoute): SidebarPath {
  if (route === "overview") {
    return "/";
  }
  return `/${route}`;
}

function SidebarVersion({
  collapsed,
  version,
}: {
  collapsed: boolean;
  version?: string | null;
}) {
  const label = `v${version ?? "-"}`;

  if (collapsed) {
    return (
      <Tooltip>
        <TooltipTrigger asChild>
          <div className="flex justify-center px-1 pb-1.5 text-[10px] text-sidebar-foreground/50">
            v
          </div>
        </TooltipTrigger>
        <TooltipContent side="right" sideOffset={8}>
          {label}
        </TooltipContent>
      </Tooltip>
    );
  }

  return (
    <div className="px-2 pb-1.5 text-[10px] text-sidebar-foreground/50">
      {label}
    </div>
  );
}

export function SidebarItem({
  icon: Icon,
  label,
  route,
  active,
  collapsed,
}: {
  icon: LucideIcon;
  label: string;
  route: SidebarRoute;
  active: boolean;
  collapsed: boolean;
}) {
  const link = (
    <Link
      aria-current={active ? "page" : undefined}
      className={cn(
        "group flex select-none items-center rounded-md transition-colors",
        collapsed ? "justify-center p-2" : "gap-2 px-2.5 py-1.5",
        active
          ? "bg-sidebar-accent font-medium text-sidebar-accent-foreground"
          : "text-sidebar-foreground/70 hover:bg-sidebar-accent/60 hover:text-sidebar-foreground",
      )}
      to={hrefFor(route)}
    >
      <Icon
        className={cn(
          "h-4 w-4 flex-shrink-0",
          active
            ? "text-sidebar-accent-foreground"
            : "text-muted-foreground group-hover:text-sidebar-foreground",
        )}
      />
      {!collapsed ? <span className="text-[13px]">{label}</span> : null}
    </Link>
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

export function SidebarStatus({
  collapsed,
  health,
}: {
  collapsed: boolean;
  health: Pick<Health, "status">;
}) {
  const { t } = useI18n();
  const isChecking = health.status === "...";
  const isOk = health.status === "ok";
  const isOffline = health.status === "offline";
  const label = isChecking
    ? t("status.checking")
    : isOk
      ? t("status.running")
      : isOffline
        ? t("status.offline")
        : health.status;

  const dot = (
    <span
      className={cn(
        "h-1.5 w-1.5 flex-shrink-0 rounded-full",
        (isOk || isChecking) && "animate-pulse-dot",
        isOk ? "bg-success" : isChecking ? "bg-muted-foreground" : "bg-destructive",
      )}
    />
  );

  if (collapsed) {
    return (
      <Tooltip>
        <TooltipTrigger asChild>
          <div className="flex items-center justify-center p-1.5">{dot}</div>
        </TooltipTrigger>
        <TooltipContent side="right" sideOffset={8}>
          {label}
        </TooltipContent>
      </Tooltip>
    );
  }

  return (
    <div className="flex items-center gap-1.5 text-xs text-fg-muted">
      {dot}
      {label}
    </div>
  );
}

export function Sidebar({
  activeRoute,
  collapsed,
  health,
  onToggle,
}: {
  activeRoute: SidebarRoute;
  collapsed: boolean;
  health: Health;
  onToggle: () => void;
}) {
  const { t } = useI18n();

  return (
    <aside
      className={cn(
        "surface-panel-subtle flex flex-shrink-0 flex-col border-r border-sidebar-border/80 bg-sidebar/92 transition-[width,background-color] duration-200 supports-[backdrop-filter]:bg-sidebar/72",
        collapsed ? "w-12" : "w-44",
      )}
    >
      <div
        className={cn(
          "flex h-11 items-center border-b border-sidebar-border",
          collapsed ? "justify-center" : "justify-center gap-1.5",
        )}
      >
        <Zap className="h-4 w-4 flex-shrink-0 text-foreground" />
        {!collapsed ? <span className="text-sm font-semibold">CliSwitch</span> : null}
      </div>

      <nav className={cn("flex-1 space-y-0.5 py-2", collapsed ? "px-1" : "px-1.5")}>
        {NAV_ITEMS.map((item) => (
          <SidebarItem
            key={item.route}
            active={activeRoute === item.route}
            collapsed={collapsed}
            icon={item.icon}
            label={t(item.labelKey)}
            route={item.route}
          />
        ))}
      </nav>

      <div className="border-t border-sidebar-border pt-2">
        <div
          className={cn(
            "flex items-center",
            collapsed ? "flex-col gap-1 px-1" : "justify-between px-2",
          )}
        >
          <SidebarStatus collapsed={collapsed} health={health} />
          <Tooltip>
            <TooltipTrigger asChild>
              <button
                aria-label={collapsed ? t("sidebar.expand") : t("sidebar.collapse")}
                className="rounded p-1.5 text-muted-foreground transition-colors hover:bg-accent/50 hover:text-foreground"
                onClick={onToggle}
                type="button"
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
        <SidebarVersion collapsed={collapsed} version={health.version} />
      </div>
    </aside>
  );
}
