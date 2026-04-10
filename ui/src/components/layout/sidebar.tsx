import { Link } from "@tanstack/react-router";
import {
  Activity,
  FileText,
  LayoutGrid,
  Radio,
  ScrollText,
  Settings,
  User,
  Zap,
  type LucideIcon,
} from "lucide-react";

import { cn } from "@/lib/utils";
import { useI18n } from "@/hooks/use-i18n";
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

function SidebarBrand() {
  return (
    <Link aria-label="CliSwitch" className="mb-3.5 inline-flex no-underline" to="/">
      <div className="flex h-9 w-9 items-center justify-center rounded-[10px] bg-gradient-to-br from-blue-500 to-blue-700 shadow-lg shadow-blue-500/20">
        <Zap className="h-4 w-4 text-white" />
      </div>
    </Link>
  );
}

function SidebarVersion({ version }: { version?: string | null }) {
  return (
    <div className="text-[8px] font-medium text-slate-500/60 dark:text-slate-400/60">
      v{version ?? "-"}
    </div>
  );
}

export function SidebarItem({
  icon: Icon,
  label,
  route,
  active,
}: {
  icon: LucideIcon;
  label: string;
  route: SidebarRoute;
  active: boolean;
}) {
  return (
    <Link
      aria-current={active ? "page" : undefined}
      className={cn(
        "group relative flex w-[58px] select-none flex-col items-center gap-[3px] rounded-[10px] pt-1.5 pb-1 text-center no-underline transition-colors",
        active
          ? "bg-blue-50 before:absolute before:-left-[9px] before:top-1/2 before:h-[18px] before:w-[3px] before:-translate-y-1/2 before:rounded-r-[3px] before:bg-blue-600 before:content-[''] dark:bg-blue-950/50 dark:before:bg-blue-500"
          : "hover:bg-blue-50 dark:hover:bg-slate-800",
      )}
      to={hrefFor(route)}
    >
      <Icon
        className={cn(
          "h-[17px] w-[17px] shrink-0",
          active
            ? "text-blue-600 dark:text-blue-400"
            : "text-slate-500 group-hover:text-blue-600 dark:text-slate-400 dark:group-hover:text-blue-400",
        )}
      />
      <span
        className={cn(
          "text-[9.5px] leading-none",
          active
            ? "font-bold text-blue-600 dark:text-blue-400"
            : "font-medium text-slate-500 group-hover:text-blue-600 dark:text-slate-400 dark:group-hover:text-blue-400",
        )}
      >
        {label}
      </span>
    </Link>
  );
}

export function SidebarStatus({
  health,
}: {
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

  return (
    <div className="flex flex-col items-center gap-[3px]">
      <span
        className={cn(
          "h-1.5 w-1.5 shrink-0 rounded-full",
          (isOk || isChecking) && "animate-pulse-dot",
          isOk
            ? "bg-emerald-500"
            : isChecking
              ? "bg-slate-400 dark:bg-slate-500"
              : "bg-red-500",
        )}
      />
      <span className="text-center text-[8px] font-medium text-slate-500 dark:text-slate-400">
        {label}
      </span>
    </div>
  );
}

export function Sidebar({
  activeRoute,
  health,
}: {
  activeRoute: SidebarRoute;
  health: Health;
}) {
  const { t } = useI18n();

  return (
    <aside className="flex w-[76px] shrink-0 flex-col items-center gap-0.5 border-r border-slate-200 bg-white pt-3 pb-2 dark:border-slate-800 dark:bg-slate-900">
      <SidebarBrand />

      <nav className="flex flex-1 flex-col items-center gap-0.5">
        {NAV_ITEMS.map((item) => (
          <SidebarItem
            key={item.route}
            active={activeRoute === item.route}
            icon={item.icon}
            label={t(item.labelKey)}
            route={item.route}
          />
        ))}
      </nav>

      <div className="flex w-12 flex-col items-center gap-1.5 border-t border-slate-200 pt-3 dark:border-slate-800">
        <SidebarStatus health={health} />
        <SidebarVersion version={health.version} />
      </div>
    </aside>
  );
}
