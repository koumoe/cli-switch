import { createFileRoute } from "@tanstack/react-router";

import { PromptsPage } from "@/pages/PromptsPage";

export const Route = createFileRoute("/prompts")({
  component: PromptsPage,
});
