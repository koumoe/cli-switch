import { describe, expect, it } from "vitest";

import { resolveChannelIdentity, resolveManagedChannelAccountName } from "./channel-display";

describe("resolveManagedChannelAccountName", () => {
  it("returns the configured account name for a managed channel", () => {
    expect(
      resolveManagedChannelAccountName(
        { managed_remote_account_id: "account-1" },
        { "account-1": "Hoxkai" },
      ),
    ).toBe("Hoxkai");
  });

  it("returns a placeholder when the account link cannot be resolved", () => {
    expect(resolveManagedChannelAccountName(undefined, {})).toBe("-");
    expect(
      resolveManagedChannelAccountName(
        { managed_remote_account_id: "missing-account" },
        {},
      ),
    ).toBe("-");
  });
});

describe("resolveChannelIdentity", () => {
  it("uses account names and current channel names with an explicit fallback for deleted channels", () => {
    expect(resolveChannelIdentity({ name: "Codex", managed_remote_account_id: "a" }, { a: " Gmail " })).toEqual({
      accountName: "Gmail", channelName: "Codex", label: "Gmail · Codex",
    });
    expect(resolveChannelIdentity(undefined, { a: "Gmail" }, "Deleted channel")).toEqual({
      accountName: "—", channelName: "Deleted channel", label: "— · Deleted channel",
    });
    expect(resolveChannelIdentity({ name: "Codex", managed_remote_account_id: "a" }, { a: " " }).accountName).toBe("—");
  });
});
