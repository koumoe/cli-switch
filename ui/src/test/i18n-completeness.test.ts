// @vitest-environment node

import fs from "node:fs";
import path from "node:path";
import ts from "typescript";
import { describe, expect, it } from "vitest";

import enUS from "@/locales/ui/en-US.json";
import zhCN from "@/locales/ui/zh-CN.json";

const HAN_RE = /\p{Script=Han}/u;
const SOURCE_ROOT = path.resolve(import.meta.dirname, "..");
const ALLOWED_LITERAL_FILES = new Set([
  path.resolve(SOURCE_ROOT, "pages/projects/ProjectMarkdownEditor.tsx"),
]);

function flattenKeys(value: unknown, prefix = ""): string[] {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return prefix ? [prefix] : [];
  }

  return Object.entries(value).flatMap(([key, child]) => {
    const nextPrefix = prefix ? `${prefix}.${key}` : key;
    return flattenKeys(child, nextPrefix);
  });
}

function walkSourceFiles(dir: string): string[] {
  return fs.readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      if (entry.name === "locales" || entry.name === "test") {
        return [];
      }
      return walkSourceFiles(fullPath);
    }

    if (!/\.(ts|tsx)$/.test(entry.name)) {
      return [];
    }

    if (/\.(test|spec)\.(ts|tsx)$/.test(entry.name) || entry.name.endsWith(".gen.ts")) {
      return [];
    }

    return [fullPath];
  });
}

type Finding = {
  filePath: string;
  line: number;
  text: string;
};

function collectChineseLiteralFindings(filePath: string): Finding[] {
  if (ALLOWED_LITERAL_FILES.has(filePath)) {
    return [];
  }

  const sourceText = fs.readFileSync(filePath, "utf8");
  const sourceFile = ts.createSourceFile(filePath, sourceText, ts.ScriptTarget.Latest, true);
  const findings: Finding[] = [];

  const pushFinding = (node: ts.Node, text: string) => {
    const normalized = text.trim();
    if (!normalized || !HAN_RE.test(normalized)) {
      return;
    }

    const { line } = sourceFile.getLineAndCharacterOfPosition(node.getStart(sourceFile));
    findings.push({
      filePath,
      line: line + 1,
      text: normalized,
    });
  };

  const visit = (node: ts.Node) => {
    if (ts.isStringLiteralLike(node)) {
      pushFinding(node, node.text);
    } else if (ts.isTemplateExpression(node)) {
      pushFinding(node.head, node.head.text);
      for (const span of node.templateSpans) {
        pushFinding(span.literal, span.literal.text);
      }
    } else if (ts.isJsxText(node)) {
      pushFinding(node, node.getText(sourceFile));
    }

    ts.forEachChild(node, visit);
  };

  visit(sourceFile);
  return findings;
}

describe("i18n completeness", () => {
  it("keeps zh-CN and en-US locale keys aligned", () => {
    const zhKeys = new Set(flattenKeys(zhCN));
    const enKeys = new Set(flattenKeys(enUS));

    const missingInEn = [...zhKeys].filter((key) => !enKeys.has(key)).sort();
    const missingInZh = [...enKeys].filter((key) => !zhKeys.has(key)).sort();

    expect(missingInEn, `Missing en-US keys:\n${missingInEn.join("\n")}`).toEqual([]);
    expect(missingInZh, `Missing zh-CN keys:\n${missingInZh.join("\n")}`).toEqual([]);
  });

  it("does not leave unexpected hardcoded Chinese literals in source files", () => {
    const findings = walkSourceFiles(SOURCE_ROOT).flatMap(collectChineseLiteralFindings);
    const formatted = findings.map(
      ({ filePath, line, text }) =>
        `${path.relative(SOURCE_ROOT, filePath)}:${line} -> ${text}`,
    );

    expect(formatted, `Unexpected Chinese literals:\n${formatted.join("\n")}`).toEqual([]);
  });
});
