#!/usr/bin/env node
/**
 * Analyze git commits since the latest tag (v*), and decide:
 * - should_release: true/false
 * - version: next semver (x.y.z) when should_release is true
 *
 * Rules (conventional commits):
 * - BREAKING CHANGE / "!" => major
 * - feat => minor
 * - fix | perf | refactor => patch
 * - others => no release
 *
 * Writes outputs to $GITHUB_OUTPUT when present.
 */

import { execFileSync } from 'node:child_process';
import { readFileSync, appendFileSync } from 'node:fs';

function git(args, { trim = true } = {}) {
  const out = execFileSync('git', args, { encoding: 'utf8' });
  return trim ? out.trim() : out;
}

function firstNonEmptyLine(text) {
  for (const line of text.split('\n')) {
    const trimmed = line.trim();
    if (trimmed) return trimmed;
  }
  return '';
}

function readCargoPackageVersion() {
  const cargoToml = readFileSync('Cargo.toml', 'utf8');
  const match = cargoToml.match(/^\[package\][\s\S]*?^\s*version\s*=\s*"([^"]+)"/m);
  if (!match) {
    throw new Error('Failed to read [package].version from Cargo.toml');
  }
  return match[1];
}

function parseSemver(version) {
  const match = String(version).match(/^(\d+)\.(\d+)\.(\d+)/);
  if (!match) {
    throw new Error(`Invalid base version: ${version}`);
  }
  return {
    major: Number(match[1]),
    minor: Number(match[2]),
    patch: Number(match[3]),
  };
}

function bumpVersion(baseVersion, bump) {
  const semver = parseSemver(baseVersion);
  if (bump === 'major') return `${semver.major + 1}.0.0`;
  if (bump === 'minor') return `${semver.major}.${semver.minor + 1}.0`;
  if (bump === 'patch') return `${semver.major}.${semver.minor}.${semver.patch + 1}`;
  return null;
}

function conventionalBumpForCommit(subject, body) {
  const header = subject.trim();
  const commitBody = body || '';

  if (!header) return null;
  if (/^chore\(release\):/i.test(header)) return null;

  const match = header.match(/^([a-zA-Z]+)(\([^)]+\))?(!)?:\s.+$/);
  const type = match?.[1]?.toLowerCase() || null;
  const breakingViaBang = Boolean(match?.[3]);
  const breakingViaBody = /BREAKING CHANGE:|BREAKING-CHANGE:/i.test(commitBody);

  if (breakingViaBang || breakingViaBody) return 'major';
  if (type === 'feat') return 'minor';
  if (type === 'fix' || type === 'perf' || type === 'refactor') return 'patch';
  return null;
}

function maxBump(current, next) {
  const order = { patch: 1, minor: 2, major: 3 };
  if (!current) return next;
  if (!next) return current;
  return order[next] > order[current] ? next : current;
}

let lastTag = null;
try {
  const tagList = git(['tag', '--list', 'v*', '--sort=-version:refname']).split('\n').filter(Boolean);
  lastTag = tagList[0] || null;
} catch {
  lastTag = null;
}

const baseVersion = lastTag ? lastTag.replace(/^v/, '') : readCargoPackageVersion();
const rangeArgs = lastTag ? [`${lastTag}..HEAD`] : [];

let logOutput = '';
try {
  logOutput = git(['log', '--pretty=format:%s%x00%b%x00', ...rangeArgs], { trim: false });
} catch {
  logOutput = '';
}

let bump = null;
if (logOutput) {
  const parts = logOutput.split('\0');
  for (let i = 0; i + 1 < parts.length; i += 2) {
    const subject = firstNonEmptyLine(parts[i] || '');
    const body = parts[i + 1] || '';
    bump = maxBump(bump, conventionalBumpForCommit(subject, body));
    if (bump === 'major') break;
  }
}

const nextVersion = bumpVersion(baseVersion, bump);
const shouldRelease = Boolean(nextVersion);

const outputs = [
  `should_release=${shouldRelease ? 'true' : 'false'}`,
  `version=${shouldRelease ? nextVersion : ''}`,
];

if (process.env.GITHUB_OUTPUT) {
  appendFileSync(process.env.GITHUB_OUTPUT, outputs.map((l) => `${l}\n`).join(''), 'utf8');
}

console.log(outputs.join('\n'));
console.log(`base_version=${baseVersion}`);
console.log(`last_tag=${lastTag || ''}`);
console.log(`bump=${bump || ''}`);
