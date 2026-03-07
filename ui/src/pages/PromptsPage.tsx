import React, { Suspense, useCallback, useEffect, useRef, useState } from "react";
import { Pencil, Plus, Settings2, Trash2 } from "lucide-react";

import {
  Badge,
  Button,
  Card,
  CardContent,
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
import { useI18n } from "@/lib/i18n";

import { DeleteConfirmDialog } from "./prompts/DeleteConfirmDialog";
import { ProjectDialog } from "./prompts/ProjectDialog";
import { PROMPT_TOOL_IDS } from "./prompts/shared";
import { EMPTY_PROJECT_DIALOG, usePromptsPageState } from "./prompts/usePromptsPageState";

const PromptEditorDialog = React.lazy(() =>
  import("./prompts/PromptEditorDialog").then((m) => ({ default: m.PromptEditorDialog }))
);

export function PromptsPage() {
  const { t } = useI18n();
  const state = usePromptsPageState();

  const [editorDialogOpen, setEditorDialogOpen] = useState(false);
  const [closingAfterSave, setClosingAfterSave] = useState(false);
  const shouldAutoStartEdit = useRef(false);

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
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-lg font-semibold">{t("prompts.title")}</h1>
          <p className="text-muted-foreground text-xs mt-0.5">{t("prompts.subtitle")}</p>
        </div>
        <Button size="sm" onClick={state.handleOpenCreateProjectDialog}>
          <Plus className="h-4 w-4 mr-2" />
          {t("prompts.projects.add")}
        </Button>
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
          <Card>
            <CardContent className="p-0">
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead className="w-24">{t("prompts.sidebar.title")}</TableHead>
                    <TableHead className="w-48">{t("prompts.projects.name")}</TableHead>
                    <TableHead>{t("prompts.projects.path")}</TableHead>
                    <TableHead className="w-28">{t("common.actions")}</TableHead>
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
                    <TableCell>
                      <Button
                        variant="ghost"
                        size="icon"
                        onClick={() => handleEditDocument("global")}
                        title={t("prompts.editor.edit")}
                      >
                        <Pencil className="h-4 w-4" />
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
                        {t("prompts.projects.empty")}
                      </TableCell>
                    </TableRow>
                  ) : (
                    state.projects.map((project) => (
                      <TableRow key={project.id}>
                        <TableCell>
                          <Badge variant="outline">{t("prompts.editor.projectBadge")}</Badge>
                        </TableCell>
                        <TableCell className="font-medium">{project.name}</TableCell>
                        <TableCell className="font-mono text-xs text-muted-foreground truncate max-w-[300px]">
                          {project.path}
                        </TableCell>
                        <TableCell>
                          <div className="flex items-center gap-1">
                            <Button
                              variant="ghost"
                              size="icon"
                              onClick={() => handleEditDocument("project", project.id)}
                              title={t("prompts.editor.edit")}
                            >
                              <Pencil className="h-4 w-4" />
                            </Button>
                            <Button
                              variant="ghost"
                              size="icon"
                              onClick={() => state.handleOpenEditProjectDialog(project)}
                              title={t("prompts.projects.edit")}
                            >
                              <Settings2 className="h-4 w-4" />
                            </Button>
                            <Button
                              variant="ghost"
                              size="icon"
                              onClick={() => state.setProjectDeleteTarget(project)}
                              title={t("prompts.projects.delete")}
                            >
                              <Trash2 className="h-4 w-4 text-destructive" />
                            </Button>
                          </div>
                        </TableCell>
                      </TableRow>
                    ))
                  )}
                </TableBody>
              </Table>
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
            onDraftChange={state.setDraftContent}
            onStartEdit={state.handleStartEdit}
            onSave={handleEditorSave}
            onClose={handleEditorDialogClose}
          />
        </Suspense>
      )}

      {/* Project CRUD dialogs */}
      <ProjectDialog
        open={state.projectDialog.open}
        mode={state.projectDialog.mode}
        name={state.projectDialog.name}
        path={state.projectDialog.path}
        saving={state.projectSaving}
        picking={state.projectPicking}
        onOpenChange={(open) => {
          if (state.projectSaving) return;
          state.setProjectDialog(open ? state.projectDialog : EMPTY_PROJECT_DIALOG);
        }}
        onNameChange={(value) => state.setProjectDialog((prev) => ({ ...prev, name: value }))}
        onPathChange={(value) => state.setProjectDialog((prev) => ({ ...prev, path: value }))}
        onPickFolder={state.handlePickFolder}
        onSubmit={state.handleSubmitProjectDialog}
      />

      <DeleteConfirmDialog
        open={!!state.projectDeleteTarget}
        title={t("prompts.projects.deleteConfirmTitle")}
        description={t("prompts.projects.deleteConfirmDescription", {
          name: state.projectDeleteTarget?.name ?? "-",
        })}
        busy={state.projectDeleting}
        onOpenChange={(open) => !open && state.setProjectDeleteTarget(null)}
        onConfirm={state.handleDeleteProject}
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
