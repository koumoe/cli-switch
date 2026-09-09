import { createFileRoute } from "@tanstack/react-router";
import { ActivitiesPage } from "@/pages/ActivitiesPage";

export const Route = createFileRoute("/activities")({
  component: ActivitiesPage,
});
