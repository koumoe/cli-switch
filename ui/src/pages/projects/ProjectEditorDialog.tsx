import React, { Suspense } from "react";
import { Save, Trash2 } from "lucide-react";

import {
  Badge,
  Button,
  Dialog,
  DialogBody,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui";
import { useI18n } from "@/hooks/use-i18n";

import { PROJECT_DOCUMENT_MAX_BYTES, formatBytes } from "./shared";

const ProjectMarkdownEditor = React.lazy(() =>
  import("./ProjectMarkdownEditor").then((module) => ({
    default: module.ProjectMarkdownEditor,
  }))
);

type ProjectEditorDialogProps = {
  open: boolean;
  title: string;
  scopeLabel: string;
  toolLabel: string;
  loading: boolean;
  draftContent: string;
  draftBytes: number;
  saving: boolean;
  deleting: boolean;
  documentExists: boolean;
  onDraftChange: (value: string) => void;
  onRequestDelete: () => void;
  onSave: () => void | Promise<void>;
  onClose: () => void;
};

export function ProjectEditorDialog({
  open,
  title,
  scopeLabel,
  toolLabel,
  loading,
  draftContent,
  draftBytes,
  saving,
  deleting,
  documentExists,
  onDraftChange,
  onRequestDelete,
  onSave,
  onClose,
}: ProjectEditorDialogProps) {
  const { t } = useI18n();
  const overLimit = draftBytes > PROJECT_DOCUMENT_MAX_BYTES;

  return (
    <Dialog
      open={open}
      onOpenChange={(isOpen) => {
        if (!isOpen) onClose();
      }}
    >
      <DialogContent className="flex h-[78vh] max-h-[90vh] flex-col overflow-hidden sm:max-w-[900px]">
        <DialogHeader className="shrink-0 pr-10">
          <div className="flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1">
            <DialogTitle className="leading-tight">{title}</DialogTitle>
            <Badge variant="secondary">{scopeLabel}</Badge>
            <Badge variant="outline">{toolLabel}</Badge>
            <span
              className={
                overLimit
                  ? "ml-1 text-xs text-destructive"
                  : "ml-1 text-xs text-muted-foreground"
              }
            >
              {t("projects.editor.size", {
                current: formatBytes(draftBytes),
                max: formatBytes(PROJECT_DOCUMENT_MAX_BYTES),
              })}
            </span>
          </div>
        </DialogHeader>

        <DialogBody className="flex-1 min-h-0 overflow-hidden">
          {loading ? (
            <div className="flex h-full min-h-[300px] items-center justify-center text-sm text-muted-foreground">
              {t("common.loading")}
            </div>
          ) : (
            <Suspense
              fallback={
                <div className="flex h-full min-h-[300px] items-center justify-center text-sm text-muted-foreground">
                  {t("common.loading")}
                </div>
              }
            >
              <ProjectMarkdownEditor
                value={draftContent}
                onChange={onDraftChange}
                placeholder={t("projects.editor.placeholder")}
                disabled={saving || deleting}
              />
            </Suspense>
          )}
        </DialogBody>

        <DialogFooter className="shrink-0 gap-3 sm:flex-row sm:items-center sm:justify-between">
          <Button
            variant="destructive"
            className="gap-2 sm:self-auto"
            onClick={onRequestDelete}
            disabled={saving || deleting || loading || !documentExists}
          >
            <Trash2 className="h-4 w-4" />
            {t("projects.actions.deleteDocument")}
          </Button>
          <div className="flex items-center gap-2 self-end sm:self-auto">
            <Button variant="outline" onClick={onClose} disabled={saving || deleting}>
              {t("common.cancel")}
            </Button>
            <Button
              className="gap-2"
              onClick={() => void onSave()}
              disabled={saving || deleting || overLimit || loading}
            >
              <Save className="h-4 w-4" />
              {saving ? "..." : t("projects.editor.save")}
            </Button>
          </div>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
