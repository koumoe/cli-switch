import { Outlet } from "@tanstack/react-router";
import { NuqsAdapter } from "nuqs/adapters/tanstack-router";

import { GlobalDialogs } from "@/components/composed/global-dialogs";
import { PageShell } from "@/components/layout/page-shell";
import { ScrollbarVisibilityController } from "@/components/scrollbar-visibility-controller";

export default function App() {
  return (
    <NuqsAdapter>
      <ScrollbarVisibilityController />
      <PageShell>
        <Outlet />
      </PageShell>
      <GlobalDialogs />
    </NuqsAdapter>
  );
}
