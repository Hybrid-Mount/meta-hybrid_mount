#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-only
"""Inject hook allocation failures into extracted production C functions."""
import argparse
import os
from pathlib import Path
import re
import shlex
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--source", type=Path, default=ROOT / "module/vfs/src/hybridmount.c")
source = parser.parse_args().source.read_text()
functions = []
for name in ["hybridmount_hijack_superblock", "hybridmount_hijack_dir_ops"]:
    match = re.search(rf"^static[^;\n]*\b{name}\([^;]*?\)\s*\{{", source, re.MULTILINE)
    end, depth = match.end(), 1
    while depth:
        depth += (source[end] == "{") - (source[end] == "}")
        end += 1
    functions.append(source[match.start():end])

with tempfile.TemporaryDirectory(prefix="hm-hooks-") as temporary:
    directory = Path(temporary)
    (directory / "production.h").write_text("\n".join(functions))
    executable = directory / "test.exe"
    subprocess.run(shlex.split(os.environ.get("CC", "cc")) + [
        "-std=gnu11", "-Wall", "-Wextra", "-Werror", "-Wno-unused-parameter",
        "-Wno-unused-function", "-I", str(directory),
        str(ROOT / "tests/kernel/hook_transaction.c"), "-o", str(executable),
    ], check=True)
    failures = []
    for point in range(6):
        result = subprocess.run([str(executable), str(point)], timeout=30)
        if result.returncode:
            failures.append(point)
    if failures:
        raise SystemExit(f"failed hook transaction points: {failures}")
