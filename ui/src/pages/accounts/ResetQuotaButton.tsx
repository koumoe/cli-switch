import { useRef, useState } from "react";

import { Button, Popover, PopoverContent, PopoverTrigger } from "@/components/ui";
import { badgeVariants } from "@/components/ui/badge";
import { useI18n } from "@/hooks/use-i18n";
import { cn } from "@/lib/utils";
import type { OpenAiRemoteAccount } from "@/types/api";

export const accountStatusBadgeClass = "h-5 min-w-[58px] justify-center py-0";

type Props = {
  account: OpenAiRemoteAccount;
  busy: boolean;
  pending: boolean;
  refreshRequired: boolean;
  onReset: (account: OpenAiRemoteAccount) => Promise<boolean>;
};

export function ResetQuotaButton({ account, busy, pending, refreshRequired, onReset }: Props) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  const cancelRef = useRef<HTMLButtonElement>(null);
  const count = account.quota_reset_available_count;
  const available = typeof count === "number" && Number.isFinite(count) && count > 0;
  const disabled = (!available && !pending) || busy || refreshRequired || account.reauth_required;
  const title = refreshRequired ? t("accounts.reset.refreshRequired")
    : busy ? t("accounts.reset.busy")
    : account.reauth_required ? t("accounts.table.reauthRequired")
    : pending ? t("accounts.reset.retryHint")
    : count === 0 ? t("accounts.reset.noCredit")
    : !available ? t("accounts.reset.unavailable")
    : t("accounts.reset.action");

  return (
    <Popover open={open} onOpenChange={(value) => { if (!busy) setOpen(value); }}>
      <PopoverTrigger asChild>
        <button
          type="button"
          className={cn(
            badgeVariants({ variant: "secondary" }), accountStatusBadgeClass,
            "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-not-allowed disabled:text-muted-foreground",
            !disabled && "bg-primary/10 text-primary hover:bg-primary/15",
          )}
          disabled={disabled}
          title={title}
          aria-busy={busy}
        >
          {t("accounts.reset.action")}
        </button>
      </PopoverTrigger>
      <PopoverContent
        align="end"
        className="w-[292px] max-w-[calc(100vw-2rem)] p-3"
        aria-label={t("accounts.reset.confirm")}
        onOpenAutoFocus={(event) => { event.preventDefault(); cancelRef.current?.focus(); }}
        onEscapeKeyDown={(event) => { if (busy) event.preventDefault(); }}
        onInteractOutside={(event) => { if (busy) event.preventDefault(); }}
      >
        <p className="text-xs leading-relaxed text-muted-foreground">
          {t("accounts.reset.description", { name: account.name.trim() || "—", count: count ?? "—" })}
        </p>
        {pending && <p className="mt-2 text-xs text-muted-foreground">{t("accounts.reset.retryHint")}</p>}
        <div className="mt-3 flex justify-end gap-2">
          <Button ref={cancelRef} variant="ghost" size="sm" disabled={busy} onClick={() => setOpen(false)}>
            {t("common.cancel")}
          </Button>
          <Button
            size="sm"
            disabled={disabled}
            aria-busy={busy}
            onClick={async () => { if (await onReset(account)) setOpen(false); }}
          >
            {t("accounts.reset.confirm")}
          </Button>
        </div>
      </PopoverContent>
    </Popover>
  );
}
