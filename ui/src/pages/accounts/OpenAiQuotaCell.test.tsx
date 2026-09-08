import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";

import { renderWithProviders } from "@/test/render";
import type { OpenAiQuotaWindow } from "@/types/api";

import { OpenAiQuotaCell, selectMainQuotaWindow } from "./OpenAiQuotaCell";
import { openAiAccount } from "./test-fixtures";

function quota(kind: string, minutes: number, percent: number): OpenAiQuotaWindow {
  return { kind, window_minutes: minutes, used_percent: percent, limit_name: null, resets_at_ms: null };
}

describe("OpenAI main quota summary", () => {
  it.each([
    [300, 10_080],
    [10_080, 43_200],
  ])("selects the shortest main window (%i / %i minutes), not the lowest usage or an additional pool", (short, long) => {
    const shortest = quota("secondary", short, 87);
    expect(selectMainQuotaWindow([
      quota("additional", 60, 1), quota("primary", long, 12), shortest,
    ])).toBe(shortest);
  });

  it("ignores invalid main periods and never substitutes an additional quota", () => {
    expect(selectMainQuotaWindow([
      quota("primary", 0, 42), quota("secondary", Number.NaN, 12), quota("additional", 300, 16),
    ])).toBeUndefined();
  });

  it("still exposes additional details when the main quota is missing", async () => {
    const user = userEvent.setup();
    renderWithProviders(<OpenAiQuotaCell account={{
      ...openAiAccount,
      quota_windows: [{ ...quota("additional", 300, 16), limit_name: "Extra" }],
    }} />);
    const trigger = screen.getByRole("button", { name: "查看额度详情" });
    expect(trigger).toHaveTextContent("—");
    await user.hover(trigger);
    expect((await screen.findAllByText("Extra"))[0]).toBeInTheDocument();
    expect(screen.getAllByText("16%")[0]).toBeInTheDocument();
    expect(screen.getAllByLabelText("未提供重置时间")[0]).toHaveTextContent("—");
    expect(screen.queryByText("主额度")).not.toBeInTheDocument();
  });
});
