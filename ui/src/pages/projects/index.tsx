import React, { Suspense, useCallback, useMemo, useState } from "react";
import type { ColumnDef } from "@tanstack/react-table";
import { Trash2 } from "lucide-react";
import { toast } from "sonner";

import { deleteProject } from "@/api";
import { DataTable } from "@/components/composed/data-table";
import {
  TableActionGroup,
  TableIconButton,
} from "@/components/composed/table-primitives";
import { PageHeader } from "@/components/PageHeader";
import { PageBody } from "@/components/layout/page-body";
import { Card, CardContent } from "@/components/ui";
import { useI18n } from "@/hooks/use-i18n";
import { humanizeApiError } from "@/lib/error";
import { cn } from "@/lib/utils";

import { DeleteConfirmDialog } from "./DeleteConfirmDialog";
import { PROJECT_TOOL_IDS, toolIconSource, type ProjectRow } from "./shared";
import { useProjectsPageState } from "./useProjectsPageState";

const ProjectEditorDialog = React.lazy(() =>
  import("./ProjectEditorDialog").then((m) => ({
    default: m.ProjectEditorDialog,
  }))
);

function ToolActionButton({
  tool,
  enabled = true,
  title,
  onClick,
}: {
  tool: (typeof PROJECT_TOOL_IDS)[number];
  enabled?: boolean;
  title: string;
  onClick: () => void;
}) {
  const icon = toolIconSource(tool);

  return (
    <TableIconButton
      variant="ghost"
      className={cn(
        "h-8 w-8 rounded-md border p-0",
        enabled
          ? "border-border/80 text-foreground hover:border-primary/50 hover:bg-accent"
          : "border-transparent opacity-30"
      )}
      title={title}
      disabled={!enabled}
      onClick={onClick}
    >
      <img
        aria-hidden="true"
        alt=""
        src={icon.light}
        className="h-4 w-4 object-contain dark:hidden"
      />
      <img
        aria-hidden="true"
        alt=""
        src={icon.dark}
        className="hidden h-4 w-4 object-contain dark:block"
      />
    </TableIconButton>
  );
}

export function ProjectsPage() {
  const { t } = useI18n();
  const state = useProjectsPageState();

  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState(20);
  const [projectDeleteOpen, setProjectDeleteOpen] = useState(false);
  const [projectDeleteTarget, setProjectDeleteTarget] = useState<ProjectRow | null>(null);
  const [projectDeleting, setProjectDeleting] = useState(false);

  const totalPages = useMemo(
    () => Math.max(1, Math.ceil(state.projects.length / pageSize)),
    [pageSize, state.projects.length]
  );

  const editorTitle =
    state.selection?.scope === "global"
      ? t("projects.global.title")
      : (state.selectedProject?.name ?? t("projects.editor.noSelection"));
  const editorScopeLabel =
    state.selection?.scope === "global"
      ? t("projects.editor.globalBadge")
      : t("projects.editor.projectBadge");
  const editorToolLabel = state.selection
    ? t(`projects.tools.${state.selection.tool}`)
    : "";

  React.useEffect(() => {
    setPage((current) => Math.min(current, totalPages));
  }, [totalPages]);

  const handleDeleteProject = useCallback(async () => {
    if (!projectDeleteTarget) return;
    const isCurrentProjectSelected =
      state.selection?.scope === "project"
      && state.selection.projectId === projectDeleteTarget.id;

    if (isCurrentProjectSelected && state.dirty && !window.confirm(t("projects.editor.unsavedConfirm"))) {
      return;
    }

    setProjectDeleting(true);
    try {
      const results = await Promise.allSettled(
        projectDeleteTarget.availableTools.map((tool) =>
          deleteProject(tool, projectDeleteTarget.id).then(() => tool)
        )
      );

      const failedTools = results.flatMap((result, index) => {
        if (result.status === "fulfilled") return [];
        const tool = projectDeleteTarget.availableTools[index];
        return [`${t(`projects.tools.${tool}`)}: ${humanizeApiError(result.reason, t)}`];
      });

      await state.refreshProjects();

      if (failedTools.length === projectDeleteTarget.availableTools.length) {
        toast.error(t("projects.toast.deleteProjectFail"), {
          description: failedTools.join(" · "),
        });
      } else if (failedTools.length > 0) {
        toast.error(t("projects.toast.deleteProjectPartialFail"), {
          description: failedTools.join(" · "),
        });
      } else {
        toast.success(
          t("projects.toast.projectDeleted", { name: projectDeleteTarget.name })
        );
      }

      setProjectDeleteOpen(false);
      setProjectDeleteTarget(null);
    } catch (e) {
      toast.error(t("projects.toast.deleteProjectFail"), {
        description: humanizeApiError(e, t),
      });
    } finally {
      setProjectDeleting(false);
    }
  }, [projectDeleteTarget, state, t]);

  const columns = useMemo<Array<ColumnDef<ProjectRow>>>(
    () => [
      {
        id: "name",
        header: t("projects.table.name"),
        accessorFn: (row) => row.name,
        cell: ({ row }) => (
          <span
            className="mx-auto block max-w-[260px] truncate text-center font-medium"
            title={row.original.name}
          >
            {row.original.name}
          </span>
        ),
        meta: {
          headerClassName: "w-56",
          skeletonClassName: "w-36 mx-auto",
        },
      },
      {
        id: "path",
        header: t("projects.table.path"),
        accessorFn: (row) => row.path,
        cell: ({ row }) => (
          <div
            className="mx-auto max-w-[420px] truncate text-center font-mono text-xs text-muted-foreground"
            title={row.original.path}
          >
            {row.original.path}
          </div>
        ),
        meta: {
          skeletonClassName: "w-64 mx-auto",
        },
      },
      {
        id: "actions",
        header: t("common.actions"),
        cell: ({ row }) => (
          <TableActionGroup className="gap-1.5">
            {PROJECT_TOOL_IDS.map((tool) => (
              <ToolActionButton
                key={`${row.original.id}-${tool}`}
                tool={tool}
                enabled={row.original.tools[tool]}
                title={t("projects.actions.editDocument", {
                  tool: t(`projects.tools.${tool}`),
                })}
                onClick={() => {
                  state.openProjectDocument(tool, row.original.id);
                }}
              />
            ))}
            <TableIconButton
              variant="ghost"
              className="h-8 w-8 rounded-md border border-destructive/30 p-0 text-destructive hover:border-destructive hover:bg-destructive/10 hover:text-destructive"
              title={t("projects.actions.deleteProject")}
              onClick={() => {
                setProjectDeleteTarget(row.original);
                setProjectDeleteOpen(true);
              }}
            >
              <Trash2 className="h-4 w-4" />
            </TableIconButton>
          </TableActionGroup>
        ),
        meta: {
          headerClassName: "w-52",
          cellClassName: "text-center",
          skeletonClassName: "w-36 mx-auto",
        },
      },
    ],
    [state, t]
  );

  return (
    <div className="flex h-full min-h-0 flex-col">
      <PageHeader
        title={t("projects.title")}
        actions={
          <div className="flex items-center gap-2">
            <span className="text-xs text-muted-foreground">
              {t("projects.global.actionsLabel")}
            </span>
            {PROJECT_TOOL_IDS.map((tool) => (
              <ToolActionButton
                key={`global-${tool}`}
                tool={tool}
                title={t("projects.actions.editGlobal", {
                  tool: t(`projects.tools.${tool}`),
                })}
                onClick={() => {
                  state.openGlobalDocument(tool);
                }}
              />
            ))}
          </div>
        }
      />
      <div className="flex-1 overflow-y-auto">
        <PageBody className="flex h-full min-h-0 flex-col gap-3">
          <Card className="animate-fade-up flex min-h-0 flex-1 flex-col overflow-hidden">
            <CardContent className="flex flex-1 min-h-0 flex-col p-0">
              <DataTable
                columns={columns}
                data={state.projects}
                loading={state.projectsLoading}
                getRowId={(row) => row.id}
                containerClassName="h-full overflow-y-auto"
                emptyState={
                  <div className="px-4 py-2 text-sm text-muted-foreground">
                    {t("projects.table.empty")}
                  </div>
                }
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
        </PageBody>
      </div>

      {state.selection && (
        <Suspense fallback={null}>
          <ProjectEditorDialog
            open={!!state.selection}
            title={editorTitle}
            scopeLabel={editorScopeLabel}
            toolLabel={editorToolLabel}
            loading={state.documentLoading}
            draftContent={state.draftContent}
            draftBytes={state.draftBytes}
            saving={state.documentSaving}
            deleting={state.documentDeleting}
            documentExists={!!state.document?.exists}
            onDraftChange={state.setDraftContent}
            onRequestDelete={state.handleRequestDeleteDocument}
            onSave={async () => {
              const saved = await state.handleSaveDocument();
              if (saved) {
                state.closeEditor({ skipConfirm: true });
              }
            }}
            onClose={() => {
              state.closeEditor();
            }}
          />
        </Suspense>
      )}

      <DeleteConfirmDialog
        open={projectDeleteOpen}
        title={t("projects.projectDeleteDialog.title")}
        description={t("projects.projectDeleteDialog.description", {
          name: projectDeleteTarget?.name ?? "",
        })}
        confirmLabel={t("projects.actions.deleteProject")}
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
        title={t("projects.documentDeleteDialog.title")}
        description={t("projects.documentDeleteDialog.description", {
          name:
            state.selection?.scope === "global"
              ? t("projects.global.title")
              : (state.selectedProject?.name ?? t("projects.editor.noSelection")),
        })}
        confirmLabel={t("projects.actions.deleteDocument")}
        busy={state.documentDeleting}
        onOpenChange={state.setDocumentDeleteOpen}
        onConfirm={() => {
          void state.handleDeleteDocument();
        }}
      />
    </div>
  );
}
