import React from "react";

import type { RemoteAccount } from "@/types/api";
import {
  Button,
  Dialog,
  DialogBody,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  Switch,
} from "@/components/ui";
import { useI18n } from "@/hooks/use-i18n";

import { resolveAccountDisplayName } from "./shared";

type DeleteAccountDialogProps = {
  open: boolean;
  target: RemoteAccount | null;
  deleteManagedChannels: boolean;
  deleteSyncRemote: boolean;
  deleting: boolean;
  onOpenChange: (open: boolean) => void;
  onDeleteManagedChannelsChange: (checked: boolean) => void;
  onDeleteSyncRemoteChange: (checked: boolean) => void;
  onConfirmDelete: () => void | Promise<void>;
};

export function DeleteAccountDialog({
  open,
  target,
  deleteManagedChannels,
  deleteSyncRemote,
  deleting,
  onOpenChange,
  onDeleteManagedChannelsChange,
  onDeleteSyncRemoteChange,
  onConfirmDelete,
}: DeleteAccountDialogProps) {
  const { t } = useI18n();
  const canDeleteManaged = !!target;
  const targetName = target ? resolveAccountDisplayName(target) : "";

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-[460px]">
        <DialogHeader>
          <DialogTitle>{t("accounts.deleteDialog.title")}</DialogTitle>
          <DialogDescription>
            {target
              ? t("accounts.deleteDialog.descriptionWithName", { name: targetName })
              : t("accounts.deleteDialog.description")}
          </DialogDescription>
        </DialogHeader>
        {canDeleteManaged ? (
          <DialogBody>
            <div className="space-y-3">
              <div className="flex items-center justify-between rounded-md border p-3">
                <div className="space-y-1">
                  <div className="text-sm font-medium">{t("accounts.deleteDialog.deleteManagedChannels")}</div>
                  <div className="text-xs text-muted-foreground">{t("accounts.deleteDialog.deleteManagedChannelsHint")}</div>
                </div>
                <Switch checked={deleteManagedChannels} onCheckedChange={onDeleteManagedChannelsChange} />
              </div>
              <div className="flex items-center justify-between rounded-md border p-3">
                <div className="space-y-1">
                  <div className="text-sm font-medium">{t("accounts.deleteDialog.syncDeleteRemote")}</div>
                  <div className="text-xs text-muted-foreground">{t("accounts.deleteDialog.syncDeleteRemoteHint")}</div>
                </div>
                <Switch checked={deleteSyncRemote} onCheckedChange={onDeleteSyncRemoteChange} />
              </div>
            </div>
          </DialogBody>
        ) : null}
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)} disabled={deleting}>
            {t("common.cancel")}
          </Button>
          <Button variant="destructive" onClick={() => void onConfirmDelete()} disabled={deleting || !target}>
            {t("common.delete")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
