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
} from "@/types/api";

export type AccountCheckinModeOption = RemoteAccountCheckinMode;

export type AccountDraft = {
  name: string;
  provider: RemoteAccountProvider;
  base_url: string;
  api_url: string;
  user_id: string;
  user_token: string;
  bearer_token: string;
  refresh_token: string;
  page_checkin_url: string;
  checkin_mode: AccountCheckinModeOption;
  auto_checkin_time: string;
  low_balance_alert_threshold: string;
  recharge_currency: RechargeCurrency;
};

export type AccountFormValues = AccountDraft & {
  stored_token_configured: boolean;
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
    name: "",
    provider: "newapi",
    base_url: "",
    api_url: "",
    user_id: "",
    user_token: "",
    bearer_token: "",
    refresh_token: "",
    page_checkin_url: "",
    checkin_mode: "disabled",
    auto_checkin_time: "00:05:00",
    low_balance_alert_threshold: "0",
    recharge_currency: rechargeCurrency,
  };
}

export function emptyAccountFormValues(
  rechargeCurrency: RechargeCurrency = "CNY",
): AccountFormValues {
  return {
    name: account.name,
    ...emptyAccountDraft(rechargeCurrency),
    stored_token_configured: false,
  };
}

export function accountToFormValues(account: RemoteAccount): AccountFormValues {
  return {
    provider: account.provider,
    base_url: account.base_url ?? "",
    api_url: account.api_url ?? "",
    user_id: account.provider === "newapi" ? account.user_id ?? "" : "",
    user_token: "",
    bearer_token: "",
    refresh_token: "",
    page_checkin_url: account.page_checkin_url ?? "",
    checkin_mode: resolveCheckinMode(account),
    auto_checkin_time: account.auto_checkin_time ?? "00:05:00",
    low_balance_alert_threshold: String(account.low_balance_alert_threshold ?? 0),
    recharge_currency: account.recharge_currency,
    stored_token_configured: account.user_token_configured,
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

export function resolveAccountDisplayName(account: Pick<RemoteAccountBase, "name">): string {
  return account.name;
}
