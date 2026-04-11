import { StrictMode } from "react";
import ReactDOM from "react-dom/client";
import { RouterProvider } from "@tanstack/react-router";
import { Toaster } from "sonner";
import { TooltipProvider } from "@/components/ui";
import "@/styles/globals.css";
import { AppProviders } from "@/providers";
import { router } from "@/router";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <AppProviders>
      <TooltipProvider delayDuration={200}>
        <RouterProvider router={router} />
        <Toaster position="top-center" richColors />
      </TooltipProvider>
    </AppProviders>
  </StrictMode>
);
