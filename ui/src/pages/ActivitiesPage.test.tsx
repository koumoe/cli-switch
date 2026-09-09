import { fireEvent, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { getActivities } from "@/api";
import { renderWithProviders } from "@/test/render";
import { ActivitiesPage } from "./ActivitiesPage";

vi.mock("@/api", async () => ({
  ...(await vi.importActual<typeof import("@/api")>("@/api")),
  getActivities: vi.fn(),
}));

describe("ActivitiesPage", () => {
  it("keeps response finished distinct from an identified completed turn", async () => {
    vi.mocked(getActivities).mockResolvedValue({
      revision: 1,
      omitted_running: 0,
      entries: [
        { id: "request", kind: "proxy_request", status: "response_finished", title: null, thread_id: null, source: "openai", protocol: "openai", model: "gpt", project: null, started_at_ms: 1, updated_at_ms: 2, finished_at_ms: 2 },
        { id: "turn", kind: "bridge_turn", status: "completed", title: "通知轮次", thread_id: null, source: "telegram", protocol: null, model: null, project: null, started_at_ms: 1, updated_at_ms: 3, finished_at_ms: 3 },
      ],
    });
    renderWithProviders(<ActivitiesPage />);
    expect(await screen.findByText("未归属请求活动")).toBeInTheDocument();
    expect(screen.getByText("响应已结束")).toBeInTheDocument();
    expect(screen.getByText("通知轮次")).toBeInTheDocument();
    expect(screen.getByText("本轮已完成")).toBeInTheDocument();
  });

  it("reloads after the native activity changed event", async () => {
    vi.mocked(getActivities).mockResolvedValue({ revision: 1, omitted_running: 0, entries: [] });
    renderWithProviders(<ActivitiesPage />);
    await waitFor(() => expect(getActivities).toHaveBeenCalledTimes(1));
    fireEvent(window, new Event("cliswitch-activities-changed"));
    await waitFor(() => expect(getActivities).toHaveBeenCalledTimes(2));
  });
});
