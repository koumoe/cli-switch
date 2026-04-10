import React, {
  Suspense,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import type { ColumnDef } from "@tanstack/react-table";
import { toast } from "sonner";

import {
  Badge,
  Button,
  Card,
  CardContent,
  Tabs,
  TabsList,
  TabsTrigger,
} from "@/components/ui";
import { DataTable } from "@/components/composed/data-table";
import { PageHeader } from "@/components/PageHeader";
import { PageBody } from "@/components/layout/page-body";
import { deletePromptProject } from "@/api";
import { useI18n } from "@/hooks/use-i18n";
import { humanizeApiError } from "@/lib/error";

import { DeleteConfirmDialog } from "./DeleteConfirmDialog";
import { PROMPT_TOOL_IDS } from "./shared";
import { usePromptsPageState } from "./usePromptsPageState";

const PromptEditorDialog = React.lazy(() =>
  import("./PromptEditorDialog").then((m) => ({
    default: m.PromptEditorDialog,
  })),
);

export function PromptsPage() {
  const { t } = useI18n();
  const state = usePromptsPageState();

  const [editorDialogOpen, setEditorDialogOpen] = useState(false);
  const [closingAfterSave, setClosingAfterSave] = useState(false);
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState(20);
  const [projectDeleteOpen, setProjectDeleteOpen] = useState(false);
  const [projectDeleteTarget, setProjectDeleteTarget] = useState<
    (typeof state.projects)[number] | null
  >(null);
  const [projectDeleting, setProjectDeleting] = useState(false);
  const shouldAutoStartEdit = useRef(false);
  type PromptTableRow =
    | { id: string; kind: "global" }
    | { id: string; kind: "project"; project: (typeof state.projects)[number] };

  const tableRows = useMemo<PromptTableRow[]>(
    () => [
      { id: `${state.activeTool}-global`, kind: "global" },
      ...state.projects.map((project) => ({
        id: project.id,
        kind: "project" as const,
        project,
      })),
    ],
    [state.activeTool, state.projects],
  );

  const totalPages = useMemo(
    () => Math.max(1, Math.ceil(tableRows.length / pageSize)),
    [pageSize, tableRows.length],
  );

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
    [
      state.selection,
      state.editorOpen,
      state.handleSelectGlobal,
      state.handleSelectProject,
      state.handleStartEdit,
    ],
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
      state.selection.scope === "project" &&
      state.selection.projectId === projectDeleteTarget.id;
    if (
      isCurrentSelection &&
      state.dirty &&
      !window.confirm(t("prompts.editor.unsavedConfirm"))
    ) {
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
      toast.success(
        t("prompts.toast.projectDeleted", { name: projectDeleteTarget.name }),
      );
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
      : (state.selectedProject?.name ?? t("prompts.editor.noSelection"));
  const editorScopeLabel =
    state.selection.scope === "global"
      ? t("prompts.editor.globalBadge")
      : t("prompts.editor.projectBadge");
  const columns = useMemo<Array<ColumnDef<PromptTableRow>>>(
    () => [
      {
        id: "scope",
        header: t("prompts.sidebar.title"),
        cell: ({ row }) =>
          row.original.kind === "global" ? (
            <Badge variant="secondary">{t("prompts.editor.globalBadge")}</Badge>
          ) : (
            <Badge variant="outline">{t("prompts.editor.projectBadge")}</Badge>
          ),
        meta: {
          headerClassName: "w-24 text-left",
          cellClassName: "text-left",
          skeletonClassName: "w-16",
        },
      },
      {
        id: "name",
        header: t("prompts.projects.name"),
        accessorFn: (row) =>
          row.kind === "global" ? t("prompts.global.title") : row.project.name,
        cell: ({ row }) => (
          <span className="font-medium">
            {row.original.kind === "global"
              ? t("prompts.global.title")
              : row.original.project.name}
          </span>
        ),
        meta: {
          headerClassName: "w-48 text-left",
          cellClassName: "text-left",
          skeletonClassName: "w-36",
        },
      },
      {
        id: "path",
        header: t("prompts.projects.path"),
        accessorFn: (row) => (row.kind === "global" ? "" : row.project.path),
        cell: ({ row }) =>
          row.original.kind === "global" ? (
            <span className="text-xs text-muted-foreground">—</span>
          ) : (
            <div
              className="max-w-[300px] truncate font-mono text-xs text-muted-foreground"
              title={row.original.project.path}
            >
              {row.original.project.path}
            </div>
          ),
        meta: {
          headerClassName: "text-left",
          cellClassName: "text-left",
          skeletonClassName: "w-56",
        },
      },
      {
        id: "actions",
        header: t("common.actions"),
        cell: ({ row }) =>
          row.original.kind === "global" ? (
            <div className="flex items-center justify-center gap-1">
              <Button
                variant="outline"
                size="sm"
                className="h-8 min-w-20 text-xs"
                onClick={() => handleEditDocument("global")}
                title={t("prompts.editor.edit")}
              >
                {t("prompts.editor.edit")}
              </Button>
            </div>
          ) : (
            (() => {
              const project = row.original.project;
              return (
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
              );
            })()
          ),
        meta: {
          headerClassName: "w-52",
          cellClassName: "text-center",
          skeletonClassName: "w-32 ml-auto",
        },
      },
    ],
    [handleEditDocument, t],
  );

  return (
    <div className="flex h-full min-h-0 flex-col">
      <PageHeader title={t("prompts.title")} />
      <div className="flex-1 overflow-y-auto">
        <PageBody className="flex h-full min-h-0 flex-col gap-3">
          <Tabs
            value={state.activeTool}
            onValueChange={state.handleToolChange}
            className="flex flex-1 min-h-0 flex-col gap-3"
          >
            <TabsList className="animate-fade-up self-start">
              {PROMPT_TOOL_IDS.map((tool) => (
                <TabsTrigger key={tool} value={tool}>
                  {t(`prompts.tabs.${tool}`)}
                </TabsTrigger>
              ))}
            </TabsList>

            <div className="flex flex-1 min-h-0 flex-col">
              <Card className="animate-fade-up anim-d1 flex min-h-0 flex-1 flex-col overflow-hidden">
                <CardContent className="flex flex-1 min-h-0 flex-col p-0">
                  {!state.projectsLoading && state.projects.length === 0 ? (
                    <div className="border-b border-border px-4 py-3 text-sm text-muted-foreground">
                      {t("prompts.projects.empty", { tool: toolLabel })}
                    </div>
                  ) : null}
                  <DataTable
                    columns={columns}
                    data={tableRows}
                    loading={state.projectsLoading}
                    getRowId={(row) => row.id}
                    containerClassName="h-full overflow-y-auto"
                    pagination={{
                      page,
                      pageSize,
                      disabled: state.projectsLoading,
                      onPageChange: setPage,
                      onPageSizeChange: (next) => {
                        setPageSize(next);
                        setPage(1);
                      },
                    }}
                  />
                </CardContent>
              </Card>
            </div>
          </Tabs>
        </PageBody>
      </div>

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
              : (state.selectedProject?.name ??
                t("prompts.editor.noSelection")),
        })}
        confirmLabel={t("prompts.actions.deleteDocument")}
        busy={state.documentDeleting}
        onOpenChange={state.setDocumentDeleteOpen}
        onConfirm={state.handleDeleteDocument}
      />
    </div>
  );
}
