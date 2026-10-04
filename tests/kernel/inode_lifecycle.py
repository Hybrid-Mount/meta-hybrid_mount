#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
"""Run deterministic interleavings against the production VFS C functions.

Kernel primitives are replaced by controlled fixtures, so these checks prove
publication/ownership decisions, not the Linux ABI or device correctness.
Usage: CC=clang python3 tests/kernel/inode_lifecycle.py
"""
import argparse
import os
from pathlib import Path
import re
import shlex
import subprocess
import tempfile


ROOT = Path(__file__).resolve().parents[2]


def extract(text, name):
    pattern = rf"^static[^;\n]*\b{re.escape(name)}\([^;]*?\)\s*\{{"
    match = re.search(pattern, text, re.MULTILINE)
    if match is None:
        raise RuntimeError(f"missing production function: {name}")
    depth = 1
    end = match.end()
    while depth:
        depth += (text[end] == "{") - (text[end] == "}")
        end += 1
    return text[match.start():end]


parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--source", type=Path, default=ROOT / "module/vfs/src/hybridmount.c")
source = parser.parse_args().source.read_text(encoding="utf-8")
functions = [
    "hybridmount_is_uid_blocked",
    "hm_destroy_virtual_inode",
    "hybridmount_resolve_rule_dentry",
    "hybridmount_hijacked_evict_inode",
    "hybridmount_generate_virtual_topology",
    "hm_free_rule",
]
with tempfile.TemporaryDirectory(prefix="hm-inode-test-") as temporary:
    directory = Path(temporary)
    (directory / "production.h").write_text(
        "\n\n".join(extract(source, name) for name in functions),
        encoding="utf-8",
    )
    executable = directory / ("inode-test.exe" if os.name == "nt" else "inode-test")
    compiler = shlex.split(os.environ.get("CC", "cc"))
    output = directory / "inode-test.obj" if os.name == "nt" else executable
    subprocess.run(
        compiler + ["-std=gnu11", "-Wall", "-Wextra", "-Werror",
                    "-Wno-unused-parameter", "-I", str(directory),
                    str(ROOT / "tests/kernel/inode_lifecycle.c"),
                    *(["-c"] if os.name == "nt" else []), "-o", str(output)],
        check=True,
    )
    if os.name == "nt":
        subprocess.run([os.environ.get("HM_TEST_LINK", "link"), "/nologo",
                        f"/out:{executable}", str(output), "libcmt.lib", "oldnames.lib"], check=True)
    failures = []
    for mode in ["new", "existing", "race", "dying", "race-dying", "retired",
                 "whiteout", "opaque", "deleted", "allocation", "evict",
                 "retired-republish", "orphan", "retire-eviction", "topology",
                 "real-parent", "uids"]:
        result = subprocess.run([str(executable), mode], timeout=30)
        if result.returncode:
            failures.append(mode)
    if failures:
        raise SystemExit("failed scenarios: " + ", ".join(failures))
