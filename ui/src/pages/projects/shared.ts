import claudeDarkIcon from "@/assets/protocol-icons/dark/claude.png";
import geminiDarkIcon from "@/assets/protocol-icons/dark/gemini.png";
import openaiDarkIcon from "@/assets/protocol-icons/dark/openai.png";
import claudeLightIcon from "@/assets/protocol-icons/light/claude.png";
import geminiLightIcon from "@/assets/protocol-icons/light/gemini.png";
import openaiLightIcon from "@/assets/protocol-icons/light/openai.png";
import type { CliToolId, ProjectRecord } from "@/types/api";

export const PROJECT_DOCUMENT_MAX_BYTES = 256 * 1024;
export const PROJECT_TOOL_IDS: CliToolId[] = ["codex", "claude", "gemini"];

export type ProjectSelection =
  | { scope: "global"; tool: CliToolId; projectId: null }
  | { scope: "project"; tool: CliToolId; projectId: string };

export type ProjectToolAvailability = Record<CliToolId, boolean>;
export type ProjectsByTool = Record<CliToolId, ProjectRecord[]>;
export type ProjectRow = ProjectRecord & {
  tools: ProjectToolAvailability;
  availableTools: CliToolId[];
};

function emptyToolAvailability(): ProjectToolAvailability {
  return {
    codex: false,
    claude: false,
    gemini: false,
  };
}

export function emptyProjectsByTool(): ProjectsByTool {
  return {
    codex: [],
    claude: [],
    gemini: [],
  };
}

export function createProjectSelection(
  tool: CliToolId,
  projectId?: string
): ProjectSelection {
  return projectId
    ? { scope: "project", tool, projectId }
    : { scope: "global", tool, projectId: null };
}

export function mergeProjectsByTool(projectsByTool: ProjectsByTool): ProjectRow[] {
  const projectMap = new Map<string, ProjectRow>();

  for (const tool of PROJECT_TOOL_IDS) {
    for (const project of projectsByTool[tool]) {
      const existing = projectMap.get(project.id);
      if (existing) {
        existing.tools[tool] = true;
        existing.availableTools.push(tool);
        existing.created_at_ms = Math.min(existing.created_at_ms, project.created_at_ms);
        existing.updated_at_ms = Math.max(existing.updated_at_ms, project.updated_at_ms);
        if (!existing.name.trim() && project.name.trim()) {
          existing.name = project.name;
        }
        continue;
      }

      const tools = emptyToolAvailability();
      tools[tool] = true;
      projectMap.set(project.id, {
        ...project,
        tools,
        availableTools: [tool],
      });
    }
  }

  return [...projectMap.values()].sort((left, right) => {
    return (
      right.updated_at_ms - left.updated_at_ms
      || left.name.localeCompare(right.name, undefined, { sensitivity: "base" })
      || left.path.localeCompare(right.path, undefined, { sensitivity: "base" })
    );
  });
}

export function findSelectedProject(
  projects: ProjectRow[],
  selection: ProjectSelection | null
): ProjectRow | null {
  if (!selection || selection.scope !== "project") return null;
  return (
    projects.find(
      (item) => item.id === selection.projectId && item.tools[selection.tool]
    ) ?? null
  );
}

export function measureDocumentBytes(content: string): number {
  return new TextEncoder().encode(content).length;
}

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(bytes >= 10 * 1024 ? 0 : 1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function toolIconSource(tool: CliToolId): { light: string; dark: string } {
  switch (tool) {
    case "codex":
      return { light: openaiLightIcon, dark: openaiDarkIcon };
    case "claude":
      return { light: claudeLightIcon, dark: claudeDarkIcon };
    case "gemini":
      return { light: geminiLightIcon, dark: geminiDarkIcon };
  }
}
