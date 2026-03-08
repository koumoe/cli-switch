import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { toast } from "sonner";

import type { CliToolId, PromptDocument, PromptProject } from "@/api";
import {
  createPromptProject,
  deletePromptDocument,
  deletePromptProject,
  getPromptDocument,
  listPromptProjects,
  pickFolder,
  savePromptDocument,
  updatePromptProject,
} from "@/api";
import { humanizeApiError, isApiRequestError } from "@/lib/error";
import { useI18n } from "@/lib/i18n";

import {
  PROMPT_DOCUMENT_MAX_BYTES,
  emptyPromptSelection,
  findSelectedProject,
  formatBytes,
  measurePromptBytes,
  type PromptSelection,
} from "./shared";

export type ProjectDialogState = {
  open: boolean;
  mode: "create" | "edit";
  projectId: string | null;
  name: string;
  path: string;
};

export const EMPTY_PROJECT_DIALOG: ProjectDialogState = {
  open: false,
  mode: "create",
  projectId: null,
  name: "",
  path: "",
};

export function usePromptsPageState() {
  const { t, locale } = useI18n();
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

  const [projectDialog, setProjectDialog] = useState<ProjectDialogState>(EMPTY_PROJECT_DIALOG);
  const [projectSaving, setProjectSaving] = useState(false);
  const [projectPicking, setProjectPicking] = useState(false);
  const [projectDeleteTarget, setProjectDeleteTarget] = useState<PromptProject | null>(null);
  const [projectDeleting, setProjectDeleting] = useState(false);

  const requestSeqRef = useRef(0);

  const selectedProject = useMemo(
    () => findSelectedProject(projects, selection),
    [projects, selection]
  );
  const dirty = editorOpen && draftContent !== savedContent;
  const draftBytes = useMemo(() => measurePromptBytes(draftContent), [draftContent]);

  const formatUpdatedAt = useCallback(
    (ts: number | null | undefined) => {
      if (!ts) return "-";
      try {
        return new Intl.DateTimeFormat(locale, {
          year: "numeric",
          month: "2-digit",
          day: "2-digit",
          hour: "2-digit",
          minute: "2-digit",
        }).format(new Date(ts));
      } catch {
        return new Date(ts).toLocaleString();
      }
    },
    [locale]
  );

  const confirmDiscardChanges = useCallback(() => {
    if (!dirty) return true;
    return window.confirm(t("prompts.editor.unsavedConfirm"));
  }, [dirty, t]);

  const refreshProjects = useCallback(async () => {
    setProjectsLoading(true);
    try {
      const next = await listPromptProjects();
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
  }, [t]);

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

  const handleOpenCreateProjectDialog = useCallback(() => {
    setProjectDialog({ ...EMPTY_PROJECT_DIALOG, open: true, mode: "create" });
  }, []);

  const handleOpenEditProjectDialog = useCallback((project: PromptProject) => {
    setProjectDialog({
      open: true,
      mode: "edit",
      projectId: project.id,
      name: project.name,
      path: project.path,
    });
  }, []);

  const handlePickFolder = useCallback(async () => {
    setProjectPicking(true);
    try {
      const res = await pickFolder({
        title: t("prompts.projectDialog.pickTitle"),
        directory: projectDialog.path || selectedProject?.path || undefined,
      });
      if (res.path) {
        setProjectDialog((prev) => ({ ...prev, path: res.path ?? prev.path }));
      }
    } catch (e) {
      toast.error(t("prompts.toast.pickFolderFail"), {
        description: humanizeApiError(e, t),
      });
    } finally {
      setProjectPicking(false);
    }
  }, [projectDialog.path, selectedProject?.path, t]);

  const handleSubmitProjectDialog = useCallback(async () => {
    setProjectSaving(true);
    try {
      if (projectDialog.mode === "create") {
        const created = await createPromptProject({
          name: projectDialog.name,
          path: projectDialog.path,
        });
        toast.success(t("prompts.toast.projectCreated"));
        await refreshProjects();
        setProjectDialog(EMPTY_PROJECT_DIALOG);
        if (!dirty) {
          setSelection({ scope: "project", projectId: created.id });
        }
      } else if (projectDialog.projectId) {
        await updatePromptProject(projectDialog.projectId, {
          name: projectDialog.name,
          path: projectDialog.path,
        });
        toast.success(t("prompts.toast.projectUpdated"));
        await refreshProjects();
        setProjectDialog(EMPTY_PROJECT_DIALOG);
      }
    } catch (e) {
      toast.error(t("prompts.toast.saveProjectFail"), {
        description: humanizeApiError(e, t),
      });
    } finally {
      setProjectSaving(false);
    }
  }, [dirty, projectDialog, refreshProjects, t]);

  const handleDeleteProject = useCallback(async () => {
    if (!projectDeleteTarget) return;
    setProjectDeleting(true);
    try {
      await deletePromptProject(projectDeleteTarget.id);
      setProjects((prev) => prev.filter((item) => item.id !== projectDeleteTarget.id));
      if (selection.scope === "project" && selection.projectId === projectDeleteTarget.id) {
        setSelection(emptyPromptSelection());
      }
      setProjectDeleteTarget(null);
      toast.success(t("prompts.toast.projectDeleted"));
    } catch (e) {
      toast.error(t("prompts.toast.deleteProjectFail"), {
        description: humanizeApiError(e, t),
      });
    } finally {
      setProjectDeleting(false);
    }
  }, [projectDeleteTarget, selection, t]);

  const handleStartEdit = useCallback(() => {
    setDraftContent(document?.content_md ?? "");
    setEditorOpen(true);
  }, [document?.content_md]);

  const handleCancelEdit = useCallback(() => {
    setDraftContent(savedContent);
    setEditorOpen(false);
  }, [savedContent]);

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
    draftContent,
    draftBytes,
    documentSaving,
    documentDeleting,
    documentDeleteOpen,
    projectDialog,
    projectSaving,
    projectPicking,
    projectDeleteTarget,
    projectDeleting,
    formatUpdatedAt,
    handleToolChange,
    handleSelectGlobal,
    handleSelectProject,
    handleOpenCreateProjectDialog,
    handleOpenEditProjectDialog,
    handlePickFolder,
    handleSubmitProjectDialog,
    handleDeleteProject,
    handleStartEdit,
    handleCancelEdit,
    handleSaveDocument,
    handleRequestDeleteDocument,
    handleDeleteDocument,
    setDraftContent,
    setProjectDialog,
    setProjectDeleteTarget,
    setDocumentDeleteOpen,
  };
}
