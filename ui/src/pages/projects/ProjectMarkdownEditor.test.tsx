import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { renderWithProviders } from "@/test/render";

import { ProjectMarkdownEditor } from "./ProjectMarkdownEditor";

vi.mock("@mdxeditor/editor", async () => {
  const { createMdxEditorMock } = await import("@/test/mocks/mdxeditor");
  return createMdxEditorMock();
});

describe("ProjectMarkdownEditor", () => {
  it("ignores markdown changes caused by MDXEditor initial normalization", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();

    renderWithProviders(
      <ProjectMarkdownEditor
        value={"# Title\n\n"}
        placeholder="Write markdown"
        onChange={onChange}
      />
    );

    await user.click(screen.getByRole("button", { name: "normalize" }));

    expect(onChange).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: "edit" }));

    expect(onChange).toHaveBeenCalledTimes(1);
    expect(onChange).toHaveBeenCalledWith("edited markdown");
  });
});
