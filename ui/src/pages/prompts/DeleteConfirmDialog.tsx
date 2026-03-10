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

type DeleteConfirmDialogProps = {
  open: boolean;
  title: string;
  description: string;
  confirmLabel?: string;
  busy?: boolean;
  onOpenChange: (open: boolean) => void;
  onConfirm: () => void;
};

export function DeleteConfirmDialog({
  open,
  title,
  description,
  confirmLabel,
  busy = false,
  onOpenChange,
  onConfirm,
}: DeleteConfirmDialogProps) {
  const { t } = useI18n();

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="top-[44%] translate-y-[-44%] sm:max-w-[480px]">
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
          <DialogDescription>{description}</DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)} disabled={busy}>
            {t("common.cancel")}
          </Button>
          <Button variant="destructive" onClick={onConfirm} disabled={busy}>
            {confirmLabel ?? t("common.delete")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
