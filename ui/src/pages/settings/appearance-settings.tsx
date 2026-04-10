import type { ElementType } from "react";

import {
  Button,
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui";
import { useI18n } from "@/hooks/use-i18n";
import type { Theme } from "@/hooks/use-theme";
import type { CurrencyMode } from "@/providers/currency-provider";
import type { Locale } from "@/types/locale";
import { SettingsFieldText, SettingsRow, SettingsSection } from "./settings-layout";

type ThemeOption = {
  value: Theme;
  label: string;
  icon: ElementType;
};

type LocaleOption = {
  value: Locale;
  label: string;
};

type AppearanceSettingsProps = {
  theme: Theme;
  themeOptions: ThemeOption[];
  onThemeChange: (theme: Theme) => void;
  locale: Locale;
  locales: LocaleOption[];
  onLocaleChange: (locale: Locale) => void | Promise<void>;
  currencyMode: CurrencyMode;
  onCurrencyModeChange: (mode: CurrencyMode) => void;
};

export function AppearanceSettings({
  theme,
  themeOptions,
  onThemeChange,
  locale,
  locales,
  onLocaleChange,
  currencyMode,
  onCurrencyModeChange,
}: AppearanceSettingsProps) {
  const { t } = useI18n();

  return (
    <>
      <SettingsSection title={t("settings.appearance.title")} first>
        <SettingsRow>
          <SettingsFieldText
            label={t("settings.appearance.theme")}
            hint={t("settings.appearance.themeHint")}
          />
          <div className="flex shrink-0 items-center gap-2">
            {themeOptions.map((option) => {
              const Icon = option.icon;
              const active = theme === option.value;
              return (
                <Button
                  key={option.value}
                  variant={active ? "default" : "outline"}
                  size="sm"
                  onClick={() => onThemeChange(option.value)}
                  className="gap-1.5"
                >
                  <Icon className="h-3.5 w-3.5" />
                  {option.label}
                </Button>
              );
            })}
          </div>
        </SettingsRow>

        <SettingsRow>
          <SettingsFieldText
            label={t("settings.language.label")}
            hint={t("settings.language.subtitle")}
          />
          <div className="w-[220px] shrink-0">
            <Select value={locale} onValueChange={(value) => void onLocaleChange(value as Locale)}>
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {locales.map((item) => (
                  <SelectItem key={item.value} value={item.value}>
                    {item.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
        </SettingsRow>

        <SettingsRow>
          <SettingsFieldText
            label={t("settings.currency.label")}
            hint={t("settings.currency.subtitle")}
          />
          <div className="w-[220px] shrink-0">
            <Select
              value={currencyMode}
              onValueChange={(value) => onCurrencyModeChange(value as CurrencyMode)}
            >
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="auto">{t("settings.currency.options.auto")}</SelectItem>
                <SelectItem value="CNY">{t("settings.currency.options.cny")}</SelectItem>
                <SelectItem value="USD">{t("settings.currency.options.usd")}</SelectItem>
              </SelectContent>
            </Select>
          </div>
        </SettingsRow>
      </SettingsSection>
    </>
  );
}
