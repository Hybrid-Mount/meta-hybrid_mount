#!/usr/bin/env python3
"""Emit a path-based kernel-change checklist for the current diff or paths.

Always exits 0; the output is a checklist, not a verdict. By default the
changed paths come from the unstaged and staged git diff; pass paths
explicitly or use --against <rev> to cover committed work.
"""

from __future__ import annotations

import argparse
import subprocess
from dataclasses import dataclass, field
from pathlib import Path


SCRIPT_DIR = Path(__file__).resolve().parent


@dataclass
class Triage:
    kind: str
    references: set[str] = field(default_factory=set)
    checks: list[str] = field(default_factory=list)
    guardrails: list[str] = field(default_factory=list)
    notes: list[str] = field(default_factory=list)


def git_changed_paths(against: str | None = None) -> list[str]:
    if against:
        commands = [["git", "diff", against, "--name-only", "--diff-filter=ACMRT"]]
    else:
        commands = [
            ["git", "diff", "--name-only", "--diff-filter=ACMRT"],
            ["git", "diff", "--cached", "--name-only", "--diff-filter=ACMRT"],
        ]
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


def add_unique(items: list[str], value: str) -> None:
    if value not in items:
        items.append(value)


def build_wrapper() -> str | None:
    """Find a repo build wrapper from the cwd upward, stopping at the git root."""
    directory = Path.cwd()
    for candidate in [directory, *directory.parents]:
        wrapper = candidate / "build.sh"
        if wrapper.is_file():
            if candidate == directory:
                return "./build.sh"
            return str(wrapper)
        if (candidate / ".git").exists():
            break
    return None


def kmake_target(target: str) -> str:
    wrapper = build_wrapper()
    if wrapper is not None:
        return f"{wrapper} kmake {target}"
    return f"make {target}"


def config_target(target: str = "olddefconfig") -> str:
    wrapper = build_wrapper()
    if wrapper is not None:
        return f"{wrapper} kernel-config" if target == "kernel-config" else f"{wrapper} kmake {target}"
    return f"make {target}"


def classify(raw_path: str) -> Triage:
    path = Path(raw_path.replace("\\", "/")).as_posix()
    name = Path(path).name
    triage = Triage(kind="general kernel change")
    refs = triage.references
    checks = triage.checks
    guardrails = triage.guardrails
    notes = triage.notes

    refs.add("references/kernel-workflow.md")
    add_unique(checks, "git diff --check")
    add_unique(checks, "scripts/get_maintainer.pl <changed-files-or-patch>")

    if path.startswith("Documentation/devicetree/bindings/"):
        triage.kind = "devicetree binding"
        refs.update({"references/device-tree-drivers.md", "references/ci-regression.md"})
        add_unique(checks, kmake_target("dt_binding_check DT_SCHEMA_FILES=" + path))
        add_unique(checks, "scripts/checkpatch.pl --strict <patch-file>")
        add_unique(notes, "Binding changes are ABI changes; keep compatibility explicit.")

    if path.endswith((".dts", ".dtsi")):
        triage.kind = "devicetree source"
        refs.update({"references/device-tree-drivers.md"})
        add_unique(checks, kmake_target("dtbs_check"))
        add_unique(checks, kmake_target("<board-dtb-target>"))
        add_unique(notes, "Confirm compatible strings and properties against binding schemas.")

    if path.startswith("drivers/"):
        triage.kind = "kernel driver"
        refs.update(
            {
                "references/device-tree-drivers.md",
                "references/driver-api-cookbook.md",
                "references/driver-review-checklists.md",
                "references/debugging-testing.md",
            }
        )
        add_unique(checks, "scripts/checkpatch.pl --strict <patch-file>")
        add_unique(checks, f"python3 {SCRIPT_DIR / 'check_kernel_api_patterns.py'} {path}")
        add_unique(checks, kmake_target("<focused-target>"))
        add_unique(checks, "runtime probe/log validation on hardware or suitable emulator")
        add_unique(
            notes,
            "Review API choice, resource lifetime, devm use, PM, locking, IRQ context, DMA, and error paths.",
        )

    if path.startswith("include/linux/"):
        triage.kind = "kernel internal API"
        refs.update(
            {
                "references/driver-api-cookbook.md",
                "references/driver-review-checklists.md",
                "references/kernel-workflow.md",
            }
        )
        add_unique(checks, "scripts/checkpatch.pl --strict <patch-file>")
        add_unique(checks, kmake_target("<focused-target>"))
        add_unique(notes, "Internal API changes need call-site review and subsystem maintainer routing.")

    if name == "Kconfig" or path.startswith("arch/arm64/configs/") or path.endswith(".config"):
        triage.kind = "Kconfig or kernel config"
        refs.update({"references/kernel-workflow.md"})
        add_unique(checks, config_target("kernel-config"))
        add_unique(checks, config_target("olddefconfig"))
        add_unique(checks, "compare requested fragment with resolved output/.config")

    if "tools/testing/selftests/" in path or "kunit" in path.lower():
        triage.kind = "kernel test"
        refs.update({"references/debugging-testing.md", "references/ci-regression.md"})
        add_unique(checks, "run the focused KUnit or kselftest target through repo-approved tooling")
        add_unique(notes, "Preserve KTAP/kselftest output when using the result as evidence.")

    if path.startswith("include/uapi/") or path.startswith("Documentation/ABI/"):
        triage.kind = "kernel userspace ABI"
        refs.update(
            {
                "references/kernel-workflow.md",
                "references/device-tree-drivers.md",
                "references/driver-review-checklists.md",
            }
        )
        add_unique(checks, "update ABI documentation and add/adjust a userspace-facing test when feasible")
        add_unique(notes, "UAPI, sysfs, ioctl, netlink, and documented ABI need compatibility review.")

    thermal_terms = ("thermal", "fan", "cooling", "hwmon", "pwm-fan")
    if any(term in path.lower() for term in thermal_terms):
        refs.update({"references/debugging-testing.md"})
        add_unique(checks, "verify thermal trips, cooling maps, fan cap, tach/fault observability")
        add_unique(notes, "Tie RPM and safety claims to schematic/datasheet or live readings.")

    return triage


def render(paths: list[str]) -> str:
    lines = ["# Kernel Change Triage", ""]
    if not paths:
        lines.extend(
            [
                "No changed paths were found. Pass paths explicitly or run from a git worktree with a diff.",
                "",
            ]
        )
        return "\n".join(lines)

    lines.extend(
        [
            "Read nearest repo guidance first. Use the repo-approved build wrapper when present; otherwise translate checks to upstream `make` targets.",
            "",
        ]
    )
    for path in paths:
        triage = classify(path)
        normalized = Path(path.replace("\\", "/")).as_posix()
        lines.append(f"## `{normalized}`")
        lines.append("")
        lines.append(f"- Type: {triage.kind}")
        lines.append("- Read: " + ", ".join(f"`{ref}`" for ref in sorted(triage.references)))
        if triage.guardrails:
            lines.append("- Guardrails:")
            for guardrail in triage.guardrails:
                lines.append(f"  - {guardrail}")
        if triage.checks:
            lines.append("- Checks:")
            for check in triage.checks:
                lines.append(f"  - `{check}`")
        if triage.notes:
            lines.append("- Notes:")
            for note in triage.notes:
                lines.append(f"  - {note}")
        lines.append("")
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("paths", nargs="*", help="Changed paths. Defaults to git diff paths.")
    parser.add_argument(
        "--against",
        metavar="REV",
        help="Diff against REV (e.g. HEAD~1, origin/main) instead of the working tree/index.",
    )
    args = parser.parse_args()
    paths = args.paths or git_changed_paths(args.against)
    print(render(paths))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
