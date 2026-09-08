import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { renderWithProviders } from "@/test/render";

import { ResetQuotaButton } from "./ResetQuotaButton";
import { openAiAccount } from "./test-fixtures";

const defaultProps = { account: openAiAccount, busy: false, pending: false, refreshRequired: false };

describe("OpenAI reset confirmation", () => {
  it("only consumes on explicit confirmation and has no visible heading", async () => {
    const user = userEvent.setup();
    const onReset = vi.fn().mockResolvedValue(true);
    renderWithProviders(<ResetQuotaButton {...defaultProps} onReset={onReset} />);
    await user.click(screen.getByRole("button", { name: "重置" }));
    expect(onReset).not.toHaveBeenCalled();
    expect(screen.getByRole("dialog", { name: "确认重置" })).toBeInTheDocument();
    expect(screen.getByText("消耗重置卡，重置 Personal OpenAI 用量，当前可用 2 张。")).toBeInTheDocument();
    expect(screen.queryByRole("heading")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "取消" })).toHaveFocus();
    await user.click(screen.getByRole("button", { name: "确认重置" }));
    expect(onReset).toHaveBeenCalledOnce();
    expect(onReset).toHaveBeenCalledWith(openAiAccount);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it.each([0, null, undefined])("keeps the reset label but disables it when cards are %s", (count) => {
    renderWithProviders(<ResetQuotaButton {...defaultProps} account={{ ...openAiAccount, quota_reset_available_count: count }} onReset={vi.fn()} />);
    const button = screen.getByRole("button", { name: "重置" });
    expect(button).toBeDisabled();
    expect(button).toHaveTextContent(/^重置$/);
  });

  it("allows confirming a pending attempt even after the card count becomes zero", () => {
    renderWithProviders(<ResetQuotaButton {...defaultProps} pending account={{ ...openAiAccount, quota_reset_available_count: 0 }} onReset={vi.fn()} />);
    expect(screen.getByRole("button", { name: "重置" })).toBeEnabled();
  });

  it("does not consume when dismissed by Escape", async () => {
    const user = userEvent.setup();
    const onReset = vi.fn();
    renderWithProviders(<ResetQuotaButton {...defaultProps} onReset={onReset} />);
    await user.click(screen.getByRole("button", { name: "重置" }));
    await user.keyboard("{Escape}");
    expect(onReset).not.toHaveBeenCalled();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "重置" })).toHaveFocus();
  });
});
