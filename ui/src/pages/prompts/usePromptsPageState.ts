import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { toast } from "sonner";

import { deletePromptDocument, getPromptDocument, listPromptProjects, savePromptDocument } from "@/api";
import { useI18n } from "@/hooks/use-i18n";
import { humanizeApiError, isApiRequestError } from "@/lib/error";
import type { CliToolId, PromptDocument, PromptProject } from "@/types/api";

import {
  PROMPT_DOCUMENT_MAX_BYTES,
  emptyPromptSelection,
  findSelectedProject,
  formatBytes,
  measurePromptBytes,
  type PromptSelection,
} from "./shared";

export function usePromptsPageState() {
  const { t } = useI18n();
  const [activeTool, setActiveTool] = useState<CliToolId>("codex");
  const [selection, setSelection] = useState<PromptSelection>(emptyPromptSelection());

  const [projects, setProjects] = useState<PromptProject[]>([]);
  const [projectsLoading, setProjectsLoading] = useState(false);

  const [document, setDocument] = useState<PromptDocument | null>(null);
  const [documentLoading, setDocumentLoading] = useState(false);
  const [editorOpen, setEditorOpen] = useState(false);
  const [savedContent, setSavedContent] = useState("");
  const [draftContent, setDraftContent] = useState("");
  const [documentSaving, setDocumentSaving] = useState(false);
  const [documentDeleting, setDocumentDeleting] = useState(false);
  const [documentDeleteOpen, setDocumentDeleteOpen] = useState(false);

  const requestSeqRef = useRef(0);

  const selectedProject = useMemo(
    () => findSelectedProject(projects, selection),
    [projects, selection]
  );
  const dirty = editorOpen && draftContent !== savedContent;
  const draftBytes = useMemo(() => measurePromptBytes(draftContent), [draftContent]);

  const confirmDiscardChanges = useCallback(() => {
    if (!dirty) return true;
    return window.confirm(t("prompts.editor.unsavedConfirm"));
  }, [dirty, t]);

  const refreshProjects = useCallback(async () => {
    setProjectsLoading(true);
    try {
      const next = await listPromptProjects(activeTool);
      setProjects(next);
      setSelection((prev) => {
        if (prev.scope === "project" && !next.some((item) => item.id === prev.projectId)) {
          return emptyPromptSelection();
        }
        return prev;
      });
    } catch (e) {
      toast.error(t("prompts.toast.loadProjectsFail"), {
        description: humanizeApiError(e, t),
      });
    } finally {
      setProjectsLoading(false);
    }
  }, [activeTool, t]);

  const refreshDocument = useCallback(
    async (opts?: { preserveDraft?: boolean; keepEditorOpen?: boolean; quiet?: boolean }) => {
      const requestSeq = ++requestSeqRef.current;
      setDocumentLoading(true);

      try {
        const next = await getPromptDocument({
          tool: activeTool,
          scope: selection.scope,
          project_id: selection.scope === "project" ? selection.projectId : null,
        });
        if (requestSeq !== requestSeqRef.current) return;
        setDocument(next);
        setSavedContent(next.content_md);
        if (!opts?.preserveDraft) {
          setDraftContent(next.content_md);
        }
        if (!opts?.keepEditorOpen) {
          setEditorOpen(false);
        }
      } catch (e) {
        if (requestSeq !== requestSeqRef.current) return;
        if (isApiRequestError(e) && e.code === "prompt_project_not_found") {
          return;
        }
        if (!opts?.quiet) {
          toast.error(t("prompts.toast.loadDocumentFail"), {
            description: humanizeApiError(e, t),
          });
        }
      } finally {
        if (requestSeq === requestSeqRef.current) {
          setDocumentLoading(false);
        }
      }
    },
    [activeTool, selection, t]
  );

  useEffect(() => {
    void refreshProjects();
  }, [refreshProjects]);

  useEffect(() => {
    void refreshDocument();
  }, [refreshDocument]);

  const maybeRecoverFromDocumentError = useCallback(
    (err: unknown, preserveDraft = true) => {
      if (
        isApiRequestError(err) &&
        [
          "prompt_document_version_conflict",
          "prompt_document_not_found",
          "prompt_project_not_found",
        ].includes(err.code ?? "")
      ) {
        void refreshDocument({
          preserveDraft,
          keepEditorOpen: preserveDraft,
          quiet: true,
        });
      }
    },
    [refreshDocument]
  );

  const handleToolChange = useCallback(
    (value: string) => {
      const nextTool = value as CliToolId;
      if (nextTool === activeTool) return;
      if (!confirmDiscardChanges()) return;
      setActiveTool(nextTool);
    },
    [activeTool, confirmDiscardChanges]
  );

  const handleSelectGlobal = useCallback(() => {
    if (selection.scope === "global") return;
    if (!confirmDiscardChanges()) return;
    setSelection(emptyPromptSelection());
  }, [confirmDiscardChanges, selection.scope]);

  const handleSelectProject = useCallback(
    (projectId: string) => {
      if (selection.scope === "project" && selection.projectId === projectId) return;
      if (!confirmDiscardChanges()) return;
      setSelection({ scope: "project", projectId });
    },
    [confirmDiscardChanges, selection]
  );

  const handleStartEdit = useCallback(() => {
    setDraftContent(document?.content_md ?? "");
    setEditorOpen(true);
  }, [document?.content_md]);

  const handleCancelEdit = useCallback(() => {
    setDraftContent(savedContent);
    setEditorOpen(false);
  }, [savedContent]);

  const resetToGlobalSelection = useCallback(() => {
    setSelection(emptyPromptSelection());
    setDocument(null);
    setSavedContent("");
    setDraftContent("");
    setEditorOpen(false);
    setDocumentDeleteOpen(false);
  }, []);

  const handleSaveDocument = useCallback(async () => {
    if (draftBytes > PROMPT_DOCUMENT_MAX_BYTES) {
      toast.error(t("errors.prompt_document_too_large"), {
        description: t("prompts.editor.size", {
          current: formatBytes(draftBytes),
          max: formatBytes(PROMPT_DOCUMENT_MAX_BYTES),
        }),
      });
      return;
    }

    setDocumentSaving(true);
    try {
      const next = await savePromptDocument({
        tool: activeTool,
        scope: selection.scope,
        project_id: selection.scope === "project" ? selection.projectId : null,
        content_md: draftContent,
        expected_updated_at_ms: document?.updated_at_ms ?? null,
      });
      setDocument(next);
      setSavedContent(next.content_md);
      setDraftContent(next.content_md);
      setEditorOpen(false);
      toast.success(t("prompts.toast.documentSaved"));
      if (selection.scope === "project") {
        void refreshProjects();
      }
    } catch (e) {
      toast.error(t("prompts.toast.saveDocumentFail"), {
        description: humanizeApiError(e, t),
      });
      maybeRecoverFromDocumentError(e, true);
    } finally {
      setDocumentSaving(false);
    }
  }, [
    activeTool,
    document?.updated_at_ms,
    draftBytes,
    draftContent,
    maybeRecoverFromDocumentError,
    refreshProjects,
    selection,
    t,
  ]);

  const handleRequestDeleteDocument = useCallback(() => {
    if (dirty && !confirmDiscardChanges()) return;
    setDocumentDeleteOpen(true);
  }, [confirmDiscardChanges, dirty]);

  const handleDeleteDocument = useCallback(async () => {
    setDocumentDeleting(true);
    try {
      await deletePromptDocument({
        tool: activeTool,
        scope: selection.scope,
        project_id: selection.scope === "project" ? selection.projectId : null,
        expected_updated_at_ms: document?.updated_at_ms ?? null,
      });
      setDocumentDeleteOpen(false);
      setSavedContent("");
      setDraftContent("");
      setEditorOpen(false);
      toast.success(t("prompts.toast.documentDeleted"));
      await refreshDocument({ quiet: true });
      if (selection.scope === "project") {
        void refreshProjects();
      }
    } catch (e) {
      toast.error(t("prompts.toast.deleteDocumentFail"), {
        description: humanizeApiError(e, t),
      });
      maybeRecoverFromDocumentError(e, false);
      setDocumentDeleteOpen(false);
    } finally {
      setDocumentDeleting(false);
    }
  }, [
    activeTool,
    document?.updated_at_ms,
    maybeRecoverFromDocumentError,
    refreshDocument,
    refreshProjects,
    selection,
    t,
  ]);

  return {
    activeTool,
    selection,
    projects,
    projectsLoading,
    selectedProject,
    document,
    documentLoading,
    editorOpen,
    dirty,
    draftContent,
    draftBytes,
    documentSaving,
    documentDeleting,
    documentDeleteOpen,
    handleToolChange,
    handleSelectGlobal,
    handleSelectProject,
    handleStartEdit,
    handleCancelEdit,
    handleSaveDocument,
    handleRequestDeleteDocument,
    handleDeleteDocument,
    refreshProjects,
    refreshDocument,
    resetToGlobalSelection,
    setDraftContent,
    setDocumentDeleteOpen,
  };
}
