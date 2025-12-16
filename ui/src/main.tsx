import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { Toaster } from "sonner";
import "./index.css";
import { I18nProvider } from "@/lib/i18n";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <I18nProvider>
      <App />
      <Toaster position="bottom-right" richColors />
    </I18nProvider>
  </React.StrictMode>
);
