import { describe, expect, it } from "vitest";

import { resolveManagedChannelAccountName } from "./channel-display";

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
