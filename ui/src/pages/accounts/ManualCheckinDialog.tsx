import React from "react";

import {
  Button,
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui";
import { useI18n } from "@/lib/i18n";
import type { RemoteAccount } from "@/api";

import { resolveAccountDisplayName } from "./shared";

type ManualCheckinDialogProps = {
  open: boolean;
  target: RemoteAccount | null;
  completing: boolean;
  onOpenChange: (open: boolean) => void;
  onCancel: () => void;
  onConfirm: () => void | Promise<void>;
};

export function ManualCheckinDialog({
  open,
  target,
  completing,
  onOpenChange,
  onCancel,
  onConfirm,
}: ManualCheckinDialogProps) {
  const { t } = useI18n();

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-[420px]">
        <DialogHeader>
          <DialogTitle>{t("accounts.checkin.dialog.title")}</DialogTitle>
          <DialogDescription>
            {t("accounts.checkin.dialog.description", {
              name: target ? resolveAccountDisplayName(target) : "",
            })}
          </DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button variant="outline" onClick={onCancel} disabled={completing}>
            {t("accounts.checkin.dialog.notDone")}
          </Button>
          <Button onClick={() => void onConfirm()} disabled={completing || !target}>
            {t("accounts.checkin.dialog.done")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
