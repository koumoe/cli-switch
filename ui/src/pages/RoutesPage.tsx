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
import { formatDateTime } from "../lib";

type RouteDraft = CreateRouteInput;

function emptyDraft(): RouteDraft {
  return { name: "", protocol: "openai", match_model: null, enabled: true };
}

export function RoutesPage() {
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
      toast.error("加载失败", { description: String(e) });
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
      if (!draft.name.trim()) throw new Error("名称不能为空");
      if (routeMode === "create") {
        await createRoute({ ...draft, name: draft.name.trim() });
        toast.success("路由创建成功");
      } else {
        if (!editRouteId) throw new Error("缺少 ID");
        await updateRoute(editRouteId, {
          name: draft.name.trim(),
          match_model: draft.match_model,
          enabled: draft.enabled,
        });
        toast.success("路由更新成功");
      }
      setRouteModalOpen(false);
      await refresh();
    } catch (e) {
      toast.error("操作失败", { description: String(e) });
    }
  }

  async function onDelete(r: Route) {
    if (!confirm(`确定删除路由 "${r.name}"？`)) return;
    try {
      await deleteRoute(r.id);
      toast.success(`已删除 ${r.name}`);
      await refresh();
    } catch (e) {
      toast.error("删除失败", { description: String(e) });
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
      toast.error("加载失败", { description: String(e) });
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
      toast.success("渠道优先级已保存");
      setManageOpen(false);
      setManageRoute(null);
    } catch (e) {
      toast.error("保存失败", { description: String(e) });
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
          <h1 className="text-2xl font-semibold tracking-tight">路由</h1>
          <p className="text-muted-foreground text-sm mt-1">
            配置请求路由规则和渠道优先级
          </p>
        </div>
        <Button onClick={openCreate}>
          <Plus className="h-4 w-4 mr-2" />
          新建路由
        </Button>
      </div>

      {/* 路由表格 */}
      <Card>
        <CardContent className="p-0">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>名称</TableHead>
                <TableHead>协议</TableHead>
                <TableHead>模型匹配</TableHead>
                <TableHead>状态</TableHead>
                <TableHead>更新时间</TableHead>
                <TableHead className="w-[120px]">操作</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {routes.length === 0 ? (
                <TableRow>
                  <TableCell
                    colSpan={6}
                    className="text-center text-muted-foreground py-8"
                  >
                    暂无路由，点击「新建路由」添加
                  </TableCell>
                </TableRow>
              ) : (
                routes.map((r) => (
                  <TableRow key={r.id}>
                    <TableCell>
                      <div className="font-medium">{r.name}</div>
                    </TableCell>
                    <TableCell>
                      <Badge variant="outline">{r.protocol}</Badge>
                    </TableCell>
                    <TableCell className="text-muted-foreground">
                      {r.match_model ?? "-"}
                    </TableCell>
                    <TableCell>
                      <Badge variant={r.enabled ? "success" : "secondary"}>
                        {r.enabled ? "启用" : "禁用"}
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
                          渠道
                        </Button>
                        <Button
                          variant="ghost"
                          size="icon"
                          onClick={() => openEdit(r)}
                          title="编辑"
                        >
                          <Pencil className="h-4 w-4" />
                        </Button>
                        <Button
                          variant="ghost"
                          size="icon"
                          onClick={() => onDelete(r)}
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

      {/* 新建/编辑路由弹窗 */}
      <Dialog open={routeModalOpen} onOpenChange={setRouteModalOpen}>
        <DialogContent className="sm:max-w-[450px]">
          <DialogHeader>
            <DialogTitle>
              {routeMode === "create" ? "新建路由" : "编辑路由"}
            </DialogTitle>
            <DialogDescription>配置路由规则</DialogDescription>
          </DialogHeader>

          <div className="space-y-4 py-4">
            <div className="grid grid-cols-2 gap-4">
              <div className="space-y-2">
                <label className="text-sm font-medium">名称</label>
                <Input
                  value={draft.name}
                  onChange={(e) =>
                    setDraft((d) => ({ ...d, name: e.target.value }))
                  }
                  placeholder="default-openai"
                />
              </div>
              <div className="space-y-2">
                <label className="text-sm font-medium">协议</label>
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
                    <SelectItem value="openai">OpenAI</SelectItem>
                    <SelectItem value="anthropic">Anthropic</SelectItem>
                    <SelectItem value="gemini">Gemini</SelectItem>
                  </SelectContent>
                </Select>
              </div>
            </div>

            <div className="space-y-2">
              <label className="text-sm font-medium">模型匹配（可选）</label>
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
                留空则匹配该协议的所有模型
              </p>
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
            <Button variant="outline" onClick={() => setRouteModalOpen(false)}>
              取消
            </Button>
            <Button onClick={submitRoute}>保存</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* 渠道优先级管理弹窗 */}
      <Dialog open={manageOpen} onOpenChange={setManageOpen}>
        <DialogContent className="sm:max-w-[600px]">
          <DialogHeader>
            <DialogTitle>渠道优先级：{manageRoute?.name}</DialogTitle>
            <DialogDescription>
              拖拽或使用按钮调整渠道优先级，从上到下依次尝试
            </DialogDescription>
          </DialogHeader>

          {manageLoading ? (
            <div className="py-8 text-center text-muted-foreground">
              加载中...
            </div>
          ) : (
            <div className="grid grid-cols-2 gap-4 py-4">
              {/* 已绑定 */}
              <div>
                <h4 className="text-sm font-medium mb-3">已绑定渠道</h4>
                {assigned.length === 0 ? (
                  <p className="text-sm text-muted-foreground">
                    暂无绑定渠道
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
                <h4 className="text-sm font-medium mb-3">可添加渠道</h4>
                {available.length === 0 ? (
                  <p className="text-sm text-muted-foreground">
                    无可添加渠道
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
                          添加
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
              取消
            </Button>
            <Button onClick={saveManage} disabled={manageLoading}>
              保存
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
