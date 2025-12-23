import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { Toaster } from "sonner";
import { TooltipProvider } from "@/components/ui";
import "./index.css";
import { I18nProvider } from "@/lib/i18n";
import { CurrencyProvider } from "@/lib/currency";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <I18nProvider>
      <CurrencyProvider>
        <TooltipProvider delayDuration={200}>
          <App />
          <Toaster position="bottom-right" richColors />
        </TooltipProvider>
      </CurrencyProvider>
    </I18nProvider>
  </React.StrictMode>
);
