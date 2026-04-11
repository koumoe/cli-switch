import type { ReactNode } from "react";

import { Badge } from "@/components/ui";
import { useI18n } from "@/hooks/use-i18n";
import {
  AppUpdateSettingsCard,
  CompatibilitySettingsCard,
  ServiceInfoSettingsCard,
} from "@/pages/settings/form-sections";
import type { AppSettings, Health } from "@/types/api";

type SystemSettingsProps = {
  settings: AppSettings | null;
  health: Health | null;
  backendStatusLabel: string;
  apiHost: string;
  apiPort: string;
  updateDialog: ReactNode;
  updateStatusText: string;
  updateServerVersion: string | null;
  updateChecking: boolean;
  onSaved: (settings: AppSettings) => void;
  onCheck: () => void | Promise<void>;
  onAutoUpdateChange: (enabled: boolean) => Promise<void>;
};

export function SystemSettings({
  settings,
  health,
  backendStatusLabel,
  apiHost,
  apiPort,
  updateDialog,
  updateStatusText,
  updateServerVersion,
  updateChecking,
  onSaved,
  onCheck,
  onAutoUpdateChange,
}: SystemSettingsProps) {
  const { t } = useI18n();

  return (
    <div className="pb-4">
      <ServiceInfoSettingsCard
        settings={settings}
        apiHost={apiHost}
        apiPort={apiPort}
        onSaved={onSaved}
      />

      <CompatibilitySettingsCard settings={settings} onSaved={onSaved} />

      <AppUpdateSettingsCard
        settings={settings}
        onSaved={onSaved}
        dialog={updateDialog}
        updateStatusText={updateStatusText}
        updateServerVersion={updateServerVersion}
        updateChecking={updateChecking}
        onCheck={onCheck}
        onAutoUpdateChange={onAutoUpdateChange}
      />
      <div className="border-t border-border px-5 pb-1 pt-2.5 text-[10px] font-bold uppercase tracking-[0.06em] text-muted-foreground">
        {t("settings.about.title")}
      </div>
      <div className="flex min-h-[50px] items-center justify-between gap-4 border-t border-border px-5 py-3 transition-colors hover:bg-secondary/35">
        <span className="text-[12.5px] font-semibold">
          {t("settings.about.appName")}
        </span>
        <span className="text-xs font-medium">CliSwitch</span>
      </div>
      <div className="flex min-h-[50px] items-center justify-between gap-4 border-t border-border px-5 py-3 transition-colors hover:bg-secondary/35">
        <span className="text-[12.5px] font-semibold">
          {t("settings.about.version")}
        </span>
        <span className="text-xs font-mono">
          {health?.version ? `v${health.version}` : "-"}
        </span>
      </div>
      <div className="flex min-h-[50px] items-center justify-between gap-4 border-t border-border px-5 py-3 transition-colors hover:bg-secondary/35">
        <span className="text-[12.5px] font-semibold">
          {t("settings.about.backendStatus")}
        </span>
        <Badge variant={health?.status === "ok" ? "success" : "destructive"}>
          {backendStatusLabel}
        </Badge>
      </div>
      <div className="flex min-h-[50px] items-center justify-between gap-4 border-t border-border px-5 py-3 transition-colors hover:bg-secondary/35">
        <span className="text-[12.5px] font-semibold">
          {t("settings.about.description")}
        </span>
        <span className="max-w-[320px] text-right text-[10.5px] text-muted-foreground">
          {t("settings.about.descText")}
        </span>
      </div>
      <div className="border-t border-border px-5 py-3">
        <p className="text-[10.5px] text-muted-foreground">
          {t("settings.about.intro")}
        </p>
      </div>
    </div>
  );
}
