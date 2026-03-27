import type { NewApiAccount, NewApiAccountCheckinMode, NewApiGroupOption, Protocol } from "@/api";

export type AccountCheckinModeOption = "disabled" | NewApiAccountCheckinMode;

export type AccountDraft = {
  base_url: string;
  api_url: string;
  user_id: string;
  user_token: string;
  page_checkin_url: string;
  checkin_mode: AccountCheckinModeOption;
  auto_checkin_time: string;
  low_balance_alert_threshold: string;
  recharge_currency: "USD" | "CNY";
};

export type ManagedChannelDraft = {
  name: string;
  protocol: Protocol;
  group_name: string;
  base_url_override: string;
};

export function emptyAccountDraft(): AccountDraft {
  return {
    base_url: "",
    api_url: "",
    user_id: "",
    user_token: "",
    page_checkin_url: "",
    checkin_mode: "disabled",
    auto_checkin_time: "00:05:00",
    low_balance_alert_threshold: "0",
    recharge_currency: "CNY",
  };
}

export function ymdLocal(ms: number): string {
  const d = new Date(ms);
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${y}-${m}-${day}`;
}

export function formatAmount(account: NewApiAccount, v: number | null): string {
  if (v === null || v === undefined || !Number.isFinite(v)) return "-";
  if (account.quota_display_type === "TOKENS") return v.toFixed(0);
  const symbol = account.quota_display_type === "CNY"
    ? "¥"
    : account.quota_display_type === "CUSTOM"
      ? (account.custom_currency_symbol || "¤")
      : "$";
  return `${symbol}${v.toFixed(2)}`;
}

export function defaultManagedName(account: NewApiAccount, protocol: Protocol): string {
  return `${account.user_id}-${protocol}`;
}

export function defaultManagedDraft(account: NewApiAccount): ManagedChannelDraft {
  return {
    name: defaultManagedName(account, "openai"),
    protocol: "openai",
    group_name: account.remote_group ?? "",
    base_url_override: "",
  };
}

export function formatGroupLabel(group: NewApiGroupOption): string {
  const ratio = group.ratio !== null && group.ratio !== undefined ? ` (x${group.ratio})` : "";
  return `${group.name}${ratio}`;
}

export function resolveCheckinMode(account: NewApiAccount): AccountCheckinModeOption {
  if (account.checkin_mode === "page_open") return "page_open";
  return account.auto_checkin_enabled ? "system_api" : "disabled";
}

export function accountHasUserApiCredentials(
  account: Pick<NewApiAccount, "user_id" | "user_token_configured">
): boolean {
  return !!account.user_id.trim() && !!account.user_token_configured;
}

export type DragState = {
  dragId: string | null;
  dragOverId: string | null;
  snapshot: NewApiAccount[] | null;
};

export type DragAction =
  | { type: "start"; dragId: string; snapshot: NewApiAccount[] }
  | { type: "over"; dragOverId: string | null }
  | { type: "clear" };

export const initialDragState: DragState = {
  dragId: null,
  dragOverId: null,
  snapshot: null,
};

export function dragReducer(state: DragState, action: DragAction): DragState {
  switch (action.type) {
    case "start":
      return { dragId: action.dragId, dragOverId: null, snapshot: action.snapshot };
    case "over":
      return { ...state, dragOverId: action.dragOverId };
    case "clear":
      return initialDragState;
    default: {
      const _exhaustive: never = action;
      return state;
    }
  }
}

export function moveInList(list: NewApiAccount[], fromId: string, toId: string): NewApiAccount[] {
  if (fromId === toId) return list;
  const fromIdx = list.findIndex((item) => item.id === fromId);
  const toIdx = list.findIndex((item) => item.id === toId);
  if (fromIdx < 0 || toIdx < 0 || fromIdx === toIdx) return list;
  const next = [...list];
  const [item] = next.splice(fromIdx, 1);
  next.splice(toIdx, 0, item);
  return next;
}

export function moveToEndList(list: NewApiAccount[], fromId: string): NewApiAccount[] {
  const fromIdx = list.findIndex((item) => item.id === fromId);
  if (fromIdx < 0) return list;
  const next = [...list];
  const [item] = next.splice(fromIdx, 1);
  next.push(item);
  return next;
}
