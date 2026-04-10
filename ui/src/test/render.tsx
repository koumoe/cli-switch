import { useMemo, type ReactElement, type ReactNode } from "react";
import { render, type RenderOptions } from "@testing-library/react";

import { TooltipProvider } from "@/components/ui";
import { CurrencyProvider } from "@/providers/currency-provider";
import {
  I18nContext,
  translateForLocale,
  type Locale,
} from "@/providers/i18n-provider";

type ProvidersProps = {
  children: ReactNode;
  locale: Locale;
};

function TestProviders({ children, locale }: ProvidersProps) {
  const value = useMemo(
    () => ({
      locale,
      setLocale: () => undefined,
      t: (key: string, vars?: Record<string, string | number>) =>
        translateForLocale(locale, key, vars),
      locales: [
        { value: "zh-CN" as const, label: translateForLocale(locale, "language.zhCN") },
        { value: "en-US" as const, label: translateForLocale(locale, "language.enUS") },
      ],
    }),
    [locale],
  );

  return (
    <I18nContext.Provider value={value}>
      <CurrencyProvider>
        <TooltipProvider delayDuration={0}>{children}</TooltipProvider>
      </CurrencyProvider>
    </I18nContext.Provider>
  );
}

type ExtendedRenderOptions = Omit<RenderOptions, "wrapper"> & {
  locale?: Locale;
};

export function renderWithProviders(
  ui: ReactElement,
  { locale = "zh-CN", ...options }: ExtendedRenderOptions = {},
) {
  return render(ui, {
    wrapper: ({ children }) => <TestProviders locale={locale}>{children}</TestProviders>,
    ...options,
  });
}
