import { describe, expect, it } from "vitest";

import { createAccountFormSchema } from "@/lib/schemas/account";
import { createChannelFormSchema } from "@/lib/schemas/channel";
import {
  createChatBridgeBaseSchema,
  createPricingDataSchema,
} from "@/lib/schemas/settings";

const t = (key: string) => key;

describe("schema validation", () => {
  it("requires page check-in url when page_open is selected", () => {
    const result = createAccountFormSchema(t).safeParse({
      provider: "newapi",
      base_url: "https://api.example.com",
      api_url: "",
      user_id: "",
      user_token: "",
      bearer_token: "",
      refresh_token: "",
      page_checkin_url: "",
      checkin_mode: "page_open",
      auto_checkin_time: "08:00:00",
      low_balance_alert_threshold: "10",
      recharge_currency: "CNY",
      stored_token_configured: false,
    });

    expect(result.success).toBe(false);
    expect(result.error?.issues).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          path: ["page_checkin_url"],
          message: "accounts.toast.pageCheckinUrlRequired",
        }),
      ]),
    );
  });

  it("validates retry times and numeric channel fields", () => {
    const result = createChannelFormSchema(t).safeParse({
      name: "Demo channel",
      protocol: "openai",
      base_url: "https://api.example.com",
      auth_ref: "secret",
      checkin_url: "",
      priority: "1",
      retry_times: "0",
      ignore_channel_protection: false,
      recharge_currency: "USD",
      real_multiplier: "1.20",
      enabled: true,
    });

    expect(result.success).toBe(false);
    expect(result.error?.issues).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          path: ["retry_times"],
          message: "channels.toast.retryTimesInvalid",
        }),
      ]),
    );
  });

  it("enforces pricing interval upper bound", () => {
    const result = createPricingDataSchema(t).safeParse({
      pricing_auto_update_enabled: true,
      pricing_auto_update_interval_hours: "9000",
    });

    expect(result.success).toBe(false);
    expect(result.error?.issues).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          path: ["pricing_auto_update_interval_hours"],
          message: "settings.pricingData.intervalInvalid",
        }),
      ]),
    );
  });

  it("allows zero timeout minutes for chat bridge", () => {
    const result = createChatBridgeBaseSchema(t).safeParse({
      chat_bridge_enabled: true,
      chat_bridge_allow_new_projects: false,
      chat_bridge_turn_timeout_minutes: "0",
    });

    expect(result.success).toBe(true);
  });
});
