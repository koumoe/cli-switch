import React, { useEffect, useMemo, useReducer, useRef, useState } from "react";
import {
  Plus,
  GripVertical,
  Pencil,
  Trash2,
  Power,
  PowerOff,
  TestTube,
  ArrowDownUp,
} from "lucide-react";
import { toast } from "sonner";
import {
  Button,
  Card,
  CardContent,
  Badge,
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
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
} from "@/components/ui";
import { useI18n } from "@/lib/i18n";
import { useCurrency, formatDecimal } from "@/lib/currency";
import {
  listChannels,
  createChannel,
  updateChannel,
  deleteChannel,
  enableChannel,
  disableChannel,
  testChannel,
  reorderChannels,
  type Channel,
  type CreateChannelInput,
  type Protocol,
} from "../api";
import { formatDateTime, protocolLabel } from "../lib";

type ChannelDraft = CreateChannelInput;

function emptyDraft(): ChannelDraft {
  return {
    name: "",
    protocol: "openai",
    base_url: "https://api.openai.com",
    auth_type: "auto",
    auth_ref: "",
    priority: 0,
    recharge_multiplier: 1,
    real_multiplier: 1,
    enabled: true,
  };
}

function defaultBaseUrl(protocol: Protocol): string {
  switch (protocol) {
    case "openai":
      return "https://api.openai.com";
    case "anthropic":
      return "https://api.anthropic.com";
    case "gemini":
      return "https://generativelanguage.googleapis.com";
  }
}

type DragSnapshot = {
  protocol: Protocol;
  list: Channel[];
};

type DragState = {
  dragId: string | null;
  dragOverId: string | null;
  snapshot: DragSnapshot | null;
};

type DragAction =
  | { type: "start"; dragId: string; snapshot: DragSnapshot }
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

export function ChannelsPage() {
  const { t } = useI18n();
  const { currency } = useCurrency();
  const [activeProtocol, setActiveProtocol] = useState<Protocol>("openai");
  const [channelsByProtocol, setChannelsByProtocol] = useState<
    Record<Protocol, Channel[]>
  >({ openai: [], anthropic: [], gemini: [] });
  const [reordering, setReordering] = useState(false);
  const [dragState, dispatchDrag] = useReducer(dragReducer, initialDragState);
  const dragId = dragState.dragId;
  const dragOverId = dragState.dragOverId;
  const dragSnapshot = dragState.snapshot;
  const dragCommittedRef = useRef(false);
  const renderNowMs = Date.now();

  const [modalOpen, setModalOpen] = useState(false);
  const [modalMode, setModalMode] = useState<"create" | "edit">("create");
  const [editId, setEditId] = useState<string | null>(null);
  const [draft, setDraft] = useState<ChannelDraft>(emptyDraft());
  const [testing, setTesting] = useState<Record<string, boolean>>({});
  const [deleteOpen, setDeleteOpen] = useState(false);
  const [deleteTarget, setDeleteTarget] = useState<Channel | null>(null);
  const [deleting, setDeleting] = useState(false);
  const [autoSortOpen, setAutoSortOpen] = useState(false);
  const [autoSortApplying, setAutoSortApplying] = useState(false);

  async function refresh() {
    try {
      const cs = await listChannels();
      const by: Record<Protocol, Channel[]> = { openai: [], anthropic: [], gemini: [] };
      for (const c of cs) by[c.protocol].push(c);
      setChannelsByProtocol(by);
    } catch (e) {
      toast.error(t("channels.toast.loadFail"), { description: String(e) });
    }
  }

  useEffect(() => {
    refresh();
  }, []);

  function effectiveCostFactor(c: Channel): number {
    const recharge = Number(c.recharge_multiplier ?? 1);
    const real = Number(c.real_multiplier ?? 1);
    if (!Number.isFinite(recharge) || recharge <= 0) return Number.POSITIVE_INFINITY;
    if (!Number.isFinite(real) || real <= 0) return Number.POSITIVE_INFINITY;
    return real / recharge;
  }

  const autoSortCurrent = channelsByProtocol[activeProtocol] ?? [];
  const autoSortSuggested = useMemo(() => {
    const list = [...autoSortCurrent];
    list.sort((a, b) => {
      const aDisabled = !a.enabled;
      const bDisabled = !b.enabled;
      if (aDisabled !== bDisabled) return aDisabled ? 1 : -1;
      const fa = effectiveCostFactor(a);
      const fb = effectiveCostFactor(b);
      if (fa !== fb) return fa - fb;
      return a.name.localeCompare(b.name);
    });
    return list;
  }, [autoSortCurrent, activeProtocol]);

  const autoSortChanged = useMemo(() => {
    if (autoSortCurrent.length !== autoSortSuggested.length) return true;
    for (let i = 0; i < autoSortCurrent.length; i += 1) {
      if (autoSortCurrent[i]?.id !== autoSortSuggested[i]?.id) return true;
    }
    return false;
  }, [autoSortCurrent, autoSortSuggested]);

  async function applyAutoSort() {
    setAutoSortApplying(true);
    try {
      await reorderChannels(activeProtocol, autoSortSuggested.map((c) => c.id));
      toast.success(t("channels.toast.reorderOk"));
      setAutoSortOpen(false);
      await refresh();
    } catch (e) {
      toast.error(t("channels.toast.reorderFail"), { description: String(e) });
    } finally {
      setAutoSortApplying(false);
    }
  }

  function openCreate() {
    setModalMode("create");
    setEditId(null);
    setDraft({
      ...emptyDraft(),
      protocol: activeProtocol,
      base_url: defaultBaseUrl(activeProtocol),
    });
    setModalOpen(true);
  }

  function openEdit(c: Channel) {
    setModalMode("edit");
    setEditId(c.id);
    setDraft({
      name: c.name,
      protocol: c.protocol,
      base_url: c.base_url,
      auth_type: "auto",
      auth_ref: c.auth_ref,
      priority: c.priority ?? 0,
      recharge_multiplier: c.recharge_multiplier ?? 1,
      real_multiplier: c.real_multiplier ?? 1,
      enabled: c.enabled,
    });
    setModalOpen(true);
  }

  async function submit() {
    try {
      if (!draft.name.trim()) throw new Error(t("channels.toast.nameRequired"));
      if (!draft.base_url.trim()) throw new Error(t("channels.toast.baseUrlRequired"));
      if (!Number.isFinite(draft.recharge_multiplier) || draft.recharge_multiplier <= 0) {
        throw new Error(t("channels.toast.rechargeMultiplierInvalid"));
      }
      if (!Number.isFinite(draft.real_multiplier) || draft.real_multiplier <= 0) {
        throw new Error(t("channels.toast.realMultiplierInvalid"));
      }

      if (modalMode === "create") {
        await createChannel({ ...draft, name: draft.name.trim(), base_url: draft.base_url.trim() });
        toast.success(t("channels.toast.createOk"));
      } else {
        if (!editId) throw new Error(t("channels.toast.missingId"));
        await updateChannel(editId, {
          name: draft.name.trim(),
          base_url: draft.base_url.trim(),
          auth_type: "auto",
          auth_ref: draft.auth_ref,
          priority: draft.priority,
          recharge_multiplier: draft.recharge_multiplier,
          real_multiplier: draft.real_multiplier,
          enabled: draft.enabled,
        });
        toast.success(t("channels.toast.updateOk"));
      }
      setModalOpen(false);
      await refresh();
    } catch (e) {
      toast.error(t("channels.toast.actionFail"), { description: String(e) });
    }
  }

  async function toggleEnabled(c: Channel) {
    try {
      const nowMs = Date.now();
      const isAutoDisabled = c.enabled && (c.auto_disabled_until_ms ?? 0) > nowMs;
      if (c.enabled && !isAutoDisabled) {
        await disableChannel(c.id);
        toast.success(t("channels.toast.disabledOk", { name: c.name }));
      } else {
        await enableChannel(c.id);
        toast.success(t("channels.toast.enabledOk", { name: c.name }));
      }
      await refresh();
    } catch (e) {
      toast.error(t("channels.toast.actionFail"), { description: String(e) });
    }
  }

  async function onDelete(c: Channel) {
    setDeleteTarget(c);
    setDeleteOpen(true);
  }

  async function confirmDelete() {
    if (!deleteTarget) return;
    setDeleting(true);
    try {
      await deleteChannel(deleteTarget.id);
      toast.success(t("channels.toast.deletedOk", { name: deleteTarget.name }));
      setDeleteOpen(false);
      setDeleteTarget(null);
      await refresh();
    } catch (e) {
      toast.error(t("channels.toast.deleteFail"), { description: String(e) });
    } finally {
      setDeleting(false);
    }
  }

  async function onTest(c: Channel) {
    setTesting((m) => ({ ...m, [c.id]: true }));
    try {
      const r = await testChannel(c.id);
      if (r.reachable && r.ok) {
        toast.success(t("channels.toast.testReachableOkTitle", { name: c.name }), {
          description: t("channels.toast.testReachableOkDesc", {
            status: r.status ?? "-",
            latency: r.latency_ms,
          }),
        });
      } else if (r.reachable) {
        toast.warning(t("channels.toast.testReachableBadTitle", { name: c.name }), {
          description: t("channels.toast.testReachableOkDesc", {
            status: r.status ?? "-",
            latency: r.latency_ms,
          }),
        });
      } else {
        toast.error(t("channels.toast.testUnreachableTitle", { name: c.name }), {
          description: r.error ?? t("channels.toast.testTimeout"),
        });
      }
    } catch (e) {
      toast.error(t("channels.toast.testFail"), { description: String(e) });
    } finally {
      setTesting((m) => ({ ...m, [c.id]: false }));
    }
  }

  async function persistOrder(protocol: Protocol, next: Channel[]) {
    setReordering(true);
    try {
      await reorderChannels(protocol, next.map((c) => c.id));
      toast.success(t("channels.toast.reorderOk"));
      await refresh();
    } catch (e) {
      toast.error(t("channels.toast.reorderFail"), { description: String(e) });
      await refresh();
    } finally {
      setReordering(false);
    }
  }

  function moveInList(list: Channel[], fromId: string, toId: string): Channel[] {
    if (fromId === toId) return list;
    const fromIdx = list.findIndex((c) => c.id === fromId);
    const toIdx = list.findIndex((c) => c.id === toId);
    if (fromIdx < 0 || toIdx < 0) return list;
    if (fromIdx === toIdx) return list;
    const next = [...list];
    const [item] = next.splice(fromIdx, 1);
    next.splice(toIdx, 0, item);
    return next;
  }

  function moveToEndList(list: Channel[], fromId: string): Channel[] {
    const fromIdx = list.findIndex((c) => c.id === fromId);
    if (fromIdx < 0) return list;
    const next = [...list];
    const [item] = next.splice(fromIdx, 1);
    next.push(item);
    return next;
  }

  function setChannelDragPreview(e: React.DragEvent, c: Channel) {
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
      el.style.minWidth = "260px";
      el.style.maxWidth = "360px";
      el.style.pointerEvents = "none";

      const title = document.createElement("div");
      title.textContent = c.name;
      title.style.fontSize = "13px";
      title.style.fontWeight = "600";
      title.style.color = "rgba(0,0,0,0.92)";

      const meta = document.createElement("div");
      meta.textContent = `${t("channels.table.priority")}: ${c.priority} · ${c.base_url}`;
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
      // ignore: fallback to browser default
    }
  }

  function renderTable(protocol: Protocol) {
    const tabChannels = channelsByProtocol[protocol];
    const colClass = {
      drag: "w-10",
      name: "w-44",
      priority: "w-20",
      status: "w-20",
      updatedAt: "w-44",
      actions: "w-32",
    } as const;
    return (
      <Card>
        <CardContent className="p-0">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead className={colClass.drag}></TableHead>
                <TableHead className={colClass.name}>{t("channels.table.name")}</TableHead>
                <TableHead className={colClass.priority}>
                  {t("channels.table.priority")}
                </TableHead>
                <TableHead className={colClass.status}>
                  {t("channels.table.status")}
                </TableHead>
                <TableHead className={colClass.updatedAt}>
                  {t("channels.table.updatedAt")}
                </TableHead>
                <TableHead className={colClass.actions}>{t("common.actions")}</TableHead>
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
                  const current = channelsByProtocol[protocol];
                  const next = moveToEndList(current, fromId);
                  dragCommittedRef.current = true;
                  setChannelsByProtocol((m) => ({ ...m, [protocol]: next }));
                  void persistOrder(protocol, next);
                }
                dispatchDrag({ type: "clear" });
              }}
            >
              {tabChannels.length === 0 ? (
                <TableRow>
                  <TableCell
                    colSpan={6}
                    className="text-center text-muted-foreground py-8"
                  >
                    {t("channels.table.empty")}
                  </TableCell>
                </TableRow>
              ) : (
                tabChannels.map((c) => {
                  const isAutoDisabled =
                    c.enabled && (c.auto_disabled_until_ms ?? 0) > renderNowMs;
                  const effectiveEnabled = c.enabled && !isAutoDisabled;
                  const autoDisabledMinutes = Math.max(
                    1,
                    Math.ceil(((c.auto_disabled_until_ms ?? 0) - renderNowMs) / 60000)
                  );

                  return (
                    <TableRow
                      key={c.id}
                      onDragOver={(e) => {
                        e.preventDefault();
                        if (!dragId || reordering) return;
                        if (dragId === c.id) return;
                        if (dragOverId === c.id) return;
                        dispatchDrag({ type: "over", dragOverId: c.id });

                        setChannelsByProtocol((m) => {
                          const current = m[protocol];
                          const next = moveInList(current, dragId, c.id);
                          if (next === current) return m;
                          return { ...m, [protocol]: next };
                        });
                      }}
                      onDragLeave={() => {
                        if (dragOverId === c.id) dispatchDrag({ type: "over", dragOverId: null });
                      }}
                      onDrop={(e) => {
                        e.stopPropagation();
                        e.preventDefault();
                        const fromId = e.dataTransfer.getData("text/plain");
                        if (fromId) {
                          const current = channelsByProtocol[protocol];
                          const next = moveInList(current, fromId, c.id);
                          dragCommittedRef.current = true;
                          setChannelsByProtocol((m) => ({ ...m, [protocol]: next }));
                          void persistOrder(protocol, next);
                        }
                        dispatchDrag({ type: "clear" });
                      }}
                      className={[
                        dragId === c.id ? "opacity-60" : "",
                        dragOverId === c.id ? "bg-accent/30" : "",
                      ]
                        .filter(Boolean)
                        .join(" ")}
                    >
                    <TableCell>
                      <button
                        className="text-muted-foreground hover:text-foreground cursor-grab active:cursor-grabbing"
                        draggable={!reordering}
                        onDragStart={(e) => {
                          dragCommittedRef.current = false;
                          e.dataTransfer.setData("text/plain", c.id);
                          e.dataTransfer.effectAllowed = "move";
                          setChannelDragPreview(e, c);
                          dispatchDrag({
                            type: "start",
                            dragId: c.id,
                            snapshot: { protocol, list: channelsByProtocol[protocol] },
                          });
                        }}
                        onDragEnd={() => {
                          if (!dragCommittedRef.current && dragSnapshot?.protocol === protocol) {
                            setChannelsByProtocol((m) => ({ ...m, [protocol]: dragSnapshot.list }));
                          }
                          dispatchDrag({ type: "clear" });
                        }}
                        title={t("channels.actions.drag")}
                      >
                        <GripVertical className="h-4 w-4" />
                      </button>
                    </TableCell>
                    <TableCell>
                      <div className="font-medium">{c.name}</div>
                    </TableCell>
                    <TableCell className="font-mono text-sm">
                      {c.priority}
                    </TableCell>
                    <TableCell>
                      {isAutoDisabled ? (
                        <Badge variant="warning">
                          {t("channels.status.autoDisabled", { minutes: autoDisabledMinutes })}
                        </Badge>
                      ) : (
                        <Badge variant={c.enabled ? "success" : "secondary"}>
                          {c.enabled ? t("common.enabled") : t("common.disabled")}
                        </Badge>
                      )}
                    </TableCell>
                    <TableCell className="text-muted-foreground text-sm">
                      {formatDateTime(c.updated_at_ms)}
                    </TableCell>
                    <TableCell>
                      <div className="flex items-center justify-center gap-1">
                        <Button
                          variant="ghost"
                          size="icon"
                          onClick={() => onTest(c)}
                          disabled={testing[c.id]}
                          title={t("channels.actions.test")}
                        >
                          <TestTube className="h-4 w-4" />
                        </Button>
                        <Button
                          variant="ghost"
                          size="icon"
                          onClick={() => toggleEnabled(c)}
                          title={
                            effectiveEnabled
                              ? t("channels.actions.disable")
                              : t("channels.actions.enable")
                          }
                        >
                          {effectiveEnabled ? (
                            <PowerOff className="h-4 w-4" />
                          ) : (
                            <Power className="h-4 w-4" />
                          )}
                        </Button>
                        <Button
                          variant="ghost"
                          size="icon"
                          onClick={() => openEdit(c)}
                          title={t("channels.actions.edit")}
                        >
                          <Pencil className="h-4 w-4" />
                        </Button>
                        <Button
                          variant="ghost"
                          size="icon"
                          onClick={() => onDelete(c)}
                          title={t("channels.actions.delete")}
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
    );
  }

  return (
    <div className="space-y-4 pb-4">
      {/* 页面标题 */}
        <div className="flex items-center justify-between">
        <div>
          <h1 className="text-lg font-semibold">{t("channels.title")}</h1>
          <p className="text-muted-foreground text-xs mt-0.5">
            {t("channels.subtitle")}
          </p>
        </div>
        <div className="flex gap-2">
          <Button
            size="sm"
            variant="outline"
            onClick={() => setAutoSortOpen(true)}
            disabled={autoSortCurrent.length <= 1}
          >
            <ArrowDownUp className="h-4 w-4 mr-2" />
            {t("channels.autoSort.button")}
          </Button>
          <Button size="sm" onClick={openCreate}>
            <Plus className="h-4 w-4 mr-2" />
            {t("channels.new")}
          </Button>
        </div>
      </div>

      {/* 渠道表格 */}
      <Tabs
        value={activeProtocol}
        onValueChange={(v) => {
          setActiveProtocol(v as Protocol);
          dispatchDrag({ type: "clear" });
        }}
      >
        <TabsList>
          <TabsTrigger value="openai">{t("channels.tabs.codex")}</TabsTrigger>
          <TabsTrigger value="anthropic">{t("channels.tabs.claude")}</TabsTrigger>
          <TabsTrigger value="gemini">{t("channels.tabs.gemini")}</TabsTrigger>
        </TabsList>

        <TabsContent value="openai">{renderTable("openai")}</TabsContent>
        <TabsContent value="anthropic">{renderTable("anthropic")}</TabsContent>
        <TabsContent value="gemini">{renderTable("gemini")}</TabsContent>
      </Tabs>

      {/* 新建/编辑弹窗 */}
      <Dialog open={modalOpen} onOpenChange={setModalOpen}>
        <DialogContent className="sm:max-w-[500px]">
          <DialogHeader>
            <DialogTitle>
              {modalMode === "create" ? t("channels.modal.createTitle") : t("channels.modal.editTitle")}
            </DialogTitle>
            <DialogDescription>
              {t("channels.modal.description")}
            </DialogDescription>
          </DialogHeader>

          <div className="space-y-4 py-4">
            <div className="grid grid-cols-2 gap-4">
              <div className="space-y-2">
                <label className="text-sm font-medium">{t("channels.modal.name")}</label>
                <Input
                  value={draft.name}
                  onChange={(e) => setDraft((d) => ({ ...d, name: e.target.value }))}
                  placeholder="openai-main"
                />
              </div>
              <div className="space-y-2">
                <label className="text-sm font-medium">{t("channels.modal.terminal")}</label>
                <Select
                  value={draft.protocol}
                  onValueChange={(v) =>
                    setDraft((d) => {
                      const nextProtocol = v as Protocol;
                      const prevDefault = defaultBaseUrl(d.protocol);
                      const nextDefault = defaultBaseUrl(nextProtocol);
                      const shouldUpdateBase =
                        !d.base_url.trim() || d.base_url.trim() === prevDefault;
                      return {
                        ...d,
                        protocol: nextProtocol,
                        auth_type: "auto",
                        base_url: shouldUpdateBase ? nextDefault : d.base_url,
                      };
                    })
                  }
                  disabled={modalMode === "edit"}
                >
                  <SelectTrigger>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="anthropic">
                      {t("channels.tabs.claude")}
                    </SelectItem>
                    <SelectItem value="openai">
                      {t("channels.tabs.codex")}
                    </SelectItem>
                    <SelectItem value="gemini">
                      {t("channels.tabs.gemini")}
                    </SelectItem>
                  </SelectContent>
                </Select>
              </div>
            </div>

            <div className="space-y-2">
              <label className="text-sm font-medium">{t("channels.modal.priority")}</label>
              <Input
                type="number"
                value={String(draft.priority ?? 0)}
                onChange={(e) =>
                  setDraft((d) => ({
                    ...d,
                    priority: Number.isFinite(Number(e.target.value))
                      ? Number(e.target.value)
                      : 0,
                  }))
                }
                placeholder="0"
              />
            </div>

            <div className="grid grid-cols-2 gap-4">
              <div className="space-y-2">
                <label className="text-sm font-medium">
                  {t("channels.modal.rechargeMultiplier", { currency })}
                </label>
                <Input
                  type="number"
                  step="0.000001"
                  min={0}
                  value={String(draft.recharge_multiplier ?? 1)}
                  onChange={(e) => {
                    const raw = e.target.value;
                    if (!raw.trim()) {
                      setDraft((d) => ({ ...d, recharge_multiplier: 1 }));
                      return;
                    }
                    const n = Number(raw);
                    setDraft((d) => ({
                      ...d,
                      recharge_multiplier: Number.isFinite(n) ? n : d.recharge_multiplier,
                    }));
                  }}
                  placeholder="1"
                />
                <div className="text-xs text-muted-foreground">
                  {t("channels.modal.rechargeMultiplierHint", {
                    currency,
                    v: formatDecimal(draft.recharge_multiplier ?? 1),
                  })}
                </div>
              </div>
              <div className="space-y-2">
                <label className="text-sm font-medium">{t("channels.modal.realMultiplier")}</label>
                <Input
                  type="number"
                  step="0.000001"
                  min={0}
                  value={String(draft.real_multiplier ?? 1)}
                  onChange={(e) => {
                    const raw = e.target.value;
                    if (!raw.trim()) {
                      setDraft((d) => ({ ...d, real_multiplier: 1 }));
                      return;
                    }
                    const n = Number(raw);
                    setDraft((d) => ({
                      ...d,
                      real_multiplier: Number.isFinite(n) ? n : d.real_multiplier,
                    }));
                  }}
                  placeholder="1"
                />
                <div className="text-xs text-muted-foreground">
                  {t("channels.modal.realMultiplierHint", {
                    v: formatDecimal(draft.real_multiplier ?? 1),
                  })}
                </div>
              </div>
            </div>

            <div className="space-y-2">
              <label className="text-sm font-medium">{t("channels.modal.baseUrl")}</label>
              <Input
                value={draft.base_url}
                onChange={(e) => setDraft((d) => ({ ...d, base_url: e.target.value }))}
                placeholder="https://api.openai.com"
              />
            </div>

            <div className="space-y-2">
              <label className="text-sm font-medium">{t("channels.modal.apiKey")}</label>
              <Input
                type="password"
                value={draft.auth_ref}
                onChange={(e) => setDraft((d) => ({ ...d, auth_ref: e.target.value }))}
                placeholder="sk-..."
              />
            </div>

            <div className="flex items-center justify-between">
              <label className="text-sm font-medium">{t("channels.modal.enabled")}</label>
              <Switch
                checked={draft.enabled}
                onCheckedChange={(v) => setDraft((d) => ({ ...d, enabled: v }))}
              />
            </div>
          </div>

          <DialogFooter>
            <Button variant="outline" onClick={() => setModalOpen(false)}>
              {t("common.cancel")}
            </Button>
            <Button onClick={submit}>{t("common.save")}</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* 自动排序预览 */}
      <Dialog open={autoSortOpen} onOpenChange={setAutoSortOpen}>
        <DialogContent className="sm:max-w-[720px]">
          <DialogHeader>
            <DialogTitle>{t("channels.autoSort.title")}</DialogTitle>
            <DialogDescription>
              {t("channels.autoSort.description", { terminal: protocolLabel(t, activeProtocol) })}
            </DialogDescription>
          </DialogHeader>

          <div className="space-y-3">
            {!autoSortChanged ? (
              <div className="text-sm text-muted-foreground">
                {t("channels.autoSort.noChange")}
              </div>
            ) : (
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead className="w-14">{t("channels.autoSort.headers.from")}</TableHead>
                    <TableHead className="w-14">{t("channels.autoSort.headers.to")}</TableHead>
                    <TableHead>{t("channels.autoSort.headers.channel")}</TableHead>
                    <TableHead className="w-36">{t("channels.autoSort.headers.factor")}</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {autoSortSuggested.map((c, newIdx) => {
                    const oldIdx = autoSortCurrent.findIndex((x) => x.id === c.id);
                    const factor = effectiveCostFactor(c);
                    return (
                      <TableRow key={c.id}>
                        <TableCell className="font-mono text-xs">{oldIdx >= 0 ? oldIdx + 1 : "-"}</TableCell>
                        <TableCell className="font-mono text-xs">{newIdx + 1}</TableCell>
                        <TableCell className="min-w-0">
                          <div className="flex items-center gap-2 min-w-0">
                            <span className="truncate">{c.name}</span>
                            {!c.enabled && (
                              <Badge variant="outline" className="text-[10px] px-1 py-0">
                                {t("common.disabled")}
                              </Badge>
                            )}
                          </div>
                        </TableCell>
                        <TableCell className="font-mono text-xs text-muted-foreground">
                          {Number.isFinite(factor) ? formatDecimal(factor, 6) : "-"}
                        </TableCell>
                      </TableRow>
                    );
                  })}
                </TableBody>
              </Table>
            )}
          </div>

          <DialogFooter>
            <Button variant="outline" onClick={() => setAutoSortOpen(false)} disabled={autoSortApplying}>
              {t("common.cancel")}
            </Button>
            <Button onClick={applyAutoSort} disabled={!autoSortChanged || autoSortApplying}>
              {t("channels.autoSort.apply")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* 删除确认弹窗 */}
      <Dialog
        open={deleteOpen}
        onOpenChange={(v) => {
          setDeleteOpen(v);
          if (!v) setDeleteTarget(null);
        }}
      >
        <DialogContent className="sm:max-w-[420px]">
          <DialogHeader>
            <DialogTitle>{t("channels.deleteDialog.title")}</DialogTitle>
            <DialogDescription>
              {deleteTarget
                ? t("channels.deleteDialog.confirmWithName", { name: deleteTarget.name })
                : t("channels.deleteDialog.confirm")}
            </DialogDescription>
          </DialogHeader>
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
            <Button variant="destructive" onClick={confirmDelete} disabled={deleting || !deleteTarget}>
              {t("common.delete")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
