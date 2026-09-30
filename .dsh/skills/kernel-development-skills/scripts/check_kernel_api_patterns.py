#!/usr/bin/env python3
"""Advisory scan for high-signal risky kernel driver API patterns.

Advisory by default: findings are printed but the exit code stays 0 unless
--strict is given. Exit codes: 0 = scanned (findings allowed), 1 = findings
with --strict, 2 = an explicitly passed path does not exist.

Limitations: comment detection is line-prefix only, so commented-out code may
still be flagged; path-scoped rules (e.g. legacy GPIO) only apply when the
scanned path contains the relevant prefix such as `drivers/`, so run from the
kernel root or pass tree-relative paths for full coverage.
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable


SKIP_DIR_PARTS = {
    ".git",
    ".agents",
    ".claude",
    ".codex-plugin",
    ".claude-plugin",
    "assets",
}

TEXT_SUFFIXES = {
    ".c",
    ".h",
    ".dts",
    ".dtsi",
    ".rst",
    ".txt",
}


@dataclass(frozen=True)
class PatternRule:
    name: str
    regex: re.Pattern[str]
    message: str
    reference: str
    path_prefixes: tuple[str, ...] = ()


RULES = (
    PatternRule(
        "bug-macro",
        re.compile(r"\bBUG(?:_ON)?\s*\("),
        "avoid BUG()/BUG_ON() in driver code; return an error or recover where possible",
        "references/driver-api-cookbook.md#deprecated-and-risky-patterns",
    ),
    PatternRule(
        "simple-strto",
        re.compile(r"\bsimple_strto(?:l|ll|ul|ull)\s*\("),
        "simple_strto*() ignores overflow; prefer kstrto*()",
        "references/driver-api-cookbook.md#deprecated-and-risky-patterns",
    ),
    PatternRule(
        "strcpy",
        re.compile(r"\bstrcpy\s*\("),
        "strcpy() has no destination bounds check; prefer strscpy() or strscpy_pad()",
        "references/driver-api-cookbook.md#deprecated-and-risky-patterns",
    ),
    PatternRule(
        "strncpy",
        re.compile(r"\bstrncpy\s*\("),
        "strncpy() is usually wrong for C strings; prefer strscpy(), strscpy_pad(), or strtomem()",
        "references/driver-api-cookbook.md#deprecated-and-risky-patterns",
    ),
    PatternRule(
        "strlcpy",
        re.compile(r"\bstrlcpy\s*\("),
        "strlcpy() was removed from the kernel in v6.8; use strscpy()",
        "references/driver-api-cookbook.md#deprecated-and-risky-patterns",
    ),
    PatternRule(
        "raw-pointer-print",
        re.compile(r'"[^"\n]*%p(?![A-Za-z0-9])'),
        "raw %p logging is rarely useful; remove it or justify a safer symbolic/privileged format",
        "references/driver-api-cookbook.md#deprecated-and-risky-patterns",
    ),
    PatternRule(
        "legacy-gpio",
        re.compile(
            r"\b(?:gpio_request|gpio_free|gpio_direction_input|gpio_direction_output|"
            r"gpio_get_value|gpio_set_value|devm_gpio_request(?:_one)?)\s*\("
        ),
        "new GPIO consumers should normally use descriptor APIs such as devm_gpiod_get*()",
        "references/driver-api-cookbook.md#embedded-api-choices",
        ("drivers/",),
    ),
    PatternRule(
        "raw-ioremap",
        re.compile(r"\bioremap(?:_wc|_uc|_cache|_np)?\s*\("),
        "prefer devm_ioremap_resource() or devm_platform_ioremap_resource() when mapping resources",
        "references/driver-api-cookbook.md#embedded-api-choices",
        ("drivers/",),
    ),
)


def git_changed_paths(against: str | None = None) -> list[str]:
    if against:
        commands: tuple[tuple[str, ...], ...] = (
            ("git", "diff", against, "--name-only", "--diff-filter=ACMRT"),
        )
    else:
        commands = (
            ("git", "diff", "--name-only", "--diff-filter=ACMRT"),
            ("git", "diff", "--cached", "--name-only", "--diff-filter=ACMRT"),
        )
    paths: list[str] = []
    seen: set[str] = set()
    for command in commands:
        try:
            result = subprocess.run(command, check=True, text=True, capture_output=True)
        except (OSError, subprocess.CalledProcessError):
            continue
        for line in result.stdout.splitlines():
            path = line.strip()
            if path and path not in seen:
                seen.add(path)
                paths.append(path)
    return paths


def is_scannable(path: Path) -> bool:
    if any(part in SKIP_DIR_PARTS for part in path.parts):
        return False
    return path.is_file() and path.suffix in TEXT_SUFFIXES


def expand_paths(raw_paths: Iterable[str]) -> list[Path]:
    paths: list[Path] = []
    for raw_path in raw_paths:
        path = Path(raw_path)
        if path.is_dir():
            for candidate in path.rglob("*"):
                if is_scannable(candidate):
                    paths.append(candidate)
        elif is_scannable(path):
            paths.append(path)
    return paths


def rule_applies(rule: PatternRule, normalized_path: str) -> bool:
    if not rule.path_prefixes:
        return True
    return any(
        normalized_path.startswith(prefix) or f"/{prefix}" in normalized_path
        for prefix in rule.path_prefixes
    )


def iter_findings(paths: Iterable[Path]) -> Iterable[str]:
    for path in paths:
        normalized_path = path.as_posix()
        if normalized_path.startswith("./"):
            normalized_path = normalized_path[2:]
        try:
            text = path.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            text = path.read_text(encoding="utf-8", errors="replace")
        for number, line in enumerate(text.splitlines(), 1):
            stripped = line.strip()
            if not stripped or stripped.startswith(("*", "/*", "//")):
                continue
            for rule in RULES:
                if rule_applies(rule, normalized_path) and rule.regex.search(line):
                    yield (
                        f"{normalized_path}:{number}: {rule.name}: "
                        f"{rule.message} ({rule.reference})"
                    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("paths", nargs="*", help="Paths to scan. Defaults to git diff paths.")
    parser.add_argument(
        "--against",
        metavar="REV",
        help="Diff against REV (e.g. HEAD~1, origin/main) instead of the working tree/index.",
    )
    parser.add_argument(
        "--strict",
        action="store_true",
        help="Exit non-zero when findings exist (default is advisory exit 0).",
    )
    args = parser.parse_args()

    if args.paths:
        missing = [raw for raw in args.paths if not Path(raw).exists()]
        if missing:
            for raw in missing:
                print(f"error: path does not exist: {raw}", file=sys.stderr)
            return 2

    raw_paths = args.paths or git_changed_paths(args.against)
    paths = expand_paths(raw_paths)
    if not raw_paths:
        print("No changed paths were found. Pass files or directories explicitly.")
        return 0
    if not paths:
        print("No scannable text files found.")
        return 0

    findings = list(iter_findings(paths))
    if not findings:
        print("No advisory kernel API pattern findings.")
        return 0

    print("Advisory kernel API pattern findings:")
    for finding in findings:
        print(f"- {finding}")
    return 1 if args.strict else 0


if __name__ == "__main__":
    raise SystemExit(main())
