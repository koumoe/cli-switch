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
