import { useEffect, useRef, useState } from "react";
import { toast } from "sonner";

import {
  deleteChannel,
  disableChannel,
  downloadUpdate,
  getCliToolsStatus,
  getSettings,
  getUpdateChangelog,
  ignoreUpdate,
  pricingStatus,
  pricingSync,
  updateChannel,
} from "@/api";
import { UpdatePromptDialog } from "@/components/UpdatePromptDialog";
import {
  Button,
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  Switch,
} from "@/components/ui";
import { useI18n } from "@/hooks/use-i18n";
import { useTheme } from "@/hooks/use-theme";
import { humanizeApiError } from "@/lib/error";
import { installCliToolWithToast } from "@/lib/cliToolInstaller";
import type {
  CliswitchRemoteGroupAddedEvent,
  CliswitchRemoteManagedChannelMissingEvent,
  CliswitchRemoteManagedChannelMultiplierEvent,
  CliswitchUpdateStatusEvent,
} from "@/lib/cliswitchEvents";
import { postIpc } from "@/lib/ipc";
import { logger, setLogLevel } from "@/lib/logger";
import { isUpdateReadyShown, markUpdateReadyShown } from "@/lib/updateReadyPrompt";
import type {
  ChangelogSection,
  CliToolId,
  CliToolsStatus,
  RemoteGroupAddedAlert,
  RemoteManagedChannelMissingPrompt,
  RemoteManagedChannelMultiplierPrompt,
} from "@/types/api";

const PRICING_ONBOARDING_SHOWN_KEY = "cliswitch-pricing-onboarding-shown";
const CLI_TOOLS_ONBOARDING_SHOWN_KEY = "cliswitch-cli-tools-onboarding-shown";

type TranslateFn = ReturnType<typeof useI18n>["t"];

function formatMultiplier(value: number): string {
  if (!Number.isFinite(value)) return "-";
  return `×${value.toFixed(2)}`;
}

function managedResourceLabel(
  prompt: RemoteManagedChannelMissingPrompt | null,
  t: TranslateFn,
): string {
  if (!prompt) {
    return t("channels.remoteMissing.resourceGeneric");
  }
  if (prompt.provider === "sub2api") {
    return t("channels.remoteMissing.resourceKey");
  }
  return t("channels.remoteMissing.resourceToken");
}

function hasRemoteGroupAddedPayload(
  payload: RemoteGroupAddedAlert | null | undefined,
): payload is RemoteGroupAddedAlert {
  return !!payload?.account_id && !!payload.group_name;
}

function managedMissingDescription(
  prompt: RemoteManagedChannelMissingPrompt | null,
  t: TranslateFn,
): string {
  if (!prompt) {
    return t("channels.remoteMissing.descriptionGeneric");
  }
  const resource = managedResourceLabel(prompt, t);
  if (prompt.missing_group && prompt.missing_resource) {
    return t("channels.remoteMissing.descriptionMissingResourceAndGroup", {
      name: prompt.channel_name,
      resource,
    });
  }
  if (prompt.missing_group) {
    return t("channels.remoteMissing.descriptionMissingGroup", {
      name: prompt.channel_name,
    });
  }
  if (prompt.missing_resource) {
    return t("channels.remoteMissing.descriptionMissingResource", {
      name: prompt.channel_name,
      resource,
    });
  }
  return t("channels.remoteMissing.descriptionGeneric");
}

function canSyncDeleteManagedMissing(prompt: RemoteManagedChannelMissingPrompt | null): boolean {
  return !!prompt && !prompt.missing_resource;
}

export function GlobalDialogs() {
  const { t, locale } = useI18n();

  useTheme();

  const [pricingOnboardingOpen, setPricingOnboardingOpen] = useState(false);
  const [pricingSyncing, setPricingSyncing] = useState(false);
  const [cliToolsOnboardingOpen, setCliToolsOnboardingOpen] = useState(false);
  const [cliToolsOnboardingStatus, setCliToolsOnboardingStatus] = useState<CliToolsStatus | null>(null);
  const [cliToolsOnboardingBusy, setCliToolsOnboardingBusy] = useState(false);
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
  const [managedMissingQueue, setManagedMissingQueue] = useState<RemoteManagedChannelMissingPrompt[]>([]);
  const [managedMissingBusyAction, setManagedMissingBusyAction] = useState<"disable" | "delete" | null>(null);
  const [managedMissingDeleteSyncRemote, setManagedMissingDeleteSyncRemote] = useState(true);
  const [managedMultiplierQueue, setManagedMultiplierQueue] = useState<RemoteManagedChannelMultiplierPrompt[]>([]);
  const [managedMultiplierBusy, setManagedMultiplierBusy] = useState(false);
  const updatePromptOpenRef = useRef(false);
  const updatePromptedVersionRef = useRef<string | null>(null);
  const activeManagedMissing = managedMissingQueue[0] ?? null;
  const activeManagedMultiplier = managedMultiplierQueue[0] ?? null;

  useEffect(() => {
    updatePromptOpenRef.current = updatePromptOpen;
  }, [updatePromptOpen]);

  useEffect(() => {
    setManagedMissingDeleteSyncRemote(canSyncDeleteManagedMissing(activeManagedMissing));
  }, [activeManagedMissing?.channel_id, activeManagedMissing?.missing_resource]);

  useEffect(() => {
    postIpc({ type: "set-locale", locale });
  }, [locale]);

  useEffect(() => {
    const onUpdateStatus = (event: Event) => {
      const status = (event as CliswitchUpdateStatusEvent).detail;
      const version = status?.pending_version;
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
    if (!updatePromptOpen || !updatePromptVersion) {
      return;
    }

    let cancelled = false;
    setUpdateChangelogLoading(true);
    setUpdateChangelogError(null);

    getUpdateChangelog(updatePromptVersion, locale)
      .then((response) => {
        if (!cancelled) {
          setUpdateChangelogSections(response.sections);
        }
      })
      .catch((error) => {
        if (!cancelled) {
          setUpdateChangelogError(humanizeApiError(error, t));
          setUpdateChangelogSections(null);
        }
      })
      .finally(() => {
        if (!cancelled) {
          setUpdateChangelogLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [locale, t, updatePromptOpen, updatePromptVersion]);

  useEffect(() => {
    const onUpdateStatus = (event: Event) => {
      const status = (event as CliswitchUpdateStatusEvent).detail;
      const latest = status?.latest_version;
      if (!latest) return;
      if (!status.auto_update_enabled) return;
      if (!status.update_available) return;
      if (status.latest_ignored) return;
      if (status.pending_version) return;
      if (status.stage !== "idle") return;
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

  useEffect(() => {
    let cancelled = false;

    getSettings()
      .then((settings) => {
        if (cancelled) {
          return;
        }
        setLogLevel(settings.log_level);
        logger.info("ui settings loaded", { log_level: settings.log_level }, "ui_settings_loaded");
      })
      .catch((error) => {
        if (cancelled) {
          return;
        }
        logger.warn("load settings failed", { error: String(error) }, "ui_settings_load_failed");
      });

    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    const shown = window.localStorage.getItem(PRICING_ONBOARDING_SHOWN_KEY) === "true";
    if (shown) {
      return;
    }

    pricingStatus()
      .then((status) => {
        if (status.count <= 0) {
          window.localStorage.setItem(PRICING_ONBOARDING_SHOWN_KEY, "true");
          setPricingOnboardingOpen(true);
        }
      })
      .catch(() => {
        // ignore
      });
  }, []);

  useEffect(() => {
    const shown = window.localStorage.getItem(CLI_TOOLS_ONBOARDING_SHOWN_KEY) === "true";
    if (shown) {
      return;
    }

    getCliToolsStatus()
      .then((status) => {
        window.localStorage.setItem(CLI_TOOLS_ONBOARDING_SHOWN_KEY, "true");
        setCliToolsOnboardingStatus(status);
        if (status.tools.some((tool) => !tool.installed)) {
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

  useEffect(() => {
    const onRemoteGroupAdded = (event: Event) => {
      const detail = (event as CliswitchRemoteGroupAddedEvent).detail;
      if (!hasRemoteGroupAddedPayload(detail)) return;

      toast.info(t("channels.remoteGroupAdded.title"), {
        description: t("channels.remoteGroupAdded.description", {
          baseUrl: detail.account_base_url,
          group: detail.group_name,
        }),
      });
    };

    window.addEventListener("cliswitch-remote-group-added", onRemoteGroupAdded as EventListener);
    return () => {
      window.removeEventListener("cliswitch-remote-group-added", onRemoteGroupAdded as EventListener);
    };
  }, [t]);

  useEffect(() => {
    const onManagedMissing = (event: Event) => {
      const detail = (event as CliswitchRemoteManagedChannelMissingEvent).detail;
      if (!detail?.channel_id) return;

      setManagedMissingQueue((current) => {
        const existingIndex = current.findIndex((item) => item.channel_id === detail.channel_id);
        if (existingIndex >= 0) {
          return current.map((item, itemIndex) => (
            itemIndex === existingIndex ? detail : item
          ));
        }
        return [...current, detail];
      });
    };

    window.addEventListener(
      "cliswitch-remote-managed-channel-missing",
      onManagedMissing as EventListener,
    );
    return () => {
      window.removeEventListener(
        "cliswitch-remote-managed-channel-missing",
        onManagedMissing as EventListener,
      );
    };
  }, []);

  useEffect(() => {
    const onManagedMultiplier = (event: Event) => {
      const detail = (event as CliswitchRemoteManagedChannelMultiplierEvent).detail;
      if (!detail?.channel_id) return;

      setManagedMultiplierQueue((current) => {
        const existingIndex = current.findIndex((item) => item.channel_id === detail.channel_id);
        if (existingIndex >= 0) {
          return current.map((item, itemIndex) => (
            itemIndex === existingIndex ? detail : item
          ));
        }
        return [...current, detail];
      });
    };

    window.addEventListener(
      "cliswitch-remote-managed-channel-multiplier",
      onManagedMultiplier as EventListener,
    );
    return () => {
      window.removeEventListener(
        "cliswitch-remote-managed-channel-multiplier",
        onManagedMultiplier as EventListener,
      );
    };
  }, []);

  const sendCloseDecision = (
    action: "minimize_to_tray" | "quit" | "cancel",
    remember: boolean,
  ) => {
    setCloseDecisionSent(true);
    postIpc({ type: "close-decision", action, remember });
    setClosePromptOpen(false);
  };

  const dismissManagedMissing = () => {
    setManagedMissingQueue((current) => current.slice(1));
  };

  const dismissManagedMultiplier = () => {
    setManagedMultiplierQueue((current) => current.slice(1));
  };

  const resolveManagedMissing = async (action: "disable" | "delete") => {
    if (!activeManagedMissing) {
      return;
    }

    setManagedMissingBusyAction(action);
    try {
      if (action === "disable") {
        await disableChannel(activeManagedMissing.channel_id);
        toast.success(t("channels.remoteMissing.disableOk", { name: activeManagedMissing.channel_name }));
      } else {
        await deleteChannel(activeManagedMissing.channel_id, {
          sync_remote_delete:
            managedMissingDeleteSyncRemote && canSyncDeleteManagedMissing(activeManagedMissing),
        });
        toast.success(t("channels.remoteMissing.deleteOk", { name: activeManagedMissing.channel_name }));
      }

      window.dispatchEvent(
        new CustomEvent("cliswitch-channels-changed", { detail: { at_ms: Date.now() } }),
      );
      dismissManagedMissing();
    } catch (error) {
      toast.error(t("channels.remoteMissing.actionFail"), {
        description: humanizeApiError(error, t),
      });
    } finally {
      setManagedMissingBusyAction(null);
    }
  };

  const resolveManagedMultiplier = async (applyUpdate: boolean) => {
    if (!activeManagedMultiplier) {
      return;
    }
    if (!applyUpdate) {
      dismissManagedMultiplier();
      return;
    }

    setManagedMultiplierBusy(true);
    try {
      await updateChannel(activeManagedMultiplier.channel_id, {
        real_multiplier: activeManagedMultiplier.remote_multiplier,
      });
      toast.success(
        t("channels.remoteMultiplier.updateOk", { name: activeManagedMultiplier.channel_name }),
        {
          description: t("channels.remoteMultiplier.updateOkDescription", {
            from: formatMultiplier(activeManagedMultiplier.current_multiplier),
            to: formatMultiplier(activeManagedMultiplier.remote_multiplier),
          }),
        },
      );
      window.dispatchEvent(
        new CustomEvent("cliswitch-channels-changed", { detail: { at_ms: Date.now() } }),
      );
      dismissManagedMultiplier();
    } catch (error) {
      toast.error(t("channels.remoteMultiplier.actionFail"), {
        description: humanizeApiError(error, t),
      });
    } finally {
      setManagedMultiplierBusy(false);
    }
  };

  return (
    <>
      <UpdatePromptDialog
        busy={updatePromptBusy}
        description={t("settings.update.promptDesc", { version: updatePromptVersion ?? "-" })}
        ignoreText={t("settings.update.ignore")}
        laterText={t("settings.update.later")}
        loadError={updateChangelogError}
        loadFailText={t("settings.update.promptLoadFail")}
        loading={updateChangelogLoading}
        loadingText={t("settings.update.promptLoading")}
        onIgnore={async () => {
          const version = updatePromptVersion;
          if (!version) return;

          setUpdatePromptBusy(true);
          try {
            await ignoreUpdate(version);
            toast.success(t("settings.update.ignoredToast", { version }));
            setUpdatePromptOpen(false);
          } catch (error) {
            toast.error(t("settings.update.ignoreFail"), {
              description: humanizeApiError(error, t),
            });
          } finally {
            setUpdatePromptBusy(false);
          }
        }}
        onLater={() => setUpdatePromptOpen(false)}
        onOpenChange={setUpdatePromptOpen}
        onUpdate={async () => {
          const version = updatePromptVersion;
          if (!version) return;

          setUpdatePromptBusy(true);
          try {
            const download = await downloadUpdate();
            if (download.started) {
              toast.success(t("settings.update.downloading"));
            }
            setUpdatePromptOpen(false);
          } catch (error) {
            toast.error(t("settings.update.downloadFail"), {
              description: humanizeApiError(error, t),
            });
          } finally {
            setUpdatePromptBusy(false);
          }
        }}
        open={updatePromptOpen}
        overviewTitle={t("settings.update.promptOverviewTitle")}
        sections={updateChangelogSections}
        title={t("settings.update.promptTitle")}
        updateText={t("settings.update.updateNow")}
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
            {typeof (window as Window & { ipc?: { postMessage?: (message: string) => void } }).ipc?.postMessage === "function" ? (
              <Button
                onClick={() => {
                  postIpc({ type: "request-quit" });
                }}
                variant="outline"
              >
                {t("update.quitToUpdate")}
              </Button>
            ) : null}
            <Button onClick={() => setUpdateReadyOpen(false)}>{t("common.ok")}</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog
        onOpenChange={(open) => {
          if (!open && closePromptOpen && !closeDecisionSent) {
            sendCloseDecision("cancel", false);
            return;
          }
          setClosePromptOpen(open);
        }}
        open={closePromptOpen}
      >
        <DialogContent className="sm:max-w-[520px]">
          <DialogHeader>
            <DialogTitle>{t("closePrompt.title")}</DialogTitle>
            <DialogDescription>{t("closePrompt.description")}</DialogDescription>
          </DialogHeader>

          <div className="flex items-center justify-between gap-3 py-1">
            <div>
              <div className="text-sm font-medium">{t("closePrompt.remember")}</div>
              <div className="text-xs text-muted-foreground">{t("closePrompt.rememberHint")}</div>
            </div>
            <Switch checked={closeRemember} onCheckedChange={setCloseRemember} />
          </div>

          <DialogFooter>
            <Button onClick={() => sendCloseDecision("cancel", false)} variant="outline">
              {t("common.cancel")}
            </Button>
            <Button
              onClick={() => sendCloseDecision("minimize_to_tray", closeRemember)}
              variant="outline"
            >
              {t("closePrompt.minimize")}
            </Button>
            <Button onClick={() => sendCloseDecision("quit", closeRemember)}>
              {t("closePrompt.quit")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog
        onOpenChange={(open) => {
          if (!open && !managedMissingBusyAction) {
            dismissManagedMissing();
          }
        }}
        open={!!activeManagedMissing}
      >
        <DialogContent className="sm:max-w-[560px]">
          <DialogHeader>
            <DialogTitle>{t("channels.remoteMissing.title")}</DialogTitle>
            <DialogDescription>
              {managedMissingDescription(activeManagedMissing, t)}
            </DialogDescription>
          </DialogHeader>

          {activeManagedMissing ? (
            <div className="space-y-3 text-sm">
              <div className="space-y-1 rounded-md border p-3">
                <div className="font-medium">{activeManagedMissing.channel_name}</div>
                <div className="text-xs text-muted-foreground">
                  {t("channels.remoteMissing.baseUrl", {
                    value: activeManagedMissing.account_base_url,
                  })}
                </div>
                {activeManagedMissing.group_name ? (
                  <div className="text-xs text-muted-foreground">
                    {t("channels.remoteMissing.group", {
                      value: activeManagedMissing.group_name,
                    })}
                  </div>
                ) : null}
                {activeManagedMissing.resource_name ? (
                  <div className="text-xs text-muted-foreground">
                    {t("channels.remoteMissing.resourceLine", {
                      label: managedResourceLabel(activeManagedMissing, t),
                      value: activeManagedMissing.resource_name,
                    })}
                  </div>
                ) : null}
              </div>
              {canSyncDeleteManagedMissing(activeManagedMissing) ? (
                <div className="flex items-center justify-between rounded-md border p-3">
                  <div className="space-y-1">
                    <div className="text-sm font-medium">{t("channels.remoteMissing.syncDeleteRemote")}</div>
                    <div className="text-xs text-muted-foreground">
                      {t("channels.remoteMissing.syncDeleteRemoteHint")}
                    </div>
                  </div>
                  <Switch
                    checked={managedMissingDeleteSyncRemote}
                    disabled={!!managedMissingBusyAction}
                    onCheckedChange={setManagedMissingDeleteSyncRemote}
                  />
                </div>
              ) : (
                <p className="text-xs text-muted-foreground">
                  {t("channels.remoteMissing.syncDeleteRemoteUnavailable")}
                </p>
              )}
              <p className="text-xs text-muted-foreground">
                {t("channels.remoteMissing.hint")}
              </p>
            </div>
          ) : null}

          <DialogFooter>
            <Button
              disabled={!!managedMissingBusyAction}
              onClick={dismissManagedMissing}
              variant="outline"
            >
              {t("channels.remoteMissing.later")}
            </Button>
            <Button
              disabled={!!managedMissingBusyAction}
              onClick={() => void resolveManagedMissing("disable")}
              variant="outline"
            >
              {t("channels.remoteMissing.disable")}
            </Button>
            <Button
              disabled={!!managedMissingBusyAction}
              onClick={() => void resolveManagedMissing("delete")}
              variant="destructive"
            >
              {t("channels.remoteMissing.delete")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog
        onOpenChange={(open) => {
          if (!open && !managedMultiplierBusy) {
            dismissManagedMultiplier();
          }
        }}
        open={!!activeManagedMultiplier}
      >
        <DialogContent className="sm:max-w-[560px]">
          <DialogHeader>
            <DialogTitle>{t("channels.remoteMultiplier.title")}</DialogTitle>
            <DialogDescription>
              {t("channels.remoteMultiplier.description", {
                name: activeManagedMultiplier?.channel_name ?? "",
              })}
            </DialogDescription>
          </DialogHeader>

          {activeManagedMultiplier ? (
            <div className="space-y-3 text-sm">
              <div className="space-y-1 rounded-md border p-3">
                <div className="font-medium">{activeManagedMultiplier.channel_name}</div>
                <div className="text-xs text-muted-foreground">
                  {t("channels.remoteMultiplier.baseUrl", {
                    value: activeManagedMultiplier.account_base_url,
                  })}
                </div>
                {activeManagedMultiplier.group_name ? (
                  <div className="text-xs text-muted-foreground">
                    {t("channels.remoteMultiplier.group", {
                      value: activeManagedMultiplier.group_name,
                    })}
                  </div>
                ) : null}
                <div className="text-xs text-muted-foreground">
                  {t("channels.remoteMultiplier.current", {
                    value: formatMultiplier(activeManagedMultiplier.current_multiplier),
                  })}
                </div>
                <div className="text-xs text-muted-foreground">
                  {t("channels.remoteMultiplier.remote", {
                    value: formatMultiplier(activeManagedMultiplier.remote_multiplier),
                  })}
                </div>
              </div>
              <p className="text-xs text-muted-foreground">
                {t("channels.remoteMultiplier.hint")}
              </p>
            </div>
          ) : null}

          <DialogFooter>
            <Button
              disabled={managedMultiplierBusy}
              onClick={() => void resolveManagedMultiplier(false)}
              variant="outline"
            >
              {t("channels.remoteMultiplier.keep")}
            </Button>
            <Button
              disabled={managedMultiplierBusy}
              onClick={() => void resolveManagedMultiplier(true)}
            >
              {t("channels.remoteMultiplier.apply")}
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
              disabled={pricingSyncing}
              onClick={() => setPricingOnboardingOpen(false)}
              variant="outline"
            >
              {t("pricing.onboarding.skip")}
            </Button>
            <Button
              disabled={pricingSyncing}
              onClick={async () => {
                setPricingSyncing(true);
                try {
                  await pricingSync();
                  toast.success(t("pricing.onboarding.syncOk"));
                  setPricingOnboardingOpen(false);
                } catch (error) {
                  toast.error(t("pricing.onboarding.syncFail"), {
                    description: String(error),
                  });
                } finally {
                  setPricingSyncing(false);
                }
              }}
            >
              {pricingSyncing ? t("pricing.onboarding.syncing") : t("pricing.onboarding.sync")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog
        onOpenChange={setCliToolsOnboardingOpen}
        open={cliToolsOnboardingOpen}
      >
        <DialogContent className="sm:max-w-[560px]">
          <DialogHeader>
            <DialogTitle>{t("settings.cliTools.onboardingTitle")}</DialogTitle>
            <DialogDescription>{t("settings.cliTools.onboardingDesc")}</DialogDescription>
          </DialogHeader>

          <div className="space-y-3">
            {(cliToolsOnboardingStatus?.tools ?? [])
              .filter((tool) => !tool.installed)
              .map((tool) => {
                const busy = cliToolOnboardingBusy[tool.id];

                return (
                  <div key={tool.id} className="flex items-center justify-between gap-4">
                    <div className="min-w-0">
                      <div className="truncate text-sm font-medium">{tool.name}</div>
                      <div className="text-xs text-muted-foreground">
                        {t("settings.cliTools.notInstalled")}
                      </div>
                    </div>
                    <Button
                      disabled={busy}
                      onClick={async () => {
                        setCliToolOnboardingBusy((prev) => ({ ...prev, [tool.id]: true }));
                        try {
                          await installCliToolWithToast({
                            tool,
                            t,
                            onToolUpdated: (nextTool) => setCliToolsOnboardingStatus((prev) => (
                              prev
                                ? {
                                    ...prev,
                                    tools: prev.tools.map((item) => (
                                      item.id === nextTool.id ? nextTool : item
                                    )),
                                  }
                                : prev
                            )),
                          });
                        } finally {
                          setCliToolOnboardingBusy((prev) => ({ ...prev, [tool.id]: false }));
                        }
                      }}
                      size="sm"
                    >
                      {t("settings.cliTools.install")}
                    </Button>
                  </div>
                );
              })}
          </div>

          <DialogFooter>
            <Button
              disabled={cliToolsOnboardingBusy}
              onClick={() => setCliToolsOnboardingOpen(false)}
              variant="outline"
            >
              {t("settings.cliTools.later")}
            </Button>
            <Button
              disabled={
                cliToolsOnboardingBusy
                || !cliToolsOnboardingStatus
                || !cliToolsOnboardingStatus.tools.some((tool) => !tool.installed)
              }
              onClick={async () => {
                if (!cliToolsOnboardingStatus) {
                  return;
                }

                const missingTools = cliToolsOnboardingStatus.tools.filter((tool) => !tool.installed);
                if (missingTools.length === 0) {
                  setCliToolsOnboardingOpen(false);
                  return;
                }

                setCliToolsOnboardingBusy(true);
                try {
                  for (const tool of missingTools) {
                    setCliToolOnboardingBusy((prev) => ({ ...prev, [tool.id]: true }));
                    try {
                      await installCliToolWithToast({
                        tool,
                        t,
                        onToolUpdated: (nextTool) => setCliToolsOnboardingStatus((prev) => (
                          prev
                            ? {
                                ...prev,
                                tools: prev.tools.map((item) => (
                                  item.id === nextTool.id ? nextTool : item
                                )),
                              }
                            : prev
                        )),
                      });
                    } finally {
                      setCliToolOnboardingBusy((prev) => ({ ...prev, [tool.id]: false }));
                    }
                  }
                } finally {
                  setCliToolsOnboardingBusy(false);
                }

                const nextStatus = await getCliToolsStatus().catch(() => null);
                if (nextStatus) {
                  setCliToolsOnboardingStatus(nextStatus);
                  if (!nextStatus.tools.some((tool) => !tool.installed)) {
                    setCliToolsOnboardingOpen(false);
                  }
                }
              }}
            >
              {cliToolsOnboardingBusy ? t("settings.cliTools.installing") : t("settings.cliTools.installAll")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );
}
