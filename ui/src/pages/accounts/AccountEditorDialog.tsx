import React, { useEffect, useMemo, useState } from "react";
import { zodResolver } from "@hookform/resolvers/zod";
import { useForm } from "react-hook-form";
import { toast } from "sonner";

import type { RemoteAccount } from "@/types/api";
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
import { createAccountFormSchema } from "@/lib/schemas/account";
import { isHttpUrl } from "@/lib/schemas/common";
import { requestSub2ApiDesktopAuth } from "@/lib/ipc";

import {
  emptyAccountFormValues,
  providerSupportsSystemCheckin,
  supportedCheckinModes,
  type AccountCheckinModeOption,
  type AccountFormValues,
} from "./shared";

type AccountEditorDialogProps = {
  open: boolean;
  account: RemoteAccount | null;
  initialValues: AccountFormValues | null;
  onOpenChange: (open: boolean) => void;
  onSave: (values: AccountFormValues) => void | Promise<void>;
};

export function AccountEditorDialog({
  open,
  account,
  initialValues,
  onOpenChange,
  onSave,
}: AccountEditorDialogProps) {
  const { t } = useI18n();
  const [saving, setSaving] = useState(false);
  const [loginOpening, setLoginOpening] = useState(false);
  const schema = useMemo(() => createAccountFormSchema(t), [t]);

  const form = useForm<AccountFormValues>({
    resolver: zodResolver(schema),
    defaultValues: initialValues ?? emptyAccountFormValues(account?.recharge_currency ?? "CNY"),
  });

  useEffect(() => {
    if (!open) {
      setSaving(false);
      setLoginOpening(false);
      return;
    }

    form.reset(initialValues ?? emptyAccountFormValues(account?.recharge_currency ?? "CNY"));
  }, [account?.recharge_currency, form, initialValues, open]);

  const provider = form.watch("provider");
  const checkinMode = form.watch("checkin_mode");
  const storedTokenConfigured = form.watch("stored_token_configured");
  const bearerToken = form.watch("bearer_token");
  const modeOptions = supportedCheckinModes(provider);
  const showSystemTime = providerSupportsSystemCheckin(provider) && checkinMode === "system_api";

  const submit = form.handleSubmit(async (values) => {
    setSaving(true);
    try {
      await onSave(values);
    } finally {
      setSaving(false);
    }
  });

  async function handleOpenLoginPage() {
    if (provider !== "sub2api") return;

    const baseUrl = form.getValues("base_url").trim();
    if (!baseUrl) {
      form.setError("base_url", { message: t("accounts.toast.baseUrlRequired") });
      return;
    }
    if (!isHttpUrl(baseUrl)) {
      form.setError("base_url", { message: t("accounts.toast.baseUrlInvalid") });
      return;
    }

    setLoginOpening(true);
    try {
      const auth = await requestSub2ApiDesktopAuth(baseUrl);
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
      setLoginOpening(false);
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-[620px] max-h-[85vh] overflow-hidden flex flex-col">
        <DialogHeader>
          <DialogTitle>{t("accounts.editor.editTitle")}</DialogTitle>
          <DialogDescription>{t("accounts.editor.description")}</DialogDescription>
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
                <div className="flex items-center gap-2 rounded-lg border bg-muted/20 px-3 py-2">
                  <Badge variant={provider === "newapi" ? "secondary" : "outline"}>
                    {t(`accounts.providers.${provider}`)}
                  </Badge>
                  <span className="text-sm text-muted-foreground">{t("accounts.editor.providerLocked")}</span>
                </div>

                {account?.provider === "sub2api" && account.reauth_required ? (
                  <div className="rounded-lg border border-destructive/40 bg-destructive/5 px-3 py-2 text-sm text-destructive">
                    <div className="font-medium">{t("accounts.editor.reauthRequiredTitle")}</div>
                    <div>{t("accounts.editor.reauthRequiredHint")}</div>
                  </div>
                ) : null}

                <FormField
                  control={form.control}
                  name="base_url"
                  render={({ field }) => (
                    <FormItem>
                      <FormLabel>{t("accounts.editor.baseUrl")}</FormLabel>
                      <FormControl>
                        <Input {...field} placeholder="https://api.example.com" />
                      </FormControl>
                      <FormMessage />
                    </FormItem>
                  )}
                />

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
                            <Input
                              {...field}
                              type="password"
                              placeholder={storedTokenConfigured ? t("accounts.editor.userTokenKeepHint") : "sk-..."}
                            />
                          </FormControl>
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
                            onClick={() => void handleOpenLoginPage()}
                            disabled={saving || loginOpening}
                          >
                            {loginOpening ? t("accounts.editor.openLoginPageOpening") : t("accounts.editor.openLoginPage")}
                          </Button>
                        </div>
                        <div className="rounded-md border bg-muted/20 px-3 py-2 text-sm">
                          {bearerToken.trim()
                            ? t("accounts.editor.bearerTokenCaptured")
                            : storedTokenConfigured
                              ? t("accounts.editor.bearerTokenExisting")
                              : t("accounts.editor.bearerTokenMissing")}
                        </div>
                        <FormDescription>{t("accounts.editor.bearerTokenHint")}</FormDescription>
                        <FormMessage />
                      </FormItem>
                    )}
                  />
                )}

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
                          onValueChange={(value) => field.onChange(value as AccountCheckinModeOption)}
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
            </DialogBody>

            <DialogFooter>
              <Button type="button" variant="outline" onClick={() => onOpenChange(false)} disabled={saving}>
                {t("common.cancel")}
              </Button>
              <Button type="submit" disabled={saving}>
                {t("common.save")}
              </Button>
            </DialogFooter>
          </form>
        </Form>
      </DialogContent>
    </Dialog>
  );
}
