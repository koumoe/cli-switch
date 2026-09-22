import { useEffect } from "react";
import { useForm } from "react-hook-form";
import { toast } from "sonner";

import { updateSettings } from "@/api";
import { Button, Switch } from "@/components/ui";
import { useI18n } from "@/hooks/use-i18n";
import { humanizeApiError } from "@/lib/error";
import type { AppSettings, UpdateSettingsInput } from "@/types/api";
import { SettingsFieldText, SettingsFooter, SettingsRow, SettingsSection } from "./settings-layout";

type CodexTicketSettingsProps = {
  settings: AppSettings | null;
  onSaved: (settings: AppSettings) => void;
};

type FormValues = {
  enabled: boolean;
};

function defaults(settings: AppSettings | null): FormValues {
  return { enabled: settings?.openai_codex_ticket_enabled ?? false };
}

export function CodexTicketSettingsCard({
  settings,
  onSaved,
}: CodexTicketSettingsProps) {
  const { t } = useI18n();
  const form = useForm<FormValues>({ defaultValues: defaults(settings) });
  const saving = form.formState.isSubmitting;
  const disabled = !settings || saving;

  useEffect(() => {
    form.reset(defaults(settings));
  }, [form, settings]);

  const submit = form.handleSubmit(async (values) => {
    const patch: UpdateSettingsInput = {
      openai_codex_ticket_enabled: values.enabled,
    };
    try {
      const next = await updateSettings(patch);
      form.reset(defaults(next));
      onSaved(next);
      toast.success(t("settings.codexTicket.saved"));
    } catch (error) {
      toast.error(t("settings.codexTicket.saveFail"), {
        description: humanizeApiError(error, t),
      });
    }
  });

  return (
    <form onSubmit={submit}>
      <SettingsSection title={t("settings.codexTicket.title")} hint={t("settings.codexTicket.hint")}>
        <SettingsRow>
          <SettingsFieldText
            label={t("settings.codexTicket.enabled")}
            hint={t("settings.codexTicket.enabledHint")}
          />
          <Switch
            checked={form.watch("enabled")}
            onCheckedChange={(value) => form.setValue("enabled", value, { shouldDirty: true })}
            disabled={disabled}
            aria-label={t("settings.codexTicket.enabled")}
          />
        </SettingsRow>
        <SettingsFooter>
          <Button size="sm" type="submit" disabled={disabled || !form.formState.isDirty}>
            {t("common.save")}
          </Button>
        </SettingsFooter>
      </SettingsSection>
    </form>
  );
}
