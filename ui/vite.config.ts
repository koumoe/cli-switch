import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "path";

function isNodeModulePkg(id: string, name: string): boolean {
  return id.includes(`/node_modules/${name}/`) || id.includes(`\\node_modules\\${name}\\`);
}

function isNodeModulePattern(id: string, pattern: string): boolean {
  const normalizedId = id.replaceAll("\\", "/");
  return normalizedId.includes(`/node_modules/${pattern}`);
}

function matchesNodeModulePatterns(id: string, patterns: string[]): boolean {
  return patterns.some((pattern) => isNodeModulePattern(id, pattern));
}

const EDITOR_LEXICAL_PATTERNS = [
  "@lexical/",
  "lexical/",
];

const EDITOR_CODE_LANG_PATTERNS = [
  "@codemirror/lang-",
  "@codemirror/language-data/",
];

const EDITOR_CODE_PARSER_PATTERNS = [
  "@lezer/",
];

const EDITOR_CODE_CORE_PATTERNS = [
  "@codemirror/",
  "codemirror/",
  "cm6-theme-basic-light/",
];

const EDITOR_MARKDOWN_PATTERNS = [
  "@mdxeditor/",
  "react-hook-form/",
  "downshift/",
  "unidiff/",
  "js-yaml/",
  "classnames/",
  "mdast-util-",
  "micromark-",
  "remark-",
  "rehype-",
  "hast-util-",
  "unist-util-",
  "vfile",
  "decode-named-character-reference/",
  "character-entities",
  "property-information/",
  "space-separated-tokens/",
  "comma-separated-tokens/",
  "mdurl/",
  "devlop/",
  "trim-lines/",
  "ccount/",
  "estree-util-",
  "bail/",
  "trough/",
  "zwitch/",
];

export default defineConfig(() => {
  const sourcemap = process.env.VITE_SOURCEMAP === "true";
  return {
    plugins: [react()],
    resolve: {
      alias: {
        "@": path.resolve(__dirname, "./src"),
      },
    },
    build: {
      outDir: "dist",
      sourcemap,
      rollupOptions: {
        output: {
          manualChunks(id) {
            if (!id.includes("node_modules")) return;

            if (isNodeModulePkg(id, "react-day-picker") || isNodeModulePkg(id, "date-fns")) {
              return "date-vendor";
            }
            if (id.includes("/node_modules/@radix-ui/") || id.includes("\\node_modules\\@radix-ui\\")) {
              return "radix-vendor";
            }
            if (isNodeModulePkg(id, "lucide-react")) return "icons-vendor";
            if (
              isNodeModulePkg(id, "react") ||
              isNodeModulePkg(id, "react-dom") ||
              isNodeModulePkg(id, "scheduler") ||
              isNodeModulePkg(id, "react-is") ||
              isNodeModulePkg(id, "use-sync-external-store")
            ) {
              return "react-vendor";
            }
            if (isNodeModulePkg(id, "@codemirror/legacy-modes")) {
              return "editor-code-legacy-vendor";
            }
            if (isNodeModulePkg(id, "@codemirror/lang-markdown")) {
              return "editor-markdown-vendor";
            }
            if (isNodeModulePkg(id, "@lezer/markdown")) {
              return "editor-code-parser-markdown-vendor";
            }
            if (isNodeModulePkg(id, "@lezer/cpp")) {
              return "editor-code-parser-cpp-vendor";
            }
            if (isNodeModulePkg(id, "@lezer/php")) {
              return "editor-code-parser-php-vendor";
            }
            if (isNodeModulePkg(id, "@lezer/javascript")) {
              return "editor-code-parser-js-vendor";
            }
            if (matchesNodeModulePatterns(id, EDITOR_LEXICAL_PATTERNS)) {
              return "editor-lexical-vendor";
            }
            if (matchesNodeModulePatterns(id, EDITOR_CODE_LANG_PATTERNS)) {
              return "editor-markdown-vendor";
            }
            if (matchesNodeModulePatterns(id, EDITOR_CODE_PARSER_PATTERNS)) {
              return "editor-code-parser-vendor";
            }
            if (matchesNodeModulePatterns(id, EDITOR_CODE_CORE_PATTERNS)) {
              return "editor-code-core-vendor";
            }
            if (matchesNodeModulePatterns(id, EDITOR_MARKDOWN_PATTERNS)) {
              return "editor-markdown-vendor";
            }
          },
        },
      },
    },
    server: {
      proxy: {
        "/api": {
          target: process.env.VITE_BACKEND_URL ?? "http://127.0.0.1:3210",
          changeOrigin: true
        },
        "/v1": {
          target: process.env.VITE_BACKEND_URL ?? "http://127.0.0.1:3210",
          changeOrigin: true
        }
      }
    }
  };
});
