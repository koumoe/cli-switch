import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  BlockTypeSelect,
  BoldItalicUnderlineToggles,
  CodeToggle,
  CreateLink,
  DiffSourceToggleWrapper,
  InsertCodeBlock,
  InsertThematicBreak,
  ListsToggle,
  MDXEditor,
  type MDXEditorMethods,
  Separator,
  UndoRedo,
  codeBlockPlugin,
  codeMirrorPlugin,
  diffSourcePlugin,
  headingsPlugin,
  linkDialogPlugin,
  linkPlugin,
  listsPlugin,
  markdownShortcutPlugin,
  quotePlugin,
  thematicBreakPlugin,
  toolbarPlugin,
  type Translation,
} from "@mdxeditor/editor";
import "@mdxeditor/editor/style.css";

import { useI18n } from "@/hooks/use-i18n";

type ProjectMarkdownEditorProps = {
  value: string;
  placeholder: string;
  disabled?: boolean;
  onChange: (value: string) => void;
};

const CODE_BLOCK_LANGUAGES: Record<string, string> = {
  "": "Plain text",
  text: "Plain text",
  markdown: "Markdown",
  md: "Markdown",
  json: "JSON",
  bash: "Bash",
  sh: "Shell",
  ts: "TypeScript",
  js: "JavaScript",
  rust: "Rust",
  toml: "TOML",
  yaml: "YAML",
  yml: "YAML",
};

const ZH_CN_TRANSLATIONS: Record<string, string> = {
  "contentArea.editableMarkdown": "可编辑 Markdown 内容",
  "toolbar.undo": "撤销 {{shortcut}}",
  "toolbar.redo": "重做 {{shortcut}}",
  "toolbar.bold": "加粗",
  "toolbar.removeBold": "取消加粗",
  "toolbar.italic": "斜体",
  "toolbar.removeItalic": "取消斜体",
  "toolbar.underline": "下划线",
  "toolbar.removeUnderline": "取消下划线",
  "toolbar.inlineCode": "行内代码",
  "toolbar.removeInlineCode": "取消行内代码",
  "toolbar.bulletedList": "无序列表",
  "toolbar.numberedList": "有序列表",
  "toolbar.checkList": "任务列表",
  "toolbar.codeBlock": "插入代码块",
  "toolbar.thematicBreak": "插入分隔线",
  "toolbar.link": "插入链接",
  "toolbar.richText": "富文本",
  "toolbar.source": "源码",
  "toolbar.blockTypes.paragraph": "正文",
  "toolbar.blockTypes.quote": "引用",
  "toolbar.blockTypes.heading": "标题 {{level}}",
  "toolbar.blockTypeSelect.selectBlockTypeTooltip": "选择块类型",
  "toolbar.blockTypeSelect.placeholder": "块类型",
  "createLink.urlPlaceholder": "输入或粘贴链接地址",
  "createLink.textTooltip": "链接显示的文本",
  "createLink.text": "链接文本",
  "createLink.saveTooltip": "设置链接",
  "createLink.cancelTooltip": "取消修改",
  "dialogControls.save": "保存",
  "dialogControls.cancel": "取消",
  "linkPreview.open": "在新窗口打开 {{url}}",
  "linkPreview.edit": "编辑链接",
  "linkPreview.copyToClipboard": "复制链接",
  "linkPreview.copied": "已复制",
  "linkPreview.remove": "移除链接",
  "codeBlock.language": "代码块语言",
};

function interpolate(template: string, values?: Record<string, unknown>) {
  if (!values) return template;
  return template.replace(/\{\{\s*([a-zA-Z0-9_]+)\s*\}\}/g, (match, key) => {
    const value = values[key];
    return value === undefined || value === null ? match : String(value);
  });
}

export function ProjectMarkdownEditor({
  value,
  placeholder,
  disabled = false,
  onChange,
}: ProjectMarkdownEditorProps) {
  const { locale, t } = useI18n();
  const editorRef = useRef<MDXEditorMethods | null>(null);
  const syncingRef = useRef(false);
  const [overlayContainer, setOverlayContainer] = useState<HTMLDivElement | null>(null);
  const sourceLabel = t("projects.editor.sourceLabel");

  const translation = useMemo<Translation>(() => {
    return (key, defaultValue, interpolations) => {
      const template = locale === "zh-CN" ? ZH_CN_TRANSLATIONS[key] ?? defaultValue : defaultValue;
      return interpolate(template, interpolations);
    };
  }, [locale]);

  useEffect(() => {
    const editor = editorRef.current;
    if (!editor) return;

    if (editor.getMarkdown() === value) {
      return;
    }

    syncingRef.current = true;
    editor.setMarkdown(value);
    queueMicrotask(() => {
      syncingRef.current = false;
    });
  }, [value]);

  const handleChange = useCallback(
    (next: string) => {
      if (syncingRef.current) return;
      onChange(next);
    },
    [onChange]
  );

  return (
    <div ref={setOverlayContainer} className="relative h-full min-h-0">
      <MDXEditor
        ref={editorRef}
        markdown={value}
        onChange={handleChange}
        placeholder={placeholder}
        readOnly={disabled}
        spellCheck={false}
        trim={false}
        overlayContainer={overlayContainer ?? undefined}
        className="prompt-mdx-editor-shell"
        contentEditableClassName="prompt-mdx-editor-content"
        translation={translation}
        plugins={[
          toolbarPlugin({
            toolbarClassName: "prompt-mdx-editor-toolbar",
            toolbarContents: () => (
              <>
                <UndoRedo />
                <Separator />
                <DiffSourceToggleWrapper
                  options={["rich-text", "source"]}
                  SourceToolbar={<span className="prompt-mdx-editor-source-label">{sourceLabel}</span>}
                >
                  <>
                    <BlockTypeSelect />
                    <Separator />
                    <BoldItalicUnderlineToggles />
                    <CodeToggle />
                    <Separator />
                    <ListsToggle />
                    <Separator />
                    <CreateLink />
                    <InsertCodeBlock />
                    <InsertThematicBreak />
                  </>
                </DiffSourceToggleWrapper>
              </>
            ),
          }),
          headingsPlugin(),
          quotePlugin(),
          listsPlugin(),
          linkPlugin(),
          linkDialogPlugin(),
          thematicBreakPlugin(),
          markdownShortcutPlugin(),
          diffSourcePlugin({
            viewMode: "rich-text",
          }),
          codeBlockPlugin({
            defaultCodeBlockLanguage: "text",
          }),
          codeMirrorPlugin({
            codeBlockLanguages: CODE_BLOCK_LANGUAGES,
          }),
        ]}
      />
    </div>
  );
}
