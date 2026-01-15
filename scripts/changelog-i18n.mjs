import fs from "node:fs";
import { execSync } from "node:child_process";

// Build a map from short commit hash to full bilingual subject from git log.
function buildCommitSubjectMap() {
  const map = new Map();
  try {
    // Get recent commits with their subjects (last 200 should be enough for changelog)
    const log = execSync("git log --oneline -200 --format=%h:%s", {
      encoding: "utf8",
      stdio: ["pipe", "pipe", "pipe"],
    });
    for (const line of log.split("\n")) {
      const idx = line.indexOf(":");
      if (idx === -1) continue;
      const hash = line.slice(0, idx).trim();
      const subject = line.slice(idx + 1).trim();
      if (hash && subject) {
        map.set(hash, subject);
      }
    }
  } catch {
    // If git is not available, return empty map
  }
  return map;
}

function parseVersionBlocks(markdown) {
  // Supports conventional-changelog style:
  //   ## [0.19.1](https://...) (2026-01-08)
  // and plain style:
  //   ## 0.19.1
  const re =
    /^##\s+(?:\[(?<v1>[^\]]+)\](?=\(|\s|$)|(?<v2>[0-9][^\s]*)(?=\(|\s|$)).*$/gm;
  const matches = [];
  for (;;) {
    const m = re.exec(markdown);
    if (!m) break;
    const v = (m.groups?.v1 ?? m.groups?.v2 ?? "").trim();
    if (!v) continue;
    matches.push({ version: v, index: m.index });
  }

  const order = matches.map((m) => m.version);
  const blocks = new Map();
  for (let i = 0; i < matches.length; i++) {
    const start = matches[i].index;
    const end = i + 1 < matches.length ? matches[i + 1].index : markdown.length;
    blocks.set(matches[i].version, markdown.slice(start, end).trimEnd());
  }
  return { order, blocks };
}

// Restore bilingual subjects from git log when conventional-changelog truncated them.
// Input line: "* english message ([hash](url))"
// If git log has "feat: english message / 中文消息", restore to:
// "* english message / 中文消息 ([hash](url))"
function restoreBilingualFromGit(markdown, commitMap) {
  return markdown
    .split("\n")
    .map((line) => {
      const m = /^(\s*\*)\s+(.*)$/.exec(line);
      if (!m) return line;

      const bullet = m[1];
      const rest = m[2];

      // Extract commit hash from markdown link: ([hash](url))
      const hashMatch = /\(\[([a-f0-9]{7,})\]\([^)]+\)\)\s*$/.exec(rest);
      if (!hashMatch) return line;

      const hash = hashMatch[1];
      const fullSubject = commitMap.get(hash);
      if (!fullSubject) return line;

      // Check if git subject has bilingual format
      const gitSep = fullSubject.indexOf(" / ");
      if (gitSep === -1) return line;

      // Extract the part after conventional commit prefix (e.g., "feat: ")
      const colonIdx = fullSubject.indexOf(": ");
      const subjectBody =
        colonIdx !== -1 ? fullSubject.slice(colonIdx + 2) : fullSubject;

      // Check if it's bilingual (right side has CJK)
      const bodySep = subjectBody.indexOf(" / ");
      if (bodySep === -1) return line;
      const rightPart = subjectBody.slice(bodySep + 3).trim();
      if (!/[\u4E00-\u9FFF]/.test(rightPart)) return line;

      // Current subject in changelog (before the link)
      const linkStart = rest.indexOf(" ([");
      if (linkStart === -1) return line;
      const suffix = rest.slice(linkStart);

      // Replace with full bilingual subject
      return `${bullet} ${subjectBody}${suffix}`;
    })
    .join("\n");
}

function normalizeBilingualBullets(markdown, keep) {
  // Normalize bilingual commit subjects like:
  //   * english message / 中文消息 ([...])
  // keep="left"  -> keep English part (for en.md)
  // keep="right" -> keep Chinese part (for cn.md)
  return markdown
    .split("\n")
    .map((line) => {
      const m = /^(\s*\*)\s+(.*)$/.exec(line);
      if (!m) return line;

      const bullet = m[1];
      const rest = m[2];

      const linkStart = rest.indexOf(" (");
      const subject = linkStart === -1 ? rest : rest.slice(0, linkStart);
      const suffix = linkStart === -1 ? "" : rest.slice(linkStart);

      const sep = subject.indexOf(" / ");
      if (sep === -1) return line;

      const left = subject.slice(0, sep).trimEnd();
      const right = subject.slice(sep + 3).trimStart();
      if (!left || !right) return line;

      // Only treat it as bilingual when the right side contains CJK characters.
      if (!/[\u4E00-\u9FFF]/.test(right)) return line;

      const chosen = keep === "right" ? right : left;
      return `${bullet} ${chosen}${suffix}`;
    })
    .join("\n");
}

function translateSectionHeadingsToCn(block) {
  return block
    .replace(/^###\s+Bug Fixes\s*$/gm, "### 修复")
    .replace(/^###\s+Features\s*$/gm, "### 功能")
    .replace(/^###\s+Performance Improvements\s*$/gm, "### 性能优化")
    .replace(/^###\s+Reverts\s*$/gm, "### 回滚");
}

function validateCnBlockHasChinese(version, block, cnPath) {
  const bulletRe = /^\s*[*-]\s+/;
  const cjkRe = /[\u4E00-\u9FFF]/;
  for (const line of block.split("\n")) {
    if (!bulletRe.test(line)) continue;
    if (cjkRe.test(line)) continue;
    console.error(
      `Chinese changelog has non-Chinese bullet in ${cnPath} (version ${version}): ${line.trim()}`
    );
    process.exit(1);
  }
}

function usage() {
  console.error(
    [
      "Usage:",
      "  node scripts/changelog-i18n.mjs check <enPath> <cnPath>",
      "  node scripts/changelog-i18n.mjs sync  <enPath> <cnPath>",
    ].join("\n")
  );
}

const [mode, enPath, cnPath] = process.argv.slice(2);
if (!mode || !enPath || !cnPath) {
  usage();
  process.exit(2);
}

if (!fs.existsSync(enPath)) {
  console.error(`Missing English changelog: ${enPath}`);
  process.exit(1);
}
if (!fs.existsSync(cnPath)) {
  console.error(`Missing Chinese changelog: ${cnPath}`);
  process.exit(1);
}

let enMd = fs.readFileSync(enPath, "utf8");
let cnMd = fs.readFileSync(cnPath, "utf8");
const cnMdOriginal = cnMd;

// Build commit map once for sync mode
const commitMap = mode === "sync" ? buildCommitSubjectMap() : new Map();

if (mode === "sync") {
  // First, restore bilingual subjects from git log (conventional-changelog may have truncated them)
  enMd = restoreBilingualFromGit(enMd, commitMap);

  const normalizedEn = normalizeBilingualBullets(enMd, "left");
  // Always write back the normalized English version
  fs.writeFileSync(enPath, normalizedEn, "utf8");
  enMd = normalizedEn;

  // Keep Chinese-only subjects in cn.md when present.
  cnMd = normalizeBilingualBullets(cnMd, "right");
}

const en = parseVersionBlocks(enMd);
const cn = parseVersionBlocks(cnMd);

if (en.order.length === 0) {
  console.error(`No versions found in English changelog: ${enPath}`);
  process.exit(1);
}

if (mode === "check") {
  const enList = en.order;
  const cnList = cn.order;

  if (enList.length !== cnList.length) {
    console.error(`Changelog version count mismatch: en=${enList.length} cn=${cnList.length}`);
    process.exit(1);
  }

  for (let i = 0; i < enList.length; i++) {
    if (enList[i] !== cnList[i]) {
      console.error(`Changelog version mismatch at #${i + 1}: en=${enList[i]} cn=${cnList[i]}`);
      process.exit(1);
    }
  }

  for (const version of cnList) {
    const cnBlock = cn.blocks.get(version);
    if (!cnBlock) continue;
    validateCnBlockHasChinese(version, cnBlock, cnPath);
  }

  process.exit(0);
}

if (mode === "sync") {
  const outBlocks = [];
  for (const version of en.order) {
    const cnBlock = cn.blocks.get(version);
    if (cnBlock) {
      outBlocks.push(normalizeBilingualBullets(cnBlock, "right"));
      continue;
    }
    // For new versions not in cn.md, restore bilingual from git and extract Chinese
    let enBlock = en.blocks.get(version);
    if (!enBlock) continue;
    enBlock = restoreBilingualFromGit(enBlock, commitMap);
    outBlocks.push(normalizeBilingualBullets(translateSectionHeadingsToCn(enBlock), "right"));
  }

  const next = `${outBlocks.join("\n")}\n`;
  if (next !== cnMdOriginal) {
    fs.writeFileSync(cnPath, next, "utf8");
  }
  process.exit(0);
}

usage();
process.exit(2);
