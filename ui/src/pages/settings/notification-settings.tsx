import {
  RemoteSystemNotificationsSettingsCard,
  SystemNotificationsSettingsCard,
} from "@/pages/settings/form-sections";
import type { AppSettings } from "@/types/api";

type NotificationSettingsProps = {
  settings: AppSettings | null;
  onSaved: (settings: AppSettings) => void;
};

export function NotificationSettings({
  settings,
  onSaved,
}: NotificationSettingsProps) {
  return (
    <div className="space-y-4">
      <SystemNotificationsSettingsCard settings={settings} onSaved={onSaved} />
      {settings?.system_notifications_enabled ? (
        <RemoteSystemNotificationsSettingsCard settings={settings} onSaved={onSaved} />
      ) : null}
    </div>
  );
}
