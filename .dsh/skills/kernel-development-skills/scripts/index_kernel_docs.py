#!/usr/bin/env python3
"""Index a Linux kernel Documentation/ tree for the kernel-development skill.

The generated route map and JSONL index are per-checkout artifacts: regenerate
them for the kernel tree you are working in and do not commit them. The header
never embeds the local checkout path; it records the indexed kernel version
instead so staleness is visible.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
from pathlib import Path


TEXT_NAMES = {"CodingStyle", "SubmittingPatches", "Kconfig", "Makefile"}
TEXT_SUFFIXES = {
    ".rst",
    ".txt",
    ".md",
    ".yaml",
    ".yml",
    ".json",
    ".c",
    ".h",
    ".py",
    ".sh",
}
KEYWORDS = (
    "submitting",
    "patch",
    "maintainer",
    "stable",
    "regression",
    "debug",
    "trace",
    "ftrace",
    "dynamic debug",
    "fault injection",
    "kunit",
    "kselftest",
    "ktap",
    "kasan",
    "kmsan",
    "kcsan",
    "kfence",
    "ubsan",
    "lockdep",
    "checkpatch",
    "sparse",
    "coccinelle",
    "devicetree",
    "dt-binding",
    "dt_binding_check",
    "dtbs_check",
    "driver",
    "kconfig",
    "abi",
    "sysfs",
    "uapi",
    "rockchip",
    "rk3576",
    "lapis",
    "pamir",
)
# Match keywords only when not preceded by a word character, so "stability"
# does not hit "abi" and "dispatch" does not hit "patch". Suffix matches such
# as "drivers" for "driver" stay intentional.
KEYWORD_RES = {
    keyword: re.compile(r"(?<![a-z0-9])" + re.escape(keyword)) for keyword in KEYWORDS
}
MAX_ROUTES_PER_KEYWORD = 12
MAX_TOP_LEVEL_DIRS = 40
CURATED_ROUTES = {
    "process": (
        "Documentation/process/submitting-patches.rst",
        "Documentation/process/submit-checklist.rst",
        "Documentation/process/coding-style.rst",
        "Documentation/process/handling-regressions.rst",
        "Documentation/process/stable-kernel-rules.rst",
        "Documentation/process/maintainer-handbooks.rst",
        "Documentation/maintainer/index.rst",
    ),
    "debug-tests": (
        "Documentation/dev-tools/testing-overview.rst",
        "Documentation/dev-tools/kunit/index.rst",
        "Documentation/dev-tools/kselftest.rst",
        "Documentation/dev-tools/ktap.rst",
        "Documentation/admin-guide/dynamic-debug-howto.rst",
        "Documentation/admin-guide/ramoops.rst",
        "Documentation/admin-guide/kdump/index.rst",
        "Documentation/dev-tools/kgdb.rst",
        "Documentation/dev-tools/gdb-kernel-debugging.rst",
    ),
    "tracing": (
        "Documentation/trace/ftrace.rst",
        "Documentation/trace/events.rst",
        "Documentation/trace/kprobes.rst",
        "Documentation/trace/fprobe.rst",
        "Documentation/trace/histogram.rst",
    ),
    "sanitizers-static-analysis": (
        "Documentation/dev-tools/checkpatch.rst",
        "Documentation/dev-tools/sparse.rst",
        "Documentation/dev-tools/coccinelle.rst",
        "Documentation/dev-tools/kasan.rst",
        "Documentation/dev-tools/kmsan.rst",
        "Documentation/dev-tools/kcsan.rst",
        "Documentation/dev-tools/kfence.rst",
        "Documentation/dev-tools/kmemleak.rst",
        "Documentation/fault-injection/fault-injection.rst",
    ),
    "devicetree-drivers": (
        "Documentation/devicetree/index.rst",
        "Documentation/devicetree/bindings/writing-schema.rst",
        "Documentation/devicetree/bindings/submitting-patches.rst",
        "Documentation/devicetree/bindings/dts-coding-style.rst",
        "Documentation/driver-api/index.rst",
        "Documentation/driver-api/driver-model/index.rst",
        "Documentation/driver-api/regulator.rst",
        "Documentation/driver-api/gpio/index.rst",
        "Documentation/driver-api/pin-control.rst",
        "Documentation/driver-api/i2c.rst",
        "Documentation/driver-api/spi.rst",
        "Documentation/driver-api/clk.rst",
        "Documentation/driver-api/thermal/index.rst",
    ),
    "kbuild-abi": (
        "Documentation/kbuild/index.rst",
        "Documentation/kbuild/kconfig-language.rst",
        "Documentation/kbuild/kconfig.rst",
        "Documentation/kbuild/makefiles.rst",
        "Documentation/admin-guide/abi.rst",
        "Documentation/ABI/README",
        "Documentation/userspace-api/index.rst",
    ),
}


def route_score(keyword: str, rel_path: str) -> tuple[int, str]:
    full_path = f"Documentation/{rel_path}"
    for group, paths in CURATED_ROUTES.items():
        if full_path in paths:
            return (0, f"{group}:{paths.index(full_path):03d}:{full_path}")

    lowered = rel_path.lower()
    preferred = (
        "process/",
        "dev-tools/",
        "admin-guide/",
        "trace/",
        "fault-injection/",
        "devicetree/",
        "driver-api/",
        "kbuild/",
        "maintainer/",
        "ABI/",
        "userspace-api/",
    )
    if lowered.startswith(tuple(item.lower() for item in preferred)):
        return (1, rel_path)
    if keyword.replace(" ", "-") in lowered or keyword.replace(" ", "_") in lowered:
        return (2, rel_path)
    return (3, rel_path)


def is_text_candidate(path: Path) -> bool:
    return path.name in TEXT_NAMES or path.suffix.lower() in TEXT_SUFFIXES


def read_text(path: Path) -> str:
    try:
        return path.read_text(encoding="utf-8")
    except UnicodeDecodeError:
        return path.read_text(encoding="utf-8", errors="replace")


def first_heading(text: str) -> str:
    lines = text.splitlines()
    for index, line in enumerate(lines):
        stripped = line.strip()
        if not stripped:
            continue
        if stripped.startswith((".. ", ":", "# SPDX", "*")):
            continue
        if index + 1 < len(lines):
            underline = lines[index + 1].strip()
            if underline and set(underline) <= set("=-~`^\"'#*+"):
                return stripped
        if stripped.startswith("# "):
            return stripped[2:].strip()
    return ""


def headings(text: str, limit: int = 8) -> list[str]:
    result: list[str] = []
    lines = text.splitlines()
    for index, line in enumerate(lines[:-1]):
        title = line.strip()
        underline = lines[index + 1].strip()
        if not title or len(title) > 120:
            continue
        if underline and len(underline) >= min(3, len(title)) and set(underline) <= set("=-~`^\"'#*+"):
            if title not in result:
                result.append(title)
        if len(result) >= limit:
            break
    return result


def keyword_hits(text: str) -> list[str]:
    lowered = text.lower()
    return [keyword for keyword in KEYWORDS if KEYWORD_RES[keyword].search(lowered)]


def kernel_version(kernel_root: Path) -> str:
    """Read VERSION/PATCHLEVEL/SUBLEVEL/EXTRAVERSION from the kernel Makefile."""
    fields = {"VERSION": "", "PATCHLEVEL": "", "SUBLEVEL": "", "EXTRAVERSION": ""}
    makefile = kernel_root / "Makefile"
    try:
        for line in makefile.read_text(encoding="utf-8", errors="replace").splitlines()[:20]:
            key, _, value = line.partition("=")
            key = key.strip()
            if key in fields and not fields[key]:
                fields[key] = value.strip()
    except OSError:
        return "unknown"
    if not fields["VERSION"]:
        return "unknown"
    version = f"{fields['VERSION']}.{fields['PATCHLEVEL']}.{fields['SUBLEVEL']}"
    return version + fields["EXTRAVERSION"]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("kernel_root", type=Path, help="Path to Linux kernel root")
    parser.add_argument("output", type=Path, help="Compact Markdown route-map output path")
    parser.add_argument(
        "--jsonl",
        type=Path,
        help="Optional full JSONL index output path for machine lookups",
    )
    args = parser.parse_args()

    docs = args.kernel_root / "Documentation"
    if not docs.is_dir():
        parser.error(f"{docs} is not a Documentation directory")

    jsonl_display = ""
    if args.jsonl:
        try:
            jsonl_display = str(args.jsonl.relative_to(args.output.parent.parent))
        except ValueError:
            # Never write an absolute local path into the generated artifact.
            jsonl_display = args.jsonl.name if args.jsonl.is_absolute() else str(args.jsonl)

    entries: list[dict[str, object]] = []
    dir_counts: dict[str, int] = {}
    keyword_paths: dict[str, list[str]] = {keyword: [] for keyword in KEYWORDS}

    for path in sorted(docs.rglob("*")):
        if not path.is_file():
            continue
        rel = path.relative_to(docs)
        data = path.read_bytes()
        digest = hashlib.sha256(data).hexdigest()[:16]
        size = len(data)
        top = rel.parts[0] if len(rel.parts) > 1 else "."
        dir_counts[top] = dir_counts.get(top, 0) + 1

        title = ""
        file_headings: list[str] = []
        hits: list[str] = []
        if is_text_candidate(path):
            text = read_text(path)
            title = first_heading(text)
            file_headings = headings(text)
            hits = keyword_hits(text)
            for hit in hits:
                keyword_paths[hit].append(str(rel))

        entries.append(
            {
                "path": f"Documentation/{rel}",
                "size": size,
                "sha256_16": digest,
                "title": title,
                "headings": file_headings,
                "keywords": hits,
                "is_symlink": path.is_symlink(),
            }
        )

    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("w", encoding="utf-8") as out:
        out.write("# Kernel Documentation Routes\n\n")
        out.write(
            "Generated per checkout; regenerate for the kernel tree you are "
            "working in and do not commit this file.\n\n"
        )
        out.write(f"Indexed kernel version: {kernel_version(args.kernel_root)}\n\n")
        out.write(f"Total file entries indexed: {len(entries)}\n\n")
        if args.jsonl:
            out.write(f"Full machine index: `{jsonl_display}`\n\n")
        out.write("## Top-Level Counts\n\n")
        out.write("| Directory | Files |\n| --- | ---: |\n")
        sorted_counts = sorted(dir_counts.items(), key=lambda item: (-item[1], item[0]))
        for name, count in sorted_counts[:MAX_TOP_LEVEL_DIRS]:
            out.write(f"| `{name}` | {count} |\n")
        if len(sorted_counts) > MAX_TOP_LEVEL_DIRS:
            out.write(f"| ... {len(sorted_counts) - MAX_TOP_LEVEL_DIRS} more | |\n")

        out.write("\n## Curated Routes\n\n")
        for group, paths in CURATED_ROUTES.items():
            out.write(f"### {group}\n\n")
            for route in paths:
                suffix = "" if (args.kernel_root / route).exists() else " (not in this tree)"
                out.write(f"- `{route}`{suffix}\n")
            out.write("\n")

        out.write("\n## Keyword Routes\n\n")
        for keyword in KEYWORDS:
            paths = sorted(keyword_paths[keyword], key=lambda value: route_score(keyword, value))
            if not paths:
                continue
            out.write(f"### {keyword}\n\n")
            for rel_path in paths[:MAX_ROUTES_PER_KEYWORD]:
                out.write(f"- `Documentation/{rel_path}`\n")
            if len(paths) > MAX_ROUTES_PER_KEYWORD:
                out.write(f"- ... {len(paths) - MAX_ROUTES_PER_KEYWORD} more; query JSONL for exact routes\n")
            out.write("\n")

        out.write("## Full Index Usage\n\n")
        out.write(
            "Do not load the full JSONL into context. Query it with `rg`, `jq`, "
            "or a short script, then open only the relevant kernel docs.\n\n"
        )
        out.write("Examples:\n\n")
        out.write("```bash\n")
        out.write("rg '\"keywords\":.*kselftest' assets/kernel-doc-index.jsonl\n")
        out.write("rg 'rk3576|lapis|pwm-fan' assets/kernel-doc-index.jsonl\n")
        out.write("```\n")

    if args.jsonl:
        args.jsonl.parent.mkdir(parents=True, exist_ok=True)
        with args.jsonl.open("w", encoding="utf-8") as jsonl:
            for entry in entries:
                jsonl.write(json.dumps(entry, sort_keys=True) + "\n")

    print(f"Indexed {len(entries)} files into {args.output}")
    if args.jsonl:
        print(f"Wrote full JSONL index into {args.jsonl}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
