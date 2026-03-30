import type {
  NewapiRemoteAccount,
  Protocol,
  RechargeCurrency,
  RemoteAccount,
  RemoteAccountBase,
  RemoteAccountCheckinMode,
  RemoteAccountProvider,
  RemoteGroupOption,
  Sub2ApiRemoteAccount,
} from "@/api";

export type AccountCheckinModeOption = RemoteAccountCheckinMode;

export type AccountDraft = {
  provider: RemoteAccountProvider;
  base_url: string;
  api_url: string;
  user_id: string;
  user_token: string;
  bearer_token: string;
  page_checkin_url: string;
  checkin_mode: AccountCheckinModeOption;
  auto_checkin_time: string;
  low_balance_alert_threshold: string;
  recharge_currency: RechargeCurrency;
};

export type ManagedChannelDraft = {
  name: string;
  protocol: Protocol | null;
  group_name: string;
  group_id: number | null;
  base_url_override: string;
};

export function emptyAccountDraft(rechargeCurrency: RechargeCurrency = "CNY"): AccountDraft {
  return {
    provider: "newapi",
    base_url: "",
    api_url: "",
    user_id: "",
    user_token: "",
    bearer_token: "",
    page_checkin_url: "",
    checkin_mode: "disabled",
    auto_checkin_time: "00:05:00",
    low_balance_alert_threshold: "0",
    recharge_currency: rechargeCurrency,
  };
}

export function ymdLocal(ms: number): string {
  const d = new Date(ms);
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${y}-${m}-${day}`;
}

export function formatAmount(account: RemoteAccount, v: number | null): string {
  if (v === null || v === undefined || !Number.isFinite(v)) return "-";
  if (account.provider === "sub2api") {
    return `${account.recharge_currency === "CNY" ? "¥" : "$"}${v.toFixed(2)}`;
  }
  if (account.quota_display_type === "TOKENS") return v.toFixed(0);
  const symbol = account.quota_display_type === "CNY"
    ? "¥"
    : account.quota_display_type === "CUSTOM"
      ? (account.custom_currency_symbol || "¤")
      : "$";
  return `${symbol}${v.toFixed(2)}`;
}

export function defaultManagedName(account: RemoteAccount, protocol: Protocol | null): string {
  const userId = account.user_id.trim() || account.remote_username?.trim() || "remote";
  if (!protocol) return userId;
  return `${userId}-${protocol}`;
}

export function defaultManagedDraft(account: RemoteAccount): ManagedChannelDraft {
  return {
    name: defaultManagedName(account, null),
    group_name: account.provider === "newapi" ? (account.remote_group ?? "") : "",
    group_id: null,
    protocol: null,
    base_url_override: "",
  };
}

export function formatGroupLabel(group: Pick<RemoteGroupOption, "name" | "ratio">): string {
  const ratio = group.ratio !== null && group.ratio !== undefined ? ` (x${group.ratio})` : "";
  return `${group.name}${ratio}`;
}

export function resolveCheckinMode(account: RemoteAccount): AccountCheckinModeOption {
  return account.checkin_mode;
}

export function providerSupportsSystemCheckin(provider: RemoteAccountProvider): boolean {
  return provider === "newapi";
}

export function supportedCheckinModes(provider: RemoteAccountProvider): AccountCheckinModeOption[] {
  return providerSupportsSystemCheckin(provider)
    ? ["disabled", "system_api", "page_open"]
    : ["disabled", "page_open"];
}

export function accountHasUserApiCredentials(
  account: Pick<RemoteAccountBase, "user_id" | "user_token_configured"> & Pick<RemoteAccount, "provider">
): boolean {
  if (account.provider === "newapi") {
    return !!account.user_id.trim() && !!account.user_token_configured;
  }
  return !!account.user_token_configured;
}

export function isNewApiAccount(account: RemoteAccount): account is NewapiRemoteAccount {
  return account.provider === "newapi";
}

export function isSub2ApiAccount(account: RemoteAccount): account is Sub2ApiRemoteAccount {
  return account.provider === "sub2api";
}

export function resolveAccountDisplayName(
  account: Pick<RemoteAccountBase, "base_url" | "remote_display_name" | "remote_username" | "user_id">
): string {
  return account.remote_display_name?.trim()
    || account.remote_username?.trim()
    || account.user_id.trim()
    || account.base_url;
}

export type DragState = {
  dragId: string | null;
  dragOverId: string | null;
  snapshot: RemoteAccount[] | null;
};

export type DragAction =
  | { type: "start"; dragId: string; snapshot: RemoteAccount[] }
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

export function moveInList(list: RemoteAccount[], fromId: string, toId: string): RemoteAccount[] {
  if (fromId === toId) return list;
  const fromIdx = list.findIndex((item) => item.id === fromId);
  const toIdx = list.findIndex((item) => item.id === toId);
  if (fromIdx < 0 || toIdx < 0 || fromIdx === toIdx) return list;
  const next = [...list];
  const [item] = next.splice(fromIdx, 1);
  next.splice(toIdx, 0, item);
  return next;
}

export function moveToEndList(list: RemoteAccount[], fromId: string): RemoteAccount[] {
  const fromIdx = list.findIndex((item) => item.id === fromId);
  if (fromIdx < 0) return list;
  const next = [...list];
  const [item] = next.splice(fromIdx, 1);
  next.push(item);
  return next;
}
