import { useCallback, useEffect, useRef, useState } from "react";
import { toast } from "sonner";

import { getCliToolsStatus } from "@/api";
import { useI18n } from "@/hooks/use-i18n";
import { installCliToolWithToast } from "@/lib/cliToolInstaller";
import { humanizeApiError } from "@/lib/error";
import type { CliToolId, CliToolsStatus } from "@/types/api";

const EMPTY_BUSY: Record<CliToolId, boolean> = {
  gemini: false,
  claude: false,
  codex: false,
};

export function useCliToolsStatus() {
  const { t } = useI18n();
  const [status, setStatus] = useState<CliToolsStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(EMPTY_BUSY);
  const busyRef = useRef(EMPTY_BUSY);
  const mounted = useRef(false);
  const requestSequence = useRef(0);
  const pendingRequest = useRef<number | null>(null);

  const load = useCallback(async (mode: "initial" | "manual" | "poll") => {
    if (!mounted.current) return;
    // Manual checks supersede cache reads; polling never supersedes an active read.
    if (mode !== "manual" && pendingRequest.current !== null) return;
    const sequence = ++requestSequence.current;
    pendingRequest.current = sequence;
    if (mode !== "poll") setLoading(true);

    try {
      const next = mode === "manual"
        ? await getCliToolsStatus({ refresh: true })
        : await getCliToolsStatus();
      if (mounted.current && sequence === requestSequence.current) {
        setStatus(next);
        setError(null);
      }
    } catch (e) {
      if (mounted.current && sequence === requestSequence.current) {
        const message = humanizeApiError(e, t);
        setError(message);
        if (mode !== "poll") {
          toast.error(t("settings.cliTools.loadFail"), { description: message });
        }
      }
    } finally {
      if (pendingRequest.current === sequence) pendingRequest.current = null;
      if (mounted.current && sequence === requestSequence.current) {
        setLoading(false);
      }
    }
  }, [t]);

  useEffect(() => {
    mounted.current = true;
    void load("initial");
    const timer = window.setInterval(() => void load("poll"), 30_000);
    return () => {
      mounted.current = false;
      window.clearInterval(timer);
      ++requestSequence.current;
      pendingRequest.current = null;
    };
  }, [load]);

  const refresh = useCallback(() => load("manual"), [load]);

  const install = async (toolId: CliToolId) => {
    const tool = status?.tools.find((item) => item.id === toolId);
    if (
      !tool || !mounted.current || loading || busyRef.current[toolId] || tool.updating ||
      (tool.installed && !tool.update_available)
    ) return;

    // Invalidate reads begun before installation so they cannot restore old versions.
    ++requestSequence.current;
    pendingRequest.current = null;
    setLoading(false);
    busyRef.current = { ...busyRef.current, [toolId]: true };
    setBusy(busyRef.current);
    try {
      await installCliToolWithToast({
        tool,
        t,
        onToolUpdated: (nextTool) => {
          if (!mounted.current) return;
          // Reads may continue during long installs, but the install result wins.
          ++requestSequence.current;
          pendingRequest.current = null;
          setLoading(false);
          setStatus((previous) => previous ? {
            ...previous,
            tools: previous.tools.map((item) => item.id === nextTool.id ? nextTool : item),
          } : previous);
        },
      });
    } finally {
      busyRef.current = { ...busyRef.current, [toolId]: false };
      if (mounted.current) setBusy(busyRef.current);
    }
  };

  return { status, loading, error, busy, refresh, install };
}
