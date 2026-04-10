import type { CliToolId, PromptProject } from "@/types/api";

export const PROMPT_DOCUMENT_MAX_BYTES = 256 * 1024;
export const PROMPT_TOOL_IDS: CliToolId[] = ["codex", "claude", "gemini"];

export type PromptSelection =
  | { scope: "global"; projectId: null }
  | { scope: "project"; projectId: string };

export function emptyPromptSelection(): PromptSelection {
  return { scope: "global", projectId: null };
}

export function findSelectedProject(
  projects: PromptProject[],
  selection: PromptSelection
): PromptProject | null {
  if (selection.scope !== "project") return null;
  return projects.find((item) => item.id === selection.projectId) ?? null;
}

export function measurePromptBytes(content: string): number {
  return new TextEncoder().encode(content).length;
}

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(bytes >= 10 * 1024 ? 0 : 1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}
