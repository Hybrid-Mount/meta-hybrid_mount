#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-only
"""Check RCU snapshots, node pins, and one UID snapshot per iteration."""
import os
from pathlib import Path
import re
import shlex
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]
source = (ROOT / "module/vfs/src/hybridmount.c").read_text()
functions = []
for name in [
    "hm_dir_get", "hm_dir_put", "__hybridmount_get_rule_info",
    "hybridmount_get_rule_info_uid", "hybridmount_get_rule_info",
    "hybridmount_actor_proxy", "hm_cached_inode_matches", "hybridmount_hijacked_lookup",
    "hybridmount_hijacked_iterate_dir", "hm_d_revalidate_common",
]:
    match = re.search(rf"^static[^;\n]*\b{name}\([^;]*?\)\s*\{{", source, re.MULTILINE)
    end, depth = match.end(), 1
    while depth:
        depth += (source[end] == "{") - (source[end] == "}")
        end += 1
    functions.append(source[match.start():end])

with tempfile.TemporaryDirectory(prefix="hm-readers-") as temporary:
    directory = Path(temporary)
    (directory / "production.h").write_text("\n".join(functions))
    executable = directory / "test.exe"
    subprocess.run(shlex.split(os.environ.get("CC", "cc")) + [
        "-std=gnu11", "-Wall", "-Wextra", "-Werror", "-Wno-unused-parameter",
        "-Wno-unused-function", "-I", str(directory),
        str(ROOT / "tests/kernel/reader_lifetime.c"), "-o", str(executable),
    ], check=True)
    failures = []
    for mode in ["lookup", "iterate", "revalidate"]:
        result = subprocess.run([str(executable), mode], timeout=30)
        if result.returncode:
            failures.append(mode)
    if failures:
        raise SystemExit("failed reader cases: " + ", ".join(failures))
