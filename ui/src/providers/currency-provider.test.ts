import { describe, expect, it } from "vitest";

import {
  calculateEstimatedSpend,
  convertCurrency,
} from "./currency-provider";

describe("currency conversion", () => {
  it("keeps amounts unchanged when currencies match without requiring a rate", () => {
    expect(convertCurrency(25.064233, "USD", "USD", null)).toBe(25.064233);
    expect(convertCurrency(11.861332, "CNY", "CNY", null)).toBe(11.861332);
  });

  it("converts USD and CNY in both directions", () => {
    expect(convertCurrency(25, "USD", "CNY", 6.72)).toBeCloseTo(168, 10);
    expect(convertCurrency(168, "CNY", "USD", 6.72)).toBeCloseTo(25, 10);
  });

  it("returns null for cross-currency conversion without a valid rate", () => {
    expect(convertCurrency(25, "USD", "CNY", null)).toBeNull();
    expect(convertCurrency(25, "USD", "CNY", 0)).toBeNull();
  });

  it("applies the multiplier in the recharge currency before display conversion", () => {
    expect(
      calculateEstimatedSpend(25.064233, 1, "USD", "CNY", 6.72),
    ).toBeCloseTo(168.43164576, 10);
    expect(
      calculateEstimatedSpend(23.722663, 0.5, "CNY", "CNY", 6.72),
    ).toBeCloseTo(11.8613315, 10);
  });

  it("preserves free channels as zero spend", () => {
    expect(calculateEstimatedSpend(100, 0, "USD", "CNY", 6.72)).toBe(0);
  });
});
