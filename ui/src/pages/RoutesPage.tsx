import React, { useEffect, useMemo, useState } from "react";
import {
  Plus,
  Pencil,
  Trash2,
  ChevronUp,
  ChevronDown,
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
} from "@/components/ui";
import { useI18n } from "@/lib/i18n";
import {
  listRoutes,
  listChannels,
  createRoute,
  updateRoute,
  deleteRoute,
  listRouteChannels,
  reorderRouteChannels,
  type Route,
  type Channel,
  type CreateRouteInput,
  type Protocol,
} from "../api";
import { formatDateTime, protocolLabel, protocolLabelKey } from "../lib";

type RouteDraft = CreateRouteInput;

function emptyDraft(): RouteDraft {
  return { name: "", protocol: "openai", match_model: null, enabled: true };
}

export function RoutesPage() {
  const { t } = useI18n();
  const [routes, setRoutes] = useState<Route[]>([]);
  const [channels, setChannels] = useState<Channel[]>([]);
  const [loading, setLoading] = useState(false);

  const [routeModalOpen, setRouteModalOpen] = useState(false);
  const [routeMode, setRouteMode] = useState<"create" | "edit">("create");
  const [editRouteId, setEditRouteId] = useState<string | null>(null);
  const [draft, setDraft] = useState<RouteDraft>(emptyDraft());

  const [manageOpen, setManageOpen] = useState(false);
  const [manageRoute, setManageRoute] = useState<Route | null>(null);
  const [assigned, setAssigned] = useState<string[]>([]);
  const [manageLoading, setManageLoading] = useState(false);
  const [deleteOpen, setDeleteOpen] = useState(false);
  const [deleteTarget, setDeleteTarget] = useState<Route | null>(null);
  const [deleting, setDeleting] = useState(false);

  const channelNames = useMemo(() => {
    const m = new Map<string, string>();
    for (const c of channels) m.set(c.id, c.name);
    return m;
  }, [channels]);

  async function refresh() {
    setLoading(true);
    try {
      const [rs, cs] = await Promise.all([listRoutes(), listChannels()]);
      setRoutes(rs);
      setChannels(cs);
    } catch (e) {
      toast.error(t("routes.toast.loadFail"), { description: String(e) });
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    refresh();
  }, []);

  function openCreate() {
    setRouteMode("create");
    setEditRouteId(null);
    setDraft(emptyDraft());
    setRouteModalOpen(true);
  }

  function openEdit(r: Route) {
    setRouteMode("edit");
    setEditRouteId(r.id);
    setDraft({
      name: r.name,
      protocol: r.protocol,
      match_model: r.match_model,
      enabled: r.enabled,
    });
    setRouteModalOpen(true);
  }

  async function submitRoute() {
    try {
      if (!draft.name.trim()) throw new Error(t("routes.toast.nameRequired"));
      if (routeMode === "create") {
        await createRoute({ ...draft, name: draft.name.trim() });
        toast.success(t("routes.toast.createOk"));
      } else {
        if (!editRouteId) throw new Error(t("routes.toast.missingId"));
        await updateRoute(editRouteId, {
          name: draft.name.trim(),
          match_model: draft.match_model,
          enabled: draft.enabled,
        });
        toast.success(t("routes.toast.updateOk"));
      }
      setRouteModalOpen(false);
      await refresh();
    } catch (e) {
      toast.error(t("routes.toast.actionFail"), { description: String(e) });
    }
  }

  async function onDelete(r: Route) {
    setDeleteTarget(r);
    setDeleteOpen(true);
  }

  async function confirmDelete() {
    if (!deleteTarget) return;
    setDeleting(true);
    try {
      await deleteRoute(deleteTarget.id);
      toast.success(t("routes.toast.deletedOk", { name: deleteTarget.name }));
      setDeleteOpen(false);
      setDeleteTarget(null);
      await refresh();
    } catch (e) {
      toast.error(t("routes.toast.deleteFail"), { description: String(e) });
    } finally {
      setDeleting(false);
    }
  }

  async function openManage(r: Route) {
    setManageOpen(true);
    setManageRoute(r);
    setManageLoading(true);
    try {
      const items = await listRouteChannels(r.id);
      const ordered = [...items]
        .sort((a, b) => a.priority - b.priority)
        .map((i) => i.channel_id);
      setAssigned(ordered);
    } catch (e) {
      toast.error(t("routes.toast.loadFail"), { description: String(e) });
    } finally {
      setManageLoading(false);
    }
  }

  function move(id: string, dir: -1 | 1) {
    setAssigned((list) => {
      const idx = list.indexOf(id);
      if (idx < 0) return list;
      const j = idx + dir;
      if (j < 0 || j >= list.length) return list;
      const next = [...list];
      [next[idx], next[j]] = [next[j], next[idx]];
      return next;
    });
  }

  function remove(id: string) {
    setAssigned((list) => list.filter((x) => x !== id));
  }

  function add(id: string) {
    setAssigned((list) => (list.includes(id) ? list : [...list, id]));
  }

  async function saveManage() {
    if (!manageRoute) return;
    setManageLoading(true);
    try {
      await reorderRouteChannels(manageRoute.id, assigned);
      toast.success(t("routes.toast.prioritySaved"));
      setManageOpen(false);
      setManageRoute(null);
    } catch (e) {
      toast.error(t("routes.toast.saveFail"), { description: String(e) });
    } finally {
      setManageLoading(false);
    }
  }

  const available = useMemo(() => {
    if (!manageRoute) return [];
    return channels
      .filter((c) => c.protocol === manageRoute.protocol)
      .filter((c) => !assigned.includes(c.id))
      .sort((a, b) => a.name.localeCompare(b.name));
  }, [channels, assigned, manageRoute]);

  return (
    <div className="space-y-6">
      {/* 页面标题 */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-semibold tracking-tight">{t("routes.title")}</h1>
          <p className="text-muted-foreground text-sm mt-1">
            {t("routes.subtitle")}
          </p>
        </div>
        <Button onClick={openCreate}>
          <Plus className="h-4 w-4 mr-2" />
          {t("routes.new")}
        </Button>
      </div>

      {/* 路由表格 */}
      <Card>
        <CardContent className="p-0">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>{t("routes.table.name")}</TableHead>
                <TableHead>{t("routes.table.terminal")}</TableHead>
                <TableHead>{t("routes.table.modelMatch")}</TableHead>
                <TableHead>{t("routes.table.status")}</TableHead>
                <TableHead>{t("routes.table.updatedAt")}</TableHead>
                <TableHead className="w-[120px]">{t("common.actions")}</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {routes.length === 0 ? (
                <TableRow>
                  <TableCell
                    colSpan={6}
                    className="text-center text-muted-foreground py-8"
                  >
                    {t("routes.table.empty")}
                  </TableCell>
                </TableRow>
              ) : (
                routes.map((r) => (
                  <TableRow key={r.id}>
                    <TableCell>
                      <div className="font-medium">{r.name}</div>
                    </TableCell>
                    <TableCell>
                      <Badge variant="outline">{protocolLabel(t, r.protocol)}</Badge>
                    </TableCell>
                    <TableCell className="text-muted-foreground">
                      {r.match_model ?? "-"}
                    </TableCell>
                    <TableCell>
                      <Badge variant={r.enabled ? "success" : "secondary"}>
                        {r.enabled ? t("common.enabled") : t("common.disabled")}
                      </Badge>
                    </TableCell>
                    <TableCell className="text-muted-foreground text-sm">
                      {formatDateTime(r.updated_at_ms)}
                    </TableCell>
                    <TableCell>
                      <div className="flex items-center gap-1">
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={() => openManage(r)}
                        >
                          {t("routes.actions.channels")}
                        </Button>
                        <Button
                          variant="ghost"
                          size="icon"
                          onClick={() => openEdit(r)}
                          title={t("routes.actions.edit")}
                        >
                          <Pencil className="h-4 w-4" />
                        </Button>
                        <Button
                          variant="ghost"
                          size="icon"
                          onClick={() => onDelete(r)}
                          title={t("routes.actions.delete")}
                        >
                          <Trash2 className="h-4 w-4 text-destructive" />
                        </Button>
                      </div>
                    </TableCell>
                  </TableRow>
                ))
              )}
            </TableBody>
          </Table>
        </CardContent>
      </Card>

      {/* 新建/编辑路由弹窗 */}
      <Dialog open={routeModalOpen} onOpenChange={setRouteModalOpen}>
        <DialogContent className="sm:max-w-[450px]">
          <DialogHeader>
            <DialogTitle>
              {routeMode === "create" ? t("routes.modal.createTitle") : t("routes.modal.editTitle")}
            </DialogTitle>
            <DialogDescription>{t("routes.modal.description")}</DialogDescription>
          </DialogHeader>

          <div className="space-y-4 py-4">
            <div className="grid grid-cols-2 gap-4">
              <div className="space-y-2">
                <label className="text-sm font-medium">{t("routes.modal.name")}</label>
                <Input
                  value={draft.name}
                  onChange={(e) =>
                    setDraft((d) => ({ ...d, name: e.target.value }))
                  }
                  placeholder="default-openai"
                />
              </div>
              <div className="space-y-2">
                <label className="text-sm font-medium">{t("routes.modal.terminal")}</label>
                <Select
                  value={draft.protocol}
                  onValueChange={(v) =>
                    setDraft((d) => ({ ...d, protocol: v as Protocol }))
                  }
                  disabled={routeMode === "edit"}
                >
                  <SelectTrigger>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="anthropic">
                      {protocolLabel(t, "anthropic")}
                    </SelectItem>
                    <SelectItem value="openai">
                      {protocolLabel(t, "openai")}
                    </SelectItem>
                    <SelectItem value="gemini">
                      {protocolLabel(t, "gemini")}
                    </SelectItem>
                  </SelectContent>
                </Select>
              </div>
            </div>

            <div className="space-y-2">
              <label className="text-sm font-medium">{t("routes.modal.modelMatchOptional")}</label>
              <Input
                value={draft.match_model ?? ""}
                onChange={(e) =>
                  setDraft((d) => ({
                    ...d,
                    match_model: e.target.value.trim() || null,
                  }))
                }
                placeholder="gpt-4o / claude-3-5-sonnet"
              />
              <p className="text-xs text-muted-foreground">
                {t("routes.modal.modelMatchHint")}
              </p>
            </div>

            <div className="flex items-center justify-between">
              <label className="text-sm font-medium">{t("routes.modal.enabled")}</label>
              <Switch
                checked={draft.enabled}
                onCheckedChange={(v) => setDraft((d) => ({ ...d, enabled: v }))}
              />
            </div>
          </div>

          <DialogFooter>
            <Button variant="outline" onClick={() => setRouteModalOpen(false)}>
              {t("common.cancel")}
            </Button>
            <Button onClick={submitRoute}>{t("common.save")}</Button>
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
            <DialogTitle>{t("routes.deleteDialog.title")}</DialogTitle>
            <DialogDescription>
              {deleteTarget
                ? t("routes.deleteDialog.confirmWithName", { name: deleteTarget.name })
                : t("routes.deleteDialog.confirm")}
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
            <Button
              variant="destructive"
              onClick={confirmDelete}
              disabled={deleting || !deleteTarget}
            >
              {t("common.delete")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* 渠道优先级管理弹窗 */}
      <Dialog open={manageOpen} onOpenChange={setManageOpen}>
        <DialogContent className="sm:max-w-[600px]">
          <DialogHeader>
            <DialogTitle>
              {t("routes.manage.title", { name: manageRoute?.name ?? "-" })}
            </DialogTitle>
            <DialogDescription>
              {t("routes.manage.description")}
            </DialogDescription>
          </DialogHeader>

          {manageLoading ? (
            <div className="py-8 text-center text-muted-foreground">
              {t("common.loading")}
            </div>
          ) : (
            <div className="grid grid-cols-2 gap-4 py-4">
              {/* 已绑定 */}
              <div>
                <h4 className="text-sm font-medium mb-3">{t("routes.manage.assignedTitle")}</h4>
                {assigned.length === 0 ? (
                  <p className="text-sm text-muted-foreground">
                    {t("routes.manage.assignedEmpty")}
                  </p>
                ) : (
                  <div className="space-y-2">
                    {assigned.map((id, idx) => (
                      <div
                        key={id}
                        className="flex items-center justify-between p-2 rounded border bg-card"
                      >
                        <div className="flex items-center gap-2">
                          <span className="text-xs text-muted-foreground w-4">
                            {idx + 1}
                          </span>
                          <span className="text-sm font-medium">
                            {channelNames.get(id) ?? id}
                          </span>
                        </div>
                        <div className="flex items-center gap-1">
                          <Button
                            variant="ghost"
                            size="icon"
                            onClick={() => move(id, -1)}
                            disabled={idx === 0}
                          >
                            <ChevronUp className="h-4 w-4" />
                          </Button>
                          <Button
                            variant="ghost"
                            size="icon"
                            onClick={() => move(id, 1)}
                            disabled={idx === assigned.length - 1}
                          >
                            <ChevronDown className="h-4 w-4" />
                          </Button>
                          <Button
                            variant="ghost"
                            size="icon"
                            onClick={() => remove(id)}
                          >
                            <Trash2 className="h-4 w-4 text-destructive" />
                          </Button>
                        </div>
                      </div>
                    ))}
                  </div>
                )}
              </div>

              {/* 可添加 */}
              <div>
                <h4 className="text-sm font-medium mb-3">{t("routes.manage.availableTitle")}</h4>
                {available.length === 0 ? (
                  <p className="text-sm text-muted-foreground">
                    {t("routes.manage.availableEmpty")}
                  </p>
                ) : (
                  <div className="space-y-2">
                    {available.map((c) => (
                      <div
                        key={c.id}
                        className="flex items-center justify-between p-2 rounded border"
                      >
                        <span className="text-sm">{c.name}</span>
                        <Button
                          variant="outline"
                          size="sm"
                          onClick={() => add(c.id)}
                        >
                          {t("common.add")}
                        </Button>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            </div>
          )}

          <DialogFooter>
            <Button variant="outline" onClick={() => setManageOpen(false)}>
              {t("common.cancel")}
            </Button>
            <Button onClick={saveManage} disabled={manageLoading}>
              {t("common.save")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
