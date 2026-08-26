import { z } from "zod";

import {
  nonNegativeNumberStringSchema,
  optionalHttpUrlSchema,
  rechargeCurrencySchema,
  requiredHttpUrlSchema,
  isDailyTime,
  type Translate,
} from "@/lib/schemas/common";

export function createAccountFormSchema(t: Translate) {
  return z
    .object({
      name: z.string().trim(),
      provider: z.enum(["newapi", "sub2api", "openai"]),
      base_url: requiredHttpUrlSchema(
        t("accounts.toast.baseUrlRequired"),
        t("accounts.toast.baseUrlInvalid"),
      ),
      api_url: optionalHttpUrlSchema(t("accounts.toast.apiUrlInvalid")),
      user_id: z.string().trim(),
      user_token: z.string().trim(),
      bearer_token: z.string().trim(),
      refresh_token: z.string().trim(),
      page_checkin_url: optionalHttpUrlSchema(t("accounts.toast.pageCheckinUrlInvalid")),
      checkin_mode: z.enum(["disabled", "system_api", "page_open"]),
      auto_checkin_time: z
        .string()
        .trim()
        .refine((value) => isDailyTime(value), {
          message: t("accounts.toast.autoCheckinTimeInvalid"),
        }),
      low_balance_alert_threshold: nonNegativeNumberStringSchema(t("accounts.toast.thresholdInvalid")),
      recharge_currency: rechargeCurrencySchema,
      stored_token_configured: z.boolean(),
    })
    .superRefine((values, ctx) => {
      if (values.checkin_mode === "page_open" && !values.page_checkin_url) {
        ctx.addIssue({
          code: z.ZodIssueCode.custom,
          path: ["page_checkin_url"],
          message: t("accounts.toast.pageCheckinUrlRequired"),
        });
      }

      if (
        values.provider === "newapi"
        && values.checkin_mode === "system_api"
        && (!values.user_id || (!values.user_token && !values.stored_token_configured))
      ) {
        const message = t("accounts.toast.credentialsRequiredForSystem");
        if (!values.user_id) {
          ctx.addIssue({
            code: z.ZodIssueCode.custom,
            path: ["user_id"],
            message,
          });
        }
        if (!values.user_token && !values.stored_token_configured) {
          ctx.addIssue({
            code: z.ZodIssueCode.custom,
            path: ["user_token"],
            message,
          });
        }
      }

      if (
        values.provider === "sub2api"
        && !values.bearer_token
        && !values.stored_token_configured
      ) {
        ctx.addIssue({
          code: z.ZodIssueCode.custom,
          path: ["bearer_token"],
          message: t("accounts.toast.bearerTokenRequired"),
        });
      }
    });
}
