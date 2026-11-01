// Fails when the earlier product's name appears in product source, build scripts or
// configuration outside the files listed in brand/allowed-mentions.txt. Each entry in that
// file needs a reason, and an entry that no longer matches a mention must be removed.
// Documentation, CI workflows and the archived backend are not scanned.
// Usage: node scripts/lint/brand.mjs
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { isMain, repoRoot, trackedFiles } from './lib.mjs';
import { isProbablyText } from './dashes.mjs';

const SCOPE = [
  /^frontend\/src\//,
  /^frontend\/src-tauri\/src\//,
  /^frontend\/src-tauri\/build(\/|\.rs$)/,
  /^frontend\/src-tauri\/templates\//,
  /^frontend\/src-tauri\/(tauri(\.windows)?\.conf\.json|Cargo\.toml)$/,
  /^frontend\/package\.json$/,
];
const ALLOWLIST = 'brand/allowed-mentions.txt';

export function inScope(file) {
  return SCOPE.some((re) => re.test(file));
}

export function findMentions(text) {
  const hits = [];
  text.split('\n').forEach((line, i) => {
    if (/meetily/i.test(line)) hits.push({ line: i + 1, text: line.trim().slice(0, 120) });
  });
  return hits;
}

export function parseAllowlist(text) {
  const entries = [];
  for (const raw of text.split('\n')) {
    const line = raw.trim();
    if (!line || line.startsWith('#')) continue;
    const [glob, ...rest] = line.split('|');
    const reason = rest.join('|').trim();
    if (!reason) throw new Error(`${ALLOWLIST}: "${glob.trim()}" needs a reason after a "|"`);
    entries.push({ glob: glob.trim(), reason });
  }
  return entries;
}

export function globToRegExp(glob) {
  let re = '';
  for (let i = 0; i < glob.length; i += 1) {
    const c = glob[i];
    if (c === '*' && glob[i + 1] === '*') {
      re += '.*';
      i += 1;
    } else if (c === '*') re += '[^/]*';
    else re += c.replace(/[.+?^${}()|[\]\\]/g, '\\$&');
  }
  return new RegExp(`^${re}$`);
}

/** files: Map(path -> mentions[]). Returns failure messages. */
export function evaluateMentions(files, allowlist) {
  const failures = [];
  const compiled = allowlist.map((e) => ({ ...e, re: globToRegExp(e.glob) }));
  for (const [file, hits] of files) {
    if (hits.length && !compiled.some((e) => e.re.test(file))) {
      for (const h of hits.slice(0, 3)) failures.push(`${file}:${h.line}: ${h.text}`);
    }
  }
  for (const e of compiled) {
    const used = [...files].some(([file, hits]) => hits.length && e.re.test(file));
    if (!used) failures.push(`${ALLOWLIST}: "${e.glob}" matches no mention any more, remove it`);
  }
  return failures;
}

function main() {
  const root = repoRoot();
  const allowlist = parseAllowlist(readFileSync(path.join(root, ALLOWLIST), 'utf8'));
  const files = new Map();
  for (const file of trackedFiles(root).filter(inScope)) {
    let buf;
    try {
      buf = readFileSync(path.join(root, file));
    } catch {
      continue;
    }
    if (isProbablyText(file, buf)) files.set(file, findMentions(buf.toString('utf8')));
  }
  const failures = evaluateMentions(files, allowlist);
  if (failures.length) {
    console.error('brand mentions: FAILED (use the brand constants, or add a reasoned entry to brand/allowed-mentions.txt)');
    for (const f of failures) console.error('  ' + f);
    process.exit(1);
  }
  console.log(`brand mentions: ok (${allowlist.length} reasoned exception(s))`);
}

if (isMain(import.meta.url)) main();
