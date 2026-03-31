import React from "react";
import { GripVertical, Link2, Pencil, RefreshCw, Trash2 } from "lucide-react";

import type { RemoteAccount } from "@/api";
import {
  Badge,
  Button,
  Card,
  CardContent,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui";
import { useI18n } from "@/lib/i18n";

import {
  accountHasUserApiCredentials,
  formatAmount,
  isNewApiAccount,
  moveInList,
  moveToEndList,
  resolveCheckinMode,
  type DragAction,
} from "./shared";

type AccountsTableProps = {
  accounts: RemoteAccount[];
  reordering: boolean;
  dragId: string | null;
  dragOverId: string | null;
  dragSnapshot: RemoteAccount[] | null;
  dragCommittedRef: React.MutableRefObject<boolean>;
  today: string;
  checkinsDate: string | null;
  checkinDoneMap: Record<string, boolean>;
  refreshing: Record<string, boolean>;
  systemChecking: Record<string, boolean>;
  pageOpening: Record<string, boolean>;
  setAccounts: React.Dispatch<React.SetStateAction<RemoteAccount[]>>;
  dispatchDrag: React.Dispatch<DragAction>;
  persistOrder: (next: RemoteAccount[]) => Promise<void>;
  onRefreshAccount: (item: RemoteAccount) => void | Promise<void>;
  onSystemCheckin: (item: RemoteAccount) => void | Promise<void>;
  onOpenManualCheckinPrompt: (item: RemoteAccount) => void | Promise<void>;
  onOpenCreateManagedChannelDialog: (item: RemoteAccount) => void | Promise<void>;
  onOpenEdit: (item: RemoteAccount) => void;
  onOpenDeleteDialog: (item: RemoteAccount) => void;
};

export function AccountsTable({
  accounts,
  reordering,
  dragId,
  dragOverId,
  dragSnapshot,
  dragCommittedRef,
  today,
  checkinsDate,
  checkinDoneMap,
  refreshing,
  systemChecking,
  pageOpening,
  setAccounts,
  dispatchDrag,
  persistOrder,
  onRefreshAccount,
  onSystemCheckin,
  onOpenManualCheckinPrompt,
  onOpenCreateManagedChannelDialog,
  onOpenEdit,
  onOpenDeleteDialog,
}: AccountsTableProps) {
  const { t } = useI18n();

  function setAccountDragPreview(e: React.DragEvent, item: RemoteAccount) {
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
      title.style.textAlign = "center";

      const meta = document.createElement("div");
      meta.textContent = t(`accounts.providers.${item.provider}`);
      meta.style.marginTop = "4px";
      meta.style.fontSize = "11px";
      meta.style.color = "rgba(0,0,0,0.6)";
      meta.style.whiteSpace = "nowrap";
      meta.style.overflow = "hidden";
      meta.style.textOverflow = "ellipsis";
      meta.style.textAlign = "center";

      el.appendChild(title);
      el.appendChild(meta);
      document.body.appendChild(el);

      e.dataTransfer.setDragImage(el, 16, 16);
      window.setTimeout(() => el.remove(), 0);
    } catch {
      // ignore
    }
  }

  return (
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
                const canManageRemote = accountHasUserApiCredentials(item);
                const newapi = isNewApiAccount(item);

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
                    <TableCell className="text-center align-middle">
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
                    <TableCell className="max-w-[360px]">
                      <div className="space-y-1 text-center">
                        <div className="truncate font-medium" title={item.base_url}>
                          {item.base_url}
                        </div>
                        <div className="flex items-center justify-center text-xs text-muted-foreground">
                          <Badge variant={newapi ? "secondary" : "outline"}>
                            {t(`accounts.providers.${item.provider}`)}
                          </Badge>
                        </div>
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
                              void onOpenManualCheckinPrompt(item);
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
                    <TableCell className="text-center align-middle">
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
                        <Button variant="ghost" size="icon" onClick={() => onOpenEdit(item)} title={t("accounts.actions.edit")}>
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
