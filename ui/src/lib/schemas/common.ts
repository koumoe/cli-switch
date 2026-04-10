import { z } from "zod";

export type Translate = (key: string, vars?: Record<string, string | number>) => string;

export const rechargeCurrencySchema = z.enum(["CNY", "USD"]);

export function isHttpUrl(value: string): boolean {
  try {
    const parsed = new URL(value);
    return parsed.protocol === "http:" || parsed.protocol === "https:";
  } catch {
    return false;
  }
}

export function isDailyTime(value: string): boolean {
  const match = /^(\d{1,2}):(\d{2}):(\d{2})$/.exec(value.trim());
  if (!match) return false;

  const hours = Number.parseInt(match[1], 10);
  const minutes = Number.parseInt(match[2], 10);
  const seconds = Number.parseInt(match[3], 10);

  return (
    Number.isInteger(hours)
    && Number.isInteger(minutes)
    && Number.isInteger(seconds)
    && hours >= 0
    && hours <= 23
    && minutes >= 0
    && minutes <= 59
    && seconds >= 0
    && seconds <= 59
  );
}

export function requiredHttpUrlSchema(requiredMessage: string, invalidMessage: string) {
  return z
    .string()
    .trim()
    .min(1, requiredMessage)
    .refine((value) => isHttpUrl(value), { message: invalidMessage });
}

export function optionalHttpUrlSchema(invalidMessage: string) {
  return z
    .string()
    .trim()
    .refine((value) => value.length === 0 || isHttpUrl(value), { message: invalidMessage });
}

export function nonNegativeNumberStringSchema(message: string) {
  return z
    .string()
    .trim()
    .refine((value) => {
      if (!value) return false;
      const number = Number(value);
      return Number.isFinite(number) && number >= 0;
    }, { message });
}

export function integerStringSchema(message: string, options?: { min?: number; max?: number }) {
  return z
    .string()
    .trim()
    .refine((value) => {
      if (!/^-?\d+$/.test(value)) return false;
      const number = Number.parseInt(value, 10);
      if (!Number.isSafeInteger(number)) return false;
      if (options?.min !== undefined && number < options.min) return false;
      if (options?.max !== undefined && number > options.max) return false;
      return true;
    }, { message });
}

export function nonNegativeTwoDecimalNumberStringSchema(message: string) {
  return z
    .string()
    .trim()
    .refine((value) => {
      if (!value) return false;
      if (!/^\d+(\.\d{0,2})?$/.test(value)) return false;
      const number = Number(value);
      return Number.isFinite(number) && number >= 0;
    }, { message });
}
