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
  const [channels, setChannels] = useState<Channel[]>([]);
  const [loading, setLoading] = useState(false);

  const [modalOpen, setModalOpen] = useState(false);
  const [modalMode, setModalMode] = useState<"create" | "edit">("create");
  const [editId, setEditId] = useState<string | null>(null);
  const [draft, setDraft] = useState<ChannelDraft>(emptyDraft());
  const [testing, setTesting] = useState<Record<string, boolean>>({});

  async function refresh() {
    setLoading(true);
    try {
      const cs = await listChannels();
      setChannels(cs);
    } catch (e) {
      toast.error("加载渠道失败", { description: String(e) });
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
      if (!draft.name.trim()) throw new Error("名称不能为空");
      if (!draft.base_url.trim()) throw new Error("Base URL 不能为空");

      if (modalMode === "create") {
        await createChannel({ ...draft, name: draft.name.trim(), base_url: draft.base_url.trim() });
        toast.success("渠道创建成功");
      } else {
        if (!editId) throw new Error("缺少 ID");
        await updateChannel(editId, {
          name: draft.name.trim(),
          base_url: draft.base_url.trim(),
          auth_type: "auto",
          auth_ref: draft.auth_ref,
          enabled: draft.enabled,
        });
        toast.success("渠道更新成功");
      }
      setModalOpen(false);
      await refresh();
    } catch (e) {
      toast.error("操作失败", { description: String(e) });
    }
  }

  async function toggleEnabled(c: Channel) {
    try {
      if (c.enabled) {
        await disableChannel(c.id);
        toast.success(`已禁用 ${c.name}`);
      } else {
        await enableChannel(c.id);
        toast.success(`已启用 ${c.name}`);
      }
      await refresh();
    } catch (e) {
      toast.error("操作失败", { description: String(e) });
    }
  }

  async function onDelete(c: Channel) {
    if (!confirm(`确定删除渠道 "${c.name}"？此操作不可恢复。`)) return;
    try {
      await deleteChannel(c.id);
      toast.success(`已删除 ${c.name}`);
      await refresh();
    } catch (e) {
      toast.error("删除失败", { description: String(e) });
    }
  }

  async function onTest(c: Channel) {
    setTesting((m) => ({ ...m, [c.id]: true }));
    try {
      const r = await testChannel(c.id);
      if (r.reachable && r.ok) {
        toast.success(`${c.name} 连通正常`, {
          description: `状态: ${r.status}, 延迟: ${r.latency_ms}ms`,
        });
      } else if (r.reachable) {
        toast.warning(`${c.name} 可达但返回异常`, {
          description: `状态: ${r.status}, 延迟: ${r.latency_ms}ms`,
        });
      } else {
        toast.error(`${c.name} 无法连接`, {
          description: r.error ?? "连接超时",
        });
      }
    } catch (e) {
      toast.error("测试失败", { description: String(e) });
    } finally {
      setTesting((m) => ({ ...m, [c.id]: false }));
    }
  }

  return (
    <div className="space-y-6">
      {/* 页面标题 */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-semibold tracking-tight">渠道</h1>
          <p className="text-muted-foreground text-sm mt-1">
            管理上游 API 渠道的配置和连接
          </p>
        </div>
        <Button onClick={openCreate}>
          <Plus className="h-4 w-4 mr-2" />
          新建渠道
        </Button>
      </div>

      {/* 渠道表格 */}
      <Card>
        <CardContent className="p-0">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>名称</TableHead>
                <TableHead>终端</TableHead>
                <TableHead>Base URL</TableHead>
                <TableHead>状态</TableHead>
                <TableHead>更新时间</TableHead>
                <TableHead className="w-[100px]">操作</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {channels.length === 0 ? (
                <TableRow>
                  <TableCell colSpan={6} className="text-center text-muted-foreground py-8">
                    暂无渠道，点击「新建渠道」添加
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
                    <TableCell>
                      <Badge variant={c.enabled ? "success" : "secondary"}>
                        {c.enabled ? "启用" : "禁用"}
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
                          title="测试连接"
                        >
                          <TestTube className="h-4 w-4" />
                        </Button>
                        <Button
                          variant="ghost"
                          size="icon"
                          onClick={() => toggleEnabled(c)}
                          title={c.enabled ? "禁用" : "启用"}
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
                          title="编辑"
                        >
                          <Pencil className="h-4 w-4" />
                        </Button>
                        <Button
                          variant="ghost"
                          size="icon"
                          onClick={() => onDelete(c)}
                          title="删除"
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
              {modalMode === "create" ? "新建渠道" : "编辑渠道"}
            </DialogTitle>
            <DialogDescription>
              配置 API 渠道的连接信息
            </DialogDescription>
          </DialogHeader>

          <div className="space-y-4 py-4">
            <div className="grid grid-cols-2 gap-4">
              <div className="space-y-2">
                <label className="text-sm font-medium">名称</label>
                <Input
                  value={draft.name}
                  onChange={(e) => setDraft((d) => ({ ...d, name: e.target.value }))}
                  placeholder="openai-main"
                />
              </div>
              <div className="space-y-2">
                <label className="text-sm font-medium">终端</label>
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
              <label className="text-sm font-medium">Base URL</label>
              <Input
                value={draft.base_url}
                onChange={(e) => setDraft((d) => ({ ...d, base_url: e.target.value }))}
                placeholder="https://api.openai.com"
              />
            </div>

            <div className="space-y-2">
              <label className="text-sm font-medium">API Key / Token</label>
              <Input
                type="password"
                value={draft.auth_ref}
                onChange={(e) => setDraft((d) => ({ ...d, auth_ref: e.target.value }))}
                placeholder="sk-..."
              />
            </div>

            <div className="flex items-center justify-between">
              <label className="text-sm font-medium">启用</label>
              <Switch
                checked={draft.enabled}
                onCheckedChange={(v) => setDraft((d) => ({ ...d, enabled: v }))}
              />
            </div>
          </div>

          <DialogFooter>
            <Button variant="outline" onClick={() => setModalOpen(false)}>
              取消
            </Button>
            <Button onClick={submit}>保存</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
