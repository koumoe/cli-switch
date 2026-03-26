import React, { useEffect, useMemo, useReducer, useRef, useState } from "react";
import { Plus, Pencil, Trash2, RefreshCw, KeyRound, GripVertical } from "lucide-react";
import { toast } from "sonner";
import {
  Badge,
  Button,
  Card,
  CardContent,
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  Input,
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
  Switch,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui";
import { PageHeader } from "@/components/PageHeader";
import { useI18n } from "@/lib/i18n";
import { humanizeApiError } from "@/lib/error";
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
  type NewApiAccount,
  type NewApiAccountCheckinMode,
  type NewApiGroupOption,
  type Protocol,
} from "@/api";

type AccountCheckinModeOption = "disabled" | NewApiAccountCheckinMode;

type AccountDraft = {
  base_url: string;
  user_id: string;
  user_token: string;
  page_checkin_url: string;
  checkin_mode: AccountCheckinModeOption;
  auto_checkin_time: string;
  low_balance_alert_threshold: string;
};

type ManagedChannelDraft = {
  name: string;
  protocol: Protocol;
  group_name: string;
  base_url_override: string;
};

function emptyAccountDraft(): AccountDraft {
  return {
    base_url: "",
    user_id: "",
    user_token: "",
    page_checkin_url: "",
    checkin_mode: "disabled",
    auto_checkin_time: "00:05:00",
    low_balance_alert_threshold: "0",
  };
}

function ymdLocal(ms: number): string {
  const d = new Date(ms);
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${y}-${m}-${day}`;
}

function formatAmount(account: NewApiAccount, v: number | null): string {
  if (v === null || v === undefined || !Number.isFinite(v)) return "-";
  if (account.quota_display_type === "TOKENS") return v.toFixed(0);
  const symbol = account.quota_display_type === "CNY"
    ? "¥"
    : account.quota_display_type === "CUSTOM"
      ? (account.custom_currency_symbol || "¤")
      : "$";
  return `${symbol}${v.toFixed(2)}`;
}

function defaultManagedName(account: NewApiAccount, protocol: Protocol): string {
  return `${account.user_id}-${protocol}`;
}

function defaultManagedDraft(account: NewApiAccount): ManagedChannelDraft {
  return {
    name: defaultManagedName(account, "openai"),
    protocol: "openai",
    group_name: account.remote_group ?? "",
    base_url_override: "",
  };
}

function resolveCheckinMode(account: NewApiAccount): AccountCheckinModeOption {
  if (account.checkin_mode === "page_open") return "page_open";
  return account.auto_checkin_enabled ? "system_api" : "disabled";
}

function accountHasUserApiCredentials(account: Pick<NewApiAccount, "user_id" | "user_token_configured">): boolean {
  return !!account.user_id.trim() && !!account.user_token_configured;
}

type DragState = {
  dragId: string | null;
  dragOverId: string | null;
  snapshot: NewApiAccount[] | null;
};

type DragAction =
  | { type: "start"; dragId: string; snapshot: NewApiAccount[] }
  | { type: "over"; dragOverId: string | null }
  | { type: "clear" };

const initialDragState: DragState = {
  dragId: null,
  dragOverId: null,
  snapshot: null,
};

function dragReducer(state: DragState, action: DragAction): DragState {
  switch (action.type) {
    case "start":
      return { dragId: action.dragId, dragOverId: null, snapshot: action.snapshot };
    case "over":
      return { ...state, dragOverId: action.dragOverId };
    case "clear":
      return initialDragState;
    default: {
      const _exhaustive: never = action;
      return state;
    }
  }
}

export function AccountsPage() {
  const { t } = useI18n();
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

  const today = useMemo(() => ymdLocal(Date.now()), []);

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
    setDraft(emptyAccountDraft());
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

  function moveInList(list: NewApiAccount[], fromId: string, toId: string): NewApiAccount[] {
    if (fromId === toId) return list;
    const fromIdx = list.findIndex((item) => item.id === fromId);
    const toIdx = list.findIndex((item) => item.id === toId);
    if (fromIdx < 0 || toIdx < 0 || fromIdx === toIdx) return list;
    const next = [...list];
    const [item] = next.splice(fromIdx, 1);
    next.splice(toIdx, 0, item);
    return next;
  }

  function moveToEndList(list: NewApiAccount[], fromId: string): NewApiAccount[] {
    const fromIdx = list.findIndex((item) => item.id === fromId);
    if (fromIdx < 0) return list;
    const next = [...list];
    const [item] = next.splice(fromIdx, 1);
    next.push(item);
    return next;
  }

  function setAccountDragPreview(e: React.DragEvent, item: NewApiAccount) {
    try {
      const el = document.createElement("div");
      el.style.position = "absolute";
      el.style.top = "-10000px";
      el.style.left = "-10000px";
      el.style.padding = "10px 12px";
      el.style.borderRadius = "10px";
      el.style.border = "1px solid rgba(0,0,0,0.12)";
      el.style.background = "white";
      el.style.boxShadow = "0 12px 30px rgba(0,0,0,0.18)";
      el.style.minWidth = "280px";
      el.style.maxWidth = "380px";
      el.style.pointerEvents = "none";

      const title = document.createElement("div");
      title.textContent = item.base_url;
      title.style.fontSize = "13px";
      title.style.fontWeight = "600";
      title.style.color = "rgba(0,0,0,0.92)";

      const meta = document.createElement("div");
      meta.textContent = item.user_id || t("accounts.checkin.none");
      meta.style.marginTop = "4px";
      meta.style.fontSize = "11px";
      meta.style.color = "rgba(0,0,0,0.6)";
      meta.style.whiteSpace = "nowrap";
      meta.style.overflow = "hidden";
      meta.style.textOverflow = "ellipsis";

      el.appendChild(title);
      el.appendChild(meta);
      document.body.appendChild(el);

      e.dataTransfer.setDragImage(el, 16, 16);
      window.setTimeout(() => el.remove(), 0);
    } catch {
      // ignore
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
    if (draft.checkin_mode !== "page_open" && !effectiveHasCredentials) {
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
    setRefreshing((m) => ({ ...m, [item.id]: true }));
    try {
      await refreshNewApiAccount(item.id);
      toast.success(t("accounts.toast.refreshOk"));
      await refreshAll();
    } catch (e) {
      toast.error(t("accounts.toast.refreshFail"), { description: humanizeApiError(e, t) });
    } finally {
      setRefreshing((m) => ({ ...m, [item.id]: false }));
    }
  }

  async function onSystemCheckin(item: NewApiAccount) {
    setSystemChecking((m) => ({ ...m, [item.id]: true }));
    try {
      await newApiSystemCheckin(item.id);
      setCheckinDoneMap((m) => ({ ...m, [item.id]: true }));
      toast.success(t("accounts.toast.systemCheckinOk"));
      await refreshAll();
    } catch (e) {
      toast.error(t("accounts.toast.systemCheckinFail"), { description: humanizeApiError(e, t) });
    } finally {
      setSystemChecking((m) => ({ ...m, [item.id]: false }));
    }
  }

  async function openManualCheckinPrompt(item: NewApiAccount) {
    const url = (item.page_checkin_url ?? "").trim();
    if (!url) {
      toast.error(t("accounts.toast.actionFail"), { description: t("accounts.toast.pageCheckinUrlRequired") });
      return;
    }
    setPageOpening((m) => ({ ...m, [item.id]: true }));
    try {
      await openInBrowser(url);
      setManualPromptTarget(item);
      setManualPromptOpen(true);
    } catch (e) {
      toast.error(t("accounts.toast.actionFail"), { description: humanizeApiError(e, t) });
    } finally {
      setPageOpening((m) => ({ ...m, [item.id]: false }));
    }
  }

  async function confirmManualCheckin() {
    if (!manualPromptTarget) return;
    setManualCompleting(true);
    try {
      await completeNewApiAccountCheckinToday(manualPromptTarget.id);
      setCheckinDoneMap((m) => ({ ...m, [manualPromptTarget.id]: true }));
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
      if (preferred && groups.some((g) => g.name === preferred)) {
        setManagedDraft((d) => (d ? { ...d, group_name: preferred } : d));
      } else if (groups[0]?.name) {
        setManagedDraft((d) => (d ? { ...d, group_name: groups[0].name } : d));
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

      <Card>
        <CardContent className="p-0">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead className="w-10"></TableHead>
                <TableHead>{t("accounts.table.baseUrl")}</TableHead>
                <TableHead>{t("accounts.table.balance")}</TableHead>
                <TableHead>{t("accounts.table.checkin")}</TableHead>
                <TableHead className="text-center">{t("common.actions")}</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody
              onDragOver={(e) => {
                if (e.target !== e.currentTarget) return;
                e.preventDefault();
                if (dragOverId !== null) dispatchDrag({ type: "over", dragOverId: null });
              }}
              onDrop={(e) => {
                if (e.defaultPrevented) return;
                e.preventDefault();
                const fromId = e.dataTransfer.getData("text/plain");
                if (fromId) {
                  const next = moveToEndList(accounts, fromId);
                  dragCommittedRef.current = true;
                  setAccounts(next);
                  void persistOrder(next);
                }
                dispatchDrag({ type: "clear" });
              }}
            >
              {accounts.length === 0 ? (
                <TableRow>
                  <TableCell colSpan={5} className="text-center text-muted-foreground py-8">
                    {t("accounts.table.empty")}
                  </TableCell>
                </TableRow>
              ) : (
                accounts.map((item) => {
                  const logicalCheckinMode = resolveCheckinMode(item);
                  const done = !!checkinDoneMap[item.id] && checkinsDate === today;
                  const checkinBadge = logicalCheckinMode === "disabled"
                    ? { text: t("accounts.checkin.none"), variant: "secondary" as const }
                    : done
                      ? { text: t("accounts.checkin.done"), variant: "success" as const }
                      : { text: t("accounts.checkin.todo"), variant: "destructive" as const };
                  const canTriggerCheckin = logicalCheckinMode !== "disabled" && !done;
                  const checkinBusy = !!systemChecking[item.id] || !!pageOpening[item.id];
                  const canManageToken = accountHasUserApiCredentials(item);
                  return (
                    <TableRow
                      key={item.id}
                      onDragOver={(e) => {
                        e.preventDefault();
                        if (!dragId || reordering) return;
                        if (dragId === item.id) return;
                        if (dragOverId === item.id) return;
                        dispatchDrag({ type: "over", dragOverId: item.id });
                        setAccounts((current) => moveInList(current, dragId, item.id));
                      }}
                      onDragLeave={() => {
                        if (dragOverId === item.id) dispatchDrag({ type: "over", dragOverId: null });
                      }}
                      onDrop={(e) => {
                        e.stopPropagation();
                        e.preventDefault();
                        const fromId = e.dataTransfer.getData("text/plain");
                        if (fromId) {
                          const next = moveInList(accounts, fromId, item.id);
                          dragCommittedRef.current = true;
                          setAccounts(next);
                          void persistOrder(next);
                        }
                        dispatchDrag({ type: "clear" });
                      }}
                      className={[
                        dragId === item.id ? "opacity-60" : "",
                        dragOverId === item.id ? "bg-accent/30" : "",
                      ].filter(Boolean).join(" ")}
                    >
                      <TableCell>
                        <button
                          className="text-muted-foreground hover:text-foreground cursor-grab active:cursor-grabbing"
                          draggable={!reordering}
                          onDragStart={(e) => {
                            dragCommittedRef.current = false;
                            e.dataTransfer.setData("text/plain", item.id);
                            e.dataTransfer.effectAllowed = "move";
                            setAccountDragPreview(e, item);
                            dispatchDrag({ type: "start", dragId: item.id, snapshot: accounts });
                          }}
                          onDragEnd={() => {
                            if (!dragCommittedRef.current && dragSnapshot) {
                              setAccounts(dragSnapshot);
                            }
                            dispatchDrag({ type: "clear" });
                          }}
                          title={t("accounts.actions.drag")}
                        >
                          <GripVertical className="h-4 w-4" />
                        </button>
                      </TableCell>
                      <TableCell className="max-w-[320px]">
                        <div className="truncate" title={item.base_url}>
                          {item.base_url}
                        </div>
                      </TableCell>
                      <TableCell>
                        <div className="font-mono">{formatAmount(item, item.last_balance_amount)}</div>
                      </TableCell>
                      <TableCell>
                        {canTriggerCheckin ? (
                          <button
                            type="button"
                            className="inline-flex"
                            onClick={() => {
                              if (logicalCheckinMode === "system_api") {
                                void onSystemCheckin(item);
                              } else {
                                void openManualCheckinPrompt(item);
                              }
                            }}
                            disabled={checkinBusy}
                            title={logicalCheckinMode === "system_api"
                              ? t("accounts.actions.systemCheckin")
                              : t("accounts.actions.manualCheckin")}
                          >
                            <Badge variant={checkinBadge.variant} className={checkinBusy ? "opacity-60" : "cursor-pointer"}>
                              {checkinBadge.text}
                            </Badge>
                          </button>
                        ) : (
                          <Badge variant={checkinBadge.variant}>
                            {checkinBadge.text}
                          </Badge>
                        )}
                      </TableCell>
                      <TableCell>
                        <div className="flex items-center justify-center gap-1">
                          <Button
                            variant="ghost"
                            size="icon"
                            onClick={() => void onRefreshAccount(item)}
                            disabled={!!refreshing[item.id]}
                            title={t("accounts.actions.refresh")}
                          >
                            <RefreshCw className="h-4 w-4" />
                          </Button>
                          <Button
                            variant="ghost"
                            size="icon"
                            onClick={() => openCreateManagedChannelDialog(item)}
                            disabled={!canManageToken}
                            title={t("accounts.actions.createManaged")}
                          >
                            <KeyRound className="h-4 w-4" />
                          </Button>
                          <Button
                            variant="ghost"
                            size="icon"
                            onClick={() => openEdit(item)}
                            title={t("accounts.actions.edit")}
                          >
                            <Pencil className="h-4 w-4" />
                          </Button>
                          <Button
                            variant="ghost"
                            size="icon"
                            onClick={() => openDeleteDialog(item)}
                            title={t("accounts.actions.delete")}
                          >
                            <Trash2 className="h-4 w-4 text-destructive" />
                          </Button>
                        </div>
                      </TableCell>
                    </TableRow>
                  );
                })
              )}
            </TableBody>
          </Table>
        </CardContent>
      </Card>

      <Dialog
        open={editorOpen}
        onOpenChange={(open) => {
          setEditorOpen(open);
          if (!open) setEditingSource(null);
        }}
      >
        <DialogContent className="sm:max-w-[560px] max-h-[85vh] overflow-hidden flex flex-col">
          <DialogHeader>
            <DialogTitle>
              {editorMode === "create" ? t("accounts.editor.createTitle") : t("accounts.editor.editTitle")}
            </DialogTitle>
            <DialogDescription>{t("accounts.editor.description")}</DialogDescription>
          </DialogHeader>

          <div className="flex-1 min-h-0 space-y-4 py-2 overflow-y-auto pr-1">
            <div className="space-y-2">
              <label className="text-sm font-medium">{t("accounts.editor.baseUrl")}</label>
              <Input
                value={draft.base_url}
                onChange={(e) => setDraft((d) => ({ ...d, base_url: e.target.value }))}
                placeholder="https://new-api.example.com"
              />
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">{t("accounts.editor.userId")}</label>
              <Input
                value={draft.user_id}
                onChange={(e) => setDraft((d) => ({ ...d, user_id: e.target.value }))}
                placeholder="1001"
              />
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">{t("accounts.editor.userToken")}</label>
              <Input
                type="password"
                value={draft.user_token}
                onChange={(e) => setDraft((d) => ({ ...d, user_token: e.target.value }))}
                placeholder={editorMode === "edit" ? t("accounts.editor.userTokenKeepHint") : "sk-..."}
              />
            </div>
            {draft.checkin_mode === "page_open" ? (
              <div className="space-y-2">
                <label className="text-sm font-medium">{t("accounts.editor.pageCheckinUrl")}</label>
                <Input
                  value={draft.page_checkin_url}
                  onChange={(e) => setDraft((d) => ({ ...d, page_checkin_url: e.target.value }))}
                  placeholder="https://new-api.example.com/user/checkin"
                />
              </div>
            ) : null}
            <div className={draft.checkin_mode === "system_api" ? "grid grid-cols-2 gap-4" : "space-y-2"}>
              <div className="space-y-2">
                <label className="text-sm font-medium">{t("accounts.editor.checkinMode")}</label>
                <Select
                  value={draft.checkin_mode}
                  onValueChange={(v) => setDraft((d) => ({ ...d, checkin_mode: v as AccountCheckinModeOption }))}
                >
                  <SelectTrigger>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="disabled">{t("accounts.checkin.modeDisabled")}</SelectItem>
                    <SelectItem value="system_api">{t("accounts.checkin.modeSystem")}</SelectItem>
                    <SelectItem value="page_open">{t("accounts.checkin.modePage")}</SelectItem>
                  </SelectContent>
                </Select>
              </div>
              {draft.checkin_mode === "system_api" ? (
                <div className="space-y-2">
                  <label className="text-sm font-medium">{t("accounts.editor.autoCheckinTime")}</label>
                  <Input
                    value={draft.auto_checkin_time}
                    onChange={(e) => setDraft((d) => ({ ...d, auto_checkin_time: e.target.value }))}
                    placeholder="00:05:00"
                  />
                </div>
              ) : null}
            </div>
            <div className="space-y-2">
              <label className="text-sm font-medium">{t("accounts.editor.lowBalanceThreshold")}</label>
              <Input
                type="number"
                step="0.01"
                min="0"
                value={draft.low_balance_alert_threshold}
                onChange={(e) => setDraft((d) => ({ ...d, low_balance_alert_threshold: e.target.value }))}
                placeholder="0"
              />
              <p className="text-xs text-muted-foreground">{t("accounts.editor.lowBalanceHint")}</p>
            </div>
          </div>

          <DialogFooter>
            <Button variant="outline" onClick={() => setEditorOpen(false)} disabled={saving}>
              {t("common.cancel")}
            </Button>
            <Button onClick={() => void saveEditor()} disabled={saving}>
              {t("common.save")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog open={manualPromptOpen} onOpenChange={setManualPromptOpen}>
        <DialogContent className="sm:max-w-[420px]">
          <DialogHeader>
            <DialogTitle>{t("accounts.checkin.dialog.title")}</DialogTitle>
            <DialogDescription>
              {t("accounts.checkin.dialog.description", {
                name: manualPromptTarget?.user_id || manualPromptTarget?.base_url || "",
              })}
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button
              variant="outline"
              onClick={() => {
                setManualPromptOpen(false);
                setManualPromptTarget(null);
              }}
              disabled={manualCompleting}
            >
              {t("accounts.checkin.dialog.notDone")}
            </Button>
            <Button onClick={() => void confirmManualCheckin()} disabled={manualCompleting || !manualPromptTarget}>
              {t("accounts.checkin.dialog.done")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog
        open={managedOpen}
        onOpenChange={(v) => {
          setManagedOpen(v);
          if (!v) {
            setManagedTarget(null);
            setManagedDraft(null);
          }
        }}
      >
        <DialogContent className="sm:max-w-[560px] max-h-[85vh] overflow-hidden flex flex-col">
          <DialogHeader>
            <DialogTitle>{t("accounts.managed.title")}</DialogTitle>
            <DialogDescription>
              {t("accounts.managed.description", { name: managedTarget?.user_id ?? "" })}
            </DialogDescription>
          </DialogHeader>
          {managedDraft ? (
            <div className="flex-1 min-h-0 space-y-4 py-2 overflow-y-auto pr-1">
              <div className="grid grid-cols-2 gap-4">
                <div className="space-y-2">
                  <label className="text-sm font-medium">{t("accounts.managed.name")}</label>
                  <Input
                    value={managedDraft.name}
                    onChange={(e) =>
                      setManagedDraft((d) => (d ? { ...d, name: e.target.value } : d))
                    }
                  />
                </div>
                <div className="space-y-2">
                  <label className="text-sm font-medium">{t("accounts.managed.protocol")}</label>
                  <Select
                    value={managedDraft.protocol}
                    onValueChange={(v) =>
                      setManagedDraft((d) => {
                        if (!d) return d;
                        const protocol = v as Protocol;
                        if (!managedTarget) return { ...d, protocol };
                        const oldAuto = defaultManagedName(managedTarget, d.protocol);
                        const nextName = d.name === oldAuto ? defaultManagedName(managedTarget, protocol) : d.name;
                        return { ...d, protocol, name: nextName };
                      })
                    }
                  >
                    <SelectTrigger>
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="openai">OpenAI</SelectItem>
                      <SelectItem value="anthropic">Anthropic</SelectItem>
                      <SelectItem value="gemini">Gemini</SelectItem>
                    </SelectContent>
                  </Select>
                </div>
              </div>
              <div className="space-y-2">
                <label className="text-sm font-medium">{t("accounts.managed.group")}</label>
                <Select
                  value={managedDraft.group_name}
                  onValueChange={(v) => setManagedDraft((d) => (d ? { ...d, group_name: v } : d))}
                  disabled={managedLoadingGroups}
                >
                  <SelectTrigger>
                    <SelectValue placeholder={t("accounts.managed.groupPlaceholder")} />
                  </SelectTrigger>
                  <SelectContent>
                    {managedGroups.map((g) => (
                      <SelectItem key={g.name} value={g.name}>
                        <div className="flex flex-col">
                          <span>
                            {g.name}
                            {g.ratio !== null && g.ratio !== undefined ? ` (x${g.ratio})` : ""}
                          </span>
                          {g.description ? (
                            <span className="text-xs text-muted-foreground">{g.description}</span>
                          ) : null}
                        </div>
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
              <div className="space-y-2">
                <label className="text-sm font-medium">{t("accounts.managed.baseUrlOverride")}</label>
                <Input
                  value={managedDraft.base_url_override}
                  onChange={(e) => setManagedDraft((d) => (d ? { ...d, base_url_override: e.target.value } : d))}
                  placeholder={managedTarget?.base_url || "https://new-api.example.com"}
                />
                <p className="text-xs text-muted-foreground">{t("accounts.managed.baseUrlOverrideHint")}</p>
              </div>
            </div>
          ) : null}
          <DialogFooter>
            <Button
              variant="outline"
              onClick={() => {
                setManagedOpen(false);
                setManagedTarget(null);
                setManagedDraft(null);
              }}
              disabled={managedCreating}
            >
              {t("common.cancel")}
            </Button>
            <Button onClick={() => void createManaged()} disabled={managedCreating || !managedDraft}>
              {t("accounts.managed.create")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog
        open={deleteOpen}
        onOpenChange={(v) => {
          setDeleteOpen(v);
          if (!v) setDeleteTarget(null);
        }}
      >
        <DialogContent className="sm:max-w-[460px]">
          <DialogHeader>
            <DialogTitle>{t("accounts.deleteDialog.title")}</DialogTitle>
            <DialogDescription>
              {deleteTarget
                ? t("accounts.deleteDialog.descriptionWithName", { name: deleteTarget.user_id })
                : t("accounts.deleteDialog.description")}
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-3">
            <div className="flex items-center justify-between rounded-md border p-3">
              <div className="space-y-1">
                <div className="text-sm font-medium">{t("accounts.deleteDialog.deleteManagedChannels")}</div>
                <div className="text-xs text-muted-foreground">{t("accounts.deleteDialog.deleteManagedChannelsHint")}</div>
              </div>
              <Switch checked={deleteManagedChannels} onCheckedChange={setDeleteManagedChannels} />
            </div>
            <div className="flex items-center justify-between rounded-md border p-3">
              <div className="space-y-1">
                <div className="text-sm font-medium">{t("accounts.deleteDialog.syncDeleteRemote")}</div>
                <div className="text-xs text-muted-foreground">{t("accounts.deleteDialog.syncDeleteRemoteHint")}</div>
              </div>
              <Switch checked={deleteSyncRemote} onCheckedChange={setDeleteSyncRemote} />
            </div>
          </div>
          <DialogFooter>
            <Button
              variant="outline"
              onClick={() => {
                setDeleteOpen(false);
                setDeleteTarget(null);
              }}
              disabled={deleting}
            >
              {t("common.cancel")}
            </Button>
            <Button variant="destructive" onClick={() => void confirmDelete()} disabled={deleting || !deleteTarget}>
              {t("common.delete")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
