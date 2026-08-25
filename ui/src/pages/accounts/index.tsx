import React, { useEffect, useState } from "react";
import { Plus, RefreshCw } from "lucide-react";
import { toast } from "sonner";

import type { RemoteAccount, RemoteGroupOption } from "@/types/api";
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
import { PageBody } from "@/components/layout/page-body";
import { Button } from "@/components/ui";
import { useCurrency } from "@/hooks/use-currency";
import { humanizeApiError, isApiRequestError } from "@/lib/error";
import { useI18n } from "@/hooks/use-i18n";

import { AccountEditorDialog } from "./AccountEditorDialog";
import { AccountWizardDialog } from "./AccountWizardDialog";
import { AccountsTable } from "./AccountsTable";
import { DeleteAccountDialog } from "./DeleteAccountDialog";
import { ManagedChannelDialog } from "./ManagedChannelDialog";
import { ManualCheckinDialog } from "./ManualCheckinDialog";
import {
  accountHasUserApiCredentials,
  accountToFormValues,
  defaultManagedDraft,
  isNewApiAccount,
  resolveAccountDisplayName,
  ymdLocal,
  type AccountFormValues,
  type ManagedChannelDraft,
} from "./shared";

export function AccountsPage() {
  const { t } = useI18n();
  const { currency } = useCurrency();
  const [accounts, setAccounts] = useState<RemoteAccount[]>([]);
  const [loading, setLoading] = useState(false);
  const [reordering, setReordering] = useState(false);
  const [checkinsDate, setCheckinsDate] = useState<string | null>(null);
  const [checkinDoneMap, setCheckinDoneMap] = useState<Record<string, boolean>>({});

  const [wizardOpen, setWizardOpen] = useState(false);

  const [editorOpen, setEditorOpen] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editingSource, setEditingSource] = useState<RemoteAccount | null>(null);

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

  function isSub2ApiReloginRequired(error: unknown): boolean {
    return isApiRequestError(error) && error.code === "remote_relogin_required";
  }

  function openCreate() {
    setWizardOpen(true);
  }

  function openEdit(item: RemoteAccount) {
    setEditingId(item.id);
    setEditingSource(item);
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

  async function saveEditor(values: AccountFormValues) {
    if (!editingId) return;
    const baseUrl = values.base_url.trim();
    const apiUrl = values.api_url.trim();
    const pageUrl = values.page_checkin_url.trim();
    const lowBalance = Number(values.low_balance_alert_threshold);

    try {
      if (values.provider === "newapi") {
        const userId = values.user_id.trim();
        const token = values.user_token.trim();
        const editingHasStoredToken = !!editingSource?.user_token_configured;
        const effectiveHasCredentials = !!userId && (!!token || editingHasStoredToken);
        const shouldClearStoredToken =
          editingSource?.provider === "newapi"
          && !userId
          && !token
          && values.checkin_mode === "page_open"
          && editingHasStoredToken;

        if (values.checkin_mode === "system_api" && !effectiveHasCredentials) {
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
          checkin_mode: values.checkin_mode,
          auto_checkin_time: values.auto_checkin_time || "00:05:00",
          low_balance_alert_threshold: lowBalance,
          recharge_currency: values.recharge_currency,
        });
      } else {
        const bearerToken = values.bearer_token.trim();
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
          refresh_token: values.refresh_token.trim() || undefined,
          page_checkin_url: pageUrl || null,
          checkin_mode: values.checkin_mode,
          auto_checkin_time: values.auto_checkin_time || "00:05:00",
          low_balance_alert_threshold: lowBalance,
          recharge_currency: values.recharge_currency,
        });
      }

      toast.success(t("accounts.toast.updateOk"));
      setEditorOpen(false);
      setEditingId(null);
      setEditingSource(null);
      await refreshAll();
    } catch (e) {
      toast.error(t("accounts.toast.actionFail"), { description: humanizeApiError(e, t) });
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
      if (isSub2ApiReloginRequired(e)) {
        await refreshAll();
      }
    } finally {
      setRefreshing((current) => ({ ...current, [item.id]: false }));
    }
  }

  async function onOpenBrowser(item: RemoteAccount) {
    try {
      await openInBrowser(item.base_url);
    } catch (e) {
      toast.error(t("accounts.toast.actionFail"), { description: humanizeApiError(e, t) });
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
      if (isSub2ApiReloginRequired(e)) {
        await refreshAll();
      }
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
      if (isSub2ApiReloginRequired(e)) {
        await refreshAll();
      }
    } finally {
      setManagedCreating(false);
    }
  }

  function handleEditorOpenChange(open: boolean) {
    setEditorOpen(open);
    if (!open) {
      setEditingId(null);
      setEditingSource(null);
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

  return (
    <div className="flex h-full min-h-0 flex-col">
      <PageHeader
        title={t("accounts.title")}
        actions={
          <>
            <Button variant="outline" size="icon" onClick={() => void refreshAll()} disabled={loading}>
              <RefreshCw className="h-3.5 w-3.5" />
            </Button>
            <Button size="sm" onClick={openCreate}>
              <Plus className="h-3.5 w-3.5" />
              {t("accounts.new")}
            </Button>
          </>
        }
      />
      <div className="flex-1 overflow-y-auto">
        <PageBody className="flex h-full min-h-0 flex-col gap-3">
          <AccountsTable
            accounts={accounts}
            loading={loading}
            reordering={reordering}
            today={today}
            checkinsDate={checkinsDate}
            checkinDoneMap={checkinDoneMap}
            refreshing={refreshing}
            systemChecking={systemChecking}
            pageOpening={pageOpening}
            setAccounts={setAccounts}
            persistOrder={persistOrder}
            onRefreshAccount={onRefreshAccount}
            onOpenBrowser={onOpenBrowser}
            onSystemCheckin={onSystemCheckin}
            onOpenManualCheckinPrompt={openManualCheckinPrompt}
            onOpenCreateManagedChannelDialog={openCreateManagedChannelDialog}
            onOpenEdit={openEdit}
            onOpenDeleteDialog={openDeleteDialog}
          />
        </PageBody>
      </div>

      <AccountWizardDialog
        open={wizardOpen}
        defaultRechargeCurrency={currency}
        onOpenChange={setWizardOpen}
        onCreated={refreshAll}
      />

      <AccountEditorDialog
        open={editorOpen}
        account={editingSource}
        initialValues={editingSource ? accountToFormValues(editingSource) : null}
        onOpenChange={handleEditorOpenChange}
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
