import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { Toaster } from "sonner";
import { TooltipProvider } from "@/components/ui";
import "./index.css";
import { I18nProvider } from "@/lib/i18n";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <I18nProvider>
      <TooltipProvider delayDuration={200}>
        <App />
        <Toaster position="bottom-right" richColors />
      </TooltipProvider>
    </I18nProvider>
  </React.StrictMode>
);
