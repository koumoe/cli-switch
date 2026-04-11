import type { ReactNode } from "react";

import { CurrencyProvider } from "@/providers/currency-provider";
import { I18nProvider } from "@/providers/i18n-provider";

export function AppProviders({ children }: { children: ReactNode }) {
  return (
    <I18nProvider>
      <CurrencyProvider>{children}</CurrencyProvider>
    </I18nProvider>
  );
}
