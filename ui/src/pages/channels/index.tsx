import React, { useEffect, useMemo, useState } from "react";
import { zodResolver } from "@hookform/resolvers/zod";
import { useForm } from "react-hook-form";
import type { ColumnDef } from "@tanstack/react-table";
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
  DialogBody,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  Form,
  FormControl,
  FormDescription,
  FormField,
  FormItem,
  FormLabel,
  FormMessage,
  Input,
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
  Switch,
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
} from "@/components/ui";
import { DataTable } from "@/components/composed/data-table";
import {
  TableActionGroup,
  TableIconButton,
} from "@/components/composed/table-primitives";
import {
  SortableDataTable,
  SortableDataTableHandle,
} from "@/components/composed/sortable-data-table";
import { PageHeader } from "@/components/PageHeader";
import { PageBody } from "@/components/layout/page-body";
import { useI18n } from "@/hooks/use-i18n";
import { useCurrency } from "@/hooks/use-currency";
import { useWindowEvent } from "@/hooks/use-window-event";
import { humanizeApiError, humanizeIssue } from "@/lib/error";
import { createChannelFormSchema } from "@/lib/schemas/channel";
import {
  listChannels,
  createChannel,
  updateChannel,
  deleteChannel,
  enableChannel,
  disableChannel,
  testChannel,
  reorderChannels,
  getSettings,
  listRemoteAccounts,
} from "@/api";
import type {
  AppSettings,
  Channel,
  RemoteAccount,
  CreateChannelInput,
  Protocol,
} from "@/types/api";
import { protocolLabel } from "../../lib";

type ChannelDraft = CreateChannelInput;

type ChannelFormValues = {
  name: string;
  protocol: Protocol;
  base_url: string;
  auth_ref: string;
  checkin_url: string;
  priority: string;
  retry_times: string;
  ignore_channel_protection: boolean;
  recharge_currency: "USD" | "CNY";
  real_multiplier: string;
  enabled: boolean;
};

function emptyFormValues(
  protocol: Protocol = "openai",
  rechargeCurrency: "USD" | "CNY" = "CNY",
): ChannelFormValues {
  return {
    name: "",
    protocol,
    base_url: defaultBaseUrl(protocol),
    auth_ref: "",
    checkin_url: "",
    priority: "0",
    retry_times: "1",
    ignore_channel_protection: false,
    recharge_currency: rechargeCurrency,
    real_multiplier: formatFixed2(1),
    enabled: true,
  };
}

function channelToFormValues(channel: Channel): ChannelFormValues {
  return {
    name: channel.name,
    protocol: channel.protocol,
    base_url: channel.base_url,
    auth_ref: channel.auth_ref,
    checkin_url: channel.checkin_url ?? "",
    priority: String(channel.priority ?? 0),
    retry_times: String(channel.retry_times ?? 1),
    ignore_channel_protection: channel.ignore_channel_protection ?? false,
    recharge_currency: channel.recharge_currency ?? "CNY",
    real_multiplier: formatFixed2(Number(channel.real_multiplier ?? 1)),
    enabled: channel.enabled,
  };
}

function formatFixed2(n: number): string {
  if (!Number.isFinite(n)) return "1.00";
  return n.toFixed(2);
}

function getRealMultiplierDisplay(
  raw: unknown,
):
  | { kind: "invalid" }
  | { kind: "free" }
  | { kind: "value"; value: number; text: string } {
  const real = Number(raw ?? 1);
  if (!Number.isFinite(real) || real < 0) return { kind: "invalid" };
  if (real === 0) return { kind: "free" };
  return { kind: "value", value: real, text: `×${formatFixed2(real)}` };
}

function clamp(n: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, n));
}

function multiplierStyle(real: number): React.CSSProperties {
  const r = clamp(real, 0, 2);
  // 系统配色: success=142 76% 36%, warning=38 92% 50%, destructive=0 84% 60%
  const hueSuccess = 142;
  const hueWarning = 38;
  const hueDestructive = 0;
  const lerp = (a: number, b: number, t: number) => a + (b - a) * t;
  const hue =
    r <= 1
      ? lerp(hueSuccess, hueWarning, r)
      : lerp(hueWarning, hueDestructive, r - 1);
  const sat = r <= 1 ? lerp(76, 92, r) : lerp(92, 84, r - 1);
  const light = r <= 1 ? lerp(36, 50, r) : lerp(50, 60, r - 1);

  // 与 Badge warning 样式完全一致: bg-warning/10, border-warning/50, text-warning
  return {
    backgroundColor: `hsl(${hue} ${sat}% ${light}% / 0.1)`,
    borderColor: `hsl(${hue} ${sat}% ${light}% / 0.5)`,
    color: `hsl(${hue} ${sat}% ${light}%)`,
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
  const { currency } = useCurrency();
  const [activeProtocol, setActiveProtocol] = useState<Protocol>("openai");
  const [channelsByProtocol, setChannelsByProtocol] = useState<
    Record<Protocol, Channel[]>
  >({ openai: [], anthropic: [], gemini: [] });
  const [appSettings, setAppSettings] = useState<AppSettings | null>(null);
  const [accounts, setAccounts] = useState<RemoteAccount[]>([]);
  const [loading, setLoading] = useState(false);
  const [reordering, setReordering] = useState(false);
  const [nowMs, setNowMs] = useState(() => Date.now());

  const [modalOpen, setModalOpen] = useState(false);
  const [modalMode, setModalMode] = useState<"create" | "edit">("create");
  const [editId, setEditId] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [testing, setTesting] = useState<Record<string, boolean>>({});
  const [deleteOpen, setDeleteOpen] = useState(false);
  const [deleteTarget, setDeleteTarget] = useState<Channel | null>(null);
  const [deleteSyncRemote, setDeleteSyncRemote] = useState(true);
  const [deleting, setDeleting] = useState(false);
  const [autoSortOpen, setAutoSortOpen] = useState(false);
  const [autoSortApplying, setAutoSortApplying] = useState(false);
  const channelFormSchema = useMemo(() => createChannelFormSchema(t), [t]);
  const channelForm = useForm<ChannelFormValues>({
    resolver: zodResolver(channelFormSchema),
    defaultValues: emptyFormValues(),
  });

  async function refresh() {
    setLoading(true);
    try {
      const [cs, settings, remoteAccounts] = await Promise.all([
        listChannels(),
        getSettings().catch(() => null),
        listRemoteAccounts().catch(() => []),
      ]);
      const by: Record<Protocol, Channel[]> = {
        openai: [],
        anthropic: [],
        gemini: [],
      };
      for (const c of cs) by[c.protocol].push(c);
      setChannelsByProtocol(by);
      setAccounts(remoteAccounts);
      if (settings) {
        setAppSettings(settings);
      }
    } catch (e) {
      toast.error(t("channels.toast.loadFail"), {
        description: humanizeApiError(e, t),
      });
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    refresh();
  }, []);

  useWindowEvent("cliswitch-channels-changed", () => {
    void refresh();
  });

  useEffect(() => {
    const timer = window.setInterval(() => setNowMs(Date.now()), 30_000);
    return () => window.clearInterval(timer);
  }, []);

  function effectiveCostFactor(c: Channel): number {
    const real = Number(c.real_multiplier ?? 1);
    if (!Number.isFinite(real) || real < 0) return Number.POSITIVE_INFINITY;
    return real;
  }

  const autoSortCurrent = channelsByProtocol[activeProtocol] ?? [];
  const autoSortSuggested = useMemo(() => {
    const list = autoSortCurrent.map((c, originalIndex) => ({
      c,
      originalIndex,
    }));
    list.sort((a, b) => {
      const fa = effectiveCostFactor(a.c);
      const fb = effectiveCostFactor(b.c);
      if (fa !== fb) return fa - fb;
      return a.originalIndex - b.originalIndex;
    });
    return list.map((x) => x.c);
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
      await reorderChannels(
        activeProtocol,
        autoSortSuggested.map((c) => c.id),
      );
      toast.success(t("channels.toast.reorderOk"));
      setAutoSortOpen(false);
      await refresh();
    } catch (e) {
      toast.error(t("channels.toast.reorderFail"), {
        description: humanizeApiError(e, t),
      });
    } finally {
      setAutoSortApplying(false);
    }
  }

  function openCreate() {
    setModalMode("create");
    setEditId(null);
    channelForm.reset(emptyFormValues(activeProtocol, currency));
    setModalOpen(true);
  }

  function openEdit(c: Channel) {
    setModalMode("edit");
    setEditId(c.id);
    channelForm.reset(channelToFormValues(c));
    setModalOpen(true);
  }

  const submit = channelForm.handleSubmit(async (values) => {
    const payload: ChannelDraft = {
      name: values.name.trim(),
      protocol: values.protocol,
      base_url: values.base_url.trim(),
      auth_type: "auto",
      auth_ref: values.auth_ref,
      checkin_url: values.checkin_url,
      priority: Number.parseInt(values.priority, 10),
      retry_times: Number.parseInt(values.retry_times, 10),
      ignore_channel_protection: values.ignore_channel_protection,
      recharge_currency: values.recharge_currency,
      real_multiplier: Number(values.real_multiplier),
      enabled: values.enabled,
    };

    setSubmitting(true);
    try {
      if (modalMode === "create") {
        await createChannel(payload);
        toast.success(t("channels.toast.createOk"));
      } else {
        if (!editId) throw new Error(t("channels.toast.missingId"));
        await updateChannel(editId, {
          name: payload.name,
          base_url: payload.base_url,
          auth_type: "auto",
          auth_ref: payload.auth_ref,
          checkin_url: payload.checkin_url,
          priority: payload.priority,
          retry_times: payload.retry_times,
          ignore_channel_protection: payload.ignore_channel_protection,
          recharge_currency: payload.recharge_currency,
          real_multiplier: payload.real_multiplier,
          enabled: payload.enabled,
        });
        toast.success(t("channels.toast.updateOk"));
      }
      setModalOpen(false);
      await refresh();
    } catch (e) {
      toast.error(t("channels.toast.actionFail"), {
        description: humanizeApiError(e, t),
      });
    } finally {
      setSubmitting(false);
    }
  });

  async function toggleEnabled(c: Channel) {
    try {
      const nowMs = Date.now();
      const isAutoDisabled =
        c.enabled && (c.auto_disabled_until_ms ?? 0) > nowMs;
      if (c.enabled && !isAutoDisabled) {
        await disableChannel(c.id);
        toast.success(t("channels.toast.disabledOk", { name: c.name }));
      } else {
        await enableChannel(c.id);
        toast.success(t("channels.toast.enabledOk", { name: c.name }));
      }
      await refresh();
    } catch (e) {
      toast.error(t("channels.toast.actionFail"), {
        description: humanizeApiError(e, t),
      });
    }
  }

  async function onDelete(c: Channel) {
    setDeleteTarget(c);
    setDeleteSyncRemote(true);
    setDeleteOpen(true);
  }

  async function confirmDelete() {
    if (!deleteTarget) return;
    setDeleting(true);
    try {
      await deleteChannel(deleteTarget.id, {
        sync_remote_delete: deleteSyncRemote,
      });
      toast.success(t("channels.toast.deletedOk", { name: deleteTarget.name }));
      setDeleteOpen(false);
      setDeleteTarget(null);
      await refresh();
    } catch (e) {
      toast.error(t("channels.toast.deleteFail"), {
        description: humanizeApiError(e, t),
      });
    } finally {
      setDeleting(false);
    }
  }

  async function onTest(c: Channel) {
    setTesting((m) => ({ ...m, [c.id]: true }));
    try {
      const r = await testChannel(c.id);
      if (r.reachable && r.ok) {
        toast.success(
          t("channels.toast.testReachableOkTitle", { name: c.name }),
          {
            description: t("channels.toast.testReachableOkDesc", {
              status: r.status ?? "-",
              latency: r.latency_ms,
            }),
          },
        );
      } else if (r.reachable) {
        toast.warning(
          t("channels.toast.testReachableBadTitle", { name: c.name }),
          {
            description: t("channels.toast.testReachableOkDesc", {
              status: r.status ?? "-",
              latency: r.latency_ms,
            }),
          },
        );
      } else {
        toast.error(
          t("channels.toast.testUnreachableTitle", { name: c.name }),
          {
            description:
              humanizeIssue(r.issue, t) ?? t("channels.toast.testTimeout"),
          },
        );
      }
    } catch (e) {
      toast.error(t("channels.toast.testFail"), {
        description: humanizeApiError(e, t),
      });
    } finally {
      setTesting((m) => ({ ...m, [c.id]: false }));
    }
  }

  async function persistOrder(protocol: Protocol, next: Channel[]) {
    setReordering(true);
    try {
      await reorderChannels(
        protocol,
        next.map((c) => c.id),
      );
      toast.success(t("channels.toast.reorderOk"));
      await refresh();
    } catch (e) {
      toast.error(t("channels.toast.reorderFail"), {
        description: humanizeApiError(e, t),
      });
      await refresh();
    } finally {
      setReordering(false);
    }
  }

  function renderTable(protocol: Protocol) {
    const tabChannels = channelsByProtocol[protocol];
    const accountsById = new Map(accounts.map((account) => [account.id, account]));
    const columns: Array<ColumnDef<Channel>> = [
      {
        id: "drag",
        header: "",
        cell: () => (
          <SortableDataTableHandle
            className="mx-auto block"
            title={t("channels.actions.drag")}
          >
            <GripVertical className="h-4 w-4" />
          </SortableDataTableHandle>
        ),
        meta: {
          headerClassName: "w-10",
          cellClassName: "text-center",
          skeletonClassName: "w-4 mx-auto",
        },
      },
      {
        id: "account",
        header: t("channels.table.account"),
        cell: ({ row }) => {
          const account = accountsById.get(
            row.original.managed_remote_account_id ?? "",
          );
          const name =
            account?.remote_display_name?.trim() ||
            account?.remote_username?.trim() ||
            account?.base_url ||
            "—";
          return (
            <div
              className="mx-auto max-w-[160px] truncate text-center"
              title={name}
            >
              {name}
            </div>
          );
        },
        meta: {
          headerClassName: "w-36",
          cellClassName: "text-center align-middle",
          skeletonClassName: "w-24 mx-auto",
        },
      },
      {
        accessorKey: "name",
        header: t("channels.table.name"),
        cell: ({ row }) => (
          <div
            className="mx-auto max-w-[220px] truncate text-center font-medium"
            title={row.original.name}
          >
            {row.original.name}
          </div>
        ),
        meta: {
          headerClassName: "w-44",
          skeletonClassName: "w-28 mx-auto",
        },
      },
      {
        accessorKey: "priority",
        header: t("channels.table.priority"),
        cell: ({ row }) => (
          <span className="font-mono text-sm">{row.original.priority}</span>
        ),
        meta: {
          headerClassName: "w-20",
          skeletonClassName: "w-8 mx-auto",
        },
      },
      {
        id: "real_multiplier",
        header: t("channels.table.realMultiplier"),
        cell: ({ row }) => {
          const realMultiplierDisplay = getRealMultiplierDisplay(
            row.original.real_multiplier,
          );

          if (realMultiplierDisplay.kind === "invalid") {
            return <Badge variant="secondary">—</Badge>;
          }

          if (realMultiplierDisplay.kind === "free") {
            return (
              <Badge
                variant="outline"
                className="border"
                style={multiplierStyle(0)}
              >
                {t("common.free")}
              </Badge>
            );
          }

          return (
            <Badge
              variant="outline"
              className="border tabular-nums"
              style={multiplierStyle(realMultiplierDisplay.value)}
            >
              {realMultiplierDisplay.text}
            </Badge>
          );
        },
        meta: {
          headerClassName: "w-24 text-center",
          skeletonClassName: "w-14 mx-auto",
        },
      },
      {
        id: "status",
        header: t("channels.table.status"),
        cell: ({ row }) => {
          const channel = row.original;
          const isAutoDisabled =
            channel.enabled && (channel.auto_disabled_until_ms ?? 0) > nowMs;
          const effectiveEnabled = channel.enabled && !isAutoDisabled;
          const autoDisabledMinutes = Math.max(
            1,
            Math.ceil(((channel.auto_disabled_until_ms ?? 0) - nowMs) / 60000),
          );

          return isAutoDisabled ? (
            <Badge variant="warning">
              {t("channels.status.autoDisabled", {
                minutes: autoDisabledMinutes,
              })}
            </Badge>
          ) : (
            <Badge variant={effectiveEnabled ? "success" : "secondary"}>
              {effectiveEnabled ? t("common.enabled") : t("common.disabled")}
            </Badge>
          );
        },
        meta: {
          headerClassName: "w-20",
          skeletonClassName: "w-14 mx-auto",
        },
      },
      {
        id: "actions",
        header: t("common.actions"),
        cell: ({ row }) => {
          const channel = row.original;
          const isAutoDisabled =
            channel.enabled && (channel.auto_disabled_until_ms ?? 0) > nowMs;
          const effectiveEnabled = channel.enabled && !isAutoDisabled;

          return (
            <TableActionGroup>
              <TableIconButton
                onClick={() => void onTest(channel)}
                disabled={testing[channel.id]}
                title={t("channels.actions.test")}
              >
                <TestTube className="h-4 w-4" />
              </TableIconButton>
              <TableIconButton
                onClick={() => void toggleEnabled(channel)}
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
              </TableIconButton>
              <TableIconButton
                onClick={() => openEdit(channel)}
                title={t("channels.actions.edit")}
              >
                <Pencil className="h-4 w-4" />
              </TableIconButton>
              <TableIconButton
                onClick={() => void onDelete(channel)}
                title={t("channels.actions.delete")}
              >
                <Trash2 className="h-4 w-4 text-destructive" />
              </TableIconButton>
            </TableActionGroup>
          );
        },
        meta: {
          headerClassName: "w-32",
          skeletonClassName: "w-24 mx-auto",
        },
      },
    ];

    return (
      <Card className="animate-fade-up anim-d1 flex min-h-0 flex-col overflow-hidden">
        <CardContent className="p-0">
          <SortableDataTable
            columns={columns}
            data={tabChannels}
            loading={loading}
            disabled={reordering}
            getRowId={(row) => row.id}
            onReorder={(next) => {
              setChannelsByProtocol((current) => ({
                ...current,
                [protocol]: next,
              }));
            }}
            onReorderCommit={(next) => persistOrder(protocol, next)}
            emptyState={t("channels.table.empty")}
          />
        </CardContent>
      </Card>
    );
  }

  const autoSortPreviewRows = useMemo(
    () =>
      autoSortSuggested.map((channel, newIndex) => ({
        id: channel.id,
        channel,
        newIndex,
        oldIndex: autoSortCurrent.findIndex((item) => item.id === channel.id),
        factor: effectiveCostFactor(channel),
      })),
    [autoSortCurrent, autoSortSuggested],
  );
  const autoSortPreviewColumns = useMemo<
    Array<ColumnDef<(typeof autoSortPreviewRows)[number]>>
  >(
    () => [
      {
        id: "from",
        header: t("channels.autoSort.headers.from"),
        cell: ({ row }) => (
          <span className="font-mono text-xs">
            {row.original.oldIndex >= 0 ? row.original.oldIndex + 1 : "-"}
          </span>
        ),
        meta: {
          headerClassName: "w-14",
          skeletonClassName: "w-6 mx-auto",
        },
      },
      {
        id: "to",
        header: t("channels.autoSort.headers.to"),
        cell: ({ row }) => (
          <span className="font-mono text-xs">{row.original.newIndex + 1}</span>
        ),
        meta: {
          headerClassName: "w-14",
          skeletonClassName: "w-6 mx-auto",
        },
      },
      {
        id: "channel",
        header: t("channels.autoSort.headers.channel"),
        cell: ({ row }) => (
          <div className="mx-auto flex max-w-[260px] min-w-0 flex-col items-center gap-1 text-center">
            <span className="truncate font-medium">
              {row.original.channel.name}
            </span>
            {!row.original.channel.enabled ? (
              <Badge variant="outline" className="px-1 py-0 text-[10px]">
                {t("common.disabled")}
              </Badge>
            ) : null}
          </div>
        ),
        meta: {
          skeletonClassName: "w-28 mx-auto",
        },
      },
      {
        id: "factor",
        header: t("channels.autoSort.headers.factor"),
        cell: ({ row }) => (
          <span className="font-mono text-xs text-muted-foreground">
            {Number.isFinite(row.original.factor)
              ? formatFixed2(row.original.factor)
              : "-"}
          </span>
        ),
        meta: {
          headerClassName: "w-36",
          skeletonClassName: "w-12 mx-auto",
        },
      },
    ],
    [t],
  );

  return (
    <div className="flex h-full min-h-0 flex-col">
      <PageHeader
        title={t("channels.title")}
        actions={
          <>
            <Button
              size="sm"
              variant="outline"
              onClick={() => setAutoSortOpen(true)}
              disabled={autoSortCurrent.length <= 1}
            >
              <ArrowDownUp className="h-3.5 w-3.5" />
              {t("channels.autoSort.button")}
            </Button>
            <Button size="sm" onClick={openCreate}>
              <Plus className="h-3.5 w-3.5" />
              {t("channels.new")}
            </Button>
          </>
        }
      />
      <div className="flex-1 overflow-y-auto">
        <PageBody className="space-y-3">
          <Tabs
            value={activeProtocol}
            onValueChange={(v) => {
              setActiveProtocol(v as Protocol);
            }}
          >
            <TabsList className="animate-fade-up">
              <TabsTrigger value="openai">
                {t("channels.tabs.codex")}
              </TabsTrigger>
              <TabsTrigger value="anthropic">
                {t("channels.tabs.claude")}
              </TabsTrigger>
              <TabsTrigger value="gemini">
                {t("channels.tabs.gemini")}
              </TabsTrigger>
            </TabsList>

            <TabsContent value="openai">{renderTable("openai")}</TabsContent>
            <TabsContent value="anthropic">
              {renderTable("anthropic")}
            </TabsContent>
            <TabsContent value="gemini">{renderTable("gemini")}</TabsContent>
          </Tabs>
        </PageBody>
      </div>

      {/* 新建/编辑弹窗 */}
      <Dialog open={modalOpen} onOpenChange={setModalOpen}>
        <DialogContent className="sm:max-w-[500px] max-h-[85vh] overflow-hidden flex flex-col">
          <DialogHeader className="shrink-0">
            <DialogTitle>
              {modalMode === "create"
                ? t("channels.modal.createTitle")
                : t("channels.modal.editTitle")}
            </DialogTitle>
            <DialogDescription>
              {t("channels.modal.description")}
            </DialogDescription>
          </DialogHeader>

          <Form {...channelForm}>
            <form
              onSubmit={submit}
              className="flex flex-1 min-h-0 flex-col overflow-hidden"
            >
              <FormField
                control={channelForm.control}
                name="checkin_url"
                render={({ field }) => (
                  <input type="hidden" value={field.value} readOnly />
                )}
              />

              <DialogBody className="flex-1 min-h-0 overflow-y-auto">
                <div className="space-y-4">
                  <div className="grid grid-cols-2 gap-4">
                    <FormField
                      control={channelForm.control}
                      name="name"
                      render={({ field }) => (
                        <FormItem>
                          <FormLabel>{t("channels.modal.name")}</FormLabel>
                          <FormControl>
                            <Input {...field} placeholder="openai-main" />
                          </FormControl>
                          <FormMessage />
                        </FormItem>
                      )}
                    />

                    <FormField
                      control={channelForm.control}
                      name="protocol"
                      render={({ field }) => (
                        <FormItem>
                          <FormLabel>{t("channels.modal.terminal")}</FormLabel>
                          <Select
                            value={field.value}
                            onValueChange={(value) => {
                              const nextProtocol = value as Protocol;
                              const currentBaseUrl = channelForm
                                .getValues("base_url")
                                .trim();
                              const prevDefault = defaultBaseUrl(field.value);
                              const nextDefault = defaultBaseUrl(nextProtocol);
                              const shouldUpdateBase =
                                !currentBaseUrl || currentBaseUrl === prevDefault;

                              field.onChange(nextProtocol);
                              if (shouldUpdateBase) {
                                channelForm.setValue("base_url", nextDefault, {
                                  shouldDirty: true,
                                  shouldValidate: true,
                                });
                              }
                            }}
                            disabled={modalMode === "edit"}
                          >
                            <FormControl>
                              <SelectTrigger>
                                <SelectValue />
                              </SelectTrigger>
                            </FormControl>
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
                          <FormMessage />
                        </FormItem>
                      )}
                    />
                  </div>

                  <FormField
                    control={channelForm.control}
                    name="priority"
                    render={({ field }) => (
                      <FormItem>
                        <FormLabel>{t("channels.modal.priority")}</FormLabel>
                        <FormControl>
                          <Input {...field} type="number" placeholder="0" />
                        </FormControl>
                        <FormMessage />
                      </FormItem>
                    )}
                  />

                  {appSettings?.channel_retry_enabled ? (
                    <FormField
                      control={channelForm.control}
                      name="retry_times"
                      render={({ field }) => (
                        <FormItem>
                          <FormLabel>{t("channels.modal.retryTimes")}</FormLabel>
                          <FormControl>
                            <Input
                              {...field}
                              type="number"
                              min="1"
                              placeholder="1"
                            />
                          </FormControl>
                          <FormDescription>
                            {t("channels.modal.retryTimesHint")}
                          </FormDescription>
                          <FormMessage />
                        </FormItem>
                      )}
                    />
                  ) : (
                    <FormField
                      control={channelForm.control}
                      name="retry_times"
                      render={({ field }) => (
                        <input type="hidden" value={field.value} readOnly />
                      )}
                    />
                  )}

                  <FormField
                    control={channelForm.control}
                    name="recharge_currency"
                    render={({ field }) => (
                      <FormItem>
                        <FormLabel>
                          {t("channels.modal.rechargeCurrency")}
                        </FormLabel>
                        <Select
                          value={field.value}
                          onValueChange={field.onChange}
                        >
                          <FormControl>
                            <SelectTrigger>
                              <SelectValue />
                            </SelectTrigger>
                          </FormControl>
                          <SelectContent>
                            <SelectItem value="CNY">
                              {t("channels.modal.rechargeCurrencyOptions.cny")}
                            </SelectItem>
                            <SelectItem value="USD">
                              {t("channels.modal.rechargeCurrencyOptions.usd")}
                            </SelectItem>
                          </SelectContent>
                        </Select>
                        <FormMessage />
                      </FormItem>
                    )}
                  />

                  <FormField
                    control={channelForm.control}
                    name="real_multiplier"
                    render={({ field }) => (
                      <FormItem>
                        <FormLabel>
                          {t("channels.modal.realMultiplier")}
                        </FormLabel>
                        <FormControl>
                          <Input
                            {...field}
                            type="text"
                            inputMode="decimal"
                            placeholder="1.00"
                            onBlur={(event) => {
                              field.onBlur();
                              const raw = event.target.value.trim();
                              if (!raw || !/^\d+(\.\d{0,2})?$/.test(raw)) return;
                              const number = Number(raw);
                              if (!Number.isFinite(number) || number < 0) return;
                              channelForm.setValue(
                                "real_multiplier",
                                formatFixed2(number),
                                {
                                  shouldDirty: true,
                                  shouldValidate: true,
                                },
                              );
                            }}
                          />
                        </FormControl>
                        <FormDescription>
                          {t("channels.modal.realMultiplierHint")}
                        </FormDescription>
                        <FormMessage />
                      </FormItem>
                    )}
                  />

                  <FormField
                    control={channelForm.control}
                    name="base_url"
                    render={({ field }) => (
                      <FormItem>
                        <FormLabel>{t("channels.modal.baseUrl")}</FormLabel>
                        <FormControl>
                          <Input
                            {...field}
                            placeholder="https://api.openai.com"
                          />
                        </FormControl>
                        <FormMessage />
                      </FormItem>
                    )}
                  />

                  <FormField
                    control={channelForm.control}
                    name="auth_ref"
                    render={({ field }) => (
                      <FormItem>
                        <FormLabel>{t("channels.modal.apiKey")}</FormLabel>
                        <FormControl>
                          <Input
                            {...field}
                            type="password"
                            placeholder="sk-..."
                          />
                        </FormControl>
                        <FormMessage />
                      </FormItem>
                    )}
                  />

                  <FormField
                    control={channelForm.control}
                    name="enabled"
                    render={({ field }) => (
                      <FormItem className="flex flex-row items-center justify-between space-y-0">
                        <FormLabel>{t("channels.modal.enabled")}</FormLabel>
                        <FormControl>
                          <Switch
                            checked={field.value}
                            onCheckedChange={field.onChange}
                          />
                        </FormControl>
                      </FormItem>
                    )}
                  />

                  <FormField
                    control={channelForm.control}
                    name="ignore_channel_protection"
                    render={({ field }) => (
                      <FormItem className="flex flex-row items-start justify-between gap-4 space-y-0">
                        <div className="space-y-1">
                          <FormLabel>
                            {t("channels.modal.ignoreChannelProtection")}
                          </FormLabel>
                          <FormDescription>
                            {t("channels.modal.ignoreChannelProtectionHint")}
                          </FormDescription>
                        </div>
                        <FormControl>
                          <Switch
                            checked={field.value}
                            onCheckedChange={field.onChange}
                          />
                        </FormControl>
                      </FormItem>
                    )}
                  />
                </div>
              </DialogBody>

              <DialogFooter className="shrink-0">
                <Button
                  type="button"
                  variant="outline"
                  onClick={() => setModalOpen(false)}
                  disabled={submitting}
                >
                  {t("common.cancel")}
                </Button>
                <Button type="submit" disabled={submitting}>
                  {t("common.save")}
                </Button>
              </DialogFooter>
            </form>
          </Form>
        </DialogContent>
      </Dialog>

      {/* 自动排序预览 */}
      <Dialog open={autoSortOpen} onOpenChange={setAutoSortOpen}>
        <DialogContent className="sm:max-w-[720px] max-h-[85vh] overflow-hidden flex flex-col">
          <DialogHeader className="shrink-0">
            <DialogTitle>{t("channels.autoSort.title")}</DialogTitle>
            <DialogDescription>
              {t("channels.autoSort.description", {
                terminal: protocolLabel(t, activeProtocol),
              })}
            </DialogDescription>
          </DialogHeader>

          <DialogBody className="flex-1 min-h-0 overflow-y-auto">
            {!autoSortChanged ? (
              <div className="text-sm text-muted-foreground">
                {t("channels.autoSort.noChange")}
              </div>
            ) : (
              <DataTable
                columns={autoSortPreviewColumns}
                data={autoSortPreviewRows}
                getRowId={(row) => row.id}
                containerClassName="overflow-x-auto"
                stickyHeader={false}
              />
            )}
          </DialogBody>

          <DialogFooter className="shrink-0">
            <Button
              variant="outline"
              onClick={() => setAutoSortOpen(false)}
              disabled={autoSortApplying}
            >
              {t("common.cancel")}
            </Button>
            <Button
              onClick={applyAutoSort}
              disabled={!autoSortChanged || autoSortApplying}
            >
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
                ? t("channels.deleteDialog.confirmWithName", {
                    name: deleteTarget.name,
                  })
                : t("channels.deleteDialog.confirm")}
            </DialogDescription>
          </DialogHeader>
          {deleteTarget?.managed_by_remote ? (
            <DialogBody>
              <div className="flex items-center justify-between rounded-md border p-3">
                <div className="space-y-1">
                  <div className="text-sm font-medium">
                    {t("channels.deleteDialog.syncDeleteRemote")}
                  </div>
                  <div className="text-xs text-muted-foreground">
                    {t("channels.deleteDialog.syncDeleteRemoteHint")}
                  </div>
                </div>
                <Switch
                  checked={deleteSyncRemote}
                  onCheckedChange={setDeleteSyncRemote}
                />
              </div>
            </DialogBody>
          ) : null}
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
    </div>
  );
}
