import { useRef, useState, type Dispatch, type SetStateAction } from "react";
import { toast } from "sonner";

import { resetOpenAiAccountQuota } from "@/api";
import { useI18n } from "@/hooks/use-i18n";
import { humanizeApiError } from "@/lib/error";
import type { OpenAiRemoteAccount, RemoteAccount } from "@/types/api";

type ResetAttempt = { key: string; state: "pending" | "refresh_required" };
const storageKey = "cliswitch.quota-reset-attempts";

// Keep unresolved attempts across page navigation and app reloads. The key is
// retained until the server confirms an outcome; retrying must not spend again.
function readAttempts(): Record<string, ResetAttempt> {
  try {
    const parsed: unknown = JSON.parse(localStorage.getItem(storageKey) ?? "{}");
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) return {};
    return Object.fromEntries(Object.entries(parsed).filter(([, attempt]) =>
      attempt && typeof attempt.key === "string"
      && (attempt.state === "pending" || attempt.state === "refresh_required")));
  } catch {
    return {};
  }
}

export function useQuotaReset(setAccounts: Dispatch<SetStateAction<RemoteAccount[]>>) {
  const { t } = useI18n();
  const [attempts, setAttempts] = useState(readAttempts);
  const attemptsRef = useRef(attempts);
  const inFlight = useRef(new Set<string>());
  const [resetting, setResetting] = useState<Record<string, boolean>>({});

  function saveAttempt(id: string, attempt: ResetAttempt | null) {
    const next = { ...attemptsRef.current };
    if (attempt) next[id] = attempt;
    else delete next[id];
    // Persist before sending a consumption request, so an interrupted app can
    // safely resume the same attempt instead of generating another key.
    localStorage.setItem(storageKey, JSON.stringify(next));
    attemptsRef.current = next;
    setAttempts(next);
  }

  function markQuotaRefreshed(account: RemoteAccount) {
    if (!account.last_sync_error && attemptsRef.current[account.id]?.state === "refresh_required") {
      saveAttempt(account.id, null);
    }
  }

  async function resetQuota(account: OpenAiRemoteAccount): Promise<boolean> {
    if (inFlight.current.has(account.id)) return false;
    const previous = attemptsRef.current[account.id];
    if (previous?.state === "refresh_required") {
      toast.error(t("accounts.reset.refreshRequired"));
      return false;
    }
    if (account.reauth_required || (!previous && !(Number.isFinite(account.quota_reset_available_count)
      && (account.quota_reset_available_count ?? 0) > 0))) return false;

    inFlight.current.add(account.id);
    setResetting((current) => ({ ...current, [account.id]: true }));
    try {
      const attempt: ResetAttempt = previous ?? { key: crypto.randomUUID(), state: "pending" };
      saveAttempt(account.id, attempt);
      const result = await resetOpenAiAccountQuota(account.id, attempt.key);
      setAccounts((current) => current.map((item) => item.id === result.account.id ? result.account : item));
      const consumed = result.outcome === "reset" || result.outcome === "already_redeemed";
      if (consumed && result.quota_refresh_error) {
        saveAttempt(account.id, { ...attempt, state: "refresh_required" });
        toast.warning(t("accounts.reset.refreshFailed"));
        return true;
      }
      saveAttempt(account.id, null);
      if (consumed) toast.success(t("accounts.reset.success"));
      else if (result.outcome === "nothing_to_reset") toast.info(t("accounts.reset.nothingToReset"));
      else toast.info(t("accounts.reset.noCredit"));
      return true;
    } catch (error) {
      toast.error(t("accounts.reset.failed"), {
        description: `${t("accounts.reset.retryHint")} ${humanizeApiError(error, t)}`,
      });
      return false;
    } finally {
      inFlight.current.delete(account.id);
      setResetting((current) => ({ ...current, [account.id]: false }));
    }
  }

  return {
    resetting,
    pending: Object.fromEntries(Object.entries(attempts).filter(([, attempt]) => attempt.state === "pending").map(([id]) => [id, true])),
    refreshRequired: Object.fromEntries(Object.entries(attempts).filter(([, attempt]) => attempt.state === "refresh_required").map(([id]) => [id, true])),
    resetQuota,
    markQuotaRefreshed,
    forgetAccount: (id: string) => saveAttempt(id, null),
    getRefreshRequiredIds: () => Object.entries(attemptsRef.current)
      .filter(([, attempt]) => attempt.state === "refresh_required").map(([id]) => id),
  };
}
