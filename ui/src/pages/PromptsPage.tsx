import React, { Suspense, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { toast } from "sonner";

import {
  Badge,
  Button,
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
  Tabs,
  TabsList,
  TabsTrigger,
} from "@/components/ui";
import { PaginationBar } from "@/components/PaginationBar";
import { deletePromptProject } from "@/api";
import { humanizeApiError } from "@/lib/error";
import { useI18n } from "@/lib/i18n";

import { DeleteConfirmDialog } from "./prompts/DeleteConfirmDialog";
import { PROMPT_TOOL_IDS } from "./prompts/shared";
import { usePromptsPageState } from "./prompts/usePromptsPageState";

const PromptEditorDialog = React.lazy(() =>
  import("./prompts/PromptEditorDialog").then((m) => ({ default: m.PromptEditorDialog }))
);

export function PromptsPage() {
  const { t } = useI18n();
  const state = usePromptsPageState();

  const [editorDialogOpen, setEditorDialogOpen] = useState(false);
  const [closingAfterSave, setClosingAfterSave] = useState(false);
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState(20);
  const [projectDeleteOpen, setProjectDeleteOpen] = useState(false);
  const [projectDeleteTarget, setProjectDeleteTarget] = useState<(typeof state.projects)[number] | null>(null);
  const [projectDeleting, setProjectDeleting] = useState(false);
  const shouldAutoStartEdit = useRef(false);

  const totalPages = useMemo(
    () => Math.max(1, Math.ceil(state.projects.length / pageSize)),
    [pageSize, state.projects.length]
  );
  const currentPage = Math.min(page, totalPages);

  const pagedProjects = useMemo(() => {
    const start = (currentPage - 1) * pageSize;
    return state.projects.slice(start, start + pageSize);
  }, [currentPage, pageSize, state.projects]);

  // Auto-start edit after document loads when switching scope via edit button
  useEffect(() => {
    if (shouldAutoStartEdit.current && !state.documentLoading) {
      state.handleStartEdit();
      shouldAutoStartEdit.current = false;
    }
  }, [state.documentLoading]);

  // Close dialog after successful save
  useEffect(() => {
    if (closingAfterSave && !state.documentSaving) {
      if (!state.editorOpen) {
        // Save succeeded — editorOpen was set to false by hook
        setEditorDialogOpen(false);
      }
      setClosingAfterSave(false);
    }
  }, [closingAfterSave, state.documentSaving, state.editorOpen]);

  useEffect(() => {
    setPage(1);
  }, [state.activeTool]);

  useEffect(() => {
    setPage((current) => Math.min(current, totalPages));
  }, [totalPages]);

  const handleEditDocument = useCallback(
    (scope: "global" | "project", projectId?: string) => {
      const isSameSelection =
        (scope === "global" && state.selection.scope === "global") ||
        (scope === "project" &&
          state.selection.scope === "project" &&
          state.selection.projectId === projectId);

      if (isSameSelection) {
        if (!state.editorOpen) {
          state.handleStartEdit();
        }
      } else {
        if (scope === "global") {
          state.handleSelectGlobal();
        } else if (projectId) {
          state.handleSelectProject(projectId);
        }
        shouldAutoStartEdit.current = true;
      }
      setEditorDialogOpen(true);
    },
    [state.selection, state.editorOpen, state.handleSelectGlobal, state.handleSelectProject, state.handleStartEdit]
  );

  const handleEditorDialogClose = useCallback(() => {
    if (state.editorOpen) {
      state.handleCancelEdit();
    }
    shouldAutoStartEdit.current = false;
    setClosingAfterSave(false);
    setEditorDialogOpen(false);
  }, [state.editorOpen, state.handleCancelEdit]);

  const handleEditorSave = useCallback(() => {
    state.handleSaveDocument();
    setClosingAfterSave(true);
  }, [state.handleSaveDocument]);

  const handleDeleteProject = useCallback(async () => {
    if (!projectDeleteTarget) return;

    const isCurrentSelection =
      state.selection.scope === "project" && state.selection.projectId === projectDeleteTarget.id;
    if (isCurrentSelection && state.dirty && !window.confirm(t("prompts.editor.unsavedConfirm"))) {
      return;
    }

    setProjectDeleting(true);
    try {
      await deletePromptProject(state.activeTool, projectDeleteTarget.id);
      if (isCurrentSelection) {
        handleEditorDialogClose();
        state.resetToGlobalSelection();
      }
      await state.refreshProjects();
      toast.success(t("prompts.toast.projectDeleted", { name: projectDeleteTarget.name }));
      setProjectDeleteOpen(false);
      setProjectDeleteTarget(null);
    } catch (e) {
      toast.error(t("prompts.toast.deleteProjectFail"), {
        description: humanizeApiError(e, t),
      });
    } finally {
      setProjectDeleting(false);
    }
  }, [
    handleEditorDialogClose,
    projectDeleteTarget,
    state.activeTool,
    state.dirty,
    state.refreshProjects,
    state.resetToGlobalSelection,
    state.selection.projectId,
    state.selection.scope,
    t,
  ]);

  const toolLabel = t(`prompts.tabs.${state.activeTool}`);
  const editorTitle =
    state.selection.scope === "global"
      ? t("prompts.global.title")
      : state.selectedProject?.name ?? t("prompts.editor.noSelection");
  const editorScopeLabel =
    state.selection.scope === "global"
      ? t("prompts.editor.globalBadge")
      : t("prompts.editor.projectBadge");

  return (
    <div className="space-y-4 pb-4">
      <div>
        <h1 className="text-lg font-semibold">{t("prompts.title")}</h1>
        <p className="text-muted-foreground text-xs mt-0.5">{t("prompts.subtitle")}</p>
      </div>

      <Tabs value={state.activeTool} onValueChange={state.handleToolChange}>
        <TabsList>
          {PROMPT_TOOL_IDS.map((tool) => (
            <TabsTrigger key={tool} value={tool}>
              {t(`prompts.tabs.${tool}`)}
            </TabsTrigger>
          ))}
        </TabsList>

        <div className="mt-4">
          <Card className="flex flex-col">
            <CardHeader>
              <div className="flex items-start justify-between gap-4">
                <div>
                  <CardTitle>{t("prompts.projects.title")}</CardTitle>
                  <CardDescription>{t("prompts.projects.subtitle")}</CardDescription>
                </div>
                <div className="text-xs text-muted-foreground">
                  {t("common.pagination.total", { total: state.projects.length.toLocaleString() })}
                </div>
              </div>
            </CardHeader>
            <CardContent className="flex min-h-0 flex-col p-0">
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead className="w-24">{t("prompts.sidebar.title")}</TableHead>
                    <TableHead className="w-48">{t("prompts.projects.name")}</TableHead>
                    <TableHead>{t("prompts.projects.path")}</TableHead>
                    <TableHead className="w-52">{t("common.actions")}</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {/* Global prompt — pinned at top */}
                  <TableRow>
                    <TableCell>
                      <Badge variant="secondary">{t("prompts.editor.globalBadge")}</Badge>
                    </TableCell>
                    <TableCell className="font-medium">{t("prompts.global.title")}</TableCell>
                    <TableCell className="text-muted-foreground text-xs">—</TableCell>
                    <TableCell className="text-center">
                      <Button
                        variant="outline"
                        size="sm"
                        className="h-8 min-w-20 text-xs"
                        onClick={() => handleEditDocument("global")}
                        title={t("prompts.editor.edit")}
                      >
                        {t("prompts.editor.edit")}
                      </Button>
                    </TableCell>
                  </TableRow>

                  {/* Project rows */}
                  {state.projectsLoading ? (
                    <TableRow>
                      <TableCell colSpan={4} className="text-center text-muted-foreground py-8">
                        {t("common.loading")}
                      </TableCell>
                    </TableRow>
                  ) : state.projects.length === 0 ? (
                    <TableRow>
                      <TableCell colSpan={4} className="text-center text-muted-foreground py-8">
                        {t("prompts.projects.empty", { tool: toolLabel })}
                      </TableCell>
                    </TableRow>
                  ) : (
                    pagedProjects.map((project) => (
                      <TableRow key={project.id}>
                        <TableCell>
                          <Badge variant="outline">{t("prompts.editor.projectBadge")}</Badge>
                        </TableCell>
                        <TableCell className="font-medium">{project.name}</TableCell>
                        <TableCell className="font-mono text-xs text-muted-foreground truncate max-w-[300px]">
                          {project.path}
                        </TableCell>
                        <TableCell>
                          <div className="flex items-center justify-center gap-1">
                            <Button
                              variant="outline"
                              size="sm"
                              className="h-8 min-w-20 text-xs"
                              onClick={() => handleEditDocument("project", project.id)}
                              title={t("prompts.editor.edit")}
                            >
                              {t("prompts.editor.edit")}
                            </Button>
                            <Button
                              variant="outline"
                              size="sm"
                              className="h-8 min-w-20 border-destructive/40 text-xs text-destructive hover:border-destructive hover:bg-destructive/10 hover:text-destructive"
                              onClick={() => {
                                setProjectDeleteTarget(project);
                                setProjectDeleteOpen(true);
                              }}
                              title={t("prompts.actions.deleteProject")}
                            >
                              {t("prompts.actions.deleteProject")}
                            </Button>
                          </div>
                        </TableCell>
                      </TableRow>
                    ))
                  )}
                </TableBody>
              </Table>
              <PaginationBar
                page={currentPage}
                total={state.projects.length}
                totalPages={totalPages}
                pageSize={pageSize}
                disabled={state.projectsLoading}
                onPageChange={setPage}
                onPageSizeChange={(next) => {
                  setPageSize(next);
                  setPage(1);
                }}
              />
            </CardContent>
          </Card>
        </div>
      </Tabs>

      {/* Markdown editor dialog */}
      {editorDialogOpen && (
        <Suspense fallback={null}>
          <PromptEditorDialog
            open={editorDialogOpen}
            title={editorTitle}
            scopeLabel={editorScopeLabel}
            toolLabel={toolLabel}
            document={state.document}
            loading={state.documentLoading}
            editorOpen={state.editorOpen}
            draftContent={state.draftContent}
            draftBytes={state.draftBytes}
            saving={state.documentSaving}
            deleting={state.documentDeleting}
            documentExists={!!state.document?.exists}
            onDraftChange={state.setDraftContent}
            onStartEdit={state.handleStartEdit}
            onRequestDelete={state.handleRequestDeleteDocument}
            onSave={handleEditorSave}
            onClose={handleEditorDialogClose}
          />
        </Suspense>
      )}

      <DeleteConfirmDialog
        open={projectDeleteOpen}
        title={t("prompts.projectDeleteDialog.title")}
        description={t("prompts.projectDeleteDialog.description", {
          name: projectDeleteTarget?.name ?? "",
        })}
        confirmLabel={t("prompts.actions.deleteProject")}
        busy={projectDeleting}
        onOpenChange={(open) => {
          setProjectDeleteOpen(open);
          if (!open) {
            setProjectDeleteTarget(null);
          }
        }}
        onConfirm={handleDeleteProject}
      />

      <DeleteConfirmDialog
        open={state.documentDeleteOpen}
        title={t("prompts.documentDeleteDialog.title")}
        description={t("prompts.documentDeleteDialog.description", {
          name:
            state.selection.scope === "global"
              ? t("prompts.global.title")
              : state.selectedProject?.name ?? t("prompts.editor.noSelection"),
        })}
        confirmLabel={t("prompts.actions.deleteDocument")}
        busy={state.documentDeleting}
        onOpenChange={state.setDocumentDeleteOpen}
        onConfirm={state.handleDeleteDocument}
      />
    </div>
  );
}
