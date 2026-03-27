import React, { useEffect, useReducer, useRef, useState } from "react";
import { Plus, RefreshCw } from "lucide-react";
import { toast } from "sonner";

import { type NewApiAccount, type NewApiAccountCheckinMode, type NewApiGroupOption } from "@/api";
import {
  completeNewApiAccountCheckinToday,
  createNewApiAccount,
  createNewApiManagedChannel,
  deleteNewApiAccount,
  listNewApiAccounts,
  listNewApiGroups,
  newApiAccountCheckinsToday,
  newApiSystemCheckin,
  openInBrowser,
  refreshNewApiAccount,
  reorderNewApiAccounts,
  updateNewApiAccount,
} from "@/api";
import { PageHeader } from "@/components/PageHeader";
import { Button } from "@/components/ui";
import { useCurrency } from "@/lib/currency";
import { humanizeApiError } from "@/lib/error";
import { useI18n } from "@/lib/i18n";

import { AccountEditorDialog } from "./accounts/AccountEditorDialog";
import { AccountsTable } from "./accounts/AccountsTable";
import { DeleteAccountDialog } from "./accounts/DeleteAccountDialog";
import { ManagedChannelDialog } from "./accounts/ManagedChannelDialog";
import { ManualCheckinDialog } from "./accounts/ManualCheckinDialog";
import {
  accountHasUserApiCredentials,
  defaultManagedDraft,
  dragReducer,
  emptyAccountDraft,
  initialDragState,
  resolveCheckinMode,
  ymdLocal,
  type AccountDraft,
  type ManagedChannelDraft,
} from "./accounts/shared";

export function AccountsPage() {
  const { t } = useI18n();
  const { currency } = useCurrency();
  const [accounts, setAccounts] = useState<NewApiAccount[]>([]);
  const [loading, setLoading] = useState(false);
  const [reordering, setReordering] = useState(false);
  const [dragState, dispatchDrag] = useReducer(dragReducer, initialDragState);
  const dragId = dragState.dragId;
  const dragOverId = dragState.dragOverId;
  const dragSnapshot = dragState.snapshot;
  const dragCommittedRef = useRef(false);
  const [checkinsDate, setCheckinsDate] = useState<string | null>(null);
  const [checkinDoneMap, setCheckinDoneMap] = useState<Record<string, boolean>>({});

  const [editorOpen, setEditorOpen] = useState(false);
  const [editorMode, setEditorMode] = useState<"create" | "edit">("create");
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editingSource, setEditingSource] = useState<NewApiAccount | null>(null);
  const [draft, setDraft] = useState<AccountDraft>(emptyAccountDraft());
  const [saving, setSaving] = useState(false);

  const [refreshing, setRefreshing] = useState<Record<string, boolean>>({});
  const [systemChecking, setSystemChecking] = useState<Record<string, boolean>>({});
  const [pageOpening, setPageOpening] = useState<Record<string, boolean>>({});

  const [manualPromptOpen, setManualPromptOpen] = useState(false);
  const [manualPromptTarget, setManualPromptTarget] = useState<NewApiAccount | null>(null);
  const [manualCompleting, setManualCompleting] = useState(false);

  const [deleteOpen, setDeleteOpen] = useState(false);
  const [deleteTarget, setDeleteTarget] = useState<NewApiAccount | null>(null);
  const [deleteManagedChannels, setDeleteManagedChannels] = useState(true);
  const [deleteSyncRemote, setDeleteSyncRemote] = useState(true);
  const [deleting, setDeleting] = useState(false);

  const [managedOpen, setManagedOpen] = useState(false);
  const [managedTarget, setManagedTarget] = useState<NewApiAccount | null>(null);
  const [managedDraft, setManagedDraft] = useState<ManagedChannelDraft | null>(null);
  const [managedGroups, setManagedGroups] = useState<NewApiGroupOption[]>([]);
  const [managedLoadingGroups, setManagedLoadingGroups] = useState(false);
  const [managedCreating, setManagedCreating] = useState(false);

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
        listNewApiAccounts(),
        newApiAccountCheckinsToday().catch(() => null),
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
    void newApiAccountCheckinsToday()
      .then((res) => {
        setCheckinsDate(res.date);
        const next: Record<string, boolean> = {};
        for (const id of res.completed_account_ids) next[id] = true;
        setCheckinDoneMap(next);
      })
      .catch(() => undefined);
  }, [checkinsDate, today]);

  function openCreate() {
    setEditorMode("create");
    setEditingId(null);
    setEditingSource(null);
    setDraft({
      ...emptyAccountDraft(),
      recharge_currency: currency,
    });
    setEditorOpen(true);
  }

  function openEdit(item: NewApiAccount) {
    setEditorMode("edit");
    setEditingId(item.id);
    setEditingSource(item);
    setDraft({
      base_url: item.base_url ?? "",
      user_id: item.user_id ?? "",
      user_token: "",
      page_checkin_url: item.page_checkin_url ?? "",
      checkin_mode: resolveCheckinMode(item),
      auto_checkin_time: item.auto_checkin_time ?? "00:05:00",
      low_balance_alert_threshold: String(item.low_balance_alert_threshold ?? 0),
      recharge_currency: item.recharge_currency,
    });
    setEditorOpen(true);
  }

  async function persistOrder(next: NewApiAccount[]) {
    setReordering(true);
    try {
      await reorderNewApiAccounts(next.map((item) => item.id));
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
    const baseUrl = draft.base_url.trim();
    const userId = draft.user_id.trim();
    const pageUrl = draft.page_checkin_url.trim();
    const token = draft.user_token.trim();
    const lowBalance = Number(draft.low_balance_alert_threshold);
    const editingHasStoredToken = !!editingSource?.user_token_configured;
    const effectiveHasCredentials = !!userId && (editorMode === "create" ? !!token : (!!token || editingHasStoredToken));
    const shouldClearStoredToken =
      editorMode === "edit"
      && !userId
      && !token
      && draft.checkin_mode === "page_open"
      && editingHasStoredToken;

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
    if (draft.checkin_mode === "system_api" && !effectiveHasCredentials) {
      toast.error(t("accounts.toast.actionFail"), { description: t("accounts.toast.credentialsRequiredForSystem") });
      return;
    }

    const requestCheckinMode: NewApiAccountCheckinMode =
      draft.checkin_mode === "page_open" ? "page_open" : "system_api";
    const autoCheckinEnabled = draft.checkin_mode === "system_api";

    setSaving(true);
    try {
      if (editorMode === "create") {
        await createNewApiAccount({
          base_url: baseUrl,
          user_id: userId,
          user_token: token,
          page_checkin_url: pageUrl || null,
          checkin_mode: requestCheckinMode,
          auto_checkin_enabled: autoCheckinEnabled,
          auto_checkin_time: draft.auto_checkin_time || "00:05:00",
          low_balance_alert_threshold: lowBalance,
          recharge_currency: draft.recharge_currency,
        });
        toast.success(t("accounts.toast.createOk"));
      } else {
        if (!editingId) return;
        await updateNewApiAccount(editingId, {
          base_url: baseUrl,
          user_id: userId,
          user_token: shouldClearStoredToken ? "" : token || undefined,
          page_checkin_url: pageUrl || null,
          checkin_mode: requestCheckinMode,
          auto_checkin_enabled: autoCheckinEnabled,
          auto_checkin_time: draft.auto_checkin_time || "00:05:00",
          low_balance_alert_threshold: lowBalance,
          recharge_currency: draft.recharge_currency,
        });
        toast.success(t("accounts.toast.updateOk"));
      }
      setEditorOpen(false);
      setEditingSource(null);
      await refreshAll();
    } catch (e) {
      toast.error(t("accounts.toast.actionFail"), { description: humanizeApiError(e, t) });
    } finally {
      setSaving(false);
    }
  }

  async function onRefreshAccount(item: NewApiAccount) {
    setRefreshing((current) => ({ ...current, [item.id]: true }));
    try {
      await refreshNewApiAccount(item.id);
      toast.success(t("accounts.toast.refreshOk"));
      await refreshAll();
    } catch (e) {
      toast.error(t("accounts.toast.refreshFail"), { description: humanizeApiError(e, t) });
    } finally {
      setRefreshing((current) => ({ ...current, [item.id]: false }));
    }
  }

  async function onSystemCheckin(item: NewApiAccount) {
    setSystemChecking((current) => ({ ...current, [item.id]: true }));
    try {
      await newApiSystemCheckin(item.id);
      setCheckinDoneMap((current) => ({ ...current, [item.id]: true }));
      toast.success(t("accounts.toast.systemCheckinOk"));
      await refreshAll();
    } catch (e) {
      toast.error(t("accounts.toast.systemCheckinFail"), { description: humanizeApiError(e, t) });
    } finally {
      setSystemChecking((current) => ({ ...current, [item.id]: false }));
    }
  }

  async function openManualCheckinPrompt(item: NewApiAccount) {
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
      await completeNewApiAccountCheckinToday(manualPromptTarget.id);
      setCheckinDoneMap((current) => ({ ...current, [manualPromptTarget.id]: true }));
      toast.success(t("accounts.toast.manualCheckinOk", {
        name: manualPromptTarget.user_id || manualPromptTarget.base_url,
      }));
      setManualPromptOpen(false);
      setManualPromptTarget(null);
    } catch (e) {
      toast.error(t("accounts.toast.actionFail"), { description: humanizeApiError(e, t) });
    } finally {
      setManualCompleting(false);
    }
  }

  function openDeleteDialog(item: NewApiAccount) {
    setDeleteTarget(item);
    setDeleteManagedChannels(true);
    setDeleteSyncRemote(true);
    setDeleteOpen(true);
  }

  async function confirmDelete() {
    if (!deleteTarget) return;
    setDeleting(true);
    try {
      await deleteNewApiAccount(deleteTarget.id, {
        delete_managed_channels: deleteManagedChannels,
        sync_remote_delete: deleteSyncRemote,
      });
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

  async function openCreateManagedChannelDialog(item: NewApiAccount) {
    if (!accountHasUserApiCredentials(item)) {
      toast.error(t("accounts.toast.actionFail"), { description: t("accounts.toast.credentialsRequiredForManaged") });
      return;
    }
    setManagedTarget(item);
    setManagedDraft(defaultManagedDraft(item));
    setManagedGroups([]);
    setManagedOpen(true);
    setManagedLoadingGroups(true);
    try {
      const groups = await listNewApiGroups(item.id);
      setManagedGroups(groups);
      const preferred = (item.remote_group ?? "").trim();
      if (preferred && groups.some((group) => group.name === preferred)) {
        setManagedDraft((current) => (current ? { ...current, group_name: preferred } : current));
      } else if (groups[0]?.name) {
        setManagedDraft((current) => (current ? { ...current, group_name: groups[0].name } : current));
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
    const groupName = managedDraft.group_name.trim();
    const baseUrlOverride = managedDraft.base_url_override.trim();
    if (!name || !groupName) {
      toast.error(t("accounts.toast.actionFail"), { description: t("accounts.toast.managedRequired") });
      return;
    }
    setManagedCreating(true);
    try {
      await createNewApiManagedChannel(managedTarget.id, {
        name,
        protocol: managedDraft.protocol,
        group_name: groupName,
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

  function handleEditorOpenChange(open: boolean) {
    setEditorOpen(open);
    if (!open) setEditingSource(null);
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
        onOpenEdit={openEdit}
        onOpenDeleteDialog={openDeleteDialog}
      />

      <AccountEditorDialog
        open={editorOpen}
        mode={editorMode}
        draft={draft}
        saving={saving}
        onOpenChange={handleEditorOpenChange}
        setDraft={setDraft}
        onSave={saveEditor}
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
    </div>
  );
}
