import { createFileRoute } from "@tanstack/react-router";

import { MonitorPage } from "@/pages/MonitorPage";

export const Route = createFileRoute("/monitor")({
  component: MonitorPage,
});
