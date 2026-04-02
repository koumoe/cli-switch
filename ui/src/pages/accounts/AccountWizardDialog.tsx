import React, { useEffect, useMemo, useState } from "react";
import { ArrowLeft, ArrowRight, ExternalLink, Search } from "lucide-react";
import { toast } from "sonner";

import type { RechargeCurrency, RemoteAccountCheckinMode, RemoteAccountDetection } from "@/api";
import { createRemoteAccount, detectRemoteAccount } from "@/api";
import {
  Badge,
  Button,
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  Input,
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui";
import { humanizeApiError } from "@/lib/error";
import { useI18n } from "@/lib/i18n";
import { requestSub2ApiDesktopAuth } from "@/lib/ipc";
import { cn } from "@/lib/utils";

import { emptyAccountDraft, supportedCheckinModes, type AccountDraft } from "./shared";

type AccountWizardDialogProps = {
  open: boolean;
  defaultRechargeCurrency: RechargeCurrency;
  onOpenChange: (open: boolean) => void;
  onCreated: () => void | Promise<void>;
};

type StepId = 1 | 2 | 3;

export function AccountWizardDialog({
  open,
  defaultRechargeCurrency,
  onOpenChange,
  onCreated,
}: AccountWizardDialogProps) {
  const { t } = useI18n();
  const [step, setStep] = useState<StepId>(1);
  const [draft, setDraft] = useState<AccountDraft>(() => emptyAccountDraft(defaultRechargeCurrency));
  const [detection, setDetection] = useState<RemoteAccountDetection | null>(null);
  const [detecting, setDetecting] = useState(false);
  const [saving, setSaving] = useState(false);
  const [openingLogin, setOpeningLogin] = useState(false);

  useEffect(() => {
    if (!open) return;
    setStep(1);
    setDetection(null);
    setDetecting(false);
    setSaving(false);
    setOpeningLogin(false);
    setDraft(emptyAccountDraft(defaultRechargeCurrency));
  }, [defaultRechargeCurrency, open]);

  const modeOptions = useMemo(
    () => detection?.supported_checkin_modes ?? supportedCheckinModes(draft.provider),
    [detection, draft.provider]
  );

  async function handleDetect() {
    const baseUrl = draft.base_url.trim();
    if (!baseUrl) {
      toast.error(t("accounts.toast.actionFail"), { description: t("accounts.toast.baseUrlRequired") });
      return;
    }
    setDetecting(true);
    try {
      const result = await detectRemoteAccount(baseUrl);
      setDetection(result);
      setDraft((current) => ({
        ...current,
        provider: result.provider,
        base_url: result.normalized_base_url,
        checkin_mode: result.supported_checkin_modes.includes(current.checkin_mode)
          ? current.checkin_mode
          : "disabled",
        user_id: result.provider === "newapi" ? current.user_id : "",
        user_token: result.provider === "newapi" ? current.user_token : "",
        bearer_token: result.provider === "sub2api" ? current.bearer_token : "",
        refresh_token: result.provider === "sub2api" ? current.refresh_token : "",
      }));
      setStep(2);
    } catch (e) {
      toast.error(t("accounts.wizard.detectFailed"), { description: humanizeApiError(e, t) });
    } finally {
      setDetecting(false);
    }
  }

  function validateAuthStep(): boolean {
    if (draft.provider === "sub2api" && !draft.bearer_token.trim()) {
      toast.error(t("accounts.toast.actionFail"), { description: t("accounts.toast.bearerTokenRequired") });
      return false;
    }
    return true;
  }

  function validateBeforeCreate(): number | null {
    const lowBalance = Number(draft.low_balance_alert_threshold);
    if (!draft.base_url.trim()) {
      toast.error(t("accounts.toast.actionFail"), { description: t("accounts.toast.baseUrlRequired") });
      return null;
    }
    if (!Number.isFinite(lowBalance) || lowBalance < 0) {
      toast.error(t("accounts.toast.actionFail"), { description: t("accounts.toast.thresholdInvalid") });
      return null;
    }
    if (draft.checkin_mode === "page_open" && !draft.page_checkin_url.trim()) {
      toast.error(t("accounts.toast.actionFail"), { description: t("accounts.toast.pageCheckinUrlRequired") });
      return null;
    }
    if (draft.provider === "newapi" && draft.checkin_mode === "system_api") {
      if (!draft.user_id.trim() || !draft.user_token.trim()) {
        toast.error(t("accounts.toast.actionFail"), { description: t("accounts.toast.credentialsRequiredForSystem") });
        return null;
      }
    }
    if (draft.provider === "sub2api" && !draft.bearer_token.trim()) {
      toast.error(t("accounts.toast.actionFail"), { description: t("accounts.toast.bearerTokenRequired") });
      return null;
    }
    return lowBalance;
  }

  async function handleCreate() {
    const lowBalance = validateBeforeCreate();
    if (lowBalance === null) return;

    setSaving(true);
    try {
      await createRemoteAccount({
        provider: draft.provider,
        base_url: draft.base_url.trim(),
        api_url: draft.api_url.trim() || null,
        user_id: draft.provider === "newapi" ? draft.user_id.trim() : null,
        user_token: draft.provider === "newapi" ? draft.user_token.trim() : null,
        bearer_token: draft.provider === "sub2api" ? draft.bearer_token.trim() : null,
        refresh_token: draft.provider === "sub2api" ? draft.refresh_token.trim() || null : null,
        page_checkin_url: draft.page_checkin_url.trim() || null,
        checkin_mode: draft.checkin_mode,
        auto_checkin_time: draft.auto_checkin_time.trim() || "00:05:00",
        low_balance_alert_threshold: lowBalance,
        recharge_currency: draft.recharge_currency,
      });
      toast.success(t("accounts.toast.createOk"));
      await onCreated();
      onOpenChange(false);
    } catch (e) {
      toast.error(t("accounts.toast.actionFail"), { description: humanizeApiError(e, t) });
    } finally {
      setSaving(false);
    }
  }

  async function handleCaptureSub2ApiAuth() {
    if (draft.provider !== "sub2api") return;
    const baseUrl = draft.base_url.trim();
    if (!baseUrl) {
      toast.error(t("accounts.toast.actionFail"), { description: t("accounts.toast.baseUrlRequired") });
      return;
    }
    setOpeningLogin(true);
    try {
      const auth = await requestSub2ApiDesktopAuth(baseUrl);
      if (!auth) {
        toast(t("accounts.toast.sub2apiAuthCancelled"));
        return;
      }
      setDraft((current) => ({
        ...current,
        bearer_token: auth.bearerToken,
        refresh_token: auth.refreshToken,
      }));
    } catch (e) {
      const error = e instanceof Error ? e.message : "";
      if (error === "sub2api_auth_unsupported") {
        toast.error(t("accounts.toast.actionFail"), {
          description: t("accounts.toast.sub2apiAuthUnsupported"),
        });
        return;
      }
      toast.error(t("accounts.toast.actionFail"), {
        description: error || t("accounts.toast.sub2apiAuthFailed"),
      });
    } finally {
      setOpeningLogin(false);
    }
  }

  const steps = [
    { id: 1 as StepId, label: t("accounts.wizard.steps.detect") },
    { id: 2 as StepId, label: t("accounts.wizard.steps.auth") },
    { id: 3 as StepId, label: t("accounts.wizard.steps.options") },
  ];

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-[700px] max-h-[90vh] overflow-hidden flex flex-col">
        <DialogHeader>
          <DialogTitle>{t("accounts.wizard.title")}</DialogTitle>
          <DialogDescription>{t("accounts.wizard.description")}</DialogDescription>
        </DialogHeader>

        <div className="grid grid-cols-3 gap-2">
          {steps.map((item) => {
            const active = step === item.id;
            const done = step > item.id;
            return (
              <div
                key={item.id}
                className={cn(
                  "rounded-lg border px-3 py-2 text-sm",
                  active ? "border-primary bg-primary/5" : "border-border",
                  done ? "text-foreground" : "text-muted-foreground"
                )}
              >
                <div className="text-xs">{t("accounts.wizard.stepLabel", { step: item.id })}</div>
                <div className="font-medium">{item.label}</div>
              </div>
            );
          })}
        </div>

        {step > 1 && detection ? (
          <div className="rounded-lg border bg-muted/20 px-4 py-3 space-y-2">
            <div className="flex items-center gap-2">
              <Badge variant={detection.provider === "newapi" ? "secondary" : "outline"}>
                {t(`accounts.providers.${detection.provider}`)}
              </Badge>
              <span className="font-mono text-sm">{draft.base_url}</span>
            </div>
          </div>
        ) : null}

        <div className="flex-1 min-h-0 overflow-y-auto pr-1">
          {step === 1 ? (
            <div className="space-y-4 py-2">
              <div className="space-y-2">
                <label className="text-sm font-medium">{t("accounts.editor.baseUrl")}</label>
                <Input
                  value={draft.base_url}
                  onChange={(e) => setDraft((current) => ({ ...current, base_url: e.target.value }))}
                  placeholder="https://api.example.com"
                />
                <p className="text-xs text-muted-foreground">{t("accounts.wizard.baseUrlHint")}</p>
              </div>
            </div>
          ) : null}

          {step === 2 ? (
            <div className="space-y-4 py-2">
              {draft.provider === "newapi" ? (
                <>
                  <div className="space-y-2">
                    <label className="text-sm font-medium">{t("accounts.editor.userId")}</label>
                    <Input
                      value={draft.user_id}
                      onChange={(e) => setDraft((current) => ({ ...current, user_id: e.target.value }))}
                      placeholder="1001"
                    />
                  </div>
                  <div className="space-y-2">
                    <label className="text-sm font-medium">{t("accounts.editor.userToken")}</label>
                    <Input
                      type="password"
                      value={draft.user_token}
                      onChange={(e) => setDraft((current) => ({ ...current, user_token: e.target.value }))}
                      placeholder="sk-..."
                    />
                    <p className="text-xs text-muted-foreground">{t("accounts.wizard.newapiAuthHint")}</p>
                  </div>
                </>
              ) : (
                <div className="space-y-2">
                  <div className="flex items-center justify-between gap-3">
                    <label className="text-sm font-medium">{t("accounts.editor.bearerToken")}</label>
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={() => void handleCaptureSub2ApiAuth()}
                      disabled={saving || detecting || openingLogin}
                    >
                      <ExternalLink className="h-4 w-4 mr-2" />
                      {openingLogin ? t("accounts.editor.openLoginPageOpening") : t("accounts.editor.openLoginPage")}
                    </Button>
                  </div>
                  <div className="rounded-md border bg-muted/20 px-3 py-2 text-sm">
                    {draft.bearer_token.trim()
                      ? t("accounts.editor.bearerTokenCaptured")
                      : t("accounts.editor.bearerTokenMissing")}
                  </div>
                  <p className="text-xs text-muted-foreground">{t("accounts.editor.bearerTokenHint")}</p>
                </div>
              )}
            </div>
          ) : null}

          {step === 3 ? (
            <div className="space-y-4 py-2">
              <div className="space-y-2">
                <label className="text-sm font-medium">{t("accounts.editor.apiUrl")}</label>
                <Input
                  value={draft.api_url}
                  onChange={(e) => setDraft((current) => ({ ...current, api_url: e.target.value }))}
                  placeholder="https://api.example.com/v1"
                />
                <p className="text-xs text-muted-foreground">{t("accounts.editor.apiUrlHint")}</p>
              </div>

              <div className="space-y-2">
                <label className="text-sm font-medium">{t("accounts.editor.rechargeCurrency")}</label>
                <Select
                  value={draft.recharge_currency}
                  onValueChange={(value) => {
                    setDraft((current) => ({
                      ...current,
                      recharge_currency: value as AccountDraft["recharge_currency"],
                    }));
                  }}
                >
                  <SelectTrigger>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="CNY">{t("accounts.editor.rechargeCurrencyOptions.cny")}</SelectItem>
                    <SelectItem value="USD">{t("accounts.editor.rechargeCurrencyOptions.usd")}</SelectItem>
                  </SelectContent>
                </Select>
              </div>

              <div className={draft.checkin_mode === "system_api" ? "grid grid-cols-2 gap-4" : "space-y-2"}>
                <div className="space-y-2">
                  <label className="text-sm font-medium">{t("accounts.editor.checkinMode")}</label>
                  <Select
                    value={draft.checkin_mode}
                    onValueChange={(value) => {
                      setDraft((current) => ({
                        ...current,
                        checkin_mode: value as RemoteAccountCheckinMode,
                      }));
                    }}
                  >
                    <SelectTrigger>
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      {modeOptions.map((mode) => (
                        <SelectItem key={mode} value={mode}>
                          {t(`accounts.checkin.mode${mode === "disabled" ? "Disabled" : mode === "system_api" ? "System" : "Page"}`)}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </div>
                {draft.checkin_mode === "system_api" ? (
                  <div className="space-y-2">
                    <label className="text-sm font-medium">{t("accounts.editor.autoCheckinTime")}</label>
                    <Input
                      value={draft.auto_checkin_time}
                      onChange={(e) => setDraft((current) => ({ ...current, auto_checkin_time: e.target.value }))}
                      placeholder="00:05:00"
                    />
                  </div>
                ) : null}
              </div>

              {draft.checkin_mode === "page_open" ? (
                <div className="space-y-2">
                  <label className="text-sm font-medium">{t("accounts.editor.pageCheckinUrl")}</label>
                  <Input
                    value={draft.page_checkin_url}
                    onChange={(e) => setDraft((current) => ({ ...current, page_checkin_url: e.target.value }))}
                    placeholder="https://api.example.com/dashboard"
                  />
                </div>
              ) : null}

              <div className="space-y-2">
                <label className="text-sm font-medium">{t("accounts.editor.lowBalanceThreshold")}</label>
                <Input
                  type="number"
                  step="0.01"
                  min="0"
                  value={draft.low_balance_alert_threshold}
                  onChange={(e) => {
                    setDraft((current) => ({
                      ...current,
                      low_balance_alert_threshold: e.target.value,
                    }));
                  }}
                  placeholder="0"
                />
                <p className="text-xs text-muted-foreground">{t("accounts.editor.lowBalanceHint")}</p>
              </div>
            </div>
          ) : null}
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)} disabled={saving || detecting}>
            {t("common.cancel")}
          </Button>
          {step > 1 ? (
            <Button
              variant="outline"
              onClick={() => setStep((current) => (current > 1 ? (current - 1) as StepId : current))}
              disabled={saving || detecting}
            >
              <ArrowLeft className="h-4 w-4 mr-2" />
              {t("accounts.wizard.back")}
            </Button>
          ) : null}
          {step < 3 ? (
            <Button
              onClick={() => {
                if (step === 1) {
                  void handleDetect();
                  return;
                }
                if (step === 2 && validateAuthStep()) {
                  setStep(3);
                }
              }}
              disabled={saving || detecting}
            >
              {step === 1 ? <Search className="h-4 w-4 mr-2" /> : <ArrowRight className="h-4 w-4 mr-2" />}
              {step === 1
                ? (detecting ? t("accounts.wizard.detecting") : t("accounts.wizard.detect"))
                : t("accounts.wizard.next")}
            </Button>
          ) : (
            <Button onClick={() => void handleCreate()} disabled={saving}>
              {saving ? t("accounts.wizard.creating") : t("accounts.wizard.create")}
            </Button>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
