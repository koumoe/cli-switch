import { describe, expect, it } from "vitest";

import type { RemoteAccount } from "@/types/api";

import { accountToFormValues, emptyAccountFormValues } from "./shared";

describe("account form values", () => {
  it("initializes a new account with an empty name", () => {
    expect(emptyAccountFormValues("USD")).toMatchObject({
      name: "",
      recharge_currency: "USD",
      stored_token_configured: false,
    });
  });

  it("copies an existing account name into the edit form", () => {
    const account = {
      name: "Primary account",
      provider: "newapi",
      base_url: "https://api.example.com",
      api_url: null,
      user_id: "42",
      page_checkin_url: null,
      checkin_mode: "disabled",
      auto_checkin_time: null,
      low_balance_alert_threshold: 0,
      recharge_currency: "CNY",
      user_token_configured: true,
    } as RemoteAccount;

    expect(accountToFormValues(account)).toMatchObject({
      name: "Primary account",
      provider: "newapi",
      stored_token_configured: true,
    });
  });
});
