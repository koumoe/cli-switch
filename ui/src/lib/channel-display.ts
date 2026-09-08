type ManagedChannelAccountReference = {
  managed_remote_account_id?: string | null;
};

export function resolveManagedChannelAccountName(
  channel: ManagedChannelAccountReference | null | undefined,
  accountNames: Readonly<Record<string, string>>,
): string {
  const accountId = channel?.managed_remote_account_id;
  return accountId ? (accountNames[accountId] ?? "-") : "-";
}

export type ChannelIdentity = {
  accountName: string;
  channelName: string;
  label: string;
};

export function resolveChannelIdentity(
  channel: (ManagedChannelAccountReference & { name: string }) | null | undefined,
  accountNames: Readonly<Record<string, string>>,
  fallbackChannelName = "—",
): ChannelIdentity {
  const resolvedName = resolveManagedChannelAccountName(channel, accountNames).trim();
  const accountName = !resolvedName || resolvedName === "-" ? "—" : resolvedName;
  const channelName = channel?.name ?? fallbackChannelName;
  return { accountName, channelName, label: `${accountName} · ${channelName}` };
}
