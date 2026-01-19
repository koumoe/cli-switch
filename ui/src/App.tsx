import React, { useEffect, useMemo, useRef, useState } from "react";
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
import { humanizeApiError } from "@/lib/error";
import {
  Button,
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  Input,
  Switch,
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui";
import { toast } from "sonner";
import { downloadUpdate, getCliToolsStatus, getHealth, getSettings, getUpdateChangelog, ignoreUpdate, installCliTool, installNpmEnv, pricingStatus, pricingSync, updateSettings, type ChangelogSection, type CliToolId, type CliToolsStatus } from "./api";
import { logger, setLogLevel } from "@/lib/logger";
import type { CliswitchUpdateStatusEvent } from "@/lib/cliswitchEvents";
import { isUpdateReadyShown, markUpdateReadyShown } from "@/lib/updateReadyPrompt";
import { UpdatePromptDialog } from "@/components/UpdatePromptDialog";

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
const CLI_TOOLS_ONBOARDING_SHOWN_KEY = "cliswitch-cli-tools-onboarding-shown";

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
  const { t, locale } = useI18n();
  const [pricingOnboardingOpen, setPricingOnboardingOpen] = useState(false);
  const [pricingSyncing, setPricingSyncing] = useState(false);
  const [cliToolsOnboardingOpen, setCliToolsOnboardingOpen] = useState(false);
  const [cliToolsOnboardingStatus, setCliToolsOnboardingStatus] = useState<CliToolsStatus | null>(null);
  const [cliToolsOnboardingBusy, setCliToolsOnboardingBusy] = useState(false);
  const [cliToolsNpmInstalling, setCliToolsNpmInstalling] = useState(false);
  const [cliToolOnboardingBusy, setCliToolOnboardingBusy] = useState<Record<CliToolId, boolean>>({
    gemini: false,
    claude: false,
    codex: false,
  });
  const [updateReadyOpen, setUpdateReadyOpen] = useState(false);
  const [updateReadyVersion, setUpdateReadyVersion] = useState<string | null>(null);
  const [updatePromptOpen, setUpdatePromptOpen] = useState(false);
  const [updatePromptVersion, setUpdatePromptVersion] = useState<string | null>(null);
  const [updateChangelogSections, setUpdateChangelogSections] = useState<ChangelogSection[] | null>(null);
  const [updateChangelogLoading, setUpdateChangelogLoading] = useState(false);
  const [updateChangelogError, setUpdateChangelogError] = useState<string | null>(null);
  const [updatePromptBusy, setUpdatePromptBusy] = useState(false);
  const [closePromptOpen, setClosePromptOpen] = useState(false);
  const [closeRemember, setCloseRemember] = useState(false);
  const [closeDecisionSent, setCloseDecisionSent] = useState(false);
  const updatePromptOpenRef = useRef(false);
  const updatePromptedVersionRef = useRef<string | null>(null);

  useEffect(() => {
    updatePromptOpenRef.current = updatePromptOpen;
  }, [updatePromptOpen]);

  // 确保主题在应用启动时被应用
  useTheme();

  useEffect(() => {
    postIpc({ type: "set-locale", locale });
  }, [locale]);

  useEffect(() => {
    const onUpdateStatus = (e: Event) => {
      const st = (e as CliswitchUpdateStatusEvent).detail;
      const version = st?.pending_version;
      if (!version) return;

      if (isUpdateReadyShown(version)) return;

      setUpdateReadyVersion(version);
      setUpdateReadyOpen(true);
      markUpdateReadyShown(version);
    };
    window.addEventListener("cliswitch-update-status", onUpdateStatus as EventListener);
    postIpc({ type: "ui-ready" });
    return () => {
      window.removeEventListener("cliswitch-update-status", onUpdateStatus as EventListener);
    };
  }, []);

  useEffect(() => {
    if (!updatePromptOpen || !updatePromptVersion) return;
    let cancelled = false;
    setUpdateChangelogLoading(true);
    setUpdateChangelogError(null);
    getUpdateChangelog(updatePromptVersion, locale)
      .then((res) => {
        if (cancelled) return;
        setUpdateChangelogSections(res.sections);
      })
      .catch((e) => {
        if (cancelled) return;
        setUpdateChangelogError(humanizeApiError(e, t));
        setUpdateChangelogSections(null);
      })
      .finally(() => {
        if (cancelled) return;
        setUpdateChangelogLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [updatePromptOpen, updatePromptVersion, locale, t]);

  useEffect(() => {
    const onUpdateStatus = (e: Event) => {
      const st = (e as CliswitchUpdateStatusEvent).detail;
      const latest = st?.latest_version;
      if (!latest) return;
      if (!st.auto_update_enabled) return;
      if (!st.update_available) return;
      if (st.latest_ignored) return;
      if (st.pending_version) return;
      if (st.stage !== "idle") return;

      if (updatePromptOpenRef.current) return;
      if (updatePromptedVersionRef.current === latest) return;

      updatePromptedVersionRef.current = latest;
      setUpdatePromptVersion(latest);
      setUpdateChangelogSections(null);
      setUpdateChangelogError(null);
      setUpdatePromptOpen(true);
    };

    window.addEventListener("cliswitch-update-status", onUpdateStatus as EventListener);
    return () => {
      window.removeEventListener("cliswitch-update-status", onUpdateStatus as EventListener);
    };
  }, []);

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
    let cancelled = false;
    getSettings()
      .then((s) => {
        if (cancelled) return;
        setLogLevel(s.log_level);
        logger.info("ui settings loaded", { log_level: s.log_level }, "ui_settings_loaded");
      })
      .catch((e) => {
        if (cancelled) return;
        logger.warn("load settings failed", { error: String(e) }, "ui_settings_load_failed");
      });
    return () => {
      cancelled = true;
    };
  }, []);

  async function autoInstallNpmEnv() {
    setCliToolsNpmInstalling(true);
    try {
      await installNpmEnv();
      const st = await getCliToolsStatus().catch(() => null);
      if (st) setCliToolsOnboardingStatus(st);
      toast.success(t("settings.cliTools.saved"));
    } catch (e) {
      toast.error(t("settings.cliTools.installFail", { name: "npm" }), { description: humanizeApiError(e, t) });
    } finally {
      setCliToolsNpmInstalling(false);
    }
  }

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
    const shown = localStorage.getItem(CLI_TOOLS_ONBOARDING_SHOWN_KEY) === "true";
    if (shown) return;

    getCliToolsStatus()
      .then((st) => {
        localStorage.setItem(CLI_TOOLS_ONBOARDING_SHOWN_KEY, "true");
        setCliToolsOnboardingStatus(st);
        if (st.tools.some((t0) => !t0.installed)) {
          setCliToolsOnboardingOpen(true);
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
      <UpdatePromptDialog
        open={updatePromptOpen}
        onOpenChange={setUpdatePromptOpen}
        title={t("settings.update.promptTitle")}
        description={t("settings.update.promptDesc", { version: updatePromptVersion ?? "-" })}
        overviewTitle={t("settings.update.promptOverviewTitle")}
        loadingText={t("settings.update.promptLoading")}
        loadFailText={t("settings.update.promptLoadFail")}
        sections={updateChangelogSections}
        loading={updateChangelogLoading}
        loadError={updateChangelogError}
        updateText={t("settings.update.updateNow")}
        laterText={t("settings.update.later")}
        ignoreText={t("settings.update.ignore")}
        busy={updatePromptBusy}
        onLater={() => setUpdatePromptOpen(false)}
        onUpdate={async () => {
          const v = updatePromptVersion;
          if (!v) return;
          setUpdatePromptBusy(true);
          try {
            const dl = await downloadUpdate();
            if (dl.started) toast.success(t("settings.update.downloading"));
            setUpdatePromptOpen(false);
          } catch (e) {
            toast.error(t("settings.update.downloadFail"), { description: humanizeApiError(e, t) });
          } finally {
            setUpdatePromptBusy(false);
          }
        }}
        onIgnore={async () => {
          const v = updatePromptVersion;
          if (!v) return;
          setUpdatePromptBusy(true);
          try {
            await ignoreUpdate(v);
            toast.success(t("settings.update.ignoredToast", { version: v }));
            setUpdatePromptOpen(false);
          } catch (e) {
            toast.error(t("settings.update.ignoreFail"), { description: humanizeApiError(e, t) });
          } finally {
            setUpdatePromptBusy(false);
          }
        }}
      />

      <Dialog open={updateReadyOpen} onOpenChange={setUpdateReadyOpen}>
        <DialogContent className="sm:max-w-[520px]">
          <DialogHeader>
            <DialogTitle>{t("update.readyTitle")}</DialogTitle>
            <DialogDescription>
              {t("update.readyDesc", { version: updateReadyVersion ?? "-" })}
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            {typeof (window as any)?.ipc?.postMessage === "function" ? (
              <Button
                variant="outline"
                onClick={() => {
                  postIpc({ type: "request-quit" });
                }}
              >
                {t("update.quitToUpdate")}
              </Button>
            ) : null}
            <Button onClick={() => setUpdateReadyOpen(false)}>{t("common.ok")}</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

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

      <Dialog
        open={cliToolsOnboardingOpen}
        onOpenChange={(open) => {
          if (!open) setCliToolsOnboardingOpen(false);
          else setCliToolsOnboardingOpen(true);
        }}
      >
        <DialogContent className="sm:max-w-[560px]">
          <DialogHeader>
            <DialogTitle>{t("settings.cliTools.onboardingTitle")}</DialogTitle>
            <DialogDescription>{t("settings.cliTools.onboardingDesc")}</DialogDescription>
          </DialogHeader>

          <div className="space-y-3">
            {cliToolsOnboardingStatus && !cliToolsOnboardingStatus.npm_available ? (
              <div className="space-y-2">
                <div className="text-sm text-muted-foreground">
                  {t("settings.cliTools.npmMissing")}
                  {" "}
                  {cliToolsOnboardingStatus.os === "macos"
                    ? t("settings.cliTools.npmHintMac")
                    : cliToolsOnboardingStatus.os === "windows"
                      ? t("settings.cliTools.npmHintWindows")
                      : t("settings.cliTools.npmHintLinux")}
                </div>
                <div className="flex gap-2">
                  <Button size="sm" onClick={autoInstallNpmEnv} disabled={cliToolsNpmInstalling}>
                    {cliToolsNpmInstalling ? t("settings.cliTools.autoInstalling") : t("settings.cliTools.autoInstall")}
                  </Button>
                  <Button
                    size="sm"
                    variant="outline"
                    onClick={() => {
                      setCliToolsOnboardingOpen(false);
                      navigate("/settings");
                    }}
                    disabled={cliToolsNpmInstalling}
                  >
                    {t("settings.cliTools.openSettings")}
                  </Button>
                </div>
              </div>
            ) : null}

            {(cliToolsOnboardingStatus?.tools ?? [])
              .filter((tool) => !tool.installed)
              .map((tool) => {
                const busy = cliToolOnboardingBusy[tool.id];
                const npmOk = cliToolsOnboardingStatus?.npm_available ?? true;
                return (
                  <div key={tool.id} className="flex items-center justify-between gap-4">
                    <div className="min-w-0">
                      <div className="font-medium text-sm truncate">{tool.name}</div>
                      <div className="text-xs text-muted-foreground">{t("settings.cliTools.notInstalled")}</div>
                    </div>
                    <Button
                      size="sm"
                      disabled={busy}
                      onClick={async () => {
                        if (!npmOk) {
                          // 用户也可能从这里触发安装，因此直接提示安装/手动指定。
                          setCliToolsOnboardingOpen(true);
                          return;
                        }
                        setCliToolOnboardingBusy((prev) => ({ ...prev, [tool.id]: true }));
                        try {
                          const res = await installCliTool(tool.id);
                          setCliToolsOnboardingStatus((prev) =>
                            prev ? { ...prev, tools: prev.tools.map((t0) => (t0.id === tool.id ? res.tool : t0)) } : prev
                          );
                          if (res.ok) {
                            toast.success(t("settings.cliTools.installOk", { name: tool.name }), {
                              description: res.terminal_shim_ok
                                ? t("settings.cliTools.terminalReady")
                                : res.terminal_shim_error
                                  ? t("settings.cliTools.terminalSetupFailed", { error: res.terminal_shim_error })
                                  : undefined,
                            });
                          } else {
                            toast.error(t("settings.cliTools.installFail", { name: tool.name }), {
                              description: res.stderr?.trim() || res.stdout?.trim() || "-",
                            });
                          }
                        } catch (e) {
                          toast.error(t("settings.cliTools.installFail", { name: tool.name }), { description: humanizeApiError(e, t) });
                        } finally {
                          setCliToolOnboardingBusy((prev) => ({ ...prev, [tool.id]: false }));
                        }
                      }}
                    >
                      {t("settings.cliTools.install")}
                    </Button>
                  </div>
                );
              })}
          </div>

          <DialogFooter>
            <Button variant="outline" onClick={() => setCliToolsOnboardingOpen(false)} disabled={cliToolsOnboardingBusy}>
              {t("settings.cliTools.later")}
            </Button>
            <Button
              onClick={async () => {
                if (!cliToolsOnboardingStatus) return;
                if (!cliToolsOnboardingStatus.npm_available) {
                  // npm 缺失时不直接继续安装，用户可在上面的提示中选择自动安装或手动指定。
                  return;
                }
                const missing = cliToolsOnboardingStatus.tools.filter((t0) => !t0.installed);
                if (missing.length === 0) {
                  setCliToolsOnboardingOpen(false);
                  return;
                }
                setCliToolsOnboardingBusy(true);
                try {
                  for (const tool of missing) {
                    setCliToolOnboardingBusy((prev) => ({ ...prev, [tool.id]: true }));
                    try {
                      const res = await installCliTool(tool.id);
                      setCliToolsOnboardingStatus((prev) =>
                        prev ? { ...prev, tools: prev.tools.map((t0) => (t0.id === tool.id ? res.tool : t0)) } : prev
                      );
                      if (res.ok) {
                        toast.success(t("settings.cliTools.installOk", { name: tool.name }), {
                          description: res.terminal_shim_ok
                            ? t("settings.cliTools.terminalReady")
                            : res.terminal_shim_error
                              ? t("settings.cliTools.terminalSetupFailed", { error: res.terminal_shim_error })
                              : undefined,
                        });
                      }
                      else {
                        toast.error(t("settings.cliTools.installFail", { name: tool.name }), {
                          description: res.stderr?.trim() || res.stdout?.trim() || "-",
                        });
                      }
                    } finally {
                      setCliToolOnboardingBusy((prev) => ({ ...prev, [tool.id]: false }));
                    }
                  }
                } finally {
                  setCliToolsOnboardingBusy(false);
                }

                const next = await getCliToolsStatus().catch(() => null);
                if (next) {
                  setCliToolsOnboardingStatus(next);
                  if (!next.tools.some((t0) => !t0.installed)) setCliToolsOnboardingOpen(false);
                }
              }}
              disabled={
                cliToolsOnboardingBusy ||
                !cliToolsOnboardingStatus ||
                !cliToolsOnboardingStatus.npm_available ||
                !(cliToolsOnboardingStatus.tools ?? []).some((t0) => !t0.installed)
              }
            >
              {cliToolsOnboardingBusy ? t("settings.cliTools.installing") : t("settings.cliTools.installAll")}
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
