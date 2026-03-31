import React from "react";

import type { RemoteAccount } from "@/api";
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
import { useI18n } from "@/lib/i18n";

import {
  providerSupportsSystemCheckin,
  supportedCheckinModes,
  type AccountCheckinModeOption,
  type AccountDraft,
} from "./shared";

type AccountEditorDialogProps = {
  open: boolean;
  account: RemoteAccount | null;
  draft: AccountDraft;
  saving: boolean;
  loginOpening: boolean;
  onOpenChange: (open: boolean) => void;
  setDraft: React.Dispatch<React.SetStateAction<AccountDraft>>;
  onSave: () => void | Promise<void>;
  onOpenLoginPage: () => void | Promise<void>;
};

export function AccountEditorDialog({
  open,
  account,
  draft,
  saving,
  loginOpening,
  onOpenChange,
  setDraft,
  onSave,
  onOpenLoginPage,
}: AccountEditorDialogProps) {
  const { t } = useI18n();
  const modeOptions = supportedCheckinModes(draft.provider);
  const showSystemTime = providerSupportsSystemCheckin(draft.provider) && draft.checkin_mode === "system_api";

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-[620px] max-h-[85vh] overflow-hidden flex flex-col">
        <DialogHeader>
          <DialogTitle>{t("accounts.editor.editTitle")}</DialogTitle>
          <DialogDescription>{t("accounts.editor.description")}</DialogDescription>
        </DialogHeader>

        <div className="flex-1 min-h-0 space-y-4 py-2 overflow-y-auto pr-1">
          <div className="flex items-center gap-2 rounded-lg border bg-muted/20 px-3 py-2">
            <Badge variant={draft.provider === "newapi" ? "secondary" : "outline"}>
              {t(`accounts.providers.${draft.provider}`)}
            </Badge>
            <span className="text-sm text-muted-foreground">{t("accounts.editor.providerLocked")}</span>
          </div>

          <div className="space-y-2">
            <label className="text-sm font-medium">{t("accounts.editor.baseUrl")}</label>
            <Input
              value={draft.base_url}
              onChange={(e) => setDraft((d) => ({ ...d, base_url: e.target.value }))}
              placeholder="https://api.example.com"
            />
          </div>

          <div className="space-y-2">
            <label className="text-sm font-medium">{t("accounts.editor.apiUrl")}</label>
            <Input
              value={draft.api_url}
              onChange={(e) => setDraft((d) => ({ ...d, api_url: e.target.value }))}
              placeholder="https://api.example.com/v1"
            />
            <p className="text-xs text-muted-foreground">{t("accounts.editor.apiUrlHint")}</p>
          </div>

          {draft.provider === "newapi" ? (
            <>
              <div className="space-y-2">
                <label className="text-sm font-medium">{t("accounts.editor.userId")}</label>
                <Input
                  value={draft.user_id}
                  onChange={(e) => setDraft((d) => ({ ...d, user_id: e.target.value }))}
                  placeholder="1001"
                />
              </div>
              <div className="space-y-2">
                <label className="text-sm font-medium">{t("accounts.editor.userToken")}</label>
                <Input
                  type="password"
                  value={draft.user_token}
                  onChange={(e) => setDraft((d) => ({ ...d, user_token: e.target.value }))}
                  placeholder={account?.user_token_configured ? t("accounts.editor.userTokenKeepHint") : "sk-..."}
                />
              </div>
            </>
          ) : (
            <div className="space-y-2">
              <div className="flex items-center justify-between gap-3">
                <label className="text-sm font-medium">{t("accounts.editor.bearerToken")}</label>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => void onOpenLoginPage()}
                  disabled={saving || loginOpening}
                >
                  {loginOpening ? t("accounts.editor.openLoginPageOpening") : t("accounts.editor.openLoginPage")}
                </Button>
              </div>
              <div className="rounded-md border bg-muted/20 px-3 py-2 text-sm">
                {draft.bearer_token.trim()
                  ? t("accounts.editor.bearerTokenCaptured")
                  : account?.user_token_configured
                    ? t("accounts.editor.bearerTokenExisting")
                    : t("accounts.editor.bearerTokenMissing")}
              </div>
              <p className="text-xs text-muted-foreground">{t("accounts.editor.bearerTokenHint")}</p>
            </div>
          )}

          <div className="space-y-2">
            <label className="text-sm font-medium">{t("accounts.editor.rechargeCurrency")}</label>
            <Select
              value={draft.recharge_currency}
              onValueChange={(value) => {
                setDraft((d) => ({ ...d, recharge_currency: value as AccountDraft["recharge_currency"] }));
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

          <div className={showSystemTime ? "grid grid-cols-2 gap-4" : "space-y-2"}>
            <div className="space-y-2">
              <label className="text-sm font-medium">{t("accounts.editor.checkinMode")}</label>
              <Select
                value={draft.checkin_mode}
                onValueChange={(value) => {
                  setDraft((d) => ({ ...d, checkin_mode: value as AccountCheckinModeOption }));
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
            {showSystemTime ? (
              <div className="space-y-2">
                <label className="text-sm font-medium">{t("accounts.editor.autoCheckinTime")}</label>
                <Input
                  value={draft.auto_checkin_time}
                  onChange={(e) => setDraft((d) => ({ ...d, auto_checkin_time: e.target.value }))}
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
                onChange={(e) => setDraft((d) => ({ ...d, page_checkin_url: e.target.value }))}
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
              onChange={(e) => setDraft((d) => ({ ...d, low_balance_alert_threshold: e.target.value }))}
              placeholder="0"
            />
            <p className="text-xs text-muted-foreground">{t("accounts.editor.lowBalanceHint")}</p>
          </div>
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)} disabled={saving}>
            {t("common.cancel")}
          </Button>
          <Button onClick={() => void onSave()} disabled={saving}>
            {t("common.save")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
