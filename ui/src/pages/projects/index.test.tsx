import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { ProjectDocument, ProjectRecord } from "@/types/api";
import { renderWithProviders } from "@/test/render";

import { ProjectsPage } from "./index";

const apiMocks = vi.hoisted(() => ({
  listProjects: vi.fn(),
  getProjectDocument: vi.fn(),
  deleteProject: vi.fn(),
  saveProjectDocument: vi.fn(),
  deleteProjectDocument: vi.fn(),
}));

vi.mock("@/api", () => ({
  getUsdCnyExchangeRate: vi.fn(async () => ({
    base_currency: "USD",
    quote_currency: "CNY",
    rate: 6.72,
    effective_date: "2026-08-28",
    source: "Frankfurter",
    fetched_at_ms: 1_777_000_000_000,
    stale: false,
  })),
  listProjects: apiMocks.listProjects,
  getProjectDocument: apiMocks.getProjectDocument,
  deleteProject: apiMocks.deleteProject,
  saveProjectDocument: apiMocks.saveProjectDocument,
  deleteProjectDocument: apiMocks.deleteProjectDocument,
}));

vi.mock("@mdxeditor/editor", async () => {
  const { createMdxEditorMock } = await import("@/test/mocks/mdxeditor");
  return createMdxEditorMock("project-page");
});

const project: ProjectRecord = {
  id: "project-1",
  name: "Demo Project",
  path: "/tmp/demo",
  created_at_ms: 1,
  updated_at_ms: 2,
};

const document: ProjectDocument = {
  tool: "codex",
  scope: "project",
  project_id: "project-1",
  content_md: "# Demo\n\n",
  exists: true,
  created_at_ms: 1,
  updated_at_ms: 2,
};

describe("ProjectsPage", () => {
  beforeEach(() => {
    apiMocks.listProjects.mockImplementation((tool: string) =>
      Promise.resolve(tool === "codex" ? [project] : [])
    );
    apiMocks.getProjectDocument.mockResolvedValue(document);
    apiMocks.deleteProject.mockResolvedValue(undefined);
    apiMocks.saveProjectDocument.mockResolvedValue(document);
    apiMocks.deleteProjectDocument.mockResolvedValue(undefined);
  });

  it("closes the editor without unsaved confirmation after initial markdown normalization", async () => {
    const user = userEvent.setup();
    const confirmSpy = vi.spyOn(window, "confirm");

    renderWithProviders(<ProjectsPage />);

    await screen.findByText("Demo Project");

    await user.click(screen.getByTitle("编辑 Codex 项目文档"));

    expect(await screen.findByRole("dialog")).toBeInTheDocument();
    await screen.findByTestId("editor-value");

    await user.click(screen.getByRole("button", { name: "normalize editor" }));
    await user.click(screen.getByRole("button", { name: "取消" }));

    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(confirmSpy).not.toHaveBeenCalled();
  });
});
