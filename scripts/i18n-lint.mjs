import fs from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

const ROOT = path.resolve(import.meta.dirname, "..");
const UI_TS_MODULE = path.join(
  ROOT,
  "ui",
  "node_modules",
  "typescript",
  "lib",
  "typescript.js",
);

if (!fs.existsSync(UI_TS_MODULE)) {
  console.error("missing TypeScript runtime at ui/node_modules/typescript; run npm ci in ui first");
  process.exit(1);
}

const ts = await import(pathToFileURL(UI_TS_MODULE).href);

const PLACEHOLDER_RE = /\{\{\s*([a-zA-Z0-9_]+)\s*\}\}/g;
const LOCALE_KEY_RE = /^[a-z][a-zA-Z0-9]*(?:\.[a-zA-Z0-9_]+)+$/;
const RUST_LITERAL_KEY_RE = /"([a-z][a-zA-Z0-9]*(?:\.[a-zA-Z0-9_]+)+)"/g;

function listFiles(root, exts, out = []) {
  for (const entry of fs.readdirSync(root, { withFileTypes: true })) {
    if (entry.name === ".git" || entry.name === "node_modules" || entry.name === "target") {
      continue;
    }
    const fullPath = path.join(root, entry.name);
    if (entry.isDirectory()) {
      listFiles(fullPath, exts, out);
      continue;
    }
    if (exts.has(path.extname(entry.name))) {
      out.push(fullPath);
    }
  }
  return out;
}

function loadJson(filePath) {
  return JSON.parse(fs.readFileSync(filePath, "utf8"));
}

function extractPlaceholders(text) {
  const out = new Set();
  for (const match of text.matchAll(PLACEHOLDER_RE)) {
    out.add(match[1]);
  }
  return out;
}

function flattenLocale(node, prefix = "", out = new Map()) {
  for (const [key, value] of Object.entries(node)) {
    const next = prefix ? `${prefix}.${key}` : key;
    if (value && typeof value === "object" && !Array.isArray(value)) {
      flattenLocale(value, next, out);
      continue;
    }
    if (typeof value === "string") {
      out.set(next, {
        text: value,
        placeholders: extractPlaceholders(value),
      });
    }
  }
  return out;
}

function sortedSet(set) {
  return [...set].sort();
}

function compareLocaleMaps(label, zhMap, enMap, problems) {
  const zhKeys = new Set(zhMap.keys());
  const enKeys = new Set(enMap.keys());
  const missingInEn = [...zhKeys].filter((key) => !enKeys.has(key)).sort();
  const missingInZh = [...enKeys].filter((key) => !zhKeys.has(key)).sort();
  if (missingInEn.length > 0) {
    problems.push(`${label}: missing keys in en-US\n${missingInEn.join("\n")}`);
  }
  if (missingInZh.length > 0) {
    problems.push(`${label}: missing keys in zh-CN\n${missingInZh.join("\n")}`);
  }

  for (const key of [...zhKeys].filter((item) => enKeys.has(item)).sort()) {
    const zhPlaceholders = sortedSet(zhMap.get(key).placeholders);
    const enPlaceholders = sortedSet(enMap.get(key).placeholders);
    if (zhPlaceholders.join("|") !== enPlaceholders.join("|")) {
      problems.push(
        `${label}: placeholder mismatch for ${key}\nzh-CN=${zhPlaceholders.join(",") || "(none)"}\nen-US=${enPlaceholders.join(",") || "(none)"}`,
      );
    }
  }
}

function isLocaleKey(value) {
  return LOCALE_KEY_RE.test(value);
}

function escapeRegex(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function templateExpressionToRegex(node) {
  const staticText = [node.head.text, ...node.templateSpans.map((span) => span.literal.text)].join("");
  if (!staticText.includes(".")) return null;

  let pattern = `^${escapeRegex(node.head.text)}`;
  for (const span of node.templateSpans) {
    pattern += "[^.]+";
    pattern += escapeRegex(span.literal.text);
  }
  pattern += "$";
  return new RegExp(pattern);
}

function mergeSpecs(left, right) {
  return {
    exact: new Set([...left.exact, ...right.exact]),
    patterns: [...left.patterns, ...right.patterns],
  };
}

function emptySpecs() {
  return { exact: new Set(), patterns: [] };
}

function extractTsKeySpecs(node) {
  if (!node) return emptySpecs();
  if (ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node)) {
    return isLocaleKey(node.text)
      ? { exact: new Set([node.text]), patterns: [] }
      : emptySpecs();
  }
  if (ts.isTemplateExpression(node)) {
    const pattern = templateExpressionToRegex(node);
    return pattern ? { exact: new Set(), patterns: [pattern] } : emptySpecs();
  }
  if (ts.isConditionalExpression(node)) {
    return mergeSpecs(extractTsKeySpecs(node.whenTrue), extractTsKeySpecs(node.whenFalse));
  }
  if (ts.isParenthesizedExpression(node)) {
    return extractTsKeySpecs(node.expression);
  }
  return emptySpecs();
}

function extractObjectKeys(node) {
  if (!node) return new Set();
  if (!ts.isObjectLiteralExpression(node)) return null;

  const out = new Set();
  for (const prop of node.properties) {
    if (ts.isPropertyAssignment(prop) || ts.isShorthandPropertyAssignment(prop)) {
      if (ts.isIdentifier(prop.name) || ts.isStringLiteral(prop.name)) {
        out.add(prop.name.text);
      }
    }
  }
  return out;
}

function lineNumber(sourceFile, node) {
  return sourceFile.getLineAndCharacterOfPosition(node.getStart(sourceFile)).line + 1;
}

function collectFrontendUsage(localeKeys) {
  const exact = new Set();
  const patterns = [];
  const checks = [];
  const files = listFiles(path.join(ROOT, "ui", "src"), new Set([".ts", ".tsx"]));

  for (const filePath of files) {
    const raw = fs.readFileSync(filePath, "utf8");
    const sourceFile = ts.createSourceFile(
      filePath,
      raw,
      ts.ScriptTarget.Latest,
      true,
      filePath.endsWith(".tsx") ? ts.ScriptKind.TSX : ts.ScriptKind.TS,
    );

    function visit(node) {
      if ((ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node)) && isLocaleKey(node.text)) {
        exact.add(node.text);
      }

      if (ts.isCallExpression(node) && ts.isIdentifier(node.expression) && node.expression.text === "t") {
        const specs = extractTsKeySpecs(node.arguments[0]);
        for (const key of specs.exact) exact.add(key);
        patterns.push(...specs.patterns);

        const argKeys = extractObjectKeys(node.arguments[1]);
        if (argKeys !== null) {
          checks.push({
            filePath,
            line: lineNumber(sourceFile, node),
            specs,
            argKeys,
          });
        }
      }

      ts.forEachChild(node, visit);
    }

    visit(sourceFile);
  }

  return {
    exact,
    patterns,
    checks,
  };
}

function collectRustUsage() {
  const srcRoot = path.join(ROOT, "src");
  const rustFiles = listFiles(srcRoot, new Set([".rs"]));
  const exact = new Set();
  const checks = [];
  const problems = [];

  const literalCallRe = /\b(?:render|render_optional)\(\s*[^,]+,\s*"([^"]+)"\s*,\s*&(?:BTreeMap::new\(\)|BTreeMap::<[^>]+>::new\(\))/gs;
  const chatBridgeTRe = /\bt\(\s*locale\s*,\s*"([^"]+)"\s*\)/g;
  const chatBridgeTArgsRe =
    /\bt_args\(\s*locale\s*,\s*"([^"]+)"\s*,\s*&args\(\[(.*?)\]\)\s*,?\s*\)/gs;
  const errorCodeRe =
    /ApiError::(?:bad_request|not_found|conflict|bad_gateway|unavailable)\(\s*"([^"]+)"|UserFacingIssue::new\(\s*"([^"]+)"/g;
  const helperBodyRe =
    /fn\s+code_for\s*\([^)]*\)\s*->\s*&'static\s+str\s*\{([\s\S]*?)\}/g;
  const helperCodeRe = /"([a-z]+(?:[._][a-z0-9]+){2,})"/g;
  const codeLiteralRe = /"([a-z]+(?:_[a-z0-9]+){2,})"/g;
  const argsKeyRe = /\(\s*"([^"]+)"/g;

  for (const filePath of rustFiles) {
    const raw = fs.readFileSync(filePath, "utf8");

    for (const match of raw.matchAll(RUST_LITERAL_KEY_RE)) {
      exact.add(match[1]);
      if (filePath.includes(`${path.sep}chat_bridge${path.sep}`) && !match[1].startsWith("chatBridge.")) {
        exact.add(`chatBridge.${match[1]}`);
      }
    }
    for (const match of raw.matchAll(codeLiteralRe)) {
      exact.add(match[1]);
      exact.add(`errors.${match[1]}`);
    }

    for (const match of raw.matchAll(literalCallRe)) {
      checks.push({
        filePath,
        line: lineNumberFromRaw(raw, match.index),
        key: match[1],
        argKeys: new Set(),
      });
    }

    if (filePath.includes(`${path.sep}chat_bridge${path.sep}`)) {
      for (const match of raw.matchAll(chatBridgeTRe)) {
        const key = `chatBridge.${match[1]}`;
        exact.add(key);
        checks.push({
          filePath,
          line: lineNumberFromRaw(raw, match.index),
          key,
          argKeys: new Set(),
        });
      }

      for (const match of raw.matchAll(chatBridgeTArgsRe)) {
        const key = `chatBridge.${match[1]}`;
        exact.add(key);
        const argKeys = new Set();
        for (const argMatch of match[2].matchAll(argsKeyRe)) {
          argKeys.add(argMatch[1]);
        }
        checks.push({
          filePath,
          line: lineNumberFromRaw(raw, match.index),
          key,
          argKeys,
        });
      }
    }

    for (const match of raw.matchAll(errorCodeRe)) {
      const code = match[1] ?? match[2];
      if (!code) continue;
      exact.add(code);
      exact.add(`errors.${code}`);
    }

    for (const body of raw.matchAll(helperBodyRe)) {
      for (const match of body[1].matchAll(helperCodeRe)) {
        exact.add(match[1]);
        exact.add(`errors.${match[1]}`);
      }
    }
  }

  return { exact, checks, problems };
}

function lineNumberFromRaw(raw, index) {
  return raw.slice(0, index).split("\n").length;
}

function keyMatchesPatterns(key, patterns) {
  return patterns.some((pattern) => pattern.test(key));
}

function candidateKeys(specs, localeMap) {
  const out = new Set();
  for (const key of specs.exact) {
    if (localeMap.has(key)) out.add(key);
  }
  if (specs.patterns.length > 0) {
    for (const key of localeMap.keys()) {
      if (keyMatchesPatterns(key, specs.patterns)) out.add(key);
    }
  }
  return out;
}

function validatePlaceholderUsage(checks, localeMap, problems, kind) {
  for (const check of checks) {
    const keys =
      "specs" in check ? candidateKeys(check.specs, localeMap) : localeMap.has(check.key) ? new Set([check.key]) : new Set();
    if (keys.size === 0) continue;

    for (const key of keys) {
      const missing = [...localeMap.get(key).placeholders].filter((name) => !check.argKeys.has(name));
      if (missing.length > 0) {
        problems.push(
          `${kind}: missing interpolation args at ${path.relative(ROOT, check.filePath)}:${check.line}\nkey=${key}\nmissing=${missing.join(",")}`,
        );
      }
    }
  }
}

function collectUnusedKeys(localeMap, exact, patterns) {
  return [...localeMap.keys()]
    .filter((key) => !exact.has(key) && !keyMatchesPatterns(key, patterns))
    .sort();
}

const sharedZh = flattenLocale(loadJson(path.join(ROOT, "i18n", "locales", "shared", "zh-CN.json")));
const sharedEn = flattenLocale(loadJson(path.join(ROOT, "i18n", "locales", "shared", "en-US.json")));
const uiZh = flattenLocale(loadJson(path.join(ROOT, "ui", "src", "locales", "ui", "zh-CN.json")));
const uiEn = flattenLocale(loadJson(path.join(ROOT, "ui", "src", "locales", "ui", "en-US.json")));

const problems = [];
compareLocaleMaps("shared locales", sharedZh, sharedEn, problems);
compareLocaleMaps("ui locales", uiZh, uiEn, problems);

const mergedFrontendLocaleMap = new Map([...sharedZh, ...uiZh]);
const frontendUsage = collectFrontendUsage(mergedFrontendLocaleMap);
const rustUsage = collectRustUsage();

validatePlaceholderUsage(frontendUsage.checks, mergedFrontendLocaleMap, problems, "frontend i18n");
validatePlaceholderUsage(rustUsage.checks, sharedZh, problems, "rust i18n");

const sharedExactUsage = new Set([...frontendUsage.exact, ...rustUsage.exact]);
const sharedPatterns = frontendUsage.patterns;
const unusedSharedKeys = collectUnusedKeys(sharedZh, sharedExactUsage, sharedPatterns);
const unusedUiKeys = collectUnusedKeys(uiZh, frontendUsage.exact, frontendUsage.patterns);

if (unusedSharedKeys.length > 0) {
  problems.push(`unused shared locale keys\n${unusedSharedKeys.join("\n")}`);
}
if (unusedUiKeys.length > 0) {
  problems.push(`unused ui locale keys\n${unusedUiKeys.join("\n")}`);
}

if (problems.length > 0) {
  console.error(problems.join("\n\n"));
  process.exit(1);
}

console.log("i18n lint passed");
