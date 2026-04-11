import { useState } from "react";
import { fireEvent, screen } from "@testing-library/react";
import type { DateRange } from "react-day-picker";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { renderWithProviders } from "@/test/render";

import { DateRangePicker } from "./date-range-picker";

function DateRangePickerExample() {
  const [value, setValue] = useState<DateRange | undefined>();

  return <DateRangePicker value={value} onChange={setValue} />;
}

describe("DateRangePicker", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date(2026, 3, 10, 12, 0, 0));
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("applies the today preset using localized labels", async () => {
    renderWithProviders(<DateRangePickerExample />);

    fireEvent.click(screen.getByRole("button", { name: "选择日期范围" }));
    fireEvent.click(screen.getByRole("button", { name: "今日" }));

    expect(screen.getByRole("button", { name: /2026-04-10/ })).toBeInTheDocument();
  });
});
