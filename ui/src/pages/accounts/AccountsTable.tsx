import React from "react";
import type { ColumnDef } from "@tanstack/react-table";
import { GripVertical, Link2, Pencil, RefreshCw, Trash2 } from "lucide-react";

import type { RemoteAccount } from "@/types/api";
import { Badge, Button, Card, CardContent } from "@/components/ui";
import {
  SortableDataTable,
  SortableDataTableHandle,
} from "@/components/composed/sortable-data-table";
import { useI18n } from "@/hooks/use-i18n";

import {
  accountHasUserApiCredentials,
  formatAmount,
  resolveCheckinMode,
} from "./shared";

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
  onSystemCheckin,
  onOpenManualCheckinPrompt,
  onOpenCreateManagedChannelDialog,
  onOpenEdit,
  onOpenDeleteDialog,
}: AccountsTableProps) {
  const { t } = useI18n();
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
        accessorKey: "base_url",
        header: t("accounts.table.baseUrl"),
        cell: ({ row }) => (
          <div className="max-w-[360px] text-left">
            <div className="truncate font-medium" title={row.original.base_url}>
              {row.original.base_url}
            </div>
            {row.original.provider === "sub2api" &&
            row.original.reauth_required ? (
              <div className="mt-1 text-xs text-destructive">
                {t("accounts.table.reauthRequired")}
              </div>
            ) : null}
          </div>
        ),
        meta: {
          headerClassName: "text-left",
          cellClassName: "text-left",
          skeletonClassName: "w-56",
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
        header: t("accounts.table.balance"),
        cell: ({ row }) => (
          <div className="font-mono">
            {formatAmount(row.original, row.original.last_balance_amount)}
          </div>
        ),
        meta: {
          skeletonClassName: "w-18 ml-auto",
        },
      },
      {
        id: "checkin",
        header: t("accounts.table.checkin"),
        cell: ({ row }) => {
          const item = row.original;
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
                onClick={() => void onOpenCreateManagedChannelDialog(item)}
                disabled={!canManageRemote}
                title={t("accounts.actions.createManaged")}
              >
                <Link2 className="h-4 w-4" />
              </Button>
              <Button
                variant="ghost"
                size="icon"
                onClick={() => onOpenEdit(item)}
                title={t("accounts.actions.edit")}
              >
                <Pencil className="h-4 w-4" />
              </Button>
              <Button
                variant="ghost"
                size="icon"
                onClick={() => onOpenDeleteDialog(item)}
                title={t("accounts.actions.delete")}
              >
                <Trash2 className="h-4 w-4 text-destructive" />
              </Button>
            </div>
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
      onRefreshAccount,
      onSystemCheckin,
      pageOpening,
      refreshing,
      systemChecking,
      t,
      today,
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
          renderDragOverlay={(item) => (
            <div className="min-w-[280px] max-w-[380px] rounded-md border border-border bg-card px-3 py-2 shadow-xl">
              <div className="text-center text-sm font-semibold">
                {item.base_url}
              </div>
              <div className="mt-1 text-center text-xs text-muted-foreground">
                {t(`accounts.providers.${item.provider}`)}
              </div>
            </div>
          )}
        />
      </CardContent>
    </Card>
  );
}
