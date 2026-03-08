import React from "react";
import { FolderOpen } from "lucide-react";

import {
  Button,
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  Input,
} from "@/components/ui";
import { useI18n } from "@/lib/i18n";

type ProjectDialogProps = {
  open: boolean;
  mode: "create" | "edit";
  name: string;
  path: string;
  saving: boolean;
  picking: boolean;
  onOpenChange: (open: boolean) => void;
  onNameChange: (value: string) => void;
  onPathChange: (value: string) => void;
  onPickFolder: () => void;
  onSubmit: () => void;
};

export function ProjectDialog({
  open,
  mode,
  name,
  path,
  saving,
  picking,
  onOpenChange,
  onNameChange,
  onPathChange,
  onPickFolder,
  onSubmit,
}: ProjectDialogProps) {
  const { t } = useI18n();

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-[500px]">
        <DialogHeader>
          <DialogTitle>
            {mode === "create"
              ? t("prompts.projectDialog.createTitle")
              : t("prompts.projectDialog.editTitle")}
          </DialogTitle>
          <DialogDescription>{t("prompts.projectDialog.description")}</DialogDescription>
        </DialogHeader>

        <div className="space-y-4 py-4">
          <div className="space-y-2">
            <div className="text-sm font-medium">{t("prompts.projectDialog.nameLabel")}</div>
            <Input
              value={name}
              onChange={(e) => onNameChange(e.target.value)}
              placeholder={t("prompts.projectDialog.namePlaceholder")}
            />
          </div>

          <div className="space-y-2">
            <div className="text-sm font-medium">{t("prompts.projectDialog.pathLabel")}</div>
            <div className="flex gap-2">
              <Input
                value={path}
                onChange={(e) => onPathChange(e.target.value)}
                placeholder={t("prompts.projectDialog.pathPlaceholder")}
                className="font-mono text-xs"
              />
              <Button variant="outline" type="button" onClick={onPickFolder} disabled={picking || saving}>
                <FolderOpen className="h-4 w-4" />
                {t("prompts.actions.browse")}
              </Button>
            </div>
          </div>
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)} disabled={saving}>
            {t("common.cancel")}
          </Button>
          <Button onClick={onSubmit} disabled={saving}>
            {t("common.save")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
