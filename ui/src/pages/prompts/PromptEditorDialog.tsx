import React, { useEffect, useMemo } from "react";
import MDEditor from "@uiw/react-md-editor";
import { Save } from "lucide-react";

import type { PromptDocument } from "@/api";
import {
  Badge,
  Button,
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui";
import { useI18n } from "@/lib/i18n";

import { PROMPT_DOCUMENT_MAX_BYTES, formatBytes } from "./shared";

type PromptEditorDialogProps = {
  open: boolean;
  title: string;
  scopeLabel: string;
  toolLabel: string;
  document: PromptDocument | null;
  loading: boolean;
  editorOpen: boolean;
  draftContent: string;
  draftBytes: number;
  saving: boolean;
  deleting: boolean;
  onDraftChange: (value: string) => void;
  onStartEdit: () => void;
  onSave: () => void;
  onClose: () => void;
};

export function PromptEditorDialog({
  open,
  title,
  scopeLabel,
  toolLabel,
  document,
  loading,
  editorOpen,
  draftContent,
  draftBytes,
  saving,
  deleting,
  onDraftChange,
  onStartEdit,
  onSave,
  onClose,
}: PromptEditorDialogProps) {
  const { t } = useI18n();
  const overLimit = draftBytes > PROMPT_DOCUMENT_MAX_BYTES;

  // Auto-start edit when document finishes loading
  useEffect(() => {
    if (open && !loading && !editorOpen) {
      onStartEdit();
    }
  }, [open, loading]);

  const colorMode = useMemo(() => {
    if (typeof window === "undefined") return "dark";
    return window.document.documentElement.classList.contains("dark") ? "dark" : "light";
  }, [open]);

  return (
    <Dialog
      open={open}
      onOpenChange={(isOpen) => {
        if (!isOpen) onClose();
      }}
    >
      <DialogContent className="sm:max-w-[900px] max-h-[85vh] overflow-hidden flex flex-col">
        <DialogHeader className="shrink-0">
          <div className="flex items-center gap-2">
            <DialogTitle>{title}</DialogTitle>
            <Badge variant="secondary">{scopeLabel}</Badge>
            <Badge variant="outline">{toolLabel}</Badge>
          </div>
        </DialogHeader>

        <div className="flex-1 min-h-0 overflow-hidden" data-color-mode={colorMode}>
          {loading ? (
            <div className="flex h-full min-h-[300px] items-center justify-center text-sm text-muted-foreground">
              {t("common.loading")}
            </div>
          ) : (
            <MDEditor
              value={draftContent}
              onChange={(val) => onDraftChange(val ?? "")}
              height="100%"
              visibleDragbar={false}
              preview="live"
            />
          )}
        </div>

        <DialogFooter className="shrink-0 flex items-center justify-between sm:justify-between">
          <div className={overLimit ? "text-xs text-destructive" : "text-xs text-muted-foreground"}>
            {t("prompts.editor.size", {
              current: formatBytes(draftBytes),
              max: formatBytes(PROMPT_DOCUMENT_MAX_BYTES),
            })}
          </div>
          <div className="flex items-center gap-2">
            <Button variant="outline" onClick={onClose} disabled={saving || deleting}>
              {t("common.cancel")}
            </Button>
            <Button className="gap-2" onClick={onSave} disabled={saving || deleting || overLimit || loading}>
              <Save className="h-4 w-4" />
              {saving ? "..." : t("prompts.editor.save")}
            </Button>
          </div>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
