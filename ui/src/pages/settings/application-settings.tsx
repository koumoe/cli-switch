import {
  StartupSettingsCard,
  WindowCloseSettingsCard,
} from "@/pages/settings/form-sections";
import type { AppSettings } from "@/types/api";
import { Button, Switch } from "@/components/ui";
import { SettingsFieldText, SettingsRow, SettingsSection } from "./settings-layout";
import { useI18n } from "@/hooks/use-i18n";
import { updateSettings } from "@/api";
import { toast } from "sonner";
import { humanizeApiError } from "@/lib/error";
import { getCodexNotifyCommand } from "@/api";
import { useState } from "react";

type ApplicationSettingsProps = {
  settings: AppSettings | null;
  onSaved: (settings: AppSettings) => void;
};

export function codexNotifyToml(command: unknown): string {
  if (
    !Array.isArray(command) ||
    command.length === 0 ||
    command.some((value) => typeof value !== "string" || !value.trim())
  ) {
    throw new Error("invalid_codex_notify_command");
  }
  const args = command as string[];
  return `notify = [${args.map((value) => JSON.stringify(value)).join(", ")}]`;
}

export function ApplicationSettings({
  settings,
  onSaved,
}: ApplicationSettingsProps) {
  const { t } = useI18n();
  const [codexNotifyOpen, setCodexNotifyOpen] = useState(false);
  const [codexNotifyBusy, setCodexNotifyBusy] = useState(false);
  const desktopPetAvailable =
    typeof window !== "undefined" &&
    typeof (window as Window & { ipc?: { postMessage?: unknown } }).ipc?.postMessage ===
      "function";
  const onPetChange = async (enabled: boolean) => {
    try {
      onSaved(await updateSettings({ desktop_pet_enabled: enabled }));
    } catch (error) {
      toast.error(t("settings.desktopPet.saveFail"), {
        description: humanizeApiError(error, t),
      });
    }
  };
  const copyCodexNotifyConfig = async () => {
    setCodexNotifyBusy(true);
    try {
      const response = await getCodexNotifyCommand();
      const text = codexNotifyToml(response.command);
      if (navigator.clipboard?.writeText) {
        await navigator.clipboard.writeText(text);
      } else {
        const textarea = document.createElement("textarea");
        textarea.value = text;
        textarea.style.position = "fixed";
        textarea.style.opacity = "0";
        document.body.appendChild(textarea);
        textarea.focus();
        textarea.select();
        const copied = document.execCommand("copy");
        textarea.remove();
        if (!copied) throw new Error("clipboard_unavailable");
      }
      toast.success(t("settings.desktopPet.codexNotify.copied"));
    } catch (error) {
      toast.error(t("settings.desktopPet.codexNotify.copyFail"), {
        description: humanizeApiError(error, t),
      });
    } finally {
      setCodexNotifyBusy(false);
    }
  };
  return (
    <div className="pb-4">
      {desktopPetAvailable ? (
        <SettingsSection title={t("settings.desktopPet.title")}>
          <SettingsRow>
            <SettingsFieldText
              label={t("settings.desktopPet.label")}
              hint={t("settings.desktopPet.hint")}
            />
            <Switch
              checked={settings?.desktop_pet_enabled ?? false}
              onCheckedChange={(value) => void onPetChange(value)}
              disabled={!settings}
              aria-label={t("settings.desktopPet.label")}
            />
          </SettingsRow>
          {settings?.desktop_pet_enabled ? (
            <div className="border-t border-border px-5 py-3">
              <button
                type="button"
                className="text-left text-[11px] font-semibold text-muted-foreground underline-offset-2 hover:underline"
                onClick={() => setCodexNotifyOpen((open) => !open)}
              >
                {t("settings.desktopPet.codexNotify.title")}
              </button>
              {codexNotifyOpen ? (
                <div className="mt-2 space-y-2">
                  <p className="text-[10.5px] leading-snug text-muted-foreground">
                    {t("settings.desktopPet.codexNotify.hint")}
                  </p>
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    disabled={codexNotifyBusy}
                    onClick={() => void copyCodexNotifyConfig()}
                  >
                    {t("settings.desktopPet.codexNotify.copy")}
                  </Button>
                </div>
              ) : null}
            </div>
          ) : null}
        </SettingsSection>
      ) : null}
      <WindowCloseSettingsCard settings={settings} onSaved={onSaved} />
      <StartupSettingsCard settings={settings} onSaved={onSaved} />
    </div>
  );
}
