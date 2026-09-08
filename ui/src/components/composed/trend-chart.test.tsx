import type { ApexOptions } from "apexcharts";
import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { TrendChart, type TrendChartSeries } from "./trend-chart";

const chart = vi.hoisted(() => ({
  props: null as null | { options: ApexOptions; series: { name: string; data: number[] }[] },
}));
vi.mock("apexcharts/line", () => ({}));
vi.mock("react-apexcharts/core", () => ({
  default: (props: NonNullable<typeof chart.props>) => {
    chart.props = props;
    return <div data-testid="apex-chart" />;
  },
}));

const labels = {
  empty: "当日无成功请求", origin: "绘图起点", name: "名称", channel: "渠道", requests: "当日成功请求",
  omitted: (count: number) => `另有 ${count} 个渠道`,
};
const series: TrendChartSeries[] = [
  { channel_id: "a", account_name: "Gmail", name: "OpenAI 20X", color: "blue", values: [12000] },
  { channel_id: "b", account_name: "Icloud", name: "OpenAI 20X", color: "red", values: [6000] },
];

function tooltip(index: number): string {
  const custom = chart.props!.options.tooltip!.custom;
  if (typeof custom !== "function") throw new Error("Missing chart tooltip");
  return custom({ dataPointIndex: index });
}

describe("TrendChart", () => {
  it("draws a synthetic zero point and a visible first-day marker without modifying source values", () => {
    render(<TrendChart days={[{ key: "2026-01-01", label: "1" }]} series={series} originLabel="起点" tooltipLabels={labels} />);
    expect(chart.props!.series).toEqual([
      { name: "a", data: [0, 12000] }, { name: "b", data: [0, 6000] },
    ]);
    expect(series[0].values).toEqual([12000]);
    expect(chart.props!.options.markers!.discrete).toEqual([
      expect.objectContaining({ seriesIndex: 0, dataPointIndex: 1, size: 4 }),
      expect.objectContaining({ seriesIndex: 1, dataPointIndex: 1, size: 4 }),
    ]);
    const axis = chart.props!.options.yaxis;
    expect(Array.isArray(axis)).toBe(false);
    if (!Array.isArray(axis)) expect(axis!.labels!.maxWidth).toBeGreaterThan(28);
    expect(screen.getByTitle("Gmail · OpenAI 20X")).toBeInTheDocument();
    expect(screen.getByTitle("Icloud · OpenAI 20X")).toBeInTheDocument();
    expect(tooltip(0)).toContain("绘图起点");
    expect(tooltip(0)).not.toContain("Gmail");
    const container = document.createElement("div");
    container.innerHTML = tooltip(1);
    expect([...container.querySelectorAll("th")].map((cell) => cell.textContent)).toEqual(["名称", "渠道", "当日成功请求"]);
    expect(container.querySelectorAll("tbody tr")).toHaveLength(2);
    expect(container.textContent).toContain("12,000");
  });

  it("escapes user labels and resolves x-axis labels without an Apex data point index", () => {
    render(<TrendChart days={[{ key: "2026-01-01", label: "1" }, { key: "2026-01-02", label: "2" }]}
      series={[{ ...series[0], account_name: "<script>bad</script>", name: "<b>channel</b>", values: [8, 0] }]}
      originLabel="起点" tooltipLabels={labels} />);
    expect(tooltip(1)).toContain("&lt;script&gt;bad&lt;/script&gt;");
    expect(tooltip(1)).not.toContain("<script>");
    expect(tooltip(2)).toContain("当日无成功请求");
    expect(tooltip(3)).toBe("");
    const formatter = chart.props!.options.xaxis!.labels!.formatter!;
    expect(formatter("__origin__")).toBe("起点");
    expect(formatter("2026-01-01")).toBe("1");
    expect(formatter("2026-01-02")).toBe("2");
  });
});
