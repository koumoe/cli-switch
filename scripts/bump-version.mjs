#!/usr/bin/env node
/**
 * Updates version in Cargo.toml and ui/package.json
 * Usage: node scripts/bump-version.mjs <version>
 */

import { readFileSync, writeFileSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const rootDir = join(__dirname, '..');

const version = process.argv[2];

if (!version) {
  console.error('Usage: node scripts/bump-version.mjs <version>');
  process.exit(1);
}

// Validate semver format
if (!/^\d+\.\d+\.\d+(-[\w.]+)?$/.test(version)) {
  console.error(`Invalid version format: ${version}`);
  process.exit(1);
}

console.log(`Bumping version to ${version}`);

// Update Cargo.toml
const cargoPath = join(rootDir, 'Cargo.toml');
let cargoContent = readFileSync(cargoPath, 'utf-8');
const cargoUpdated = cargoContent.replace(
  /^(version\s*=\s*)"[^"]+"/m,
  `$1"${version}"`
);
if (cargoUpdated === cargoContent) {
  console.error(`Failed to update version in ${cargoPath} (no match for version field)`);
  process.exit(1);
}
writeFileSync(cargoPath, cargoUpdated);
console.log(`Updated ${cargoPath}`);

// Update Cargo.lock (keep workspace package version consistent)
const cargoLockPath = join(rootDir, 'Cargo.lock');
let lockContent = readFileSync(cargoLockPath, 'utf-8');
const lines = lockContent.split('\n');
let inPackage = false;
let isTargetPackage = false;
let lockChanged = false;

for (let i = 0; i < lines.length; i++) {
  const line = lines[i];
  if (line.trim() === '[[package]]') {
    inPackage = true;
    isTargetPackage = false;
    continue;
  }
  if (inPackage && /^name\s*=\s*"cliswitch"\s*$/.test(line)) {
    isTargetPackage = true;
    continue;
  }
  if (inPackage && isTargetPackage && /^version\s*=\s*".*"\s*$/.test(line)) {
    const nextLine = line.replace(/^version\s*=\s*".*"\s*$/, `version = "${version}"`);
    if (nextLine !== line) {
      lines[i] = nextLine;
      lockChanged = true;
    }
    inPackage = false;
    isTargetPackage = false;
    continue;
  }
}

if (!lockChanged) {
  console.error(`Failed to update ${cargoLockPath} (package "cliswitch" not found or version unchanged)`);
  process.exit(1);
}
writeFileSync(cargoLockPath, lines.join('\n'));
console.log(`Updated ${cargoLockPath}`);

// Update ui/package.json
const pkgPath = join(rootDir, 'ui', 'package.json');
const pkgRaw = readFileSync(pkgPath, 'utf-8');
const pkg = JSON.parse(pkgRaw);
pkg.version = version;
writeFileSync(pkgPath, JSON.stringify(pkg, null, 2) + '\n');
console.log(`Updated ${pkgPath}`);

// Update ui/package-lock.json (keep npm ci consistent)
const lockPath = join(rootDir, 'ui', 'package-lock.json');
const lockRaw = readFileSync(lockPath, 'utf-8');
const lock = JSON.parse(lockRaw);
if (typeof lock === 'object' && lock) {
  lock.version = version;
  if (lock.packages && lock.packages['']) {
    lock.packages[''].version = version;
  }
}
writeFileSync(lockPath, JSON.stringify(lock, null, 2) + '\n');
console.log(`Updated ${lockPath}`);

console.log('Version bump complete');
