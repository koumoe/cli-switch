import React, { useEffect, useReducer, useRef, useState } from "react";
import { Plus, RefreshCw } from "lucide-react";
import { toast } from "sonner";

import {
  type RemoteAccount,
  type RemoteGroupOption,
  type Sub2ApiRemoteAccount,
} from "@/api";
import {
  createRemoteManagedChannel,
  deleteRemoteAccount,
  listRemoteAccountGroups,
  listRemoteAccounts,
  openInBrowser,
  refreshRemoteAccount,
  remoteAccountSystemCheckin,
  remoteAccountCheckinsToday,
  reorderRemoteAccounts,
  updateRemoteAccount,
  completeRemoteAccountCheckinToday,
} from "@/api";
import { PageHeader } from "@/components/PageHeader";
import { Button } from "@/components/ui";
import { useCurrency } from "@/lib/currency";
import { humanizeApiError } from "@/lib/error";
import { useI18n } from "@/lib/i18n";
import { requestSub2ApiDesktopAuth } from "@/lib/ipc";

import { AccountEditorDialog } from "./accounts/AccountEditorDialog";
import { AccountWizardDialog } from "./accounts/AccountWizardDialog";
import { AccountsTable } from "./accounts/AccountsTable";
import { CreateKeyDialog } from "./accounts/CreateKeyDialog";
import { DeleteAccountDialog } from "./accounts/DeleteAccountDialog";
import { ManagedChannelDialog } from "./accounts/ManagedChannelDialog";
import { ManualCheckinDialog } from "./accounts/ManualCheckinDialog";
import {
  accountHasUserApiCredentials,
  defaultManagedDraft,
  dragReducer,
  emptyAccountDraft,
  initialDragState,
  isNewApiAccount,
  isSub2ApiAccount,
  resolveCheckinMode,
  resolveAccountDisplayName,
  ymdLocal,
  type AccountDraft,
  type ManagedChannelDraft,
} from "./accounts/shared";

export function AccountsPage() {
  const { t } = useI18n();
  const { currency } = useCurrency();
  const [accounts, setAccounts] = useState<RemoteAccount[]>([]);
  const [loading, setLoading] = useState(false);
  const [reordering, setReordering] = useState(false);
  const [dragState, dispatchDrag] = useReducer(dragReducer, initialDragState);
  const dragId = dragState.dragId;
  const dragOverId = dragState.dragOverId;
  const dragSnapshot = dragState.snapshot;
  const dragCommittedRef = useRef(false);
  const [checkinsDate, setCheckinsDate] = useState<string | null>(null);
  const [checkinDoneMap, setCheckinDoneMap] = useState<Record<string, boolean>>({});

  const [wizardOpen, setWizardOpen] = useState(false);

  const [editorOpen, setEditorOpen] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editingSource, setEditingSource] = useState<RemoteAccount | null>(null);
  const [draft, setDraft] = useState<AccountDraft>(emptyAccountDraft(currency));
  const [saving, setSaving] = useState(false);
  const [loginOpening, setLoginOpening] = useState(false);

  const [refreshing, setRefreshing] = useState<Record<string, boolean>>({});
  const [systemChecking, setSystemChecking] = useState<Record<string, boolean>>({});
  const [pageOpening, setPageOpening] = useState<Record<string, boolean>>({});

  const [manualPromptOpen, setManualPromptOpen] = useState(false);
  const [manualPromptTarget, setManualPromptTarget] = useState<RemoteAccount | null>(null);
  const [manualCompleting, setManualCompleting] = useState(false);

  const [deleteOpen, setDeleteOpen] = useState(false);
  const [deleteTarget, setDeleteTarget] = useState<RemoteAccount | null>(null);
  const [deleteManagedChannels, setDeleteManagedChannels] = useState(true);
  const [deleteSyncRemote, setDeleteSyncRemote] = useState(true);
  const [deleting, setDeleting] = useState(false);

  const [managedOpen, setManagedOpen] = useState(false);
  const [managedTarget, setManagedTarget] = useState<RemoteAccount | null>(null);
  const [managedDraft, setManagedDraft] = useState<ManagedChannelDraft | null>(null);
  const [managedGroups, setManagedGroups] = useState<RemoteGroupOption[]>([]);
  const [managedLoadingGroups, setManagedLoadingGroups] = useState(false);
  const [managedCreating, setManagedCreating] = useState(false);

  const [createKeyOpen, setCreateKeyOpen] = useState(false);
  const [createKeyTarget, setCreateKeyTarget] = useState<Sub2ApiRemoteAccount | null>(null);

  const [today, setToday] = useState(() => ymdLocal(Date.now()));

  useEffect(() => {
    const timer = window.setInterval(() => {
      setToday(ymdLocal(Date.now()));
    }, 60_000);
    return () => window.clearInterval(timer);
  }, []);

  async function refreshAll() {
    setLoading(true);
    try {
      const [items, checkins] = await Promise.all([
        listRemoteAccounts(),
        remoteAccountCheckinsToday().catch(() => null),
      ]);
      setAccounts(items);
      if (checkins) {
        setCheckinsDate(checkins.date);
        const next: Record<string, boolean> = {};
        for (const id of checkins.completed_account_ids) next[id] = true;
        setCheckinDoneMap(next);
      } else {
        setCheckinsDate(null);
        setCheckinDoneMap({});
      }
    } catch (e) {
      toast.error(t("accounts.toast.loadFail"), { description: humanizeApiError(e, t) });
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void refreshAll();
  }, []);

  useEffect(() => {
    if (!checkinsDate) return;
    if (checkinsDate === today) return;
    void remoteAccountCheckinsToday()
      .then((res) => {
        setCheckinsDate(res.date);
        const next: Record<string, boolean> = {};
        for (const id of res.completed_account_ids) next[id] = true;
        setCheckinDoneMap(next);
      })
      .catch(() => undefined);
  }, [checkinsDate, today]);

  function openCreate() {
    setWizardOpen(true);
  }

  function openEdit(item: RemoteAccount) {
    setEditingId(item.id);
    setEditingSource(item);
    setDraft({
      provider: item.provider,
      base_url: item.base_url ?? "",
      api_url: item.api_url ?? "",
      user_id: item.provider === "newapi" ? item.user_id ?? "" : "",
      user_token: "",
      bearer_token: "",
      page_checkin_url: item.page_checkin_url ?? "",
      checkin_mode: resolveCheckinMode(item),
      auto_checkin_time: item.auto_checkin_time ?? "00:05:00",
      low_balance_alert_threshold: String(item.low_balance_alert_threshold ?? 0),
      recharge_currency: item.recharge_currency,
    });
    setEditorOpen(true);
  }

  async function persistOrder(next: RemoteAccount[]) {
    setReordering(true);
    try {
      await reorderRemoteAccounts(next.map((item) => item.id));
      toast.success(t("accounts.toast.reorderOk"));
      await refreshAll();
    } catch (e) {
      toast.error(t("accounts.toast.reorderFail"), { description: humanizeApiError(e, t) });
      await refreshAll();
    } finally {
      setReordering(false);
    }
  }

  async function saveEditor() {
    if (!editingId) return;
    const baseUrl = draft.base_url.trim();
    const apiUrl = draft.api_url.trim();
    const pageUrl = draft.page_checkin_url.trim();
    const lowBalance = Number(draft.low_balance_alert_threshold);

    if (!baseUrl) {
      toast.error(t("accounts.toast.actionFail"), { description: t("accounts.toast.baseUrlRequired") });
      return;
    }
    if (!Number.isFinite(lowBalance) || lowBalance < 0) {
      toast.error(t("accounts.toast.actionFail"), { description: t("accounts.toast.thresholdInvalid") });
      return;
    }
    if (draft.checkin_mode === "page_open" && !pageUrl) {
      toast.error(t("accounts.toast.actionFail"), { description: t("accounts.toast.pageCheckinUrlRequired") });
      return;
    }

    setSaving(true);
    try {
      if (draft.provider === "newapi") {
        const userId = draft.user_id.trim();
        const token = draft.user_token.trim();
        const editingHasStoredToken = !!editingSource?.user_token_configured;
        const effectiveHasCredentials = !!userId && (!!token || editingHasStoredToken);
        const shouldClearStoredToken =
          editingSource?.provider === "newapi"
          && !userId
          && !token
          && draft.checkin_mode === "page_open"
          && editingHasStoredToken;

        if (draft.checkin_mode === "system_api" && !effectiveHasCredentials) {
          toast.error(t("accounts.toast.actionFail"), { description: t("accounts.toast.credentialsRequiredForSystem") });
          return;
        }

        await updateRemoteAccount(editingId, {
          provider: "newapi",
          base_url: baseUrl,
          api_url: apiUrl || null,
          user_id: userId,
          user_token: shouldClearStoredToken ? "" : token || undefined,
          page_checkin_url: pageUrl || null,
          checkin_mode: draft.checkin_mode,
          auto_checkin_time: draft.auto_checkin_time || "00:05:00",
          low_balance_alert_threshold: lowBalance,
          recharge_currency: draft.recharge_currency,
        });
      } else {
        const bearerToken = draft.bearer_token.trim();
        const editingHasStoredToken = !!editingSource?.user_token_configured;
        if (!bearerToken && !editingHasStoredToken) {
          toast.error(t("accounts.toast.actionFail"), { description: t("accounts.toast.bearerTokenRequired") });
          return;
        }
        await updateRemoteAccount(editingId, {
          provider: "sub2api",
          base_url: baseUrl,
          api_url: apiUrl || null,
          bearer_token: bearerToken || undefined,
          page_checkin_url: pageUrl || null,
          checkin_mode: draft.checkin_mode,
          auto_checkin_time: draft.auto_checkin_time || "00:05:00",
          low_balance_alert_threshold: lowBalance,
          recharge_currency: draft.recharge_currency,
        });
      }

      toast.success(t("accounts.toast.updateOk"));
      setEditorOpen(false);
      setEditingSource(null);
      await refreshAll();
    } catch (e) {
      toast.error(t("accounts.toast.actionFail"), { description: humanizeApiError(e, t) });
    } finally {
      setSaving(false);
    }
  }

  async function openLoginPageForEditor() {
    if (draft.provider !== "sub2api") return;
    const baseUrl = draft.base_url.trim();
    if (!baseUrl) {
      toast.error(t("accounts.toast.actionFail"), { description: t("accounts.toast.baseUrlRequired") });
      return;
    }
    setLoginOpening(true);
    try {
      const token = await requestSub2ApiDesktopAuth(baseUrl);
      if (!token) {
        toast(t("accounts.toast.sub2apiAuthCancelled"));
        return;
      }
      setDraft((current) => ({ ...current, bearer_token: token }));
    } catch (e) {
      const error = e instanceof Error ? e.message : "";
      if (error === "sub2api_auth_unsupported") {
        toast.error(t("accounts.toast.actionFail"), {
          description: t("accounts.toast.sub2apiAuthUnsupported"),
        });
        return;
      }
      toast.error(t("accounts.toast.actionFail"), {
        description: error || t("accounts.toast.sub2apiAuthFailed"),
      });
    } finally {
      setLoginOpening(false);
    }
  }

  async function onRefreshAccount(item: RemoteAccount) {
    setRefreshing((current) => ({ ...current, [item.id]: true }));
    try {
      await refreshRemoteAccount(item.id);
      toast.success(t("accounts.toast.refreshOk"));
      await refreshAll();
    } catch (e) {
      toast.error(t("accounts.toast.refreshFail"), { description: humanizeApiError(e, t) });
    } finally {
      setRefreshing((current) => ({ ...current, [item.id]: false }));
    }
  }

  async function onSystemCheckin(item: RemoteAccount) {
    if (!isNewApiAccount(item)) return;
    setSystemChecking((current) => ({ ...current, [item.id]: true }));
    try {
      await remoteAccountSystemCheckin(item.id);
      setCheckinDoneMap((current) => ({ ...current, [item.id]: true }));
      toast.success(t("accounts.toast.systemCheckinOk"));
      await refreshAll();
    } catch (e) {
      toast.error(t("accounts.toast.systemCheckinFail"), { description: humanizeApiError(e, t) });
    } finally {
      setSystemChecking((current) => ({ ...current, [item.id]: false }));
    }
  }

  async function openManualCheckinPrompt(item: RemoteAccount) {
    const url = (item.page_checkin_url ?? "").trim();
    if (!url) {
      toast.error(t("accounts.toast.actionFail"), { description: t("accounts.toast.pageCheckinUrlRequired") });
      return;
    }
    setPageOpening((current) => ({ ...current, [item.id]: true }));
    try {
      await openInBrowser(url);
      setManualPromptTarget(item);
      setManualPromptOpen(true);
    } catch (e) {
      toast.error(t("accounts.toast.actionFail"), { description: humanizeApiError(e, t) });
    } finally {
      setPageOpening((current) => ({ ...current, [item.id]: false }));
    }
  }

  async function confirmManualCheckin() {
    if (!manualPromptTarget) return;
    setManualCompleting(true);
    try {
      await completeRemoteAccountCheckinToday(manualPromptTarget.id);
      setCheckinDoneMap((current) => ({ ...current, [manualPromptTarget.id]: true }));
      toast.success(t("accounts.toast.manualCheckinOk", {
        name: resolveAccountDisplayName(manualPromptTarget),
      }));
      setManualPromptOpen(false);
      setManualPromptTarget(null);
    } catch (e) {
      toast.error(t("accounts.toast.actionFail"), { description: humanizeApiError(e, t) });
    } finally {
      setManualCompleting(false);
    }
  }

  function openDeleteDialog(item: RemoteAccount) {
    setDeleteTarget(item);
    setDeleteManagedChannels(true);
    setDeleteSyncRemote(true);
    setDeleteOpen(true);
  }

  async function confirmDelete() {
    if (!deleteTarget) return;
    setDeleting(true);
    try {
      await deleteRemoteAccount(
        deleteTarget.id,
        {
          delete_managed_channels: deleteManagedChannels,
          sync_remote_delete: deleteSyncRemote,
        }
      );
      toast.success(t("accounts.toast.deleteOk"));
      setDeleteOpen(false);
      setDeleteTarget(null);
      await refreshAll();
    } catch (e) {
      toast.error(t("accounts.toast.deleteFail"), { description: humanizeApiError(e, t) });
    } finally {
      setDeleting(false);
    }
  }

  function handleDeleteManagedChannelsChange(checked: boolean) {
    setDeleteManagedChannels(checked);
    if (!checked) setDeleteSyncRemote(false);
  }

  function handleDeleteSyncRemoteChange(checked: boolean) {
    setDeleteSyncRemote(checked);
    if (checked) setDeleteManagedChannels(true);
  }

  async function openCreateManagedChannelDialog(item: RemoteAccount) {
    if (!accountHasUserApiCredentials(item)) {
      toast.error(t("accounts.toast.actionFail"), {
        description: t(
          item.provider === "newapi"
            ? "accounts.toast.credentialsRequiredForManaged"
            : "accounts.toast.credentialsRequiredForKey"
        ),
      });
      return;
    }
    setManagedTarget(item);
    setManagedDraft(defaultManagedDraft(item));
    setManagedGroups([]);
    setManagedOpen(true);
    setManagedLoadingGroups(true);
    try {
      const groups = await listRemoteAccountGroups(item.id);
      setManagedGroups(groups);
      const preferred = (item.provider === "newapi" ? item.remote_group : "")?.trim() ?? "";
      const preferredGroup = preferred
        ? groups.find((group) => group.name === preferred) ?? null
        : null;
      const fallbackGroup = groups[0] ?? null;
      const nextGroup = preferredGroup ?? fallbackGroup;
      if (nextGroup) {
        setManagedDraft((current) => (current ? {
          ...current,
          group_name: nextGroup.name,
          group_id: nextGroup.id,
        } : current));
      }
    } catch (e) {
      toast.error(t("accounts.toast.loadGroupsFail"), { description: humanizeApiError(e, t) });
    } finally {
      setManagedLoadingGroups(false);
    }
  }

  async function createManaged() {
    if (!managedTarget || !managedDraft) return;
    const name = managedDraft.name.trim();
    const protocol = managedDraft.protocol;
    const groupName = managedDraft.group_name.trim();
    const baseUrlOverride = managedDraft.base_url_override.trim();
    const selectedGroup = managedDraft.group_id !== null
      ? managedGroups.find((group) => group.id === managedDraft.group_id) ?? null
      : managedGroups.find((group) => group.name === groupName) ?? null;
    if (!name || !groupName || !protocol) {
      toast.error(t("accounts.toast.actionFail"), { description: t("accounts.toast.managedRequired") });
      return;
    }
    setManagedCreating(true);
    try {
      await createRemoteManagedChannel(managedTarget.id, {
        name,
        protocol,
        group_name: groupName,
        group_id: selectedGroup?.id ?? null,
        base_url_override: baseUrlOverride || null,
      });
      toast.success(t("accounts.toast.createManagedOk"));
      setManagedOpen(false);
      setManagedTarget(null);
      setManagedDraft(null);
    } catch (e) {
      toast.error(t("accounts.toast.createManagedFail"), { description: humanizeApiError(e, t) });
    } finally {
      setManagedCreating(false);
    }
  }

  function openCreateKeyDialog(item: RemoteAccount) {
    if (!isSub2ApiAccount(item)) return;
    if (!accountHasUserApiCredentials(item)) {
      toast.error(t("accounts.toast.actionFail"), { description: t("accounts.toast.credentialsRequiredForKey") });
      return;
    }
    setCreateKeyTarget(item);
    setCreateKeyOpen(true);
  }

  function handleEditorOpenChange(open: boolean) {
    setEditorOpen(open);
    if (!open) {
      setEditingSource(null);
      setLoginOpening(false);
    }
  }

  function handleDeleteOpenChange(open: boolean) {
    setDeleteOpen(open);
    if (!open) setDeleteTarget(null);
  }

  function handleManagedOpenChange(open: boolean) {
    setManagedOpen(open);
    if (!open) {
      setManagedTarget(null);
      setManagedDraft(null);
    }
  }

  function handleManualCheckinCancel() {
    setManualPromptOpen(false);
    setManualPromptTarget(null);
  }

  function handleCreateKeyOpenChange(open: boolean) {
    setCreateKeyOpen(open);
    if (!open) setCreateKeyTarget(null);
  }

  return (
    <div className="space-y-4 pb-4">
      <PageHeader
        title={t("accounts.title")}
        actions={
          <>
            <Button variant="outline" size="sm" onClick={() => void refreshAll()} disabled={loading}>
              <RefreshCw className="h-4 w-4 mr-2" />
              {t("common.refresh")}
            </Button>
            <Button size="sm" onClick={openCreate}>
              <Plus className="h-4 w-4 mr-2" />
              {t("accounts.new")}
            </Button>
          </>
        }
      />

      <AccountsTable
        accounts={accounts}
        reordering={reordering}
        dragId={dragId}
        dragOverId={dragOverId}
        dragSnapshot={dragSnapshot}
        dragCommittedRef={dragCommittedRef}
        today={today}
        checkinsDate={checkinsDate}
        checkinDoneMap={checkinDoneMap}
        refreshing={refreshing}
        systemChecking={systemChecking}
        pageOpening={pageOpening}
        setAccounts={setAccounts}
        dispatchDrag={dispatchDrag}
        persistOrder={persistOrder}
        onRefreshAccount={onRefreshAccount}
        onSystemCheckin={onSystemCheckin}
        onOpenManualCheckinPrompt={openManualCheckinPrompt}
        onOpenCreateManagedChannelDialog={openCreateManagedChannelDialog}
        onOpenCreateKeyDialog={openCreateKeyDialog}
        onOpenEdit={openEdit}
        onOpenDeleteDialog={openDeleteDialog}
      />

      <AccountWizardDialog
        open={wizardOpen}
        defaultRechargeCurrency={currency}
        onOpenChange={setWizardOpen}
        onCreated={refreshAll}
      />

      <AccountEditorDialog
        open={editorOpen}
        account={editingSource}
        draft={draft}
        saving={saving}
        loginOpening={loginOpening}
        onOpenChange={handleEditorOpenChange}
        setDraft={setDraft}
        onSave={saveEditor}
        onOpenLoginPage={openLoginPageForEditor}
      />

      <ManualCheckinDialog
        open={manualPromptOpen}
        target={manualPromptTarget}
        completing={manualCompleting}
        onOpenChange={setManualPromptOpen}
        onCancel={handleManualCheckinCancel}
        onConfirm={confirmManualCheckin}
      />

      <ManagedChannelDialog
        open={managedOpen}
        target={managedTarget}
        draft={managedDraft}
        groups={managedGroups}
        loadingGroups={managedLoadingGroups}
        creating={managedCreating}
        onOpenChange={handleManagedOpenChange}
        setDraft={setManagedDraft}
        onCreate={createManaged}
      />

      <DeleteAccountDialog
        open={deleteOpen}
        target={deleteTarget}
        deleteManagedChannels={deleteManagedChannels}
        deleteSyncRemote={deleteSyncRemote}
        deleting={deleting}
        onOpenChange={handleDeleteOpenChange}
        onDeleteManagedChannelsChange={handleDeleteManagedChannelsChange}
        onDeleteSyncRemoteChange={handleDeleteSyncRemoteChange}
        onConfirmDelete={confirmDelete}
      />

      <CreateKeyDialog
        open={createKeyOpen}
        target={createKeyTarget}
        onOpenChange={handleCreateKeyOpenChange}
      />
    </div>
  );
}
