import React, { useEffect, useMemo, useRef, useState } from "react";
import { zodResolver } from "@hookform/resolvers/zod";
import { useForm } from "react-hook-form";
import { ArrowLeft, ArrowRight, ExternalLink, LoaderCircle, LogIn, Search } from "lucide-react";
import { toast } from "sonner";

import {
  createRemoteAccount,
  detectRemoteAccount,
  getOpenAiOAuthSession,
  openInBrowser,
  startOpenAiOAuth,
  updateRemoteAccount,
} from "@/api";
import {
  Badge,
  Button,
  Dialog,
  DialogBody,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  Form,
  FormControl,
  FormDescription,
  FormField,
  FormItem,
  FormLabel,
  FormMessage,
  Input,
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui";
import { useI18n } from "@/hooks/use-i18n";
import { humanizeApiError } from "@/lib/error";
import { requestSub2ApiDesktopAuth } from "@/lib/ipc";
import { createAccountFormSchema } from "@/lib/schemas/account";
import { cn } from "@/lib/utils";
import type {
  RechargeCurrency,
  RemoteAccountCheckinMode,
  RemoteAccountDetection,
  RemoteAccountProvider,
} from "@/types/api";

import {
  emptyAccountFormValues,
  providerSupportsSystemCheckin,
  supportedCheckinModes,
  type AccountFormValues,
} from "./shared";

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
  const [detection, setDetection] = useState<RemoteAccountDetection | null>(null);
  const [detecting, setDetecting] = useState(false);
  const [saving, setSaving] = useState(false);
  const [openingLogin, setOpeningLogin] = useState(false);
  const [oauthStatus, setOauthStatus] = useState<"idle" | "opening" | "pending">("idle");
  const oauthRunRef = useRef(0);
  const schema = useMemo(() => createAccountFormSchema(t), [t]);

  const form = useForm<AccountFormValues>({
    resolver: zodResolver(schema),
    defaultValues: emptyAccountFormValues(defaultRechargeCurrency),
  });

  useEffect(() => {
    oauthRunRef.current += 1;
    if (!open) {
      setOauthStatus("idle");
      return;
    }

    setStep(1);
    setDetection(null);
    setDetecting(false);
    setSaving(false);
    setOpeningLogin(false);
    setOauthStatus("idle");
    form.reset(emptyAccountFormValues(defaultRechargeCurrency));
  }, [defaultRechargeCurrency, form, open]);

  const provider = form.watch("provider");
  const checkinMode = form.watch("checkin_mode");
  const baseUrl = form.watch("base_url");
  const bearerToken = form.watch("bearer_token");
  const modeOptions = useMemo(
    () => detection?.supported_checkin_modes ?? supportedCheckinModes(provider),
    [detection, provider],
  );
  const showSystemTime = providerSupportsSystemCheckin(provider) && checkinMode === "system_api";

  function selectProvider(next: RemoteAccountProvider) {
    form.setValue("provider", next, { shouldDirty: true });
    setDetection(null);
    if (next === "openai") {
      form.setValue("base_url", "https://chatgpt.com", { shouldDirty: true });
      form.setValue("checkin_mode", "disabled", { shouldDirty: true });
    } else if (form.getValues("base_url") === "https://chatgpt.com") {
      form.setValue("base_url", "", { shouldDirty: true });
    }
  }

  async function handleOpenAiLogin() {
    const runId = oauthRunRef.current + 1;
    oauthRunRef.current = runId;
    setOauthStatus("opening");
    try {
      const session = await startOpenAiOAuth();
      if (oauthRunRef.current !== runId) return;
      await openInBrowser(session.authorization_url);
      if (oauthRunRef.current !== runId) return;
      setOauthStatus("pending");

      while (oauthRunRef.current === runId) {
        const result = await getOpenAiOAuthSession(session.request_id);
        if (oauthRunRef.current !== runId) return;
        if (result.status === "completed") {
          const name = form.getValues("name").trim();
          if (name && result.account) {
            await updateRemoteAccount(result.account.id, { name });
          }
          toast.success(t("accounts.toast.openaiAuthOk"));
          await onCreated();
          onOpenChange(false);
          return;
        }
        if (result.status === "failed") {
          throw new Error(result.error || t("accounts.toast.openaiAuthFailed"));
        }
        if (result.status === "expired" || Date.now() >= session.expires_at_ms) {
          throw new Error(t("accounts.toast.openaiAuthExpired"));
        }
        await new Promise((resolve) => window.setTimeout(resolve, 1000));
      }
    } catch (e) {
      if (oauthRunRef.current !== runId) return;
      toast.error(t("accounts.toast.openaiAuthFailed"), {
        description: humanizeApiError(e, t),
      });
    } finally {
      if (oauthRunRef.current === runId) setOauthStatus("idle");
    }
  }

  async function handleDetect() {
    const valid = await form.trigger("base_url");
    if (!valid) return;

    setDetecting(true);
    try {
      const current = form.getValues();
      const result = await detectRemoteAccount(current.base_url.trim());
      setDetection(result);
      form.reset({
        ...current,
        provider: result.provider,
        base_url: result.normalized_base_url,
        api_url: result.recommended_api_url ?? current.api_url,
        page_checkin_url: result.suggested_page_checkin_url ?? current.page_checkin_url,
        checkin_mode: result.supported_checkin_modes.includes(current.checkin_mode)
          ? current.checkin_mode
          : "disabled",
        user_id: result.provider === "newapi" ? current.user_id : "",
        user_token: result.provider === "newapi" ? current.user_token : "",
        bearer_token: result.provider === "sub2api" ? current.bearer_token : "",
        refresh_token: result.provider === "sub2api" ? current.refresh_token : "",
        stored_token_configured: result.provider === "sub2api" ? current.stored_token_configured : false,
      });
      setStep(2);
    } catch (e) {
      toast.error(t("accounts.wizard.detectFailed"), { description: humanizeApiError(e, t) });
    } finally {
      setDetecting(false);
    }
  }

  async function handleCaptureSub2ApiAuth() {
    if (provider !== "sub2api") return;

    const currentBaseUrl = form.getValues("base_url").trim();
    if (!currentBaseUrl) {
      form.setError("base_url", { message: t("accounts.toast.baseUrlRequired") });
      return;
    }

    setOpeningLogin(true);
    try {
      const auth = await requestSub2ApiDesktopAuth(currentBaseUrl);
      if (!auth) {
        toast(t("accounts.toast.sub2apiAuthCancelled"));
        return;
      }
      form.setValue("bearer_token", auth.bearerToken, {
        shouldDirty: true,
        shouldValidate: true,
      });
      form.setValue("refresh_token", auth.refreshToken, {
        shouldDirty: true,
      });
      form.setValue("stored_token_configured", true, {
        shouldDirty: true,
        shouldValidate: true,
      });
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

  async function handleNextFromAuth() {
    if (provider === "sub2api") {
      const valid = await form.trigger("bearer_token");
      if (!valid) return;
    }
    setStep(3);
  }

  const submit = form.handleSubmit(async (values) => {
    setSaving(true);
    try {
      await createRemoteAccount({
        name: values.name.trim(),
        provider: values.provider,
        base_url: values.base_url.trim(),
        api_url: values.api_url.trim() || null,
        user_id: values.provider === "newapi" ? values.user_id.trim() || null : null,
        user_token: values.provider === "newapi" ? values.user_token.trim() || null : null,
        bearer_token: values.provider === "sub2api" ? values.bearer_token.trim() || null : null,
        refresh_token: values.provider === "sub2api" ? values.refresh_token.trim() || null : null,
        page_checkin_url: values.page_checkin_url.trim() || null,
        checkin_mode: values.checkin_mode,
        auto_checkin_time: values.auto_checkin_time.trim(),
        low_balance_alert_threshold: Number(values.low_balance_alert_threshold),
        recharge_currency: values.recharge_currency,
      });
      toast.success(t("accounts.toast.createOk"));
      await onCreated();
      onOpenChange(false);
    } catch (e) {
      toast.error(t("accounts.toast.actionFail"), { description: humanizeApiError(e, t) });
    } finally {
      setSaving(false);
    }
  });

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

        <Form {...form}>
          <form onSubmit={submit} className="flex flex-1 min-h-0 flex-col overflow-hidden">
            <FormField
              control={form.control}
              name="stored_token_configured"
              render={({ field }) => <input type="hidden" value={field.value ? "true" : "false"} readOnly />}
            />

            <DialogBody className="flex-1 min-h-0 overflow-y-auto">
              <div className="space-y-4">
                {provider === "openai" ? (
                  <div className="rounded-lg border border-primary bg-primary/5 px-3 py-2 text-sm">
                    <div className="text-xs text-muted-foreground">OpenAI</div>
                    <div className="font-medium">{t("accounts.wizard.steps.openai")}</div>
                  </div>
                ) : (
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
                          done ? "text-foreground" : "text-muted-foreground",
                        )}
                      >
                        <div className="text-xs">{t("accounts.wizard.stepLabel", { step: item.id })}</div>
                        <div className="font-medium">{item.label}</div>
                      </div>
                    );
                  })}
                </div>
                )}

                {step > 1 && detection ? (
                  <div className="space-y-2 rounded-lg border bg-muted/20 px-4 py-3">
                    <div className="flex items-center gap-2">
                      <Badge variant={detection.provider === "newapi" ? "secondary" : "outline"}>
                        {t(`accounts.providers.${detection.provider}`)}
                      </Badge>
                      <span className="font-mono text-sm">{baseUrl}</span>
                    </div>
                  </div>
                ) : null}

                {step === 1 ? (
                  <div className="space-y-4">
                    <div className="space-y-2">
                      <div className="text-sm font-medium">{t("accounts.wizard.provider")}</div>
                      <div className="grid grid-cols-3 gap-2">
                        {(["newapi", "sub2api", "openai"] as const).map((item) => (
                          <Button
                            key={item}
                            type="button"
                            variant={provider === item ? "default" : "outline"}
                            className="h-auto min-h-16 flex-col gap-1"
                            onClick={() => selectProvider(item)}
                            disabled={oauthStatus !== "idle"}
                          >
                            <span>{t(`accounts.providers.${item}`)}</span>
                            <span className="text-xs font-normal opacity-80">
                              {t(`accounts.wizard.providerHints.${item}`)}
                            </span>
                          </Button>
                        ))}
                      </div>
                    </div>
            <FormField
              control={form.control}
              name="name"
              render={({ field }) => (
                <FormItem>
                  <FormLabel>{t("accounts.editor.name")}</FormLabel>
                  <FormControl><Input placeholder={t("accounts.editor.namePlaceholder")} {...field} /></FormControl>
                  <FormMessage />
                </FormItem>
              )}
            />
            {provider !== "openai" ? (
            <FormField
              control={form.control}
              name="base_url"
                      render={({ field }) => (
                        <FormItem>
                          <FormLabel>{t("accounts.editor.baseUrl")}</FormLabel>
                          <FormControl>
                            <Input {...field} placeholder="https://api.example.com" />
                          </FormControl>
                          <FormDescription>{t("accounts.wizard.baseUrlHint")}</FormDescription>
                          <FormMessage />
                        </FormItem>
                      )}
                    />
            ) : (
              <div className="rounded-lg border bg-muted/20 px-4 py-4">
                <div className="font-medium">{t("accounts.wizard.openaiTitle")}</div>
                <p className="mt-1 text-sm text-muted-foreground">
                  {t("accounts.wizard.openaiHint")}
                </p>
              </div>
            )}
                  </div>
                ) : null}

                {step === 2 ? (
                  <div className="space-y-4">
                    {provider === "newapi" ? (
                      <>
                        <FormField
                          control={form.control}
                          name="user_id"
                          render={({ field }) => (
                            <FormItem>
                              <FormLabel>{t("accounts.editor.userId")}</FormLabel>
                              <FormControl>
                                <Input {...field} placeholder="1001" />
                              </FormControl>
                              <FormMessage />
                            </FormItem>
                          )}
                        />

                        <FormField
                          control={form.control}
                          name="user_token"
                          render={({ field }) => (
                            <FormItem>
                              <FormLabel>{t("accounts.editor.userToken")}</FormLabel>
                              <FormControl>
                                <Input {...field} type="password" placeholder="sk-..." />
                              </FormControl>
                              <FormDescription>{t("accounts.wizard.newapiAuthHint")}</FormDescription>
                              <FormMessage />
                            </FormItem>
                          )}
                        />
                      </>
                    ) : (
                      <FormField
                        control={form.control}
                        name="bearer_token"
                        render={() => (
                          <FormItem>
                            <div className="flex items-center justify-between gap-3">
                              <FormLabel>{t("accounts.editor.bearerToken")}</FormLabel>
                              <Button
                                type="button"
                                variant="outline"
                                size="sm"
                                onClick={() => void handleCaptureSub2ApiAuth()}
                                disabled={saving || detecting || openingLogin}
                              >
                                <ExternalLink className="mr-2 h-4 w-4" />
                                {openingLogin ? t("accounts.editor.openLoginPageOpening") : t("accounts.editor.openLoginPage")}
                              </Button>
                            </div>
                            <div className="rounded-md border bg-muted/20 px-3 py-2 text-sm">
                              {bearerToken.trim()
                                ? t("accounts.editor.bearerTokenCaptured")
                                : t("accounts.editor.bearerTokenMissing")}
                            </div>
                            <FormDescription>{t("accounts.editor.bearerTokenHint")}</FormDescription>
                            <FormMessage />
                          </FormItem>
                        )}
                      />
                    )}
                  </div>
                ) : null}

                {step === 3 ? (
                  <div className="space-y-4">
                    <FormField
                      control={form.control}
                      name="api_url"
                      render={({ field }) => (
                        <FormItem>
                          <FormLabel>{t("accounts.editor.apiUrl")}</FormLabel>
                          <FormControl>
                            <Input {...field} placeholder="https://api.example.com/v1" />
                          </FormControl>
                          <FormDescription>{t("accounts.editor.apiUrlHint")}</FormDescription>
                          <FormMessage />
                        </FormItem>
                      )}
                    />

                    <FormField
                      control={form.control}
                      name="recharge_currency"
                      render={({ field }) => (
                        <FormItem>
                          <FormLabel>{t("accounts.editor.rechargeCurrency")}</FormLabel>
                          <Select value={field.value} onValueChange={field.onChange}>
                            <FormControl>
                              <SelectTrigger>
                                <SelectValue />
                              </SelectTrigger>
                            </FormControl>
                            <SelectContent>
                              <SelectItem value="CNY">{t("accounts.editor.rechargeCurrencyOptions.cny")}</SelectItem>
                              <SelectItem value="USD">{t("accounts.editor.rechargeCurrencyOptions.usd")}</SelectItem>
                            </SelectContent>
                          </Select>
                          <FormMessage />
                        </FormItem>
                      )}
                    />

                    <div className={showSystemTime ? "grid grid-cols-2 gap-4" : "space-y-2"}>
                      <FormField
                        control={form.control}
                        name="checkin_mode"
                        render={({ field }) => (
                          <FormItem>
                            <FormLabel>{t("accounts.editor.checkinMode")}</FormLabel>
                            <Select
                              value={field.value}
                              onValueChange={(value) => field.onChange(value as RemoteAccountCheckinMode)}
                            >
                              <FormControl>
                                <SelectTrigger>
                                  <SelectValue />
                                </SelectTrigger>
                              </FormControl>
                              <SelectContent>
                                {modeOptions.map((mode) => (
                                  <SelectItem key={mode} value={mode}>
                                    {t(`accounts.checkin.mode${mode === "disabled" ? "Disabled" : mode === "system_api" ? "System" : "Page"}`)}
                                  </SelectItem>
                                ))}
                              </SelectContent>
                            </Select>
                            <FormMessage />
                          </FormItem>
                        )}
                      />

                      {showSystemTime ? (
                        <FormField
                          control={form.control}
                          name="auto_checkin_time"
                          render={({ field }) => (
                            <FormItem>
                              <FormLabel>{t("accounts.editor.autoCheckinTime")}</FormLabel>
                              <FormControl>
                                <Input {...field} placeholder="00:05:00" />
                              </FormControl>
                              <FormMessage />
                            </FormItem>
                          )}
                        />
                      ) : null}
                    </div>

                    {checkinMode === "page_open" ? (
                      <FormField
                        control={form.control}
                        name="page_checkin_url"
                        render={({ field }) => (
                          <FormItem>
                            <FormLabel>{t("accounts.editor.pageCheckinUrl")}</FormLabel>
                            <FormControl>
                              <Input {...field} placeholder="https://api.example.com/dashboard" />
                            </FormControl>
                            <FormMessage />
                          </FormItem>
                        )}
                      />
                    ) : null}

                    {!showSystemTime ? (
                      <FormField
                        control={form.control}
                        name="auto_checkin_time"
                        render={({ field }) => <input type="hidden" {...field} />}
                      />
                    ) : null}

                    <FormField
                      control={form.control}
                      name="low_balance_alert_threshold"
                      render={({ field }) => (
                        <FormItem>
                          <FormLabel>{t("accounts.editor.lowBalanceThreshold")}</FormLabel>
                          <FormControl>
                            <Input {...field} type="number" step="0.01" min="0" placeholder="0" />
                          </FormControl>
                          <FormDescription>{t("accounts.editor.lowBalanceHint")}</FormDescription>
                          <FormMessage />
                        </FormItem>
                      )}
                    />
                  </div>
                ) : null}
              </div>
            </DialogBody>

            <DialogFooter>
              <Button type="button" variant="outline" onClick={() => onOpenChange(false)} disabled={saving || detecting}>
                {t("common.cancel")}
              </Button>

              {step > 1 ? (
                <Button
                  type="button"
                  variant="outline"
                  onClick={() => setStep((current) => (current > 1 ? ((current - 1) as StepId) : current))}
                  disabled={saving || detecting}
                >
                  <ArrowLeft className="h-4 w-4 mr-2" />
                  {t("accounts.wizard.back")}
                </Button>
              ) : null}

              {step === 1 && provider === "openai" ? (
                <Button
                  type="button"
                  onClick={() => void handleOpenAiLogin()}
                  disabled={oauthStatus !== "idle"}
                >
                  {oauthStatus === "idle" ? <LogIn className="mr-2 h-4 w-4" /> : <LoaderCircle className="mr-2 h-4 w-4 animate-spin" />}
                  {oauthStatus === "opening"
                    ? t("accounts.wizard.openaiOpening")
                    : oauthStatus === "pending"
                      ? t("accounts.wizard.openaiWaiting")
                      : t("accounts.wizard.openaiLogin")}
                </Button>
              ) : step < 3 ? (
                <Button
                  type="button"
                  onClick={() => {
                    if (step === 1) {
                      void handleDetect();
                      return;
                    }
                    if (step === 2) {
                      void handleNextFromAuth();
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
                <Button type="submit" disabled={saving}>
                  {saving ? t("accounts.wizard.creating") : t("accounts.wizard.create")}
                </Button>
              )}
            </DialogFooter>
          </form>
        </Form>
      </DialogContent>
    </Dialog>
  );
}
