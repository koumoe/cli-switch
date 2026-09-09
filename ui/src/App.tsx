import { Outlet, useNavigate } from "@tanstack/react-router";
import { useLayoutEffect } from "react";
import { NuqsAdapter } from "nuqs/adapters/tanstack-router";

import { GlobalDialogs } from "@/components/composed/global-dialogs";
import { PageShell } from "@/components/layout/page-shell";
import { ScrollbarVisibilityController } from "@/components/scrollbar-visibility-controller";

export default function App() {
  const navigate = useNavigate();
  useLayoutEffect(() => {
    const openActivities = () => {
      void navigate({ to: "/activities" });
    };
    window.addEventListener("cliswitch-open-activities", openActivities);
    return () => window.removeEventListener("cliswitch-open-activities", openActivities);
  }, [navigate]);
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
