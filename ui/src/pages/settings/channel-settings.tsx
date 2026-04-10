import {
  ChannelProtectionSettingsCard,
  ChannelRetrySettingsCard,
  NewApiManagedSettingsCard,
  PricingDataSettingsCard,
} from "@/pages/settings/form-sections";
import type { AppSettings, PricingStatus } from "@/types/api";

type ChannelSettingsProps = {
  settings: AppSettings | null;
  pricing: PricingStatus | null;
  syncing: boolean;
  onSaved: (settings: AppSettings) => void;
  onSync: () => void | Promise<void>;
};

export function ChannelSettings({
  settings,
  pricing,
  syncing,
  onSaved,
  onSync,
}: ChannelSettingsProps) {
  return (
    <div className="pb-4">
      <ChannelProtectionSettingsCard settings={settings} onSaved={onSaved} />
      <ChannelRetrySettingsCard settings={settings} onSaved={onSaved} />
      <NewApiManagedSettingsCard settings={settings} onSaved={onSaved} />
      <PricingDataSettingsCard
        settings={settings}
        pricing={pricing}
        syncing={syncing}
        onSaved={onSaved}
        onSync={onSync}
      />
    </div>
  );
}
