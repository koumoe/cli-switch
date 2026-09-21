import { useEffect, useState } from "react";
import { useForm } from "react-hook-form";
import { toast } from "sonner";

import { getCodexTicketStatus, updateSettings } from "@/api";
import { Button, Input, Switch } from "@/components/ui";
import { useI18n } from "@/hooks/use-i18n";
import { humanizeApiError } from "@/lib/error";
import type { AppSettings, CodexTicketStatus, UpdateSettingsInput } from "@/types/api";
import {
  SettingsFieldText,
  SettingsFooter,
  SettingsRow,
  SettingsSection,
} from "./settings-layout";

type CodexTicketSettingsProps = {
  settings: AppSettings | null;
  onSaved: (settings: AppSettings) => void;
};

type FormValues = {
  enabled: boolean;
  failClosed: boolean;
  models: string;
  proxyUrl: string;
  clearProxy: boolean;
  versionOverride: string;
};

function defaults(settings: AppSettings | null): FormValues {
  return {
    enabled: settings?.openai_codex_ticket_enabled ?? false,
    failClosed: settings?.openai_codex_ticket_fail_closed ?? true,
    models: settings?.openai_codex_ticket_models?.join(", ") ?? "",
    proxyUrl: "",
    clearProxy: false,
    versionOverride: settings?.openai_codex_ticket_version_override ?? "",
  };
}

export function CodexTicketSettingsCard({
  settings,
  onSaved,
}: CodexTicketSettingsProps) {
  const { t } = useI18n();
  const [saving, setSaving] = useState(false);
  const [ticketStatus, setTicketStatus] = useState<CodexTicketStatus | null>(null);
  const form = useForm<FormValues>({ defaultValues: defaults(settings) });
  const configured = settings?.openai_codex_ticket_harvest_proxy_configured ?? false;
  const clearProxy = form.watch("clearProxy");
  const disabled = !settings || saving;

  useEffect(() => {
    form.reset(defaults(settings));
  }, [form, settings]);

  useEffect(() => {
    let disposed = false;
    let inFlight = false;
    const refresh = async () => {
      if (inFlight) return;
      inFlight = true;
      try {
        const next = await getCodexTicketStatus();
        if (!disposed) setTicketStatus(next);
      } catch {
        // The settings form remains usable while the optional status endpoint is unavailable.
      } finally {
        inFlight = false;
      }
    };
    void refresh();
    const timer = window.setInterval(() => void refresh(), 10_000);
    return () => {
      disposed = true;
      window.clearInterval(timer);
    };
  }, []);

  const submit = form.handleSubmit(async (values) => {
    const models = [...new Set(values.models.split(/[,，\n]/).map((model) => model.trim()).filter(Boolean))];
    if (models.length === 0) {
      form.setError("models", { message: t("settings.codexTicket.modelsRequired") });
      return;
    }

    const proxyUrl = values.proxyUrl.trim();
    if (proxyUrl && !values.clearProxy) {
      try {
        const url = new URL(proxyUrl);
        if (
          !["http:", "https:", "socks5:", "socks5h:"].includes(url.protocol) ||
          !url.hostname || url.search || url.hash || url.port === "0"
        ) {
          throw new Error("invalid_proxy_url");
        }
      } catch {
        form.setError("proxyUrl", { message: t("settings.codexTicket.proxyInvalid") });
        return;
      }
    }

    const patch: UpdateSettingsInput = {
      openai_codex_ticket_enabled: values.enabled,
      openai_codex_ticket_fail_closed: values.failClosed,
      openai_codex_ticket_models: models,
      openai_codex_ticket_version_override: values.versionOverride.trim(),
      ...(values.clearProxy
        ? { openai_codex_ticket_clear_harvest_proxy: true }
        : proxyUrl ? { openai_codex_ticket_harvest_proxy_url: proxyUrl } : {}),
    };
    setSaving(true);
    try {
      const next = await updateSettings(patch);
      form.reset(defaults(next));
      onSaved(next);
      toast.success(t("settings.codexTicket.saved"));
    } catch (error) {
      toast.error(t("settings.codexTicket.saveFail"), {
        description: humanizeApiError(error, t),
      });
    } finally {
      setSaving(false);
    }
  });

  return (
    <form onSubmit={submit}>
      <SettingsSection title={t("settings.codexTicket.title")} hint={t("settings.codexTicket.hint")}>
        <SettingsRow>
          <SettingsFieldText label={t("settings.codexTicket.enabled")} hint={t("settings.codexTicket.enabledHint")} />
          <Switch
            checked={form.watch("enabled")}
            onCheckedChange={(value) => form.setValue("enabled", value)}
            disabled={disabled}
            aria-label={t("settings.codexTicket.enabled")}
          />
        </SettingsRow>
        <SettingsRow>
          <SettingsFieldText label={t("settings.codexTicket.failClosed")} hint={t("settings.codexTicket.failClosedHint")} />
          <Switch
            checked={form.watch("failClosed")}
            onCheckedChange={(value) => form.setValue("failClosed", value)}
            disabled={disabled}
            aria-label={t("settings.codexTicket.failClosed")}
          />
        </SettingsRow>
        <SettingsRow className="flex-col items-stretch gap-2">
          <SettingsFieldText
            label={<label htmlFor="codex-ticket-models">{t("settings.codexTicket.models")}</label>}
            hint={t("settings.codexTicket.modelsHint")}
          />
          <Input id="codex-ticket-models" {...form.register("models")} disabled={disabled} aria-invalid={!!form.formState.errors.models} />
          {form.formState.errors.models ? <p role="alert" className="text-xs text-destructive">{form.formState.errors.models.message}</p> : null}
        </SettingsRow>
        <SettingsRow className="flex-col items-stretch gap-2">
          <SettingsFieldText
            label={<label htmlFor="codex-ticket-version">{t("settings.codexTicket.versionOverride")}</label>}
            hint={t("settings.codexTicket.versionOverrideHint")}
          />
          <Input
            id="codex-ticket-version"
            inputMode="text"
            spellCheck={false}
            {...form.register("versionOverride")}
            placeholder={t("settings.codexTicket.versionOverridePlaceholder")}
            disabled={disabled}
            aria-invalid={!!form.formState.errors.versionOverride}
          />
          {form.formState.errors.versionOverride ? <p role="alert" className="text-xs text-destructive">{form.formState.errors.versionOverride.message}</p> : null}
        </SettingsRow>
        <SettingsRow className="flex-col items-stretch gap-2">
          <SettingsFieldText
            label={<label htmlFor="codex-ticket-proxy">{t("settings.codexTicket.proxy")}</label>}
            hint={t("settings.codexTicket.proxyHint")}
          />
          <Input
            id="codex-ticket-proxy"
            type="password"
            autoComplete="new-password"
            spellCheck={false}
            {...form.register("proxyUrl")}
            placeholder={t(configured ? "settings.codexTicket.proxyConfigured" : "settings.codexTicket.proxyPlaceholder")}
            disabled={disabled || clearProxy}
            aria-invalid={!!form.formState.errors.proxyUrl}
          />
          {form.formState.errors.proxyUrl ? <p role="alert" className="text-xs text-destructive">{form.formState.errors.proxyUrl.message}</p> : null}
        </SettingsRow>
        {configured ? (
          <SettingsRow>
            <SettingsFieldText label={t("settings.codexTicket.clearProxy")} hint={t("settings.codexTicket.clearProxyHint")} />
            <Switch
              checked={clearProxy}
              onCheckedChange={(value) => form.setValue("clearProxy", value)}
              disabled={disabled}
              aria-label={t("settings.codexTicket.clearProxy")}
            />
          </SettingsRow>
        ) : null}
        <CodexTicketStatusView status={ticketStatus} />
        <SettingsFooter>
          <Button size="sm" type="submit" disabled={disabled}>{t("common.save")}</Button>
        </SettingsFooter>
      </SettingsSection>
    </form>
  );
}

function CodexTicketStatusView({ status }: { status: CodexTicketStatus | null }) {
  const { t } = useI18n();
  if (!status) return null;
  const issueLabel = status.issue
    ? t(`settings.codexTicket.statusIssue.${status.issue}`)
    : t("settings.codexTicket.statusReady");
  return (
    <div className="space-y-2 border-t border-border px-5 py-3 text-xs">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <span className="font-semibold">{t("settings.codexTicket.statusTitle")}</span>
        <span className="text-muted-foreground">
          {t("settings.codexTicket.versionSource", {
            version: status.version ?? t("settings.codexTicket.versionMissing"),
            source: status.version_source
              ? t(`settings.codexTicket.sources.${status.version_source}`)
              : t("settings.codexTicket.sourceNone"),
          })}
        </span>
      </div>
      {status.issue ? (
        <div className="text-amber-600 dark:text-amber-400">
          {issueLabel} {status.issue === "missing_version" ? t("settings.codexTicket.missingVersionAction") : ""}
        </div>
      ) : null}
      {status.tickets.length > 0 ? (
        <div className="space-y-1">
          {status.tickets.map((ticket) => (
            <div key={`${ticket.account_id}:${ticket.model}`} className="flex flex-wrap items-center justify-between gap-x-3 gap-y-1 rounded border px-2 py-1.5">
              <span>{ticket.account_name} · {ticket.model}</span>
              <span className={ticket.ready ? "text-emerald-600 dark:text-emerald-400" : "text-muted-foreground"}>
                {ticket.ready
                  ? t("settings.codexTicket.ticketReady", { length: ticket.length ?? "?" })
                  : ticket.last_error
                    ? t(`settings.codexTicket.ticketErrors.${ticket.last_error}`)
                    : t("settings.codexTicket.ticketMissing")}
              </span>
            </div>
          ))}
        </div>
      ) : null}
    </div>
  );
}
