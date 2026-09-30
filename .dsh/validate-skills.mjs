#!/usr/bin/env node
// SPDX-License-Identifier: GPL-3.0-only
//
// Validates the skills under .dsh/skills/ without any third-party dependency:
//
//   1. every skill directory has a SKILL.md whose YAML frontmatter parses,
//      whose `name` equals the directory name, and whose `description` is
//      non-empty;
//   2. every relative Markdown link inside a skill resolves to a real file.
//
// The frontmatter parser only understands the subset these skills use:
// top-level `key: value`, top-level `key: >` / `key: |` block scalars, and
// nested blocks that are skipped. That is deliberate — it keeps this checker
// runnable in CI images that have Node but no YAML package.
//
// Link checking only looks at targets that carry a file extension. Several
// skills document Markdown or rustdoc syntax, whose examples (`link/to/adr-12`,
// `struct@Error`, `path`) are not files; extensionless targets are therefore
// treated as prose, not references.
//
// Usage: node .dsh/validate-skills.mjs [skills-root]

import { readFileSync, readdirSync, existsSync, statSync } from 'node:fs';
import path from 'node:path';
import process from 'node:process';

const root = path.resolve(process.argv[2] ?? '.dsh/skills');
const problems = [];

/** Parse the frontmatter subset described above. Returns null when malformed. */
function parseFrontmatter(text) {
  const normalised = text.replace(/\r\n/g, '\n');
  if (!normalised.startsWith('---\n')) return null;
  const end = normalised.indexOf('\n---', 4);
  if (end < 0) return null;

  const fields = {};
  const lines = normalised.slice(4, end).split('\n');
  for (let i = 0; i < lines.length; i += 1) {
    const line = lines[i];
    if (line.trim() === '' || line.startsWith('#')) continue;
    if (/^\s/.test(line)) continue; // nested value, handled by its parent key
    const match = /^([A-Za-z_][\w-]*):\s*(.*)$/.exec(line);
    if (!match) return null;
    const [, key, rawValue] = match;
    const value = rawValue.trim();
    if (value === '>' || value === '|' || value === '>-' || value === '|-') {
      const block = [];
      while (i + 1 < lines.length && (/^\s+\S/.test(lines[i + 1]) || lines[i + 1].trim() === '')) {
        block.push(lines[i + 1].trim());
        i += 1;
      }
      fields[key] = block.join(' ').trim();
    } else if (value === '') {
      fields[key] = {}; // nested block; contents are not validated here
    } else {
      fields[key] = value.replace(/^["']|["']$/g, '');
    }
  }
  return fields;
}

function markdownFiles(dir) {
  const found = [];
  const walk = (current) => {
    for (const entry of readdirSync(current, { withFileTypes: true })) {
      const full = path.join(current, entry.name);
      if (entry.isDirectory()) walk(full);
      else if (entry.name.endsWith('.md')) found.push(full);
    }
  };
  walk(dir);
  return found;
}

if (!existsSync(root) || !statSync(root).isDirectory()) {
  console.error(`skills root not found: ${root}`);
  process.exit(1);
}

const skills = readdirSync(root, { withFileTypes: true })
  .filter((entry) => entry.isDirectory())
  .map((entry) => entry.name)
  .sort();

let checkedFiles = 0;
for (const skill of skills) {
  const dir = path.join(root, skill);
  const entry = path.join(dir, 'SKILL.md');
  if (!existsSync(entry)) {
    problems.push(`${skill}: no SKILL.md`);
    continue;
  }

  const text = readFileSync(entry, 'utf8');
  const fields = parseFrontmatter(text);
  if (!fields) {
    problems.push(`${skill}: SKILL.md frontmatter is missing or malformed`);
  } else {
    if (fields.name !== skill) {
      problems.push(`${skill}: frontmatter name is ${JSON.stringify(fields.name)}, expected ${JSON.stringify(skill)}`);
    }
    if (typeof fields.description !== 'string' || fields.description.trim().length < 10) {
      problems.push(`${skill}: frontmatter description is missing or too short`);
    }
  }

  for (const file of markdownFiles(dir)) {
    checkedFiles += 1;
    const body = readFileSync(file, 'utf8');
    for (const match of body.matchAll(/\]\(([^)\s]+)\)/g)) {
      const target = match[1];
      if (/^[a-z][a-z0-9+.-]*:/i.test(target) || target.startsWith('#')) continue; // absolute URL or anchor
      const bare = target.split('#')[0];
      if (!bare || !path.basename(bare).includes('.')) continue; // prose, not a file reference
      const resolved = path.resolve(path.dirname(file), bare);
      if (!existsSync(resolved)) {
        problems.push(`${skill}: ${path.relative(root, file)} links to missing ${target}`);
      }
    }
  }
}

for (const problem of problems) console.error(`FAIL ${problem}`);
console.log(`${skills.length} skills, ${checkedFiles} markdown files, ${problems.length} problems`);
process.exit(problems.length === 0 ? 0 : 1);
