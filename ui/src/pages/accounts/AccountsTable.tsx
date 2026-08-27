import React from "react";
import type { ColumnDef } from "@tanstack/react-table";
import { ExternalLink, GripVertical, Link2, Pencil, RefreshCw, Trash2 } from "lucide-react";

import type { OpenAiQuotaWindow, RemoteAccount } from "@/types/api";
import {
  Badge,
  Card,
  CardContent,
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui";
import {
  SortableDataTable,
  SortableDataTableHandle,
} from "@/components/composed/sortable-data-table";
import {
  TableActionGroup,
  TableIconButton,
} from "@/components/composed/table-primitives";
import { useI18n } from "@/hooks/use-i18n";

import {
  accountHasUserApiCredentials,
  formatAmount,
  isOpenAiAccount,
  resolveCheckinMode,
} from "./shared";

function quotaWindowLabel(
  kind: string,
  windowMinutes: number,
  t: (key: string, vars?: Record<string, string | number>) => string,
): string {
  if (kind === "weekly" || windowMinutes === 10_080) return t("accounts.quota.weekly");
  if (windowMinutes > 0 && windowMinutes % 1_440 === 0) {
    return t("accounts.quota.days", { count: windowMinutes / 1_440 });
  }
  if (windowMinutes > 0 && windowMinutes % 60 === 0) {
    return t("accounts.quota.hours", { count: windowMinutes / 60 });
  }
  return kind || t("accounts.quota.window");
}

function sortQuotaWindows(windows: OpenAiQuotaWindow[]): OpenAiQuotaWindow[] {
  return [...windows].sort((a, b) => {
    const aMinutes = a.window_minutes > 0 ? a.window_minutes : Number.POSITIVE_INFINITY;
    const bMinutes = b.window_minutes > 0 ? b.window_minutes : Number.POSITIVE_INFINITY;
    return aMinutes - bMinutes;
  });
}

type AccountsTableProps = {
  accounts: RemoteAccount[];
  loading: boolean;
  reordering: boolean;
  today: string;
  checkinsDate: string | null;
  checkinDoneMap: Record<string, boolean>;
  refreshing: Record<string, boolean>;
  systemChecking: Record<string, boolean>;
  pageOpening: Record<string, boolean>;
  setAccounts: (next: RemoteAccount[]) => void;
  persistOrder: (next: RemoteAccount[]) => Promise<void>;
  onRefreshAccount: (item: RemoteAccount) => void | Promise<void>;
  onOpenBaseUrl: (item: RemoteAccount) => void | Promise<void>;
  onSystemCheckin: (item: RemoteAccount) => void | Promise<void>;
  onOpenManualCheckinPrompt: (item: RemoteAccount) => void | Promise<void>;
  onOpenCreateManagedChannelDialog: (
    item: RemoteAccount,
  ) => void | Promise<void>;
  onOpenEdit: (item: RemoteAccount) => void;
  onOpenDeleteDialog: (item: RemoteAccount) => void;
};

export function AccountsTable({
  accounts,
  loading,
  reordering,
  today,
  checkinsDate,
  checkinDoneMap,
  refreshing,
  systemChecking,
  pageOpening,
  setAccounts,
  persistOrder,
  onRefreshAccount,
  onOpenBaseUrl,
  onSystemCheckin,
  onOpenManualCheckinPrompt,
  onOpenCreateManagedChannelDialog,
  onOpenEdit,
  onOpenDeleteDialog,
}: AccountsTableProps) {
  const { locale, t } = useI18n();
  const columns = React.useMemo<Array<ColumnDef<RemoteAccount>>>(
    () => [
      {
        id: "drag",
        header: "",
        cell: () => (
          <SortableDataTableHandle
            className="mx-auto block"
            title={t("accounts.actions.drag")}
          >
            <GripVertical className="h-4 w-4" />
          </SortableDataTableHandle>
        ),
        meta: {
          headerClassName: "w-10",
          cellClassName: "text-center align-middle",
          skeletonClassName: "w-4 mx-auto",
        },
      },
      {
        id: "name",
        header: t("accounts.table.name"),
        cell: ({ row }) => {
          const item = row.original;
          const name = item.name.trim() || "-";
          return (
            <div className="mx-auto max-w-[180px] text-center">
              <div className="truncate font-medium" title={name}>
                {name}
              </div>
              {item.reauth_required ? (
                <div className="mt-1 text-xs text-destructive">
                  {t("accounts.table.reauthRequired")}
                </div>
              ) : null}
            </div>
          );
        },
        meta: {
          headerClassName: "w-44",
          skeletonClassName: "w-28 mx-auto",
        },
      },
      {
        id: "base_url",
        header: t("accounts.table.baseUrl"),
        cell: ({ row }) => (
          <div
            className="mx-auto max-w-[260px] truncate text-center text-muted-foreground"
            title={row.original.base_url}
          >
            {row.original.base_url || "-"}
          </div>
        ),
        meta: {
          headerClassName: "w-60",
          skeletonClassName: "w-40 mx-auto",
        },
      },
      {
        id: "provider",
        header: t("accounts.table.provider"),
        cell: ({ row }) => (
          <div className="flex items-center justify-center">
            <Badge
              variant={
                row.original.provider === "newapi" ? "secondary" : "outline"
              }
            >
              {t(`accounts.providers.${row.original.provider}`)}
            </Badge>
          </div>
        ),
        meta: {
          headerClassName: "w-28",
          skeletonClassName: "w-16 mx-auto",
        },
      },
      {
        id: "balance",
        header: t("accounts.table.quota"),
        cell: ({ row }) => {
          const item = row.original;
          if (!isOpenAiAccount(item)) {
            return <div className="font-mono">{formatAmount(item, item.last_balance_amount)}</div>;
          }
          const windows = sortQuotaWindows(item.quota_windows ?? []);
          if (windows.length === 0) {
            return <span className="text-muted-foreground">{t("accounts.quota.unavailable")}</span>;
          }
          const shortestWindow = windows[0];
          const renderReset = (window: OpenAiQuotaWindow) => {
            if (!window.resets_at_ms) return null;
            const reset = new Intl.DateTimeFormat(locale, {
              month: "numeric",
              day: "numeric",
              hour: "2-digit",
              minute: "2-digit",
            }).format(new Date(window.resets_at_ms));
            return t("accounts.quota.resetAt", { time: reset });
          };
          const shortestReset = renderReset(shortestWindow);
          return (
            <Tooltip>
              <TooltipTrigger asChild>
                <div className="inline-flex cursor-help whitespace-nowrap text-sm">
                  <span className="font-medium">{Math.round(shortestWindow.used_percent)}%</span>
                  {shortestReset ? (
                    <span className="text-muted-foreground"> · {shortestReset}</span>
                  ) : null}
                </div>
              </TooltipTrigger>
              <TooltipContent className="space-y-1.5 px-3 py-2">
                {windows.map((window, index) => {
                  const reset = renderReset(window);
                  return (
                    <div
                      key={`${window.kind}-${window.window_minutes}-${index}`}
                      className="whitespace-nowrap"
                    >
                      <span className="font-medium">
                        {quotaWindowLabel(window.kind, window.window_minutes, t)} {Math.round(window.used_percent)}%
                      </span>
                      {reset ? <span className="opacity-80"> · {reset}</span> : null}
                    </div>
                  );
                })}
              </TooltipContent>
            </Tooltip>
          );
        },
        meta: {
          skeletonClassName: "w-18 mx-auto",
        },
      },
      {
        id: "checkin",
        header: t("accounts.table.checkin"),
        cell: ({ row }) => {
          const item = row.original;
          if (item.provider === "openai") {
            return <Badge variant="secondary">{t("accounts.checkin.none")}</Badge>;
          }
          const logicalCheckinMode = resolveCheckinMode(item);
          const done = !!checkinDoneMap[item.id] && checkinsDate === today;
          const checkinBadge =
            logicalCheckinMode === "disabled"
              ? {
                  text: t("accounts.checkin.none"),
                  variant: "secondary" as const,
                }
              : done
                ? {
                    text: t("accounts.checkin.done"),
                    variant: "success" as const,
                  }
                : {
                    text: t("accounts.checkin.todo"),
                    variant: "destructive" as const,
                  };
          const canTriggerCheckin = logicalCheckinMode !== "disabled" && !done;
          const checkinBusy =
            !!systemChecking[item.id] || !!pageOpening[item.id];

          return canTriggerCheckin ? (
            <button
              type="button"
              className="inline-flex"
              onClick={() => {
                if (logicalCheckinMode === "system_api") {
                  void onSystemCheckin(item);
                } else {
                  void onOpenManualCheckinPrompt(item);
                }
              }}
              disabled={checkinBusy}
              title={
                logicalCheckinMode === "system_api"
                  ? t("accounts.actions.systemCheckin")
                  : t("accounts.actions.manualCheckin")
              }
            >
              <Badge
                variant={checkinBadge.variant}
                className={checkinBusy ? "opacity-60" : "cursor-pointer"}
              >
                {checkinBadge.text}
              </Badge>
            </button>
          ) : (
            <Badge variant={checkinBadge.variant}>{checkinBadge.text}</Badge>
          );
        },
        meta: {
          skeletonClassName: "w-14 mx-auto",
        },
      },
      {
        id: "actions",
        header: t("common.actions"),
        cell: ({ row }) => {
          const item = row.original;
          const canManageRemote = accountHasUserApiCredentials(item);

          return (
            <TableActionGroup>
              <TableIconButton
                onClick={() => void onOpenBaseUrl(item)}
                title={t("accounts.actions.openBaseUrl")}
              >
                <ExternalLink className="h-4 w-4" />
              </TableIconButton>
              <TableIconButton
                onClick={() => void onRefreshAccount(item)}
                disabled={!!refreshing[item.id]}
                title={t("accounts.actions.refresh")}
              >
                <RefreshCw className="h-4 w-4" />
              </TableIconButton>
              <TableIconButton
                onClick={() => void onOpenCreateManagedChannelDialog(item)}
                disabled={!canManageRemote}
                title={t(
                  item.provider === "openai"
                    ? "accounts.actions.createOpenAiManaged"
                    : "accounts.actions.createManaged",
                )}
              >
                <Link2 className="h-4 w-4" />
              </TableIconButton>
              <TableIconButton
                onClick={() => onOpenEdit(item)}
                title={t("accounts.actions.edit")}
              >
                <Pencil className="h-4 w-4" />
              </TableIconButton>
              <TableIconButton
                onClick={() => onOpenDeleteDialog(item)}
                title={t("accounts.actions.delete")}
              >
                <Trash2 className="h-4 w-4 text-destructive" />
              </TableIconButton>
            </TableActionGroup>
          );
        },
        meta: {
          headerClassName: "text-center",
          cellClassName: "text-center align-middle",
          skeletonClassName: "w-24 mx-auto",
        },
      },
    ],
    [
      checkinDoneMap,
      checkinsDate,
      onOpenCreateManagedChannelDialog,
      onOpenDeleteDialog,
      onOpenEdit,
      onOpenManualCheckinPrompt,
      onOpenBaseUrl,
      onRefreshAccount,
      onSystemCheckin,
      pageOpening,
      refreshing,
      systemChecking,
      t,
      today,
      locale,
    ],
  );

  return (
    <Card className="animate-fade-up flex min-h-0 flex-1 flex-col overflow-hidden">
      <CardContent className="p-0">
        <SortableDataTable
          columns={columns}
          data={accounts}
          loading={loading}
          disabled={reordering}
          getRowId={(row) => row.id}
          onReorder={setAccounts}
          onReorderCommit={persistOrder}
          emptyState={t("accounts.table.empty")}
        />
      </CardContent>
    </Card>
  );
}
