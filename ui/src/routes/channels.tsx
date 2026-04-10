import { createFileRoute } from "@tanstack/react-router";

import { ChannelsPage } from "@/pages/ChannelsPage";

export const Route = createFileRoute("/channels")({
  component: ChannelsPage,
});
