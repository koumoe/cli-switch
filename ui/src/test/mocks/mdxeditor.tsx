import React from "react";

type MockMdxEditorProps = {
  markdown: string;
  onChange: (markdown: string, initialMarkdownNormalize?: boolean) => void;
};

type MockMdxEditorScenario = "editor" | "project-page";

const NullComponent = () => null;

function createMdxEditorComponent(scenario: MockMdxEditorScenario) {
  return React.forwardRef<unknown, MockMdxEditorProps>((props, ref) => {
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

    if (scenario === "project-page") {
      return (
        <div>
          <div data-testid="editor-value">{props.markdown}</div>
          <button
            type="button"
            onClick={() => props.onChange("normalized markdown", true)}
          >
            normalize editor
          </button>
        </div>
      );
    }

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
}

export function createMdxEditorMock(scenario: MockMdxEditorScenario = "editor") {
  return {
    MDXEditor: createMdxEditorComponent(scenario),
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
}
