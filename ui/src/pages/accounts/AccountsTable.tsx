import React from "react";
import type { ColumnDef } from "@tanstack/react-table";
import { ExternalLink, GripVertical, Link2, Pencil, RefreshCw, Trash2 } from "lucide-react";

import type { OpenAiRemoteAccount, RemoteAccount } from "@/types/api";
import {
  Badge,
  Card,
  CardContent,
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

import { OpenAiQuotaCell } from "./OpenAiQuotaCell";
import { accountStatusBadgeClass, ResetQuotaButton } from "./ResetQuotaButton";

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
  resetting: Record<string, boolean>;
  resetPending: Record<string, boolean>;
  resetRefreshRequired: Record<string, boolean>;
  onResetQuota: (item: OpenAiRemoteAccount) => Promise<boolean>;
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
  resetting,
  resetPending,
  resetRefreshRequired,
  onResetQuota,
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
          return <OpenAiQuotaCell account={item} />;
        },
        meta: {
          skeletonClassName: "w-18 mx-auto",
        },
      },
      {
        id: "checkin",
        header: t("accounts.table.checkinOrReset"),
        cell: ({ row }) => {
          const item = row.original;
          if (item.provider === "openai") {
            return (
              <ResetQuotaButton
                account={item}
                busy={!!resetting[item.id] || !!refreshing[item.id]}
                pending={!!resetPending[item.id]}
                refreshRequired={!!resetRefreshRequired[item.id]}
                onReset={onResetQuota}
              />
            );
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
                className={`${accountStatusBadgeClass} ${checkinBusy ? "opacity-60" : "cursor-pointer"}`}
              >
                {checkinBadge.text}
              </Badge>
            </button>
          ) : (
            <Badge variant={checkinBadge.variant} className={accountStatusBadgeClass}>{checkinBadge.text}</Badge>
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
                disabled={!!refreshing[item.id] || !!resetting[item.id]}
                title={t("accounts.actions.refresh")}
              >
                <RefreshCw className="h-4 w-4" />
              </TableIconButton>
              <TableIconButton
                onClick={() => void onOpenCreateManagedChannelDialog(item)}
                disabled={!canManageRemote || !!resetting[item.id]}
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
                disabled={!!resetting[item.id]}
                title={t("accounts.actions.edit")}
              >
                <Pencil className="h-4 w-4" />
              </TableIconButton>
              <TableIconButton
                onClick={() => onOpenDeleteDialog(item)}
                disabled={!!resetting[item.id]}
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
      onResetQuota,
      onSystemCheckin,
      pageOpening,
      refreshing,
      resetting,
      resetPending,
      resetRefreshRequired,
      systemChecking,
      t,
      today,
    ],
  );

  return (
    <Card className="animate-fade-up flex min-h-0 flex-1 flex-col overflow-hidden">
      <CardContent className="flex min-h-0 flex-1 flex-col p-0">
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
