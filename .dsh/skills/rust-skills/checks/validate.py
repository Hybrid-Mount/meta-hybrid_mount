#!/usr/bin/env python3
"""Structural / link / index validation for the rule library.

Checks (no Rust toolchain needed):
  - every rules/<id>.md starts with `# <id>` matching its filename
  - has a `> ` one-line summary near the top
  - has `## Why It Matters` and `## See Also`
  - every `](other.md)` link resolves to an existing rule file
  - SKILL.md links exactly the set of files in rules/ (no broken links, no orphans)

Exits non-zero (and prints every problem) if anything fails.
"""
import re, pathlib, sys

HERE = pathlib.Path(__file__).resolve().parent
ROOT = HERE.parent
RULES = ROOT / "rules"
SKILL = ROOT / "SKILL.md"

errors = []
def err(msg): errors.append(msg)

rule_files = sorted(RULES.glob("*.md"))
rule_names = {p.name for p in rule_files}

link_re = re.compile(r'\]\((?:\./)?([a-z0-9-]+\.md)\)')

for p in rule_files:
    text = p.read_text(encoding="utf-8")
    lines = text.splitlines()
    head = lines[0].strip() if lines else ""
    if head != f"# {p.stem}":
        err(f"{p.name}: first line is {head!r}, expected '# {p.stem}'")
    if not any(l.startswith("> ") for l in lines[:6]):
        err(f"{p.name}: missing '> ' summary line near the top")
    for section in ("## Why It Matters", "## See Also"):
        if section not in text:
            err(f"{p.name}: missing '{section}' section")
    for tgt in link_re.findall(text):
        if tgt not in rule_names:
            err(f"{p.name}: broken link -> {tgt}")

# SKILL.md index parity
skill = SKILL.read_text(encoding="utf-8")
linked = set(re.findall(r'rules/([a-z0-9-]+\.md)', skill))
for tgt in sorted(linked):
    if tgt not in rule_names:
        err(f"SKILL.md: links missing file rules/{tgt}")
for name in sorted(rule_names):
    if name not in linked:
        err(f"SKILL.md: rule rules/{name} is not listed in the index")

if errors:
    print(f"VALIDATION FAILED ({len(errors)} problem(s)):\n")
    for e in errors:
        print(f"  - {e}")
    sys.exit(1)
print(f"OK: {len(rule_files)} rules valid; index lists all {len(linked)} of them.")
