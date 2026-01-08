import fs from "node:fs";

function parseVersionBlocks(markdown) {
  const re = /^##\s+(?:\[(?<v1>[^\]]+)\]|(?<v2>[0-9][^\s]*))\b.*$/gm;
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

function translateSectionHeadingsToCn(block) {
  return block
    .replace(/^###\s+Bug Fixes\s*$/gm, "### 修复")
    .replace(/^###\s+Features\s*$/gm, "### 功能")
    .replace(/^###\s+Performance Improvements\s*$/gm, "### 性能优化")
    .replace(/^###\s+Reverts\s*$/gm, "### 回滚");
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

const enMd = fs.readFileSync(enPath, "utf8");
const cnMd = fs.readFileSync(cnPath, "utf8");

const en = parseVersionBlocks(enMd);
const cn = parseVersionBlocks(cnMd);

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

  process.exit(0);
}

if (mode === "sync") {
  const outBlocks = [];
  for (const version of en.order) {
    const cnBlock = cn.blocks.get(version);
    if (cnBlock) {
      outBlocks.push(cnBlock);
      continue;
    }
    const enBlock = en.blocks.get(version);
    if (!enBlock) continue;
    outBlocks.push(translateSectionHeadingsToCn(enBlock));
  }

  const next = `${outBlocks.join("\n")}\n`;
  if (next !== cnMd) {
    fs.writeFileSync(cnPath, next, "utf8");
  }
  process.exit(0);
}

usage();
process.exit(2);
