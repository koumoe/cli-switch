import React from "react";

import {
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

import type { AccountCheckinModeOption, AccountDraft } from "./shared";

type AccountEditorDialogProps = {
  open: boolean;
  mode: "create" | "edit";
  draft: AccountDraft;
  saving: boolean;
  onOpenChange: (open: boolean) => void;
  setDraft: React.Dispatch<React.SetStateAction<AccountDraft>>;
  onSave: () => void | Promise<void>;
};

export function AccountEditorDialog({
  open,
  mode,
  draft,
  saving,
  onOpenChange,
  setDraft,
  onSave,
}: AccountEditorDialogProps) {
  const { t } = useI18n();

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-[560px] max-h-[85vh] overflow-hidden flex flex-col">
        <DialogHeader>
          <DialogTitle>
            {mode === "create" ? t("accounts.editor.createTitle") : t("accounts.editor.editTitle")}
          </DialogTitle>
          <DialogDescription>{t("accounts.editor.description")}</DialogDescription>
        </DialogHeader>

        <div className="flex-1 min-h-0 space-y-4 py-2 overflow-y-auto pr-1">
          <div className="space-y-2">
            <label className="text-sm font-medium">{t("accounts.editor.baseUrl")}</label>
            <Input
              value={draft.base_url}
              onChange={(e) => setDraft((d) => ({ ...d, base_url: e.target.value }))}
              placeholder="https://new-api.example.com"
            />
          </div>
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
              placeholder={mode === "edit" ? t("accounts.editor.userTokenKeepHint") : "sk-..."}
            />
          </div>
          {draft.checkin_mode === "page_open" ? (
            <div className="space-y-2">
              <label className="text-sm font-medium">{t("accounts.editor.pageCheckinUrl")}</label>
              <Input
                value={draft.page_checkin_url}
                onChange={(e) => setDraft((d) => ({ ...d, page_checkin_url: e.target.value }))}
                placeholder="https://new-api.example.com/user/checkin"
              />
            </div>
          ) : null}
          <div className={draft.checkin_mode === "system_api" ? "grid grid-cols-2 gap-4" : "space-y-2"}>
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
                  <SelectItem value="disabled">{t("accounts.checkin.modeDisabled")}</SelectItem>
                  <SelectItem value="system_api">{t("accounts.checkin.modeSystem")}</SelectItem>
                  <SelectItem value="page_open">{t("accounts.checkin.modePage")}</SelectItem>
                </SelectContent>
              </Select>
            </div>
            {draft.checkin_mode === "system_api" ? (
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
