import { useState } from "react";

import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui";
import { useI18n } from "@/hooks/use-i18n";
import { cn } from "@/lib/utils";
import type { OpenAiQuotaWindow, OpenAiRemoteAccount } from "@/types/api";

function isMainWindow(window: OpenAiQuotaWindow): boolean {
  return window.kind === "primary" || window.kind === "secondary";
}

function windowOrder(a: OpenAiQuotaWindow, b: OpenAiQuotaWindow): number {
  const minutes = (window: OpenAiQuotaWindow) =>
    Number.isFinite(window.window_minutes) && window.window_minutes > 0
      ? window.window_minutes
      : Number.POSITIVE_INFINITY;
  return minutes(a) - minutes(b);
}

export function selectMainQuotaWindow(windows: OpenAiQuotaWindow[]): OpenAiQuotaWindow | undefined {
  return windows
    .filter((window) => isMainWindow(window)
      && Number.isFinite(window.used_percent)
      && Number.isFinite(window.window_minutes)
      && window.window_minutes > 0)
    .sort(windowOrder)[0];
}

export function groupQuotaWindows(windows: OpenAiQuotaWindow[]) {
  const groups = new Map<string, { main: boolean; name: string | null; windows: OpenAiQuotaWindow[] }>();
  for (const window of windows) {
    const main = isMainWindow(window);
    const key = main ? "main" : `additional:${window.limit_name ?? window.kind}`;
    const group = groups.get(key) ?? { main, name: window.limit_name, windows: [] };
    group.windows.push(window);
    groups.set(key, group);
  }
  return [...groups.values()]
    .sort((a, b) => Number(b.main) - Number(a.main))
    .map((group) => ({ ...group, windows: [...group.windows].sort(windowOrder) }));
}

export function OpenAiQuotaCell({ account }: { account: OpenAiRemoteAccount }) {
  const { locale, t } = useI18n();
  const [now, setNow] = useState(Date.now);
  const windows = account.quota_windows ?? [];
  const summary = selectMainQuotaWindow(windows);
  const groups = groupQuotaWindows(windows);
  const percent = summary ? Math.round(summary.used_percent) : null;

  function period(window: OpenAiQuotaWindow): string {
    const minutes = window.window_minutes;
    if (!Number.isFinite(minutes) || minutes <= 0) return "—";
    if (minutes % 1_440 === 0) return t("accounts.quota.daysShort", { count: minutes / 1_440 });
    if (minutes % 60 === 0) return t("accounts.quota.hoursShort", { count: minutes / 60 });
    return t("accounts.quota.minutesShort", { count: minutes });
  }

  function resetTime(window: OpenAiQuotaWindow): string {
    if (!window.resets_at_ms || !Number.isFinite(new Date(window.resets_at_ms).getTime())) return "—";
    if (window.resets_at_ms <= now) return t("accounts.quota.pending");
    return new Intl.DateTimeFormat(locale, {
      month: "numeric", day: "numeric", hour: "2-digit", minute: "2-digit", hour12: false,
    }).format(new Date(window.resets_at_ms));
  }

  if (windows.length === 0) {
    return <span className="text-muted-foreground">{t("accounts.quota.unavailable")}</span>;
  }

  return (
    <Tooltip onOpenChange={(open) => { if (open) setNow(Date.now()); }}>
      <TooltipTrigger asChild>
        <button
          type="button"
          className={cn(
            "inline-flex cursor-help whitespace-nowrap rounded-sm text-sm font-medium focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
            percent === null && "text-muted-foreground",
          )}
          aria-label={percent === null
            ? t("accounts.quota.details")
            : t("accounts.quota.summaryLabel", { percent })}
        >
          {percent === null ? "—" : `${percent}%`}
        </button>
      </TooltipTrigger>
      <TooltipContent
        className="max-h-[min(24rem,var(--radix-tooltip-content-available-height))] w-[30rem] max-w-[calc(100vw-2rem)] overflow-auto border bg-popover p-3 text-popover-foreground shadow-md"
        collisionPadding={12}
      >
        <table className="w-full table-fixed border-collapse text-xs">
          <colgroup><col className="w-[40%]" /><col className="w-[15%]" /><col className="w-[13%]" /><col className="w-[32%]" /></colgroup>
          <thead className="text-left font-normal text-muted-foreground">
            <tr className="border-b">
              <th className="pb-2 pr-3 font-normal">{t("accounts.quota.pool")}</th>
              <th className="pb-2 pr-3 font-normal">{t("accounts.quota.period")}</th>
              <th className="pb-2 pr-3 font-normal">{t("accounts.quota.used")}</th>
              <th className="pb-2 text-right font-normal">{t("accounts.quota.resetTime")}</th>
            </tr>
          </thead>
          {groups.map((group, groupIndex) => (
            <tbody key={groupIndex} className={groupIndex > 0 ? "border-t" : undefined}>
              {group.windows.map((window, index) => (
                <tr key={`${window.kind}-${window.window_minutes}-${index}`}>
                  {index === 0 && (
                    <td rowSpan={group.windows.length} className="break-words py-2 pr-3 text-left align-top">
                      {group.main ? t("accounts.quota.main") : group.name || t("accounts.quota.additional")}
                    </td>
                  )}
                  <td className="whitespace-nowrap py-2 pr-3 text-left">{period(window)}</td>
                  <td className="whitespace-nowrap py-2 pr-3 text-left tabular-nums">
                    {Number.isFinite(window.used_percent) ? `${Math.round(window.used_percent)}%` : "—"}
                  </td>
                  <td
                    className="whitespace-nowrap py-2 text-right tabular-nums"
                    aria-label={!window.resets_at_ms ? t("accounts.quota.missingReset") : undefined}
                  >
                    {resetTime(window)}
                  </td>
                </tr>
              ))}
            </tbody>
          ))}
        </table>
        {account.last_sync_error && (
          <p className="mt-2 border-t pt-2 text-xs text-muted-foreground">{t("accounts.quota.syncFailed")}</p>
        )}
      </TooltipContent>
    </Tooltip>
  );
}
