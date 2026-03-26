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
import type { NewApiAccount } from "@/api";

type ManualCheckinDialogProps = {
  open: boolean;
  target: NewApiAccount | null;
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
              name: target?.user_id || target?.base_url || "",
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
