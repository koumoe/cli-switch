import React from "react";
import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { renderWithProviders } from "@/test/render";

import { ProjectMarkdownEditor } from "./ProjectMarkdownEditor";

vi.mock("@mdxeditor/editor", async () => {
  const React = await import("react");

  const NullComponent = () => null;

  const MDXEditor = React.forwardRef<unknown, {
    markdown: string;
    onChange: (markdown: string, initialMarkdownNormalize?: boolean) => void;
  }>((props, ref) => {
    const markdownRef = React.useRef(props.markdown);

    React.useEffect(() => {
      markdownRef.current = props.markdown;
    }, [props.markdown]);

    React.useImperativeHandle(ref, () => ({
      getMarkdown: () => markdownRef.current,
      setMarkdown: (next: string) => {
        markdownRef.current = next;
      },
    }));

    return (
      <div>
        <button
          type="button"
          onClick={() => props.onChange("normalized markdown", true)}
        >
          normalize
        </button>
        <button
          type="button"
          onClick={() => props.onChange("edited markdown", false)}
        >
          edit
        </button>
      </div>
    );
  });

  return {
    MDXEditor,
    BlockTypeSelect: NullComponent,
    BoldItalicUnderlineToggles: NullComponent,
    CodeToggle: NullComponent,
    CreateLink: NullComponent,
    DiffSourceToggleWrapper: ({ children }: { children: React.ReactNode }) => (
      <>{children}</>
    ),
    InsertCodeBlock: NullComponent,
    InsertThematicBreak: NullComponent,
    ListsToggle: NullComponent,
    Separator: NullComponent,
    UndoRedo: NullComponent,
    codeBlockPlugin: () => ({}),
    codeMirrorPlugin: () => ({}),
    diffSourcePlugin: () => ({}),
    headingsPlugin: () => ({}),
    linkDialogPlugin: () => ({}),
    linkPlugin: () => ({}),
    listsPlugin: () => ({}),
    markdownShortcutPlugin: () => ({}),
    quotePlugin: () => ({}),
    thematicBreakPlugin: () => ({}),
    toolbarPlugin: () => ({}),
  };
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
