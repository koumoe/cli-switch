import React, { useEffect, useState } from "react";
import {
  Plus,
  Pencil,
  Trash2,
  Power,
  PowerOff,
  TestTube,
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
  listChannels,
  createChannel,
  updateChannel,
  deleteChannel,
  enableChannel,
  disableChannel,
  testChannel,
  type Channel,
  type CreateChannelInput,
  type Protocol,
} from "../api";
import { formatDateTime, clampStr, terminalLabel } from "../lib";

type ChannelDraft = CreateChannelInput;

function emptyDraft(): ChannelDraft {
  return {
    name: "",
    protocol: "openai",
    base_url: "https://api.openai.com",
    auth_type: "auto",
    auth_ref: "",
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

export function ChannelsPage() {
  const { t } = useI18n();
  const [channels, setChannels] = useState<Channel[]>([]);
  const [loading, setLoading] = useState(false);

  const [modalOpen, setModalOpen] = useState(false);
  const [modalMode, setModalMode] = useState<"create" | "edit">("create");
  const [editId, setEditId] = useState<string | null>(null);
  const [draft, setDraft] = useState<ChannelDraft>(emptyDraft());
  const [testing, setTesting] = useState<Record<string, boolean>>({});
  const [deleteOpen, setDeleteOpen] = useState(false);
  const [deleteTarget, setDeleteTarget] = useState<Channel | null>(null);
  const [deleting, setDeleting] = useState(false);

  async function refresh() {
    setLoading(true);
    try {
      const cs = await listChannels();
      setChannels(cs);
    } catch (e) {
      toast.error(t("channels.toast.loadFail"), { description: String(e) });
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    refresh();
  }, []);

  function openCreate() {
    setModalMode("create");
    setEditId(null);
    setDraft(emptyDraft());
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
      enabled: c.enabled,
    });
    setModalOpen(true);
  }

  async function submit() {
    try {
      if (!draft.name.trim()) throw new Error(t("channels.toast.nameRequired"));
      if (!draft.base_url.trim()) throw new Error(t("channels.toast.baseUrlRequired"));

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
      if (c.enabled) {
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

  return (
    <div className="space-y-4">
      {/* 页面标题 */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-lg font-semibold">{t("channels.title")}</h1>
          <p className="text-muted-foreground text-xs mt-0.5">
            {t("channels.subtitle")}
          </p>
        </div>
        <Button size="sm" onClick={openCreate}>
          <Plus className="h-4 w-4 mr-2" />
          {t("channels.new")}
        </Button>
      </div>

      {/* 渠道表格 */}
      <Card>
        <CardContent className="p-0">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>{t("channels.table.name")}</TableHead>
                <TableHead>{t("channels.table.terminal")}</TableHead>
                <TableHead>{t("channels.table.baseUrl")}</TableHead>
                <TableHead className="w-[70px] text-center">{t("channels.table.status")}</TableHead>
                <TableHead>{t("channels.table.updatedAt")}</TableHead>
                <TableHead className="w-[100px]">{t("common.actions")}</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {channels.length === 0 ? (
                <TableRow>
                  <TableCell colSpan={6} className="text-center text-muted-foreground py-8">
                    {t("channels.table.empty")}
                  </TableCell>
                </TableRow>
              ) : (
                channels.map((c) => (
                  <TableRow key={c.id}>
                    <TableCell>
                      <div className="font-medium">{c.name}</div>
                    </TableCell>
                    <TableCell>
                      <Badge variant="outline">{terminalLabel(c.protocol)}</Badge>
                    </TableCell>
                    <TableCell>
                      <code className="text-xs text-muted-foreground">
                        {clampStr(c.base_url, 40)}
                      </code>
                    </TableCell>
                    <TableCell className="text-center">
                      <Badge variant={c.enabled ? "success" : "secondary"}>
                        {c.enabled ? t("common.enabled") : t("common.disabled")}
                      </Badge>
                    </TableCell>
                    <TableCell className="text-muted-foreground text-sm">
                      {formatDateTime(c.updated_at_ms)}
                    </TableCell>
                    <TableCell>
                      <div className="flex items-center gap-1">
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
                          title={c.enabled ? t("channels.actions.disable") : t("channels.actions.enable")}
                        >
                          {c.enabled ? (
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
                ))
              )}
            </TableBody>
          </Table>
        </CardContent>
      </Card>

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
                      Claude Code
                    </SelectItem>
                    <SelectItem value="openai">
                      Codex
                    </SelectItem>
                    <SelectItem value="gemini">
                      Gemini
                    </SelectItem>
                  </SelectContent>
                </Select>
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
