import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { toast } from "sonner";

import {
  deleteProjectDocument,
  getProjectDocument,
  listProjects,
  saveProjectDocument,
} from "@/api";
import { useI18n } from "@/hooks/use-i18n";
import { humanizeApiError, isApiRequestError } from "@/lib/error";
import type { ProjectDocument } from "@/types/api";

import {
  PROJECT_DOCUMENT_MAX_BYTES,
  PROJECT_TOOL_IDS,
  createProjectSelection,
  emptyProjectsByTool,
  findSelectedProject,
  formatBytes,
  measureDocumentBytes,
  mergeProjectsByTool,
  type ProjectSelection,
} from "./shared";

export function useProjectsPageState() {
  const { t } = useI18n();
  const [projectsByTool, setProjectsByTool] = useState(() => emptyProjectsByTool());
  const [projectsLoading, setProjectsLoading] = useState(false);
  const [selection, setSelection] = useState<ProjectSelection | null>(null);

  const [document, setDocument] = useState<ProjectDocument | null>(null);
  const [documentLoading, setDocumentLoading] = useState(false);
  const [savedContent, setSavedContent] = useState("");
  const [draftContent, setDraftContent] = useState("");
  const [documentSaving, setDocumentSaving] = useState(false);
  const [documentDeleting, setDocumentDeleting] = useState(false);
  const [documentDeleteOpen, setDocumentDeleteOpen] = useState(false);

  const requestSeqRef = useRef(0);

  const projects = useMemo(
    () => mergeProjectsByTool(projectsByTool),
    [projectsByTool]
  );
  const selectedProject = useMemo(
    () => findSelectedProject(projects, selection),
    [projects, selection]
  );
  const dirty = selection !== null && draftContent !== savedContent;
  const draftBytes = useMemo(() => measureDocumentBytes(draftContent), [draftContent]);

  const clearEditorState = useCallback(() => {
    requestSeqRef.current += 1;
    setSelection(null);
    setDocument(null);
    setDocumentLoading(false);
    setSavedContent("");
    setDraftContent("");
    setDocumentDeleteOpen(false);
  }, []);

  const confirmDiscardChanges = useCallback(() => {
    if (!dirty) return true;
    return window.confirm(t("projects.editor.unsavedConfirm"));
  }, [dirty, t]);

  const refreshProjects = useCallback(async () => {
    setProjectsLoading(true);
    try {
      const results = await Promise.allSettled(
        PROJECT_TOOL_IDS.map(async (tool) => ({
          tool,
          projects: await listProjects(tool),
        }))
      );

      const nextProjectsByTool = emptyProjectsByTool();
      const failedTools: string[] = [];

      results.forEach((result, index) => {
        const tool = PROJECT_TOOL_IDS[index];
        if (result.status === "fulfilled") {
          nextProjectsByTool[result.value.tool] = result.value.projects;
          return;
        }

        failedTools.push(`${t(`projects.tools.${tool}`)}: ${humanizeApiError(result.reason, t)}`);
      });

      const nextProjects = mergeProjectsByTool(nextProjectsByTool);
      setProjectsByTool(nextProjectsByTool);
      setSelection((current) => {
        if (!current || current.scope !== "project") return current;
        const stillExists = nextProjects.some(
          (project) =>
            project.id === current.projectId && project.tools[current.tool]
        );
        return stillExists ? current : null;
      });

      if (failedTools.length > 0) {
        toast.error(t("projects.toast.loadProjectsFail"), {
          description: failedTools.join(" · "),
        });
      }
    } finally {
      setProjectsLoading(false);
    }
  }, [t]);

  const refreshDocument = useCallback(
    async (opts?: { preserveDraft?: boolean; quiet?: boolean }) => {
      if (!selection) return;

      const requestSeq = ++requestSeqRef.current;
      setDocumentLoading(true);

      try {
        const next = await getProjectDocument({
          tool: selection.tool,
          scope: selection.scope,
          project_id: selection.scope === "project" ? selection.projectId : null,
        });

        if (requestSeq !== requestSeqRef.current) return;

        setDocument(next);
        setSavedContent(next.content_md);
        if (!opts?.preserveDraft) {
          setDraftContent(next.content_md);
        }
      } catch (e) {
        if (requestSeq !== requestSeqRef.current) return;
        if (isApiRequestError(e) && e.code === "project_not_found") {
          clearEditorState();
          return;
        }
        if (!opts?.quiet) {
          toast.error(t("projects.toast.loadDocumentFail"), {
            description: humanizeApiError(e, t),
          });
        }
      } finally {
        if (requestSeq === requestSeqRef.current) {
          setDocumentLoading(false);
        }
      }
    },
    [clearEditorState, selection, t]
  );

  useEffect(() => {
    void refreshProjects();
  }, [refreshProjects]);

  useEffect(() => {
    if (!selection) {
      requestSeqRef.current += 1;
      setDocument(null);
      setDocumentLoading(false);
      setSavedContent("");
      setDraftContent("");
      setDocumentDeleteOpen(false);
      return;
    }

    void refreshDocument();
  }, [refreshDocument, selection]);

  const maybeRecoverFromDocumentError = useCallback(
    (err: unknown, preserveDraft = true) => {
      if (
        isApiRequestError(err) &&
        [
          "project_document_version_conflict",
          "project_document_not_found",
          "project_not_found",
        ].includes(err.code ?? "")
      ) {
        void refreshDocument({
          preserveDraft,
          quiet: true,
        });
      }
    },
    [refreshDocument]
  );

  const openGlobalDocument = useCallback(
    (tool: ProjectSelection["tool"]) => {
      const nextSelection = createProjectSelection(tool);
      const sameSelection =
        selection?.scope === nextSelection.scope
        && selection.tool === nextSelection.tool;

      if (sameSelection) return true;
      if (!confirmDiscardChanges()) return false;

      setSelection(nextSelection);
      setDocument(null);
      setSavedContent("");
      setDraftContent("");
      setDocumentDeleteOpen(false);
      return true;
    },
    [confirmDiscardChanges, selection]
  );

  const openProjectDocument = useCallback(
    (tool: ProjectSelection["tool"], projectId: string) => {
      const nextSelection = createProjectSelection(tool, projectId);
      const sameSelection =
        selection?.scope === nextSelection.scope
        && selection.tool === nextSelection.tool
        && selection.projectId === nextSelection.projectId;

      if (sameSelection) return true;
      if (!confirmDiscardChanges()) return false;

      setSelection(nextSelection);
      setDocument(null);
      setSavedContent("");
      setDraftContent("");
      setDocumentDeleteOpen(false);
      return true;
    },
    [confirmDiscardChanges, selection]
  );

  const closeEditor = useCallback(
    (opts?: { skipConfirm?: boolean }) => {
      if (!opts?.skipConfirm && !confirmDiscardChanges()) {
        return false;
      }
      clearEditorState();
      return true;
    },
    [clearEditorState, confirmDiscardChanges]
  );

  const handleSaveDocument = useCallback(async () => {
    if (!selection) return false;

    if (draftBytes > PROJECT_DOCUMENT_MAX_BYTES) {
      toast.error(t("errors.project_document_too_large"), {
        description: t("projects.editor.size", {
          current: formatBytes(draftBytes),
          max: formatBytes(PROJECT_DOCUMENT_MAX_BYTES),
        }),
      });
      return false;
    }

    setDocumentSaving(true);
    try {
      const next = await saveProjectDocument({
        tool: selection.tool,
        scope: selection.scope,
        project_id: selection.scope === "project" ? selection.projectId : null,
        content_md: draftContent,
        expected_updated_at_ms: document?.updated_at_ms ?? null,
      });
      setDocument(next);
      setSavedContent(next.content_md);
      setDraftContent(next.content_md);
      if (selection.scope === "project") {
        await refreshProjects();
      }
      toast.success(t("projects.toast.documentSaved"));
      return true;
    } catch (e) {
      toast.error(t("projects.toast.saveDocumentFail"), {
        description: humanizeApiError(e, t),
      });
      maybeRecoverFromDocumentError(e, true);
      return false;
    } finally {
      setDocumentSaving(false);
    }
  }, [
    document?.updated_at_ms,
    draftBytes,
    draftContent,
    maybeRecoverFromDocumentError,
    refreshProjects,
    selection,
    t,
  ]);

  const handleRequestDeleteDocument = useCallback(() => {
    if (!selection) return;
    if (dirty && !confirmDiscardChanges()) return;
    setDocumentDeleteOpen(true);
  }, [confirmDiscardChanges, dirty, selection]);

  const handleDeleteDocument = useCallback(async () => {
    if (!selection) return false;

    setDocumentDeleting(true);
    try {
      await deleteProjectDocument({
        tool: selection.tool,
        scope: selection.scope,
        project_id: selection.scope === "project" ? selection.projectId : null,
        expected_updated_at_ms: document?.updated_at_ms ?? null,
      });
      setDocumentDeleteOpen(false);
      setSavedContent("");
      setDraftContent("");
      toast.success(t("projects.toast.documentDeleted"));
      if (selection.scope === "project") {
        await refreshProjects();
      }
      await refreshDocument({ quiet: true });
      return true;
    } catch (e) {
      toast.error(t("projects.toast.deleteDocumentFail"), {
        description: humanizeApiError(e, t),
      });
      maybeRecoverFromDocumentError(e, false);
      setDocumentDeleteOpen(false);
      return false;
    } finally {
      setDocumentDeleting(false);
    }
  }, [
    document?.updated_at_ms,
    maybeRecoverFromDocumentError,
    refreshDocument,
    refreshProjects,
    selection,
    t,
  ]);

  return {
    projects,
    projectsLoading,
    selection,
    selectedProject,
    document,
    documentLoading,
    dirty,
    draftContent,
    draftBytes,
    documentSaving,
    documentDeleting,
    documentDeleteOpen,
    openGlobalDocument,
    openProjectDocument,
    closeEditor,
    handleSaveDocument,
    handleRequestDeleteDocument,
    handleDeleteDocument,
    refreshProjects,
    setDraftContent,
    setDocumentDeleteOpen,
  };
}
