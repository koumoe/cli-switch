import type { ElementType } from "react";
import { DollarSign, Languages, Monitor, Moon, Sun } from "lucide-react";

import {
  Button,
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
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
    <div className="space-y-4">
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Sun className="h-4 w-4" />
            {t("settings.appearance.title")}
          </CardTitle>
          <CardDescription>{t("settings.appearance.subtitle")}</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex items-center justify-between">
            <div>
              <div className="text-sm font-medium">{t("settings.appearance.theme")}</div>
              <div className="text-xs text-muted-foreground">
                {t("settings.appearance.themeHint")}
              </div>
            </div>
            <div className="flex gap-2">
              {themeOptions.map((option) => {
                const Icon = option.icon;
                const active = theme === option.value;
                return (
                  <Button
                    key={option.value}
                    variant={active ? "default" : "outline"}
                    size="sm"
                    onClick={() => onThemeChange(option.value)}
                    className="gap-2"
                  >
                    <Icon className="h-4 w-4" />
                    {option.label}
                  </Button>
                );
              })}
            </div>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Languages className="h-4 w-4" />
            {t("settings.language.title")}
          </CardTitle>
          <CardDescription>{t("settings.language.subtitle")}</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex items-center justify-between gap-4">
            <div className="text-sm font-medium">{t("settings.language.label")}</div>
            <div className="w-[220px]">
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
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <DollarSign className="h-4 w-4" />
            {t("settings.currency.title")}
          </CardTitle>
          <CardDescription>{t("settings.currency.subtitle")}</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex items-center justify-between gap-4">
            <div className="text-sm font-medium">{t("settings.currency.label")}</div>
            <div className="w-[220px]">
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
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
