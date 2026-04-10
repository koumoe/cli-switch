import {
  StartupSettingsCard,
  WindowCloseSettingsCard,
} from "@/pages/settings/form-sections";
import type { AppSettings } from "@/types/api";

type ApplicationSettingsProps = {
  settings: AppSettings | null;
  onSaved: (settings: AppSettings) => void;
};

export function ApplicationSettings({
  settings,
  onSaved,
}: ApplicationSettingsProps) {
  return (
    <div className="space-y-4">
      <WindowCloseSettingsCard settings={settings} onSaved={onSaved} />
      <StartupSettingsCard settings={settings} onSaved={onSaved} />
    </div>
  );
}
