import { z } from "zod";

import {
  integerStringSchema,
  nonNegativeTwoDecimalNumberStringSchema,
  rechargeCurrencySchema,
  requiredHttpUrlSchema,
  type Translate,
} from "@/lib/schemas/common";

export function createChannelFormSchema(t: Translate) {
  return z.object({
    name: z.string().trim().min(1, t("channels.toast.nameRequired")),
    protocol: z.enum(["openai", "anthropic", "gemini"]),
    base_url: requiredHttpUrlSchema(
      t("channels.toast.baseUrlRequired"),
      t("channels.toast.baseUrlInvalid"),
    ),
    auth_ref: z.string(),
    checkin_url: z.string(),
    priority: integerStringSchema(t("channels.toast.priorityInvalid")),
    retry_times: integerStringSchema(t("channels.toast.retryTimesInvalid"), { min: 1 }),
    ignore_channel_protection: z.boolean(),
    recharge_currency: rechargeCurrencySchema,
    real_multiplier: nonNegativeTwoDecimalNumberStringSchema(t("channels.toast.realMultiplierInvalid")),
    enabled: z.boolean(),
  });
}
