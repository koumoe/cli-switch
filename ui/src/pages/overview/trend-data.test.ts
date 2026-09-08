import { describe, expect, it } from "vitest";

import type { Channel, TrendPoint } from "@/types/api";
import { buildMonthDays, buildMonthTrend } from "./trend-data";

const start = new Date(2026, 0, 1).getTime();
const channel = (id: string, accountId?: string) => ({
  id, name: "OpenAI 20X", managed_remote_account_id: accountId,
}) as Channel;
const point = (channelId: string, day: number, success: number): TrendPoint => ({
  channel_id: channelId, name: "OpenAI 20X", success,
  bucket_start_ms: new Date(2026, 0, day, 12).getTime(),
});

describe("monthly request trend", () => {
  it("includes only elapsed local dates and stops at the month boundary", () => {
    expect(buildMonthDays(start, new Date(2026, 0, 1, 23))).toEqual([
      { key: "2026-01-01", label: "1" },
    ]);
    const previousMonth = buildMonthDays(new Date(2025, 11, 1).getTime(), new Date(2026, 0, 1));
    expect(previousMonth).toHaveLength(31);
    expect(previousMonth.at(-1)?.key).toBe("2025-12-31");
    expect(buildMonthDays(new Date(2028, 1, 1).getTime(), new Date(2028, 2, 1))).toHaveLength(29);
  });

  it("preserves distinct channel IDs, fills missing dates, and excludes future observations", () => {
    const result = buildMonthTrend({
      startMs: start, now: new Date(2026, 0, 3),
      channels: [channel("a", "account-a"), channel("b", "account-b")],
      accountNames: { "account-a": "Gmail", "account-b": "Icloud" },
      items: [point("a", 1, 12), point("a", 1, 6), point("a", 3, 4), point("b", 2, 8), point("a", 4, 999)],
      colorForChannel: (id) => id,
    });
    expect(result.series).toHaveLength(2);
    expect(result.series[0]).toMatchObject({ channel_id: "a", account_name: "Gmail", name: "OpenAI 20X", values: [18, 0, 4] });
    expect(result.series[1]).toMatchObject({ channel_id: "b", account_name: "Icloud", values: [0, 8, 0] });
    expect(result.days).toHaveLength(3);
    expect(result.series.flatMap((item) => item.values).reduce((sum, n) => sum + n, 0)).toBe(30);
  });

  it("uses current account and channel names without guessing a missing association", () => {
    const inputs = {
      startMs: start, now: new Date(2026, 0, 1),
      items: [point("a", 1, 1), point("deleted", 1, 1)],
      channels: [{ ...channel("a", "account-a"), name: "Renamed channel" }],
      colorForChannel: () => "blue",
    };
    const before = buildMonthTrend({ ...inputs, accountNames: { "account-a": "Before" } });
    const after = buildMonthTrend({ ...inputs, accountNames: { "account-a": "After" } });
    expect(before.series[0].account_name).toBe("Before");
    expect(after.series[0]).toMatchObject({ account_name: "After", name: "Renamed channel" });
    expect(after.series[1]).toMatchObject({ account_name: "—", name: "OpenAI 20X" });
    expect(after.series.map((item) => item.values)).toEqual(before.series.map((item) => item.values));
  });

  it("keeps an empty state for a month with no successful requests", () => {
    expect(buildMonthTrend({
      startMs: start, now: new Date(2026, 0, 1),
      items: [point("a", 1, 0)], channels: [], accountNames: {}, colorForChannel: () => "blue",
    }).series).toEqual([]);
  });
});
